#!/usr/bin/env python3
"""Generate PearHost UIKit stubs from census reports and the public UIKit headers.

Everything an app imports from UIKit (classes, functions, data symbols) must be
exported by our UIKit image or dyld refuses to start the app. This script
emits a placeholder for every imported name that host/uikit/src does not
implement by hand:

  classes    -> empty static class with the public superclass; every message
                they receive is caught by the missing-call reporter
  functions  -> assembly stub: report "<name>()" with the caller, return zero
  data       -> NSString constants hold their own name (the conventional value);
                integer constants get placeholder values and are listed in the
                generated header comment

Output is Rust, rust/crates/pear-uikit/src/generated_stubs.rs (built into
UIKit.framework by host/build-host.sh, which runs this first). Inputs are
interface-only: census JSON (from the app's own binary) and public SDK
headers. No Apple binary is read.

  tools/gen-uikit-stubs.py docs/census/*.json
"""
import glob
import json
import os
import re
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "rust/crates/pear-uikit/src")
OUT = os.path.join(SRC, "generated_stubs.rs")

STRING_TYPES = re.compile(r"\b(NSString|NSNotificationName|UIActivityType|NSAttributedStringKey|"
                          r"NSAttributedStringDocumentType|NSAttributedStringDocumentAttributeKey|"
                          r"UIApplicationOpenURLOptionsKey|UIApplicationLaunchOptionsKey|"
                          r"UIAccessibilityNotifications|NSString\s*\*|UI\w*Key|UI\w*Name|UI\w*Type|"
                          r"UI\w*Identifier|UIPasteboardName|UIFontTextStyle|UIFontDescriptor\w*|"
                          r"UIContentSizeCategory|UIPasteboardOption|NSRunLoopMode)\b")


# Window levels are documented ordering values (normal < status bar < alert).
WINDOW_LEVELS = {"UIWindowLevelNormal": 0.0, "UIWindowLevelStatusBar": 1000.0, "UIWindowLevelAlert": 2000.0}

# Scalar (CGFloat/double) constants and their values: font weights are the
# ones macOS exports as the identical NSFontWeight* constants; deceleration
# rates are the documented normal/fast values. Others default to 0.
DOUBLE_TYPES = re.compile(r"\b(UIWindowLevel|CGFloat|double|NSTimeInterval|UIFontWeight|UIScrollViewDecelerationRate)\b")
DOUBLE_VALUES = {**WINDOW_LEVELS,
                 "UIFontWeightUltraLight": -0.800000011920929, "UIFontWeightThin": -0.6000000238418579,
                 "UIFontWeightLight": -0.4000000059604645, "UIFontWeightRegular": 0.0,
                 "UIFontWeightMedium": 0.23000000417232513, "UIFontWeightSemibold": 0.30000001192092896,
                 "UIFontWeightBold": 0.4000000059604645, "UIFontWeightHeavy": 0.5600000023841858,
                 "UIFontWeightBlack": 0.6200000047683716,
                 "UIScrollViewDecelerationRateNormal": 0.998, "UIScrollViewDecelerationRateFast": 0.99}
# Geometry constants: number of CGFloat fields (zero unless documented).
STRUCT_FIELDS = {"CGPoint": 2, "CGSize": 2, "UIOffset": 2, "CGRect": 4, "UIEdgeInsets": 4,
                 "NSDirectionalEdgeInsets": 4, "CGAffineTransform": 6}
STRUCT_VALUES = {"UILayoutFittingExpandedSize": [1e6, 1e6]}

# Names UIFoundation already exports (the text system: NSParagraphStyle,
# NSFontAttributeName, ...). Our UIKit re-exports UIFoundation for these, as
# iOS's does (host/UIFoundation.tbd), instead of defining a second copy.
# AppKit is not re-exported, so AppKit-only names are stubbed like any other.
def macos_exports():
    sdk = subprocess.check_output(["xcrun", "--sdk", "macosx", "--show-sdk-path"], text=True).strip()
    names = set()
    for fw in ("UIFoundation",):
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
    """Classes and symbols UIKit.framework already defines (pear-uikit and the
    Rust crates linked into it), not counting this script's own output."""
    classes, symbols = set(), set()
    for f in glob.glob(os.path.join(ROOT, "rust/crates/*/src/**/*.rs"), recursive=True):
        if os.path.abspath(f) == OUT:
            continue
        t = open(f).read()
        classes |= set(re.findall(r'#\[export_name = "OBJC_CLASS_\$_(\w+)"\]', t))
        symbols |= set(re.findall(r'#\[no_mangle\]\s+pub\s+(?:unsafe\s+)?extern\s+"C"\s+fn\s+(\w+)', t))
        symbols |= set(re.findall(r'#\[no_mangle\]\s+pub\s+static\s+(\w+)', t))
        symbols |= set(re.findall(r'nsstring_constant!\((\w+)\s*=', t))
    return classes, symbols


