#!/usr/bin/env python3
"""Generate PearHost UIKit stubs from census reports and the public UIKit headers.

Everything an app imports from UIKit (classes, functions, data symbols) must be
exported by our UIKit image or dyld refuses to start the app. This script
emits a placeholder for every imported name that host/uikit/src does not
implement by hand:

  classes    -> empty @implementation with the public superclass; every message
                they receive is caught by the missing-call reporter
  functions  -> assembly stub: report "<name>()" with the caller, return zero
  data       -> NSString constants hold their own name (the conventional value);
                integer constants get placeholder values and are listed in the
                generated header comment

Inputs are interface-only: census JSON (from the app's own binary) and public
SDK headers. No Apple binary is read.

  tools/gen-uikit-stubs.py docs/census/*.json
"""
import glob
import json
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "host/uikit/src")
OUT = os.path.join(ROOT, "host/uikit/generated")

STRING_TYPES = re.compile(r"\b(NSString|NSNotificationName|UIActivityType|NSAttributedStringKey|"
                          r"NSAttributedStringDocumentType|NSAttributedStringDocumentAttributeKey|"
                          r"UIApplicationOpenURLOptionsKey|UIApplicationLaunchOptionsKey|"
                          r"UIAccessibilityNotifications|NSString\s*\*|UI\w*Key|UI\w*Name|UI\w*Type|"
                          r"UI\w*Identifier|UIPasteboardName|UIFontTextStyle|UIFontDescriptor\w*)\b")


# Window levels are documented ordering values (normal < status bar < alert).
WINDOW_LEVELS = {"UIWindowLevelNormal": 0.0, "UIWindowLevelStatusBar": 1000.0, "UIWindowLevelAlert": 2000.0}

# Names a macOS framework already exports (AppKit re-exports UIFoundation:
# NSParagraphStyle, NSFontAttributeName, ...). Our UIKit re-exports AppKit for
# these instead of defining a second copy.
def macos_exports():
    sdk = subprocess.check_output(["xcrun", "--sdk", "macosx", "--show-sdk-path"], text=True).strip()
    names = set()
    for fw in ("AppKit", "UIFoundation"):
        for t in glob.glob(os.path.join(sdk, f"System/Library/*Frameworks/{fw}.framework/*.tbd")):
            text = open(t, errors="ignore").read()
            names |= set(re.findall(r"\b_?(NS\w+)\b", text))
    return names


def headers():
    sdk = subprocess.check_output(["xcrun", "--sdk", "macosx", "--show-sdk-path"], text=True).strip()
    hd = os.path.join(sdk, "System/iOSSupport/System/Library/Frameworks/UIKit.framework/Headers")
    text = {}
    for f in sorted(os.listdir(hd)):
        if f.endswith(".h"):
            text[f] = open(os.path.join(hd, f), errors="ignore").read()
    return text


def superclasses(hdrs):
    sup = {}
    for t in hdrs.values():
        for m in re.finditer(r"@interface\s+(\w+)\s*(?:<[^>]*>)?\s*:\s*(\w+)", t):
            sup.setdefault(m.group(1), m.group(2))
    return sup


def extern_decls(hdrs):
    """name -> ('function', type text) | ('data', type text). Statement-based, no backtracking."""
    out = {}
    attr = re.compile(r"\b(?:API_\w+|NS_\w+|UIKIT_\w+|__\w+)\s*(?:\((?:[^()]|\([^()]*\))*\))?")
    for t in hdrs.values():
        t = re.sub(r"/\*.*?\*/", "", t, flags=re.S)
        t = re.sub(r"//[^\n]*", "", t)
        for stmt in t.split(";"):
            stmt = " ".join(stmt.split())
            if not re.search(r"\b(?:UIKIT_EXTERN|extern)\b", stmt):
                continue
            # The chunk may start with unrelated text (@end, a brace); keep only
            # what follows the last extern keyword.
            body = stmt[stmt.rfind("extern") + 6:] if "UIKIT_EXTERN" not in stmt else stmt[stmt.rfind("UIKIT_EXTERN") + 12:]
            if "{" in body or "@" in body or "}" in body:
                continue
            body = attr.sub(" ", body).strip()
            m = re.match(r"(.*?)\b(\w+)\s*(\(.*\))?\s*$", body)
            if not m:
                continue
            pre, name, args = m.group(1), m.group(2), m.group(3)
            out.setdefault(name, ("function", pre.strip()) if args else ("data", pre.strip()))
    return out


def implemented():
    classes, symbols = set(), set()
    for f in glob.glob(os.path.join(SRC, "*.m")):
        t = open(f).read()
        classes |= set(re.findall(r"@implementation\s+(\w+)\b(?!\s*\()", t))
        symbols |= set(re.findall(r"PEAR_UIKIT_EXPORT[^;{=]*?\b(\w+)\s*(?:\(|=)", t))
    return classes, symbols


