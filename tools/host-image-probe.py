#!/usr/bin/env python3
"""UIImage decoding of a PearHost build against ImageIO, file by file.

  find Game.app -name '*.png' | tools/host-image-probe.py build/host

Loads UIKit from <host build dir>, makes `UIImage imageWithData:` of each
file and compares its CGImage with the one ImageIO makes of the same bytes:
layout (bits per component/pixel, bitmap info, colour model, bytes per row),
raw provider bytes, and pixels drawn into an sRGB RGBA8 bitmap. Prints a
line per file that is not identical in all three, and a summary. With
PEAR_IMAGE_DECODER=imageio UIKit uses ImageIO itself (a control run).
"""
import collections
import ctypes
import ctypes.util
import sys

V = ctypes.c_void_p


def main(host):
    objc = ctypes.CDLL(ctypes.util.find_library("objc"))
    ctypes.CDLL("/System/Library/Frameworks/Foundation.framework/Foundation")
    uikit = ctypes.CDLL(f"{host}/UIKit.framework/UIKit", mode=ctypes.RTLD_GLOBAL)
    cf = ctypes.CDLL("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation")
    io = ctypes.CDLL("/System/Library/Frameworks/ImageIO.framework/ImageIO")
    cg = ctypes.CDLL("/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics")
    objc.sel_registerName.restype = V
    objc.sel_registerName.argtypes = [ctypes.c_char_p]
    send = ctypes.cast(objc.objc_msgSend, V).value

    def msg(restype, obj, sel, *args, argtypes=()):
        return ctypes.CFUNCTYPE(restype, V, V, *argtypes)(send)(obj, objc.sel_registerName(sel.encode()), *args)

    uiimage = ctypes.addressof(ctypes.c_char.in_dll(uikit, "OBJC_CLASS_$_UIImage"))
    cf.CFDataCreate.restype = V
    cf.CFDataCreate.argtypes = [V, ctypes.c_char_p, ctypes.c_long]
    cf.CFRelease.argtypes = [V]
    cf.CFDataGetLength.restype = ctypes.c_long
    cf.CFDataGetLength.argtypes = [V]
    cf.CFDataGetBytePtr.restype = V
    cf.CFDataGetBytePtr.argtypes = [V]
    io.CGImageSourceCreateWithData.restype = V
    io.CGImageSourceCreateWithData.argtypes = [V, V]
    io.CGImageSourceCreateImageAtIndex.restype = V
    io.CGImageSourceCreateImageAtIndex.argtypes = [V, ctypes.c_size_t, V]
    for f in ("CGImageGetWidth", "CGImageGetHeight", "CGImageGetBitsPerComponent", "CGImageGetBitsPerPixel",
              "CGImageGetBytesPerRow", "CGImageGetBitmapInfo"):
        getattr(cg, f).restype = ctypes.c_size_t
        getattr(cg, f).argtypes = [V]
    for f in ("CGImageGetColorSpace", "CGImageGetDataProvider", "CGDataProviderCopyData"):
        getattr(cg, f).restype = V
        getattr(cg, f).argtypes = [V]
    cg.CGColorSpaceGetModel.argtypes = [V]
    cg.CGColorSpaceCreateWithName.restype = V
    cg.CGColorSpaceCreateWithName.argtypes = [V]
    cg.CGBitmapContextCreate.restype = V
    cg.CGBitmapContextCreate.argtypes = [V, ctypes.c_size_t, ctypes.c_size_t, ctypes.c_size_t, ctypes.c_size_t, V,
                                         ctypes.c_uint32]

    class Rect(ctypes.Structure):
        _fields_ = [("x", ctypes.c_double), ("y", ctypes.c_double), ("w", ctypes.c_double), ("h", ctypes.c_double)]
    cg.CGContextDrawImage.argtypes = [V, Rect, V]
    cg.CGContextRelease.argtypes = [V]
    srgb = cg.CGColorSpaceCreateWithName(V.in_dll(cg, "kCGColorSpaceSRGB").value)

    def layout(img):
        return (cg.CGImageGetWidth(img), cg.CGImageGetHeight(img), cg.CGImageGetBitsPerComponent(img),
                cg.CGImageGetBitsPerPixel(img), cg.CGImageGetBytesPerRow(img), cg.CGImageGetBitmapInfo(img),
                cg.CGColorSpaceGetModel(cg.CGImageGetColorSpace(img)))

    def raw(img):
        d = cg.CGDataProviderCopyData(cg.CGImageGetDataProvider(img))
        out = ctypes.string_at(cf.CFDataGetBytePtr(d), cf.CFDataGetLength(d))
        cf.CFRelease(d)
        return out

    def drawn(img):
        w, h = cg.CGImageGetWidth(img), cg.CGImageGetHeight(img)
        buf = ctypes.create_string_buffer(w * h * 4)
        ctx = cg.CGBitmapContextCreate(buf, w, h, 8, w * 4, srgb, 1 | (4 << 12))
        cg.CGContextDrawImage(ctx, Rect(0, 0, w, h), img)
        cg.CGContextRelease(ctx)
        return buf.raw

    counts = collections.Counter()
    for path in sys.stdin.read().split("\n"):
        if not path:
            continue
        data = open(path, "rb").read()
        if not (data.startswith(b"\x89PNG") or data.startswith(b"\xff\xd8\xff")):
            continue
        cfdata = cf.CFDataCreate(None, data, len(data))
        src = io.CGImageSourceCreateWithData(cfdata, None)
        theirs = io.CGImageSourceCreateImageAtIndex(src, 0, None)
        image = msg(V, uiimage, "imageWithData:", cfdata, argtypes=(V,))
        ours = msg(V, image, "CGImage") if image else None
        if not ours or not theirs:
            counts["missing image"] += 1
            print(f"MISSING ours={bool(ours)} imageio={bool(theirs)}\t{path}")
            continue
        la, lb = layout(ours), layout(theirs)
        same_raw = raw(ours) == raw(theirs)
        da, db = drawn(ours), drawn(theirs)
        maxdiff = max((abs(a - b) for a, b in zip(da, db)), default=0) if da != db else 0
        key = ("layout " if la != lb else "") + ("bytes " if not same_raw else "") + (f"drawn" if maxdiff else "")
        counts[key.strip() or "identical"] += 1
        if key:
            print(f"{key.strip()}\tmax={maxdiff}\tours={la} imageio={lb}\t{path}")
        cf.CFRelease(theirs)
        cf.CFRelease(src)
        cf.CFRelease(cfdata)
    print("summary: " + ", ".join(f"{n} {k}" for k, n in counts.most_common()), file=sys.stderr)


if __name__ == "__main__":
    main(sys.argv[1])
