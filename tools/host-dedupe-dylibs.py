#!/usr/bin/env python3
"""Rewrites duplicate dylib load commands of a Mach-O image in place.

dyld refuses an image that names the same dylib twice ("duplicate linked
dylib"). A bundle prepared for Mac Catalyst can carry such a pair: its
MobileCoreServices load command was rewritten to CoreServices, which the
binary already linked. Every later occurrence of a known duplicate is pointed
at a PearHost framework that re-exports the same library, so both load
commands keep their ordinals and every import still binds.

  tools/host-dedupe-dylibs.py <Mach-O> [...]

Exit status 0 when nothing needed changing or every duplicate was rewritten.
"""
import struct
import sys

LC_REQ_DYLD = 0x80000000
DYLIB_COMMANDS = {0xC, 0x18 | LC_REQ_DYLD, 0x1F | LC_REQ_DYLD, 0x23 | LC_REQ_DYLD, 0x20 | LC_REQ_DYLD}
# Duplicate -> replacement: a PearHost framework re-exporting the same library.
REPLACEMENTS = {
    "/System/Library/Frameworks/CoreServices.framework/CoreServices":
        "@rpath/MobileCoreServices.framework/MobileCoreServices",
}


def dedupe(path):
    with open(path, "r+b") as f:
        data = bytearray(f.read())
        magic, = struct.unpack_from("<I", data, 0)
        if magic != 0xFEEDFACF:
            return 0  # only thin 64-bit images are prepared
        ncmds, = struct.unpack_from("<I", data, 16)
        offset = 32
        seen = set()
        changed = 0
        for _ in range(ncmds):
            cmd, size = struct.unpack_from("<II", data, offset)
            if cmd in DYLIB_COMMANDS:
                name_off, = struct.unpack_from("<I", data, offset + 8)
                start = offset + name_off
                end = data.index(b"\0", start)
                name = data[start:end].decode()
                if name in seen:
                    new = REPLACEMENTS.get(name)
                    if new is None or len(new) + 1 > size - name_off:
                        print(f"{path}: cannot rewrite duplicate {name}", file=sys.stderr)
                        return 1
                    data[start:offset + size] = new.encode().ljust(size - name_off, b"\0")
                    print(f"   {path}: duplicate {name} -> {new}")
                    changed += 1
                seen.add(name)
            offset += size
        if changed:
            f.seek(0)
            f.write(data)
    return 0


if __name__ == "__main__":
    sys.exit(max((dedupe(p) for p in sys.argv[1:]), default=0))
