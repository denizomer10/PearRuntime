#!/usr/bin/env python3
"""ios-import-census: what does an iOS app actually ask the system for?

Clean-room input only. It reads two things:
  * the app's own Mach-O binaries (imports and selector names), via dyld_info/otool
  * the public SDK text stubs (.tbd) under <MacOSX SDK>/System/iOSSupport,
    which list exported names only - no implementation is read.

It never opens /System/iOSSupport binaries or the dyld shared cache.

Usage:
  tools/ios-import-census.py App.app [More.app ...] [--json out.json]
  tools/ios-import-census.py Payload/   (an unzipped .ipa)

Report: per framework, the imported C symbols and ObjC classes, whether the
framework exists only in iOSSupport (must be reimplemented to drop it) or is
shared with macOS, and the selector names the app sends.
"""
import argparse
import json
import os
import re
import subprocess
import sys
from collections import defaultdict

MACHO_MAGICS = {b"\xcf\xfa\xed\xfe", b"\xca\xfe\xba\xbe", b"\xbe\xba\xfe\xca"}
IMPORT_RE = re.compile(r"^\s*(?:0x[0-9A-Fa-f]+\s+)?(\S+)\s+\(from (\S+)\)")


def sdk_path():
    return subprocess.check_output(["xcrun", "--sdk", "macosx", "--show-sdk-path"], text=True).strip()


def ios_only_frameworks(sdk):
    """Framework names whose stubs live under iOSSupport (public interface only)."""
    names = set()
    root = os.path.join(sdk, "System/iOSSupport")
    for dp, _, fs in os.walk(root):
        for f in fs:
            if f.endswith(".tbd"):
                m = re.search(r"/([^/]+)\.framework/", dp + "/")
                names.add(m.group(1) if m else f[:-4])
    # A framework that macOS also ships at the same path does not need a
    # reimplementation on macOS (it does on Linux).
    mac = os.path.join(sdk, "System/Library/Frameworks")
    return {n for n in names if not os.path.isdir(os.path.join(mac, n + ".framework"))}


def is_macho(path):
    try:
        with open(path, "rb") as fh:
            return fh.read(4) in MACHO_MAGICS
    except OSError:
        return False


def bundle_binaries(bundle):
    for dp, _, fs in os.walk(bundle):
        for f in fs:
            p = os.path.join(dp, f)
            if not os.path.islink(p) and os.path.getsize(p) > 4096 and is_macho(p):
                yield p


def run(cmd):
    r = subprocess.run(cmd, capture_output=True, text=True)
    return r.stdout


def pick_arch(path):
    archs = run(["lipo", "-archs", path]).split()
    for want in ("arm64", "arm64e"):
        if want in archs:
            return want
    return archs[0] if archs else "arm64"


def scan_binary(path):
    arch = pick_arch(path)
    imports = defaultdict(set)
    for line in run(["dyld_info", "-arch", arch, "-imports", path]).splitlines():
        m = IMPORT_RE.match(line)
        if m:
            imports[m.group(2)].add(m.group(1))
    selectors = set()
    out = run(["otool", "-arch", arch, "-v", "-s", "__TEXT", "__objc_methname", path])
    for line in out.splitlines():
        m = re.match(r"^[0-9a-f]+\s+(.+)$", line.strip())
        if m:
            selectors.add(m.group(1))
    return imports, selectors


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("bundles", nargs="+")
    ap.add_argument("--json")
    a = ap.parse_args()

    ios_only = ios_only_frameworks(sdk_path())
    own = set()  # images shipped inside the bundles themselves
    per_lib = defaultdict(lambda: {"symbols": set(), "classes": set(), "apps": set()})
    selectors = defaultdict(set)

    for bundle in a.bundles:
        app = os.path.basename(os.path.normpath(bundle))
        bins = list(bundle_binaries(bundle))
        own |= {os.path.basename(b) for b in bins}
        for b in bins:
            imports, sels = scan_binary(b)
            for lib, syms in imports.items():
                e = per_lib[lib]
                e["apps"].add(app)
                for s in syms:
                    if s.startswith("_OBJC_CLASS_$_"):
                        e["classes"].add(s[len("_OBJC_CLASS_$_"):])
                    elif not s.startswith("_OBJC_METACLASS_$_"):
                        e["symbols"].add(s)
            for s in sels:
                selectors[s].add(app)

    rows = []
    for lib, e in per_lib.items():
        if lib in own:
            continue
        kind = "iOS-only" if lib in ios_only else "macOS-has-it"
        rows.append((lib, kind, e))
    rows.sort(key=lambda r: (r[1] != "iOSSupport-only", -len(r[2]["classes"]) - len(r[2]["symbols"])))

    print(f"{'library':34} {'kind':18} {'apps':>4} {'classes':>7} {'symbols':>7}")
    for lib, kind, e in rows:
        print(f"{lib:34} {kind:18} {len(e['apps']):>4} {len(e['classes']):>7} {len(e['symbols']):>7}")
    print(f"\nselectors referenced: {len(selectors)}")

    if a.json:
        with open(a.json, "w") as fh:
            json.dump({
                "libraries": {lib: {"kind": kind, "apps": sorted(e["apps"]),
                                    "classes": sorted(e["classes"]), "symbols": sorted(e["symbols"])}
                              for lib, kind, e in rows},
                "selectors": {s: sorted(v) for s, v in sorted(selectors.items())},
            }, fh, indent=1)


if __name__ == "__main__":
    sys.exit(main())
