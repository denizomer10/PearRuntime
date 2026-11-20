#!/usr/bin/env python3
"""Input path probe of a PearHost build, without an app or a visible window.

  tools/host-event-probe.py build/host

Loads UIKit from <host build dir>, makes a UIWindow with a recording UIView,
asks the host backend for the native window (never ordered on screen) and
sends synthetic AppKit events straight to its content view. The events go
AppKit -> pear-host-macos -> pear_host::events() -> pear-uikit, so the output
shows whether the guest view got touchesBegan/Moved/Ended and pressesBegan/
Ended, and where. Exits 1 if any expected delivery is missing.
"""
import ctypes
import ctypes.util
import sys


class CGPoint(ctypes.Structure):
    _fields_ = [("x", ctypes.c_double), ("y", ctypes.c_double)]


class CGRect(ctypes.Structure):
    _fields_ = [("x", ctypes.c_double), ("y", ctypes.c_double), ("w", ctypes.c_double), ("h", ctypes.c_double)]


def main(host):
    objc = ctypes.CDLL(ctypes.util.find_library("objc"))
    ctypes.CDLL("/System/Library/Frameworks/AppKit.framework/AppKit")
    uikit = ctypes.CDLL(f"{host}/UIKit.framework/UIKit", mode=ctypes.RTLD_GLOBAL)
    for name in ("objc_getClass", "sel_registerName", "objc_allocateClassPair"):
        getattr(objc, name).restype = ctypes.c_void_p
    objc.objc_getClass.argtypes = objc.sel_registerName.argtypes = [ctypes.c_char_p]
    objc.objc_allocateClassPair.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_size_t]
    objc.class_addMethod.argtypes = [ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_char_p]
    objc.objc_registerClassPair.argtypes = [ctypes.c_void_p]
    send = ctypes.cast(objc.objc_msgSend, ctypes.c_void_p).value

    def sel(s):
        return objc.sel_registerName(s.encode())

    def msg(restype, obj, s, *args, argtypes=()):
        return ctypes.CFUNCTYPE(restype, ctypes.c_void_p, ctypes.c_void_p, *argtypes)(send)(obj, sel(s), *args)

    def cls(name):
        return ctypes.addressof(ctypes.c_char.in_dll(uikit, "OBJC_CLASS_$_" + name))

    log = []
    imps = []

    def recorder(kind):
        def imp(this, _cmd, items, _event):
            any_item = msg(ctypes.c_void_p, items, "anyObject")
            if kind.startswith("touches"):
                p = msg(CGPoint, any_item, "locationInView:", this, argtypes=(ctypes.c_void_p,))
                log.append(f"{kind} at ({p.x:.0f},{p.y:.0f})")
            else:
                key = msg(ctypes.c_void_p, any_item, "key")
                usage = msg(ctypes.c_long, key, "keyCode") if key else -1
                log.append(f"{kind} usage=0x{usage:x}")
        f = ctypes.CFUNCTYPE(None, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p)(imp)
        imps.append(f)
        return f

    view_cls = objc.objc_allocateClassPair(cls("UIView"), b"PearProbeRecordingView", 0)
    for kind in ("touchesBegan", "touchesMoved", "touchesEnded", "pressesBegan", "pressesEnded"):
        objc.class_addMethod(view_cls, sel(kind + ":withEvent:"), ctypes.cast(recorder(kind), ctypes.c_void_p),
                             b"v@:@@")
    yes = ctypes.CFUNCTYPE(ctypes.c_bool, ctypes.c_void_p, ctypes.c_void_p)(lambda s, c: True)
    imps.append(yes)
    objc.class_addMethod(view_cls, sel("canBecomeFirstResponder"), ctypes.cast(yes, ctypes.c_void_p), b"B@:")
    objc.objc_registerClassPair(view_cls)

    msg(ctypes.c_void_p, objc.objc_getClass(b"NSApplication"), "sharedApplication")
    frame = CGRect(0, 0, 320, 480)
    window = msg(ctypes.c_void_p, msg(ctypes.c_void_p, cls("UIWindow"), "alloc"), "initWithFrame:", frame,
                 argtypes=(CGRect,))
    view = msg(ctypes.c_void_p, msg(ctypes.c_void_p, view_cls, "alloc"), "initWithFrame:", frame, argtypes=(CGRect,))
    msg(None, window, "addSubview:", view, argtypes=(ctypes.c_void_p,))
    log.append(f"becomeFirstResponder={msg(ctypes.c_bool, view, 'becomeFirstResponder')}")

    create = uikit.PearBackendCreateWindow
    create.restype, create.argtypes = ctypes.c_void_p, [ctypes.c_void_p, CGRect]
    native = create(window, frame)
    content = msg(ctypes.c_void_p, native, "contentView")
    log.append("native window ok" if native and content else "native window missing")
    number = msg(ctypes.c_long, native, "windowNumber")

    event_cls = objc.objc_getClass(b"NSEvent")
    mouse_args = (ctypes.c_ulong, CGPoint, ctypes.c_ulong, ctypes.c_double, ctypes.c_long, ctypes.c_void_p,
                  ctypes.c_long, ctypes.c_long, ctypes.c_float)

    def mouse(kind, x, y, t):
        return msg(ctypes.c_void_p, event_cls,
                   "mouseEventWithType:location:modifierFlags:timestamp:windowNumber:context:eventNumber:clickCount:pressure:",
                   kind, CGPoint(x, y), 0, t, number, None, 0, 1, 1.0, argtypes=mouse_args)

    def nsstring(s):
        return msg(ctypes.c_void_p, objc.objc_getClass(b"NSString"), "stringWithUTF8String:", s.encode(),
                   argtypes=(ctypes.c_char_p,))

    def key(kind, chars, code, t):
        return msg(ctypes.c_void_p, event_cls,
                   "keyEventWithType:location:modifierFlags:timestamp:windowNumber:context:characters:"
                   "charactersIgnoringModifiers:isARepeat:keyCode:",
                   kind, CGPoint(0, 0), 0, t, number, None, nsstring(chars), nsstring(chars), False, code,
                   argtypes=(ctypes.c_ulong, CGPoint, ctypes.c_ulong, ctypes.c_double, ctypes.c_long,
                             ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_bool, ctypes.c_ushort))

    # NSEventType: LeftMouseDown=1, LeftMouseUp=2, LeftMouseDragged=6, KeyDown=10, KeyUp=11.
    # Window coordinates have their origin at the bottom left; (100, 380) is (100, 100) from the top.
    for selector, ev in (("mouseDown:", mouse(1, 100, 380, 1.0)), ("mouseDragged:", mouse(6, 120, 360, 1.1)),
                         ("mouseUp:", mouse(2, 120, 360, 1.2)), ("keyDown:", key(10, "a", 0, 2.0)),
                         ("keyUp:", key(11, "a", 0, 2.1))):
        msg(None, content, selector, ev, argtypes=(ctypes.c_void_p,))

    msg(None, native, "close")
    print("\n".join(log))
    expected = ["touchesBegan at (100,100)", "touchesMoved at (120,120)", "touchesEnded at (120,120)",
                "pressesBegan usage=0x4", "pressesEnded usage=0x4"]
    missing = [e for e in expected if e not in log]
    for e in missing:
        print("missing: " + e)
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
