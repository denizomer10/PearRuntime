#!/usr/bin/env python3
"""Rewrites duplicate dylib load commands of a Mach-O image in place.

dyld refuses an image that names the same dylib twice ("duplicate linked
dylib"). A bundle prepared for Mac Catalyst can carry such a pair: its
MobileCoreServices load command was rewritten to CoreServices, which the
binary already linked. Every later occurrence of a known duplicate is pointed
at a PearHost framework that re-exports the same library, so both load
commands keep their ordinals and every import still binds.

  tools/host-dedupe-dylibs.py [--load-uikit] <Mach-O> [...]

Use --load-uikit on a prepared executable to load PearHost at startup, so
dyld sees its interposers even if UnityFramework is opened later.

Exit status 0 when nothing needed changing or every duplicate was rewritten.
"""
import struct
import sys

LC_REQ_DYLD = 0x80000000
DYLIB_COMMANDS = {0xC, 0x18 | LC_REQ_DYLD, 0x1F | LC_REQ_DYLD, 0x23 | LC_REQ_DYLD, 0x20 | LC_REQ_DYLD}
UIKIT = "@rpath/UIKit.framework/UIKit"
# Duplicate -> replacement: a PearHost framework re-exporting the same library.
REPLACEMENTS = {
    "/System/Library/Frameworks/CoreServices.framework/CoreServices":
        "@rpath/MobileCoreServices.framework/MobileCoreServices",
}


def dedupe(path, load_uikit=False):
    with open(path, "r+b") as f:
        data = bytearray(f.read())
        magic, = struct.unpack_from("<I", data, 0)
        if magic != 0xFEEDFACF:
            return 0  # only thin 64-bit images are prepared
        ncmds, command_bytes = struct.unpack_from("<II", data, 16)
        offset = 32
        seen = set()
        changed = 0
        first_section = len(data)
        for _ in range(ncmds):
            cmd, size = struct.unpack_from("<II", data, offset)
            if load_uikit and cmd == 0x19:  # LC_SEGMENT_64
                section_count, = struct.unpack_from("<I", data, offset + 64)
                for i in range(section_count):
                    section = offset + 72 + i * 80
                    file_offset, = struct.unpack_from("<I", data, section + 48)
                    if file_offset:
                        first_section = min(first_section, file_offset)
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
        if load_uikit and UIKIT not in seen:
            name = UIKIT.encode() + b"\0"
            size = (24 + len(name) + 7) & ~7
            if offset != 32 + command_bytes or offset + size > first_section or any(data[offset:offset + size]):
                print(f"{path}: insufficient header padding to load PearHost UIKit", file=sys.stderr)
                return 1
            struct.pack_into("<IIIIII", data, offset, 0xC, size, 24, 0, 0, 0)
            data[offset + 24:offset + size] = name.ljust(size - 24, b"\0")
            struct.pack_into("<II", data, 16, ncmds + 1, command_bytes + size)
            changed += 1
        if changed:
            f.seek(0)
            f.write(data)
    return 0


if __name__ == "__main__":
    args = sys.argv[1:]
    load_uikit = bool(args and args[0] == "--load-uikit")
    paths = args[1:] if load_uikit else args
    sys.exit(max((dedupe(p, load_uikit) for p in paths), default=0))
