#!/usr/bin/env python3
"""Behaviour probe of a PearHost build, without an app.

  tools/host-behavior-probe.py build/host > probe.txt
  tools/host-behavior-probe.py build/host --compare earlier-probe.txt

Loads UIKit, OpenGLES and MetalKit from <host build dir> into this (macOS,
arm64) Python process and exercises them the way an iOS app would: class
hierarchy, UIView accessors and deallocation (.cxx_destruct), KVC and the
display-link probes through the Objective-C exception boundaries (a real
exception is raised and caught inside the image), exported constants, and
EAGL contexts for GLES 1, 2 and 3. Output is one fact per line, so two
builds can be compared line by line (--compare exits 1 on any difference).
"""
import argparse
import ctypes
import ctypes.util
import sys


def probe(host):
    objc = ctypes.CDLL(ctypes.util.find_library("objc"))
    ctypes.CDLL("/System/Library/Frameworks/Foundation.framework/Foundation")
    uikit = ctypes.CDLL(f"{host}/UIKit.framework/UIKit", mode=ctypes.RTLD_GLOBAL)
    gles = ctypes.CDLL(f"{host}/OpenGLES.framework/OpenGLES", mode=ctypes.RTLD_GLOBAL)
    mtk = ctypes.CDLL(f"{host}/MetalKit.framework/MetalKit", mode=ctypes.RTLD_GLOBAL)
    for name in ("objc_getClass", "sel_registerName", "class_getSuperclass"):
        getattr(objc, name).restype = ctypes.c_void_p
    objc.objc_getClass.argtypes = objc.sel_registerName.argtypes = [ctypes.c_char_p]
    objc.class_getSuperclass.argtypes = [ctypes.c_void_p]
    objc.class_getName.restype, objc.class_getName.argtypes = ctypes.c_char_p, [ctypes.c_void_p]
    send = ctypes.cast(objc.objc_msgSend, ctypes.c_void_p).value

    def msg(restype, obj, sel, *args, argtypes=()):
        f = ctypes.CFUNCTYPE(restype, ctypes.c_void_p, ctypes.c_void_p, *argtypes)(send)
        return f(obj, objc.sel_registerName(sel.encode()), *args)

    def cls(name, lib=uikit):
        return ctypes.addressof(ctypes.c_char.in_dll(lib, "OBJC_CLASS_$_" + name))

    def nsstring(s):
        return msg(ctypes.c_void_p, objc.objc_getClass(b"NSString"), "stringWithUTF8String:", s.encode(),
                   argtypes=(ctypes.c_char_p,))

    def text(o):
        return msg(ctypes.c_char_p, o, "UTF8String").decode() if o else None

    out = []
    for lib, names in ((uikit, ["UIView", "UIWindow", "UIFont", "UILabel", "UITextView", "UIScene", "UIButton"]),
                       (gles, ["EAGLContext", "EAGLSharegroup"]), (mtk, ["MTKView"])):
        for n in names:
            out.append(f"{n} super={objc.class_getName(objc.class_getSuperclass(cls(n, lib))).decode()}")

    view = msg(ctypes.c_void_p, cls("UIView"), "new")
    msg(None, view, "setAlpha:", ctypes.c_double(0.25), argtypes=(ctypes.c_double,))
    out.append(f"alpha={msg(ctypes.c_double, view, 'alpha')}")
    msg(None, view, "setTag:", 42, argtypes=(ctypes.c_long,))
    out.append(f"tag={msg(ctypes.c_long, view, 'tag')}")
    out.append(f"hidden={msg(ctypes.c_bool, view, 'isHidden')}")

    try_kvc = uikit.PearUIKitTrySetValueForKey
    try_kvc.restype, try_kvc.argtypes = ctypes.c_bool, [ctypes.c_void_p] * 3
    number = msg(ctypes.c_void_p, objc.objc_getClass(b"NSNumber"), "numberWithInt:", 7, argtypes=(ctypes.c_int,))
    out.append(f"kvc tag={try_kvc(view, number, nsstring('tag'))} -> {msg(ctypes.c_long, view, 'tag')}")
    plain = msg(ctypes.c_void_p, objc.objc_getClass(b"NSObject"), "new")
    out.append(f"kvc unknown key={try_kvc(plain, number, nsstring('noSuchKey'))}")

    probe_sel = uikit.PearUIKitTryProbeDisplayLinkSelector
    probe_sel.restype, probe_sel.argtypes = ctypes.c_bool, [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_bool]
    out.append(f"probe getter ok={probe_sel(view, objc.sel_registerName(b'tag'), False)}")
    out.append(f"probe setter ok={probe_sel(view, objc.sel_registerName(b'setTag:'), True)} "
               f"tag={msg(ctypes.c_long, view, 'tag')}")
    out.append(f"probe raising={probe_sel(plain, objc.sel_registerName(b'noSuchSelectorAtAll'), False)}")

    for lib, name in ((uikit, "UIKeyboardWillShowNotification"), (uikit, "UIApplicationOpenSettingsURLString"),
                      (uikit, "UIContentSizeCategoryLarge"), (gles, "kEAGLColorFormatRGBA8")):
        out.append(f"{name}={text(ctypes.c_void_p.in_dll(lib, name).value)}")
    out.append(f"UIFontWeightBold={ctypes.c_double.in_dll(uikit, 'UIFontWeightBold').value}")
    out.append(f"UIWindowLevelNormal={ctypes.c_double.in_dll(uikit, 'UIWindowLevelNormal').value}")

    font_class = uikit.pear_uikit_uifont_class
    font_class.restype = ctypes.c_void_p
    out.append(f"uifont class is image's={font_class() == cls('UIFont')}")

    # A named pasteboard only: the user's general pasteboard is never touched.
    board = msg(ctypes.c_void_p, cls("UIPasteboard"), "pasteboardWithName:create:", nsstring("pear-probe"), True,
                argtypes=(ctypes.c_void_p, ctypes.c_bool))
    out.append(f"pasteboard name={text(msg(ctypes.c_void_p, board, 'name'))}")
    before = msg(ctypes.c_long, board, "changeCount")
    msg(None, board, "setString:", nsstring("pear text"), argtypes=(ctypes.c_void_p,))
    out.append(f"pasteboard string={text(msg(ctypes.c_void_p, board, 'string'))} "
               f"hasStrings={msg(ctypes.c_bool, board, 'hasStrings')} hasURLs={msg(ctypes.c_bool, board, 'hasURLs')} "
               f"changed={msg(ctypes.c_long, board, 'changeCount') > before}")
    url = msg(ctypes.c_void_p, objc.objc_getClass(b"NSURL"), "URLWithString:", nsstring("https://example.com/pear"),
              argtypes=(ctypes.c_void_p,))
    msg(None, board, "setURL:", url, argtypes=(ctypes.c_void_p,))
    got = msg(ctypes.c_void_p, board, "URL")
    out.append(f"pasteboard URL={text(msg(ctypes.c_void_p, got, 'absoluteString')) if got else None} "
               f"hasURLs={msg(ctypes.c_bool, board, 'hasURLs')}")
    data = msg(ctypes.c_void_p, objc.objc_getClass(b"NSData"), "dataWithBytes:length:", b"\x01\x02\x03", 3,
               argtypes=(ctypes.c_char_p, ctypes.c_ulong))
    msg(None, board, "setData:forPasteboardType:", data, nsstring("com.pearsilicon.probe"),
        argtypes=(ctypes.c_void_p, ctypes.c_void_p))
    back = msg(ctypes.c_void_p, board, "dataForPasteboardType:", nsstring("com.pearsilicon.probe"), argtypes=(ctypes.c_void_p,))
    out.append(f"pasteboard data length={msg(ctypes.c_ulong, back, 'length') if back else None} "
               f"string={text(msg(ctypes.c_void_p, board, 'string'))}")
    msg(None, board, "setString:", None, argtypes=(ctypes.c_void_p,))
    out.append(f"pasteboard cleared hasStrings={msg(ctypes.c_bool, board, 'hasStrings')}")
    # UIFont: which font each constructor picks, and its metrics.
    font = cls("UIFont")

    def font_fact(label, f):
        if not f:
            return f"font {label}=nil"
        return (f"font {label}={text(msg(ctypes.c_void_p, f, 'fontName'))} "
                f"family={text(msg(ctypes.c_void_p, f, 'familyName'))} size={msg(ctypes.c_double, f, 'pointSize'):g} "
                f"line={msg(ctypes.c_double, f, 'lineHeight'):.2f}")
    d = ctypes.c_double
    out.append(font_fact("system 17", msg(ctypes.c_void_p, font, "systemFontOfSize:", d(17), argtypes=(d,))))
    out.append(font_fact("bold 12", msg(ctypes.c_void_p, font, "boldSystemFontOfSize:", d(12), argtypes=(d,))))
    out.append(font_fact("italic 12", msg(ctypes.c_void_p, font, "italicSystemFontOfSize:", d(12), argtypes=(d,))))
    for w in (-0.4, 0.3, 0.62):
        out.append(font_fact(f"weight {w}", msg(ctypes.c_void_p, font, "systemFontOfSize:weight:", d(14), d(w),
                                                 argtypes=(d, d))))
    for n in ("Helvetica-Bold", "Arial", "Courier New", "NoSuchFont"):
        out.append(font_fact(f"named {n}", msg(ctypes.c_void_p, font, "fontWithName:size:", nsstring(n), d(15),
                                                argtypes=(ctypes.c_void_p, d))))
    resized = msg(ctypes.c_void_p, msg(ctypes.c_void_p, font, "systemFontOfSize:", d(10), argtypes=(d,)),
                  "fontWithSize:", d(22), argtypes=(d,))
    out.append(font_fact("resized 22", resized))
    for fam in ("Futura", "Avenir Next", "NoSuch"):
        names = msg(ctypes.c_void_p, font, "fontNamesForFamilyName:", nsstring(fam), argtypes=(ctypes.c_void_p,))
        count = msg(ctypes.c_ulong, names, "count")
        out.append(f"font family {fam} count={count} names="
                   + ",".join(text(msg(ctypes.c_void_p, names, "objectAtIndex:", i, argtypes=(ctypes.c_ulong,)))
                              for i in range(count)))
    # A UILabel drawn into an image context: its pixels, hashed.
    class Rect(ctypes.Structure):
        _fields_ = [("x", d), ("y", d), ("w", d), ("h", d)]

    class Size(ctypes.Structure):
        _fields_ = [("w", d), ("h", d)]
    label = msg(ctypes.c_void_p, msg(ctypes.c_void_p, cls("UILabel"), "alloc"), "initWithFrame:",
                Rect(0, 0, 160, 40), argtypes=(Rect,))
    msg(None, label, "setText:", nsstring("Pear 123 ağ"), argtypes=(ctypes.c_void_p,))
    msg(None, label, "setFont:", msg(ctypes.c_void_p, font, "boldSystemFontOfSize:", d(18), argtypes=(d,)),
        argtypes=(ctypes.c_void_p,))
    fits = msg(Size, label, "sizeThatFits:", Size(1000, 1000), argtypes=(Size,))
    begin = uikit.UIGraphicsBeginImageContextWithOptions
    begin.argtypes = [Size, ctypes.c_bool, d]
    begin(Size(160, 40), False, 2.0)
    msg(None, label, "drawRect:", Rect(0, 0, 160, 40), argtypes=(Rect,))
    get_image = uikit.UIGraphicsGetImageFromCurrentImageContext
    get_image.restype = ctypes.c_void_p
    image = get_image()
    uikit.UIGraphicsEndImageContext()
    cg = msg(ctypes.c_void_p, image, "CGImage")
    cgl = ctypes.CDLL("/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics")
    for name in ("CGImageGetDataProvider", "CGDataProviderCopyData"):
        getattr(cgl, name).restype = ctypes.c_void_p
        getattr(cgl, name).argtypes = [ctypes.c_void_p]
    pixels = cgl.CGDataProviderCopyData(cgl.CGImageGetDataProvider(cg))
    raw = ctypes.string_at(msg(ctypes.c_void_p, pixels, "bytes"), msg(ctypes.c_ulong, pixels, "length"))
    import hashlib
    out.append(f"label fits={fits.w:.2f}x{fits.h:.2f} pixels={len(raw)} inked={sum(1 for b in raw[3::4] if b)} "
               f"sha={hashlib.sha256(raw).hexdigest()[:16]}")
    families = msg(ctypes.c_void_p, font, "familyNames")
    out.append(f"font familyNames nonempty={msg(ctypes.c_ulong, families, 'count') > 0}")

    voice = uikit.UIAccessibilityIsVoiceOverRunning
    voice.restype = ctypes.c_bool
    out.append(f"VoiceOver running={voice()}")

    eagl = cls("EAGLContext", gles)
    for api in (1, 2, 3):
        ctx = msg(ctypes.c_void_p, msg(ctypes.c_void_p, eagl, "alloc"), "initWithAPI:", api,
                  argtypes=(ctypes.c_ulong,))
        current = msg(ctypes.c_bool, eagl, "setCurrentContext:", ctx, argtypes=(ctypes.c_void_p,))
        fb = ctypes.c_uint()
        getattr(gles, "glGenFramebuffersOES" if api == 1 else "glGenFramebuffers")(1, ctypes.byref(fb))
        out.append(f"EAGL GLES{api}: context={bool(ctx)} current={current} framebuffer={fb.value != 0}")
    msg(ctypes.c_bool, eagl, "setCurrentContext:", None, argtypes=(ctypes.c_void_p,))

    window = msg(ctypes.c_void_p, cls("UIWindow"), "new")
    msg(None, window, "release")
    msg(None, view, "release")
    out.append("dealloc ok")
    return out


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("build", help="host build directory (contains UIKit.framework)")
    parser.add_argument("--compare", help="earlier probe output to compare with")
    args = parser.parse_args()
    lines = probe(args.build)
    if not args.compare:
        print("\n".join(lines))
        return 0
    before = open(args.compare).read().splitlines()
    if before == lines:
        print(f"probe identical ({len(lines)} facts)")
        return 0
    for line in sorted(set(before) - set(lines)):
        print("- " + line)
    for line in sorted(set(lines) - set(before)):
        print("+ " + line)
    return 1


if __name__ == "__main__":
    sys.exit(main())