def main(argv):
    if len(argv) < 2:
        print(__doc__)
        return 2
    want_classes, want_syms = set(), set()
    for p in argv[1:]:
        d = json.load(open(p))
        lib = d["libraries"].get("UIKit")
        if lib:
            want_classes |= set(lib["classes"])
            want_syms |= {s[1:] for s in lib["symbols"]}

    hdrs = headers()
    sup = superclasses(hdrs)
    decls = extern_decls(hdrs)
    have_classes, have_syms = implemented()
    from_macos = macos_exports()
    want_classes = {c for c in want_classes if not (c.startswith("NS") and c in from_macos)}
    reexported = {n for n in want_syms if n.startswith("NS") and n in from_macos}
    want_syms -= reexported

    # Close over superclasses that UIKit itself defines.
    todo = sorted(want_classes)
    stub_classes = []
    seen = set()
    while todo:
        c = todo.pop(0)
        if c in seen:
            continue
        seen.add(c)
        s = sup.get(c, "NSObject")
        if s in sup or s.startswith("UI"):
            todo.append(s)
        if c not in have_classes:
            stub_classes.append(c)

    # Order parents before children.
    order = []
    def visit(c):
        if c in order or c not in stub_classes:
            return
        visit(sup.get(c, "NSObject"))
        order.append(c)
    for c in sorted(stub_classes):
        visit(c)

    os.makedirs(OUT, exist_ok=True)
    m = ["// GENERATED by tools/gen-uikit-stubs.py from census reports and public UIKit",
         "// headers. Do not edit; implement the class in host/uikit/src and regenerate.",
         "#import \"PearUIKit.h\"", "", "#pragma GCC visibility push(default)"]
    placeholders = []
    for c in order:
        s = sup.get(c, "NSObject")
        if not (s in have_classes or s in order or s.startswith("NS")):
            s = "NSObject"
        m.append(f"@interface {c} : {s}\n@end\n@implementation {c}\n@end\n")

    funcs = []
    for name in sorted(want_syms - have_syms):
        kind, typ = decls.get(name, ("unknown", ""))
        if kind in ("function", "unknown"):
            # Not declared in the public headers we have: Apple documents these
            # as functions far more often than data; a stub that reports is the
            # safe guess either way.
            funcs.append(name)
        elif kind == "data" and re.search(r"\b(UIWindowLevel|CGFloat|double)\b", typ):
            m.append(f"PEAR_UIKIT_EXPORT const double {name} = {WINDOW_LEVELS.get(name, 0.0)}; // CGFloat")
        elif kind == "data" and re.search(r"\bUIBackgroundTaskIdentifier\b", typ):
            m.append(f"PEAR_UIKIT_EXPORT const unsigned long {name} = 0;  // documented invalid identifier")
        elif kind == "data" and "UIAccessibilityTraits" in typ:
            bit = 0 if name.endswith("None") else 1 << (len(placeholders) + 1)
            m.append(f"PEAR_UIKIT_EXPORT const uint64_t {name} = {bit}ULL; // trait bit chosen by PearHost")
            placeholders.append(name)
        elif kind == "data" and STRING_TYPES.search(typ):
            m.append(f'PEAR_UIKIT_EXPORT NSString *const {name} = @"{name}";')
        else:
            m.append(f"PEAR_UIKIT_EXPORT const uint64_t {name} = 0; // placeholder ({kind}: {typ or '?'})")
            placeholders.append(name)

    m.append("#pragma GCC visibility pop")
    with open(os.path.join(OUT, "UIKitStubs.m"), "w") as fh:
        fh.write("\n".join(m) + "\n")

    s = ["// GENERATED by tools/gen-uikit-stubs.py. Each stub reports itself, then",
         "// returns zero through pear_missing_zero.", "    .text"]
    for i, name in enumerate(funcs):
        s += [f"    .p2align 2", f"    .globl _{name}", f"_{name}:",
              "    stp x29, x30, [sp, #-16]!", "    mov x29, sp",
              "    mov x0, #2", "    mov x1, #0",
              f"    adrp x2, Lname{i}@PAGE", f"    add x2, x2, Lname{i}@PAGEOFF",
              "    mov x3, x30", "    bl _PearMissingReport",
              "    ldp x29, x30, [sp], #16", "    b _pear_missing_zero"]
    s += ["    .section __TEXT,__cstring,cstring_literals"]
    for i, name in enumerate(funcs):
        s += [f"Lname{i}:", f'    .asciz "{name}"']
    with open(os.path.join(OUT, "UIKitStubs.s"), "w") as fh:
        fh.write("\n".join(s) + "\n")

    print(f"re-exported from AppKit: {' '.join(sorted(reexported)) or '-'}")
    print(f"classes: {len(want_classes)} wanted, {len(order)} stubbed, {len(want_classes & have_classes)} implemented")
    print(f"symbols: {len(want_syms)} wanted, {len(funcs)} function stubs, {len(placeholders)} placeholder data, "
          f"{len(want_syms & have_syms)} implemented")
    if placeholders:
        print("placeholder data symbols:", " ".join(placeholders))
    print("stubbed classes:", " ".join(order))
    print("stubbed functions:", " ".join(funcs))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
