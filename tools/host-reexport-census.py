#!/usr/bin/env python3
"""Which symbols prepared apps bind through UIKit that UIKit does not define.

  tools/host-reexport-census.py build/host APP.app [APP.app ...]

Every Mach-O image of each app (its executable, frameworks and dylibs,
including PearRuntime's own copies such as libswiftUIKit.dylib) is read with
`dyld_info -imports`. An import "(from UIKit)" that PearHost's UIKit does not
export itself only resolves through a framework UIKit re-exports (AppKit).
Prints each such symbol, the library in AppKit's re-export closure that
supplies it today ("nowhere": unresolved, so only a weak import survives it),
how many images import it (strong: not weak, so the image fails to load
without it) and a few of them.
"""
import collections
import os
import re
import subprocess
import sys


def dyld_info(flag, path):
    out = subprocess.run(["xcrun", "dyld_info", flag, path], capture_output=True, text=True).stdout
    return out.splitlines()


def exports(path):
    names = set()
    for line in dyld_info("-exports", path):
        m = re.match(r"\s+(?:0x[0-9A-Fa-f]+|\[re-export\])\s+(\S+)", line)
        if m:
            names.add(m.group(1))
    return names


def reexport_closure(path, seen=None):
    """[(library, its exports)] for `path` and what it re-exports, in order."""
    seen = seen if seen is not None else set()
    if path in seen:
        return []
    seen.add(path)
    found = [(os.path.basename(path), exports(path))]
    for line in dyld_info("-linked_dylibs", path):
        m = re.match(r"\s+re-export\s+(\S+)", line)
        if m:
            found += reexport_closure(m.group(1), seen)
    return found


def is_macho(path):
    try:
        with open(path, "rb") as f:
            return f.read(4) in (b"\xcf\xfa\xed\xfe", b"\xca\xfe\xba\xbe", b"\xbe\xba\xfe\xca")
    except OSError:
        return False


def images(app):
    for root, _dirs, files in os.walk(app):
        if "_CodeSignature" in root:
            continue
        for name in files:
            path = os.path.join(root, name)
            if not os.path.islink(path) and os.path.isfile(path) and is_macho(path):
                yield path


def main():
    host, apps = sys.argv[1], sys.argv[2:]
    own = exports(f"{host}/UIKit.framework/UIKit")
    appkit = reexport_closure("/System/Library/Frameworks/AppKit.framework/AppKit")
    needed = collections.defaultdict(set)
    for app in apps:
        for image in images(app):
            if image.endswith("/UIKit.framework/UIKit"):
                continue
            for line in dyld_info("-imports", image):
                m = re.match(r"\s+(\S+)\s+(\[weak-import\]\s+)?\(from UIKit\)", line)
                if m and m.group(1) not in own:
                    weak = " (weak)" if m.group(2) else ""
                    needed[m.group(1)].add(os.path.relpath(image, os.path.dirname(app)) + weak)
    for symbol in sorted(needed):
        where = next((lib for lib, names in appkit if symbol in names), "nowhere")
        strong = sum(1 for image in needed[symbol] if not image.endswith("(weak)"))
        print(f"{symbol}\t{where}\t{len(needed[symbol])} images, {strong} strong\t"
              + "; ".join(sorted(needed[symbol])[:3]))
    print(f"{len(needed)} symbols bound through UIKit's re-exports", file=sys.stderr)


if __name__ == "__main__":
    main()
