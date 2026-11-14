#!/usr/bin/env python3
"""Dump a compiled iOS nib in the NIBArchive container format.

Reads the app's own resource file; used to understand what host/uikit/src/
UINib.m must decode. Layout (all integers little endian):

  "NIBArchive" u32 1 u32 10
  u32 objectCount, objectsOffset, keyCount, keysOffset,
      valueCount, valuesOffset, classCount, classesOffset
  object : varint classIndex, varint firstValue, varint valueCount
  key    : varint length, bytes
  value  : varint keyIndex, u8 type, payload
           0 i8, 1 i16, 2 i32, 3 i64, 4 true, 5 false, 6 f32, 7 f64,
           8 data (varint length + bytes), 9 nil, 10 object ref (u32)
  class  : varint length, varint extraCount, extraCount * u32, name (NUL-terminated)

varint: 7 bits per byte, least significant group first; the byte with the
high bit SET is the last one.

  tools/nibdump.py path/to/MainWindow.nib
"""
import struct
import sys


def varint(b, i):
    v = shift = 0
    while True:
        c = b[i]
        i += 1
        v |= (c & 0x7F) << shift
        shift += 7
        if c & 0x80:
            return v, i


def parse(b):
    assert b[:10] == b"NIBArchive", "not a NIBArchive"
    (oc, oo, kc, ko, vc, vo, cc, co) = struct.unpack_from("<8I", b, 18)
    keys, i = [], ko
    for _ in range(kc):
        n, i = varint(b, i)
        keys.append(b[i:i + n].decode())
        i += n
    classes, i = [], co
    for _ in range(cc):
        n, i = varint(b, i)
        extra, i = varint(b, i)
        i += 4 * extra
        classes.append(b[i:i + n].rstrip(b"\0").decode())
        i += n
    values, i = [], vo
    for _ in range(vc):
        k, i = varint(b, i)
        t = b[i]
        i += 1
        if t == 0: v = struct.unpack_from("<b", b, i)[0]; i += 1
        elif t == 1: v = struct.unpack_from("<h", b, i)[0]; i += 2
        elif t == 2: v = struct.unpack_from("<i", b, i)[0]; i += 4
        elif t == 3: v = struct.unpack_from("<q", b, i)[0]; i += 8
        elif t == 4: v = True
        elif t == 5: v = False
        elif t == 6: v = struct.unpack_from("<f", b, i)[0]; i += 4
        elif t == 7: v = struct.unpack_from("<d", b, i)[0]; i += 8
        elif t == 8:
            n, i = varint(b, i)
            v = b[i:i + n]
            i += n
        elif t == 9: v = None
        elif t == 10: v = ("@", struct.unpack_from("<I", b, i)[0]); i += 4
        else: raise ValueError(f"value type {t}")
        values.append((keys[k], t, v))
    objects, i = [], oo
    for _ in range(oc):
        c, i = varint(b, i)
        f, i = varint(b, i)
        n, i = varint(b, i)
        objects.append((classes[c], values[f:f + n]))
    return objects


def main():
    objs = parse(open(sys.argv[1], "rb").read())
    for n, (cls, vals) in enumerate(objs):
        print(f"#{n} {cls}")
        for k, t, v in vals:
            if isinstance(v, bytes):
                v = v.decode(errors="replace") if v.isascii() and len(v) < 80 else v.hex()
            print(f"    {k} = {v!r}")


if __name__ == "__main__":
    main()