def data_constant(name, decls, placeholders):
    """Rust for a data symbol `name` from its header type, or None when it is
    (or may be) a function. Placeholder values are appended to placeholders."""
    kind, typ = decls.get(name, ("unknown", ""))
    export = "#[no_mangle]\npub static"
    # Undeclared (deprecated or unavailable API missing from the SDK headers):
    # names in Apple's string-constant style are NSString constants.
    if kind == "unknown" and re.search(r"(Notification|Key|Name|Identifier|Attribute|Option|Parameter)\w*$", name) \
            and not re.search(r"(Create|Get|Set|Copy|Is|With|From|To)\w*$", name):
        return f'pear_objc::nsstring_constant!({name} = "{name}");'
    if kind in ("function", "unknown"):
        # Not declared in the public headers we have: Apple documents these
        # as functions far more often than data; a stub that reports is the
        # safe guess either way.
        return None
    if DOUBLE_TYPES.search(typ):
        return f"{export} {name}: f64 = {DOUBLE_VALUES.get(name, 0.0)!r}; // CGFloat/double"
    if any(re.search(rf"\b{s}\b", typ) for s in STRUCT_FIELDS):
        s = next(s for s in STRUCT_FIELDS if re.search(rf"\b{s}\b", typ))
        values = STRUCT_VALUES.get(name, [0.0] * STRUCT_FIELDS[s])
        return f"{export} {name}: [f64; {STRUCT_FIELDS[s]}] = [{', '.join(repr(float(v)) for v in values)}]; // {s}"
    if re.search(r"\bUIBackgroundTaskIdentifier\b", typ):
        return f"{export} {name}: u64 = 0; // documented invalid identifier"
    if "UIAccessibilityTraits" in typ:
        bit = 0 if name.endswith("None") else 1 << (len(placeholders) + 1)
        placeholders.append(name)
        return f"{export} {name}: u64 = {bit}; // trait bit chosen by PearHost"
    if STRING_TYPES.search(typ) or re.search(r"\bNSString\b|Notification|Key\b", typ):
        return f'pear_objc::nsstring_constant!({name} = "{name}");'
    placeholders.append(name)
    return f"{export} {name}: u64 = 0; // placeholder ({kind}: {typ or '?'})"


def function_stubs(funcs, label):
    """global_asm! source for C function stubs that report themselves, then
    return zero through pear_missing_zero."""
    s = ["    .text"]
    for i, name in enumerate(funcs):
        s += ["    .p2align 2", f"    .globl _{name}", f"_{name}:",
              "    stp x29, x30, [sp, #-16]!", "    mov x29, sp",
              "    mov x0, #2", "    mov x1, #0",
              f"    adrp x2, L{label}_name{i}@PAGE", f"    add x2, x2, L{label}_name{i}@PAGEOFF",
              "    mov x3, x30", "    bl _PearMissingReport",
              "    ldp x29, x30, [sp], #16", "    b _pear_missing_zero"]
    s += ["    .section __TEXT,__cstring,cstring_literals"]
    for i, name in enumerate(funcs):
        s += [f"L{label}_name{i}:", f'    .asciz "{name}"']
    s += ["    .text"]
    return ["", "std::arch::global_asm!(", '    r#"'] + s + ['"#', ");"]


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

    m = ["// GENERATED by tools/gen-uikit-stubs.py from census reports and public UIKit",
         "// headers. Do not edit; implement the class or symbol in pear-uikit and regenerate.",
         "#![allow(non_upper_case_globals)]", "",
         "use std::ffi::c_void;", ""]
    ns_supers = sorted({sup.get(c, "NSObject") for c in order if sup.get(c, "NSObject").startswith("NS")
                        and sup.get(c, "NSObject") not in have_classes
                        and sup.get(c, "NSObject") not in order} | {"NSObject"})
    m.append('extern "C" {')
    for s in ns_supers:
        m += [f'    #[link_name = "OBJC_CLASS_$_{s}"]', f"    static {s}_CLASS: c_void;",
              f'    #[link_name = "OBJC_METACLASS_$_{s}"]', f"    static {s}_META: c_void;"]
    m += ["}", ""]
    placeholders = []
    for c in order:
        s = sup.get(c, "NSObject")
        if not (s in have_classes or s in order or s.startswith("NS")):
            s = "NSObject"
        if s in have_classes:
            ref = f"crate::objc_classes::{s}"
            m.append(f"pear_objc::empty_class!({c} : {ref}::CLASS.ptr(), {ref}::META.ptr());")
        elif s in order:
            m.append(f"pear_objc::empty_class!({c} : super::{s}::CLASS.ptr(), super::{s}::META.ptr());")
        else:
            m.append(f"pear_objc::empty_class!({c} : &raw const super::{s}_CLASS, &raw const super::{s}_META);")

    funcs = []
    for name in sorted(want_syms - have_syms):
        line = data_constant(name, decls, placeholders)
        if line is None:
            funcs.append(name)
        else:
            m.append(line)

    m += function_stubs(funcs, "pear_stub")
    with open(OUT, "w") as fh:
        fh.write("\n".join(m).replace("\n\n\n", "\n\n") + "\n")

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
