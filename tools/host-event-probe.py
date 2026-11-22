#!/usr/bin/env python3
"""Input path probe of a PearHost build, without an app or a visible window.

  tools/host-event-probe.py build/host

Loads UIKit from <host build dir>, makes a UIWindow with a recording UIView,
asks the host backend for the native window (never ordered on screen) and
sends synthetic AppKit events straight to its content view. The events go
AppKit -> pear-host-macos -> pear_host::events() -> pear-uikit, so the output
checks touch coordinates, key delivery, and view layout before and after
shrinking and enlarging the native window.
"""
import ctypes
import ctypes.util
import sys


class CGPoint(ctypes.Structure):
    _fields_ = [("x", ctypes.c_double), ("y", ctypes.c_double)]


class CGSize(ctypes.Structure):
    _fields_ = [("w", ctypes.c_double), ("h", ctypes.c_double)]


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
    for kind in ("touchesBegan", "touchesMoved", "touchesEnded", "touchesCancelled",
                 "pressesBegan", "pressesEnded"):
        objc.class_addMethod(view_cls, sel(kind + ":withEvent:"), ctypes.cast(recorder(kind), ctypes.c_void_p),
                             b"v@:@@")
    yes = ctypes.CFUNCTYPE(ctypes.c_bool, ctypes.c_void_p, ctypes.c_void_p)(lambda s, c: True)
    imps.append(yes)
    objc.class_addMethod(view_cls, sel("canBecomeFirstResponder"), ctypes.cast(yes, ctypes.c_void_p), b"B@:")

    @ctypes.CFUNCTYPE(None, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p)
    def gesture_action(view, cmd, recognizer):
        state = msg(ctypes.c_long, recognizer, "state")
        if cmd == sel("panAction:"):
            travel = msg(CGPoint, recognizer, "translationInView:", view, argtypes=(ctypes.c_void_p,))
            log.append(f"pan state={state} x={travel.x:.0f}")
        else:
            log.append(f"swipe state={state}")

    for action in ("panAction:", "swipeAction:"):
        objc.class_addMethod(view_cls, sel(action), ctypes.cast(gesture_action, ctypes.c_void_p), b"v@:@")
    objc.objc_registerClassPair(view_cls)

    msg(ctypes.c_void_p, objc.objc_getClass(b"NSApplication"), "sharedApplication")
    early_screen = msg(ctypes.c_void_p, cls("UIScreen"), "mainScreen")
    msg(CGRect, early_screen, "bounds")
    initialize = uikit.PearBackendInitialize
    initialize.argtypes, initialize.restype = [], None
    initialize()
    screen = msg(ctypes.c_void_p, cls("UIScreen"), "mainScreen")
    screen_bounds = msg(CGRect, screen, "bounds")
    if screen == early_screen and screen_bounds.w > 0 and screen_bounds.h > 0:
        log.append("early UIScreen received host bounds")

    orientation_queries = []
    orientation_selector = "supportedInterfaceOrientationsForWindow:"

    @ctypes.CFUNCTYPE(ctypes.c_ulong, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p)
    def delegate_orientations(delegate, cmd, application, window):
        orientation_queries.append(True)
        if len(orientation_queries) > 1:
            return 0
        return msg(ctypes.c_ulong, application, orientation_selector, window, argtypes=(ctypes.c_void_p,))

    delegate_cls = objc.objc_allocateClassPair(objc.objc_getClass(b"NSObject"), b"PearProbeOrientationDelegate", 0)
    objc.class_addMethod(delegate_cls, sel("application:supportedInterfaceOrientationsForWindow:"),
                         ctypes.cast(delegate_orientations, ctypes.c_void_p), b"Q@:@@")
    objc.objc_registerClassPair(delegate_cls)
    application = msg(ctypes.c_void_p, cls("UIApplication"), "new")
    delegate = msg(ctypes.c_void_p, delegate_cls, "new")
    msg(None, application, "setDelegate:", delegate, argtypes=(ctypes.c_void_p,))
    mask = msg(ctypes.c_ulong, application, orientation_selector, None, argtypes=(ctypes.c_void_p,))
    if mask and len(orientation_queries) == 1:
        log.append("recursive orientation query used the bundle policy")
    msg(None, application, "setDelegate:", None, argtypes=(ctypes.c_void_p,))
    msg(None, delegate, "release")

    frame = CGRect(0, 0, 320, 480)
    window = msg(ctypes.c_void_p, msg(ctypes.c_void_p, cls("UIWindow"), "alloc"), "initWithFrame:", frame,
                 argtypes=(CGRect,))
    view = msg(ctypes.c_void_p, msg(ctypes.c_void_p, view_cls, "alloc"), "initWithFrame:", frame, argtypes=(CGRect,))
    msg(None, view, "setAutoresizingMask:", (1 << 1) | (1 << 4), argtypes=(ctypes.c_ulong,))
    msg(None, window, "addSubview:", view, argtypes=(ctypes.c_void_p,))
    log.append(f"becomeFirstResponder={msg(ctypes.c_bool, view, 'becomeFirstResponder')}")

    create = uikit.PearBackendCreateWindow
    create.restype, create.argtypes = ctypes.c_void_p, [ctypes.c_void_p, CGRect]
    native = create(window, frame)
    mount = uikit.PearBackendSetRootLayer
    mount.argtypes, mount.restype = [ctypes.c_void_p, ctypes.c_void_p], None
    mount(native, msg(ctypes.c_void_p, window, "layer"))
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

    for width, height in ((240, 360), (400, 600)):
        msg(None, native, "setContentSize:", CGSize(width, height), argtypes=(CGSize,))
        bounds = msg(CGRect, view, "bounds")
        log.append(f"view resized to {bounds.w:.0f}x{bounds.h:.0f}")
        # Tap near the right edge, outside the old bounds after enlarging.
        x = width - 5
        for selector, kind, t in (("mouseDown:", 1, 3.0), ("mouseUp:", 2, 3.1)):
            msg(None, content, selector, mouse(kind, x, height - 25, t), argtypes=(ctypes.c_void_p,))

    def gesture(class_name, action, settings, start, end):
        recognizer = msg(ctypes.c_void_p, msg(ctypes.c_void_p, cls(class_name), "alloc"),
                         "initWithTarget:action:", view, sel(action),
                         argtypes=(ctypes.c_void_p, ctypes.c_void_p))
        for setter, value in settings:
            msg(None, recognizer, setter, value, argtypes=(ctypes.c_ulong,))
        msg(None, view, "addGestureRecognizer:", recognizer, argtypes=(ctypes.c_void_p,))
        for selector, kind, x, timestamp in (("mouseDown:", 1, start, 4.0),
                                             ("mouseDragged:", 6, (start + end) / 2, 4.1),
                                             ("mouseUp:", 2, end, 4.2)):
            msg(None, content, selector, mouse(kind, x, 300, timestamp), argtypes=(ctypes.c_void_p,))
        msg(None, view, "removeGestureRecognizer:", recognizer, argtypes=(ctypes.c_void_p,))
        msg(None, recognizer, "release")

    gesture("UIPanGestureRecognizer", "panAction:",
            (("setMaximumNumberOfTouches:", 1), ("setAllowedScrollTypesMask:", 1)), 100, 170)
    gesture("UISwipeGestureRecognizer", "swipeAction:",
            (("setDirection:", 2), ("setNumberOfTouchesRequired:", 1)), 170, 100)
    msg(None, native, "close")
    print("\n".join(log))
    expected = ["early UIScreen received host bounds", "recursive orientation query used the bundle policy",
                "touchesBegan at (100,100)", "touchesMoved at (120,120)", "touchesEnded at (120,120)",
                "pressesBegan usage=0x4", "pressesEnded usage=0x4",
                "view resized to 240x360", "touchesBegan at (235,25)", "touchesEnded at (235,25)",
                "view resized to 400x600", "touchesBegan at (395,25)", "touchesEnded at (395,25)",
                "pan state=1 x=35", "pan state=3 x=70", "swipe state=3",
                "touchesCancelled at (135,300)", "touchesCancelled at (100,300)"]
    missing = [e for e in expected if e not in log]
    for e in missing:
        print("missing: " + e)
    return 1 if missing else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1]))
