//! UIView, UIScrollView, UIViewController, UIWindow, UINavigationController,
//! UITabBarController, UIBarItem, UIBarButtonItem, UIAlertAction, UIAlertController,
//! UIActivityViewController, UIDocumentPickerViewController, UIImagePickerController,
//! UIDocument, UIPopoverController, and PearUIKitWindowDidResize.

use pear_objc::{
    cls, is_kind_of, msg0, msg1, msg2, msg3, nsarray_count, nsarray_from_ids,
    nsarray_object_at, nsdata_from_bytes, nsdata_to_vec, nsnumber_bool, nsnumber_f64, nsnumber_i64,
    nsstring, nsstring_to_string, objc_getAssociatedObject, objc_msgSendSuper, objc_release,
    objc_retain, objc_setAssociatedObject, replace_method as add_method, responds_to, sel, CGPoint,
    CGRect, CGSize, Class, Id, Imp, Sel, UIEdgeInsets, NIL, OBJC_ASSOCIATION_RETAIN,
};
use std::ffi::{c_char, c_int, c_void, CString};

#[repr(C)]
struct ObjcSuper {
    receiver: Id,
    super_class: Class,
}

#[repr(C)]
struct BlockHeader {
    isa: *const c_void,
    flags: i32,
    reserved: i32,
    invoke: *const c_void,
}

extern "C" {
    static _dispatch_main_q: c_void;
    fn dispatch_async_f(queue: *const c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
    fn _Block_copy(aBlock: *const c_void) -> *mut c_void;
    fn _Block_release(aBlock: *const c_void);
    fn object_getClass(obj: Id) -> Class;
    fn class_getSuperclass(cls: Class) -> Class;
    fn class_getName(cls: Class) -> *const c_char;
    fn PearMissingReport(kind: c_int, class_name: *const c_char, selector: *const c_char, caller: *const c_void);
    fn PearHostLogMessage(tag: *const c_char, message: *const c_char);
    fn PearUIKitRegisterWindow(window: Id);
    fn PearUIKitSetKeyWindow(window: Id);
    fn PearUIKitKeyWindow() -> Id;
    fn PearUIKitScreenDidChange(point_size: CGSize, scale: f64);
}

unsafe fn add_class_method(c: Class, selector: &str, imp: Imp, types: &str) {
    let meta = object_getClass(c as Id);
    add_method(meta, selector, imp, types);
}

fn assoc_get(obj: Id, key: &u8) -> Id {
    unsafe { objc_getAssociatedObject(obj, key as *const u8 as *const c_void) }
}

fn assoc_set(obj: Id, key: &u8, val: Id) {
    unsafe {
        objc_setAssociatedObject(obj, key as *const u8 as *const c_void, val, OBJC_ASSOCIATION_RETAIN);
    }
}

unsafe fn super0<R>(obj: Id, base_cls_name: &str, selector: &str) -> R {
    let mut sup = ObjcSuper {
        receiver: obj,
        super_class: class_getSuperclass(cls(base_cls_name)),
    };
    let f: unsafe extern "C" fn(&mut ObjcSuper, Sel) -> R =
        std::mem::transmute(objc_msgSendSuper as *const c_void);
    f(&mut sup, sel(selector))
}

unsafe fn super1<A1, R>(obj: Id, base_cls_name: &str, selector: &str, a1: A1) -> R {
    let mut sup = ObjcSuper {
        receiver: obj,
        super_class: class_getSuperclass(cls(base_cls_name)),
    };
    let f: unsafe extern "C" fn(&mut ObjcSuper, Sel, A1) -> R =
        std::mem::transmute(objc_msgSendSuper as *const c_void);
    f(&mut sup, sel(selector), a1)
}

unsafe fn super2<A1, A2, R>(obj: Id, base_cls_name: &str, selector: &str, a1: A1, a2: A2) -> R {
    let mut sup = ObjcSuper {
        receiver: obj,
        super_class: class_getSuperclass(cls(base_cls_name)),
    };
    let f: unsafe extern "C" fn(&mut ObjcSuper, Sel, A1, A2) -> R =
        std::mem::transmute(objc_msgSendSuper as *const c_void);
    f(&mut sup, sel(selector), a1, a2)
}

unsafe fn invoke_void_block(block: Id) {
    if block.is_null() {
        return;
    }
    let hdr = &*(block as *const BlockHeader);
    let f: unsafe extern "C" fn(Id) = std::mem::transmute(hdr.invoke);
    f(block);
}

unsafe fn invoke_id_block(block: Id, arg: Id) {
    if block.is_null() {
        return;
    }
    let hdr = &*(block as *const BlockHeader);
    let f: unsafe extern "C" fn(Id, Id) = std::mem::transmute(hdr.invoke);
    f(block, arg);
}

extern "C" fn dispatch_block_trampoline(ctx: *mut c_void) {
    unsafe {
        let block = ctx as Id;
        invoke_void_block(block);
        _Block_release(ctx);
    }
}

unsafe fn dispatch_main_block(block: Id) {
    if block.is_null() {
        return;
    }
    let copied = _Block_copy(block as *const c_void);
    dispatch_async_f(&_dispatch_main_q as *const c_void, copied, dispatch_block_trampoline);
}

unsafe fn mut_array_for_key(obj: Id, key: &u8) -> Id {
    let arr = assoc_get(obj, key);
    if !arr.is_null() {
        return arr;
    }
    let arr: Id = msg0(cls("NSMutableArray"), "array");
    assoc_set(obj, key, arr);
    arr
}

fn rect_contains(r: CGRect, p: CGPoint) -> bool {
    p.x >= r.origin.x
        && p.x <= r.origin.x + r.size.width
        && p.y >= r.origin.y
        && p.y <= r.origin.y + r.size.height
}

// ============================================================================
// UIView
// ============================================================================

static VIEW_LAYER_KEY: u8 = 0;
static VIEW_SUPERVIEW_KEY: u8 = 0;
static VIEW_SUBVIEWS_KEY: u8 = 0;
static VIEW_WINDOW_KEY: u8 = 0;
static VIEW_VC_KEY: u8 = 0;
static VIEW_BG_KEY: u8 = 0;
static VIEW_FLAGS_KEY: u8 = 0;

#[repr(C)]
#[derive(Copy, Clone)]
struct ViewFlags {
    tag: isize,
    autoresizing: usize,
    multiple_touch: bool,
    user_interaction: bool,
    clears_context: bool,
    needs_layout: bool,
}

impl Default for ViewFlags {
    fn default() -> Self {
        Self {
            tag: 0,
            autoresizing: 0,
            multiple_touch: false,
            user_interaction: true,
            clears_context: true,
            needs_layout: false,
        }
    }
}

unsafe fn get_view_flags(v: Id) -> ViewFlags {
    let data = assoc_get(v, &VIEW_FLAGS_KEY);
    if let Some(bytes) = nsdata_to_vec(data) {
        if bytes.len() == std::mem::size_of::<ViewFlags>() {
            return std::ptr::read_unaligned(bytes.as_ptr() as *const ViewFlags);
        }
    }
    ViewFlags::default()
}

unsafe fn set_view_flags(v: Id, f: ViewFlags) {
    let bytes = std::slice::from_raw_parts(&f as *const ViewFlags as *const u8, std::mem::size_of::<ViewFlags>());
    assoc_set(v, &VIEW_FLAGS_KEY, nsdata_from_bytes(bytes));
}

unsafe fn view_layer(v: Id) -> Id {
    assoc_get(v, &VIEW_LAYER_KEY)
}

unsafe fn view_common_init(v: Id, frame: CGRect) {
    set_view_flags(v, ViewFlags::default());
    let self_cls = object_getClass(v);
    let layer_cls: Class = msg0(self_cls as Id, "layerClass");
    let l_cls = if layer_cls.is_null() { cls("CALayer") } else { layer_cls };
    let layer: Id = msg0(l_cls as Id, "new");
    assoc_set(v, &VIEW_LAYER_KEY, layer);
    objc_release(layer);
    let layer = view_layer(v);
    msg1::<Id, ()>(layer, "setDelegate:", v);
    msg1::<CGPoint, ()>(layer, "setAnchorPoint:", CGPoint { x: 0.5, y: 0.5 });
    let scr: Id = msg0(cls("UIScreen"), "mainScreen");
    let scale: f64 = msg0(scr, "scale");
    msg1::<f64, ()>(layer, "setContentsScale:", if scale > 0.0 { scale } else { 1.0 });
    msg1::<bool, ()>(layer, "setGeometryFlipped:", false);
    msg1::<CGRect, ()>(v, "setFrame:", frame);
}

unsafe extern "C" fn view_layer_class(_self_cls: Id, _cmd: Sel) -> Class {
    cls("CALayer")
}

unsafe extern "C" fn view_init(self_obj: Id, _cmd: Sel) -> Id {
    msg1(self_obj, "initWithFrame:", CGRect::default())
}

unsafe extern "C" fn view_init_with_frame(self_obj: Id, _cmd: Sel, frame: CGRect) -> Id {
    let obj: Id = super0(self_obj, "UIView", "init");
    if !obj.is_null() {
        view_common_init(obj, frame);
    }
    obj
}

unsafe extern "C" fn view_init_with_coder(self_obj: Id, _cmd: Sel, coder: Id) -> Id {
    let obj: Id = super0(self_obj, "UIView", "init");
    if obj.is_null() {
        return NIL;
    }
    view_common_init(obj, CGRect::default());
    if !coder.is_null() {
        if msg1::<Id, bool>(coder, "containsValueForKey:", nsstring("UIBounds")) {
            let b: CGRect = msg1(coder, "decodeCGRectForKey:", nsstring("UIBounds"));
            msg1::<CGRect, ()>(obj, "setBounds:", b);
        }
        if msg1::<Id, bool>(coder, "containsValueForKey:", nsstring("UICenter")) {
            let c: CGPoint = msg1(coder, "decodeCGPointForKey:", nsstring("UICenter"));
            msg1::<CGPoint, ()>(obj, "setCenter:", c);
        }
        if msg1::<Id, bool>(coder, "containsValueForKey:", nsstring("UIAutoresizingMask")) {
            let m: isize = msg1(coder, "decodeIntegerForKey:", nsstring("UIAutoresizingMask"));
            let mut f = get_view_flags(obj);
            f.autoresizing = m as usize;
            set_view_flags(obj, f);
        }
        if msg1::<Id, bool>(coder, "containsValueForKey:", nsstring("UIOpaque")) {
            let b: bool = msg1(coder, "decodeBoolForKey:", nsstring("UIOpaque"));
            msg1::<bool, ()>(obj, "setOpaque:", b);
        }
        if msg1::<Id, bool>(coder, "containsValueForKey:", nsstring("UIHidden")) {
            let b: bool = msg1(coder, "decodeBoolForKey:", nsstring("UIHidden"));
            msg1::<bool, ()>(obj, "setHidden:", b);
        }
        if msg1::<Id, bool>(coder, "containsValueForKey:", nsstring("UIAlpha")) {
            let a: f64 = msg1(coder, "decodeDoubleForKey:", nsstring("UIAlpha"));
            msg1::<f64, ()>(obj, "setAlpha:", a);
        }
        if msg1::<Id, bool>(coder, "containsValueForKey:", nsstring("UITag")) {
            let t: isize = msg1(coder, "decodeIntegerForKey:", nsstring("UITag"));
            let mut f = get_view_flags(obj);
            f.tag = t;
            set_view_flags(obj, f);
        }
        if msg1::<Id, bool>(coder, "containsValueForKey:", nsstring("UIClearsContextBeforeDrawing")) {
            let b: bool = msg1(coder, "decodeBoolForKey:", nsstring("UIClearsContextBeforeDrawing"));
            let mut f = get_view_flags(obj);
            f.clears_context = b;
            set_view_flags(obj, f);
        }
        if msg1::<Id, bool>(coder, "containsValueForKey:", nsstring("UIMultipleTouchEnabled")) {
            let b: bool = msg1(coder, "decodeBoolForKey:", nsstring("UIMultipleTouchEnabled"));
            let mut f = get_view_flags(obj);
            f.multiple_touch = b;
            set_view_flags(obj, f);
        }
        if msg1::<Id, bool>(coder, "containsValueForKey:", nsstring("UIUserInteractionDisabled")) {
            let b: bool = msg1(coder, "decodeBoolForKey:", nsstring("UIUserInteractionDisabled"));
            let mut f = get_view_flags(obj);
            f.user_interaction = !b;
            set_view_flags(obj, f);
        }
        let bg: Id = msg1(coder, "decodeObjectForKey:", nsstring("UIBackgroundColor"));
        if is_kind_of(bg, "UIColor") {
            msg1::<Id, ()>(obj, "setBackgroundColor:", bg);
        }
        let subs: Id = msg1(coder, "decodeObjectForKey:", nsstring("UISubviews"));
        if is_kind_of(subs, "NSArray") {
            for i in 0..nsarray_count(subs) {
                let sub = nsarray_object_at(subs, i);
                if is_kind_of(sub, "UIView") {
                    msg1::<Id, ()>(obj, "addSubview:", sub);
                }
            }
        }
    }
    obj
}

unsafe extern "C" fn view_get_layer(self_obj: Id, _cmd: Sel) -> Id {
    view_layer(self_obj)
}

unsafe extern "C" fn view_action_for_layer(_self: Id, _cmd: Sel, _layer: Id, _key: Id) -> Id {
    msg0(cls("NSNull"), "null")
}

unsafe fn view_bounds_size_changed(v: Id, old: CGSize) {
    let layer = view_layer(v);
    let now: CGRect = msg0(layer, "bounds");
    let dw = now.size.width - old.width;
    let dh = now.size.height - old.height;
    let subs = mut_array_for_key(v, &VIEW_SUBVIEWS_KEY);
    for i in 0..nsarray_count(subs) {
        let sub = nsarray_object_at(subs, i);
        let m = get_view_flags(sub).autoresizing;
        if m == 0 {
            continue;
        }
        let mut f: CGRect = msg0(sub, "frame");
        if (m & (1 << 1)) != 0 {
            f.size.width += dw;
        } else if (m & (1 << 0)) != 0 {
            f.origin.x += dw;
        }
        if (m & (1 << 4)) != 0 {
            f.size.height += dh;
        } else if (m & (1 << 3)) != 0 {
            f.origin.y += dh;
        }
        msg1::<CGRect, ()>(sub, "setFrame:", f);
    }
    msg0::<()>(v, "setNeedsLayout");
}

unsafe extern "C" fn view_frame(self_obj: Id, _cmd: Sel) -> CGRect {
    let l = view_layer(self_obj);
    if l.is_null() {
        CGRect::default()
    } else {
        msg0(l, "frame")
    }
}

unsafe extern "C" fn view_set_frame(self_obj: Id, _cmd: Sel, frame: CGRect) {
    let l = view_layer(self_obj);
    if l.is_null() {
        return;
    }
    let old_b: CGRect = msg0(l, "bounds");
    let ca = cls("CATransaction");
    msg0::<()>(ca, "begin");
    msg1::<bool, ()>(ca, "setDisableActions:", true);
    msg1::<CGRect, ()>(l, "setFrame:", frame);
    msg0::<()>(ca, "commit");
    if old_b.size != frame.size {
        view_bounds_size_changed(self_obj, old_b.size);
    }
}

unsafe extern "C" fn view_bounds(self_obj: Id, _cmd: Sel) -> CGRect {
    let l = view_layer(self_obj);
    if l.is_null() {
        CGRect::default()
    } else {
        msg0(l, "bounds")
    }
}

unsafe extern "C" fn view_set_bounds(self_obj: Id, _cmd: Sel, bounds: CGRect) {
    let l = view_layer(self_obj);
    if l.is_null() {
        return;
    }
    let old_b: CGRect = msg0(l, "bounds");
    let ca = cls("CATransaction");
    msg0::<()>(ca, "begin");
    msg1::<bool, ()>(ca, "setDisableActions:", true);
    msg1::<CGRect, ()>(l, "setBounds:", bounds);
    msg0::<()>(ca, "commit");
    if old_b.size != bounds.size {
        view_bounds_size_changed(self_obj, old_b.size);
    }
}

unsafe extern "C" fn view_center(self_obj: Id, _cmd: Sel) -> CGPoint {
    let l = view_layer(self_obj);
    if l.is_null() {
        CGPoint::default()
    } else {
        msg0(l, "position")
    }
}

unsafe extern "C" fn view_set_center(self_obj: Id, _cmd: Sel, pt: CGPoint) {
    let l = view_layer(self_obj);
    if l.is_null() {
        return;
    }
    let ca = cls("CATransaction");
    msg0::<()>(ca, "begin");
    msg1::<bool, ()>(ca, "setDisableActions:", true);
    msg1::<CGPoint, ()>(l, "setPosition:", pt);
    msg0::<()>(ca, "commit");
}

unsafe extern "C" fn view_superview(self_obj: Id, _cmd: Sel) -> Id {
    pear_objc::assoc_get_weak(self_obj, &VIEW_SUPERVIEW_KEY)
}

unsafe extern "C" fn view_subviews(self_obj: Id, _cmd: Sel) -> Id {
    let arr = mut_array_for_key(self_obj, &VIEW_SUBVIEWS_KEY);
    msg1(cls("NSArray"), "arrayWithArray:", arr)
}

unsafe extern "C" fn view_window(self_obj: Id, _cmd: Sel) -> Id {
    pear_objc::assoc_get_weak(self_obj, &VIEW_WINDOW_KEY)
}

unsafe extern "C" fn view_next_responder(self_obj: Id, _cmd: Sel) -> Id {
    let vc = pear_objc::assoc_get_weak(self_obj, &VIEW_VC_KEY);
    if !vc.is_null() {
        vc
    } else {
        pear_objc::assoc_get_weak(self_obj, &VIEW_SUPERVIEW_KEY)
    }
}

unsafe extern "C" fn view_set_vc(self_obj: Id, _cmd: Sel, vc: Id) {
    pear_objc::assoc_set_weak(self_obj, &VIEW_VC_KEY, vc);
}

unsafe extern "C" fn view_set_window(self_obj: Id, _cmd: Sel, window: Id) {
    if pear_objc::assoc_get_weak(self_obj, &VIEW_WINDOW_KEY) == window {
        return;
    }
    msg1::<Id, ()>(self_obj, "willMoveToWindow:", window);
    pear_objc::assoc_set_weak(self_obj, &VIEW_WINDOW_KEY, window);
    let subs = mut_array_for_key(self_obj, &VIEW_SUBVIEWS_KEY);
    for i in 0..nsarray_count(subs) {
        let sub = nsarray_object_at(subs, i);
        msg1::<Id, ()>(sub, "pear_setWindow:", window);
    }
    msg0::<()>(self_obj, "didMoveToWindow");
}

unsafe extern "C" fn view_insert_subview(self_obj: Id, _cmd: Sel, view: Id, index: isize) {
    if view.is_null() || view == self_obj {
        return;
    }
    if !pear_objc::assoc_get_weak(view, &VIEW_SUPERVIEW_KEY).is_null() {
        msg0::<()>(view, "removeFromSuperview");
    }
    msg1::<Id, ()>(view, "willMoveToSuperview:", self_obj);
    let subs = mut_array_for_key(self_obj, &VIEW_SUBVIEWS_KEY);
    let count = nsarray_count(subs) as isize;
    let clamped = index.max(0).min(count) as usize;
    msg2::<Id, usize, ()>(subs, "insertObject:atIndex:", view, clamped);
    let l = view_layer(self_obj);
    let sub_l = view_layer(view);
    if !l.is_null() && !sub_l.is_null() {
        msg2::<Id, u32, ()>(l, "insertSublayer:atIndex:", sub_l, clamped as u32);
    }
    pear_objc::assoc_set_weak(view, &VIEW_SUPERVIEW_KEY, self_obj);
    let win = pear_objc::assoc_get_weak(self_obj, &VIEW_WINDOW_KEY);
    msg1::<Id, ()>(view, "pear_setWindow:", win);
    msg0::<()>(view, "didMoveToSuperview");
    msg1::<Id, ()>(self_obj, "didAddSubview:", view);
    msg0::<()>(self_obj, "setNeedsLayout");
}

unsafe extern "C" fn view_add_subview(self_obj: Id, _cmd: Sel, view: Id) {
    let subs = mut_array_for_key(self_obj, &VIEW_SUBVIEWS_KEY);
    let count = nsarray_count(subs) as isize;
    msg2::<Id, isize, ()>(self_obj, "insertSubview:atIndex:", view, count);
}

unsafe extern "C" fn view_bring_to_front(self_obj: Id, _cmd: Sel, view: Id) {
    if view.is_null() || pear_objc::assoc_get_weak(view, &VIEW_SUPERVIEW_KEY) != self_obj {
        return;
    }
    let subs = mut_array_for_key(self_obj, &VIEW_SUBVIEWS_KEY);
    msg1::<Id, ()>(subs, "removeObjectIdenticalTo:", view);
    msg1::<Id, ()>(subs, "addObject:", view);
    let sub_l = view_layer(view);
    let l = view_layer(self_obj);
    if !sub_l.is_null() && !l.is_null() {
        msg0::<()>(sub_l, "removeFromSuperlayer");
        msg1::<Id, ()>(l, "addSublayer:", sub_l);
    }
}

unsafe extern "C" fn view_send_to_back(self_obj: Id, _cmd: Sel, view: Id) {
    if view.is_null() || pear_objc::assoc_get_weak(view, &VIEW_SUPERVIEW_KEY) != self_obj {
        return;
    }
    let subs = mut_array_for_key(self_obj, &VIEW_SUBVIEWS_KEY);
    msg1::<Id, ()>(subs, "removeObjectIdenticalTo:", view);
    msg2::<Id, usize, ()>(subs, "insertObject:atIndex:", view, 0);
    let sub_l = view_layer(view);
    let l = view_layer(self_obj);
    if !sub_l.is_null() && !l.is_null() {
        msg0::<()>(sub_l, "removeFromSuperlayer");
        msg2::<Id, u32, ()>(l, "insertSublayer:atIndex:", sub_l, 0);
    }
}

unsafe extern "C" fn view_remove_from_superview(self_obj: Id, _cmd: Sel) {
    let parent = pear_objc::assoc_get_weak(self_obj, &VIEW_SUPERVIEW_KEY);
    if parent.is_null() {
        return;
    }
    msg1::<Id, ()>(parent, "willRemoveSubview:", self_obj);
    msg1::<Id, ()>(self_obj, "willMoveToSuperview:", NIL);
    let subs = mut_array_for_key(parent, &VIEW_SUBVIEWS_KEY);
    msg1::<Id, ()>(subs, "removeObjectIdenticalTo:", self_obj);
    let l = view_layer(self_obj);
    if !l.is_null() {
        msg0::<()>(l, "removeFromSuperlayer");
    }
    pear_objc::assoc_set_weak(self_obj, &VIEW_SUPERVIEW_KEY, NIL);
    msg1::<Id, ()>(self_obj, "pear_setWindow:", NIL);
    msg0::<()>(self_obj, "didMoveToSuperview");
}

unsafe extern "C" fn view_noop(_self: Id, _cmd: Sel) {}
unsafe extern "C" fn view_noop_id(_self: Id, _cmd: Sel, _a: Id) {}
unsafe extern "C" fn view_noop_rect(_self: Id, _cmd: Sel, _r: CGRect) {}
unsafe extern "C" fn view_noop_bool(_self: Id, _cmd: Sel, _b: bool) {}
unsafe extern "C" fn view_noop_int(_self: Id, _cmd: Sel, _i: isize) {}
unsafe extern "C" fn view_noop_u64(_self: Id, _cmd: Sel, _i: u64) {}

unsafe extern "C" fn view_is_descendant(self_obj: Id, _cmd: Sel, view: Id) -> bool {
    let mut cur = self_obj;
    while !cur.is_null() {
        if cur == view {
            return true;
        }
        cur = pear_objc::assoc_get_weak(cur, &VIEW_SUPERVIEW_KEY);
    }
    false
}

unsafe extern "C" fn view_with_tag(self_obj: Id, _cmd: Sel, tag: isize) -> Id {
    if get_view_flags(self_obj).tag == tag {
        return self_obj;
    }
    let subs = mut_array_for_key(self_obj, &VIEW_SUBVIEWS_KEY);
    for i in 0..nsarray_count(subs) {
        let sub = nsarray_object_at(subs, i);
        let hit: Id = msg1(sub, "viewWithTag:", tag);
        if !hit.is_null() {
            return hit;
        }
    }
    NIL
}

unsafe extern "C" fn view_tag(self_obj: Id, _cmd: Sel) -> isize {
    get_view_flags(self_obj).tag
}
unsafe extern "C" fn view_set_tag(self_obj: Id, _cmd: Sel, tag: isize) {
    let mut f = get_view_flags(self_obj);
    f.tag = tag;
    set_view_flags(self_obj, f);
}

extern "C" fn view_layout_async(ctx: *mut c_void) {
    unsafe {
        // The pending layout holds the view weakly: a view released before
        // the main queue gets here is simply skipped.
        let slot = ctx as *mut Id;
        let v = pear_objc::objc_loadWeakRetained(slot);
        pear_objc::objc_destroyWeak(slot);
        drop(Box::from_raw(slot));
        if !v.is_null() {
            msg0::<()>(v, "layoutIfNeeded");
            objc_release(v);
        }
    }
}

unsafe extern "C" fn view_set_needs_layout(self_obj: Id, _cmd: Sel) {
    let mut f = get_view_flags(self_obj);
    if f.needs_layout {
        return; // a layout pass is already queued
    }
    f.needs_layout = true;
    set_view_flags(self_obj, f);
    let slot = Box::into_raw(Box::new(NIL));
    pear_objc::objc_storeWeak(slot, self_obj);
    dispatch_async_f(&_dispatch_main_q as *const c_void, slot as *mut c_void, view_layout_async);
}

unsafe extern "C" fn view_layout_if_needed(self_obj: Id, _cmd: Sel) {
    let mut f = get_view_flags(self_obj);
    if !f.needs_layout {
        return;
    }
    f.needs_layout = false;
    set_view_flags(self_obj, f);
    let vc = pear_objc::assoc_get_weak(self_obj, &VIEW_VC_KEY);
    if !vc.is_null() {
        msg0::<()>(vc, "viewWillLayoutSubviews");
    }
    msg0::<()>(self_obj, "layoutSubviews");
    if !vc.is_null() {
        msg0::<()>(vc, "viewDidLayoutSubviews");
    }
    let subs = mut_array_for_key(self_obj, &VIEW_SUBVIEWS_KEY);
    for i in 0..nsarray_count(subs) {
        msg0::<()>(nsarray_object_at(subs, i), "layoutIfNeeded");
    }
}

unsafe extern "C" fn view_set_needs_display(self_obj: Id, _cmd: Sel) {
    let l = view_layer(self_obj);
    if !l.is_null() {
        msg0::<()>(l, "setNeedsDisplay");
    }
}
unsafe extern "C" fn view_set_needs_display_rect(self_obj: Id, _cmd: Sel, rect: CGRect) {
    let l = view_layer(self_obj);
    if !l.is_null() {
        msg1::<CGRect, ()>(l, "setNeedsDisplayInRect:", rect);
    }
}
unsafe extern "C" fn view_size_that_fits(self_obj: Id, _cmd: Sel, _size: CGSize) -> CGSize {
    view_bounds(self_obj, sel("bounds")).size
}
unsafe extern "C" fn view_size_to_fit(self_obj: Id, _cmd: Sel) {
    let mut f = view_frame(self_obj, sel("frame"));
    f.size = msg1(self_obj, "sizeThatFits:", f.size);
    msg1::<CGRect, ()>(self_obj, "setFrame:", f);
}
unsafe extern "C" fn view_content_scale(self_obj: Id, _cmd: Sel) -> f64 {
    let l = view_layer(self_obj);
    if l.is_null() {
        1.0
    } else {
        msg0(l, "contentsScale")
    }
}
unsafe extern "C" fn view_set_content_scale(self_obj: Id, _cmd: Sel, s: f64) {
    let l = view_layer(self_obj);
    if !l.is_null() {
        msg1::<f64, ()>(l, "setContentsScale:", s);
    }
}
unsafe extern "C" fn view_is_multi_touch(self_obj: Id, _cmd: Sel) -> bool {
    get_view_flags(self_obj).multiple_touch
}
unsafe extern "C" fn view_set_multi_touch(self_obj: Id, _cmd: Sel, b: bool) {
    let mut f = get_view_flags(self_obj);
    f.multiple_touch = b;
    set_view_flags(self_obj, f);
}
unsafe extern "C" fn view_false(_self: Id, _cmd: Sel) -> bool {
    false
}
unsafe extern "C" fn view_true(_self: Id, _cmd: Sel) -> bool {
    true
}
unsafe extern "C" fn view_user_interaction(self_obj: Id, _cmd: Sel) -> bool {
    get_view_flags(self_obj).user_interaction
}
unsafe extern "C" fn view_set_user_interaction(self_obj: Id, _cmd: Sel, b: bool) {
    let mut f = get_view_flags(self_obj);
    f.user_interaction = b;
    set_view_flags(self_obj, f);
}
unsafe extern "C" fn view_is_hidden(self_obj: Id, _cmd: Sel) -> bool {
    let l = view_layer(self_obj);
    !l.is_null() && msg0::<bool>(l, "isHidden")
}
unsafe extern "C" fn view_set_hidden(self_obj: Id, _cmd: Sel, h: bool) {
    let l = view_layer(self_obj);
    if !l.is_null() {
        msg1::<bool, ()>(l, "setHidden:", h);
    }
}
unsafe extern "C" fn view_is_opaque(self_obj: Id, _cmd: Sel) -> bool {
    let l = view_layer(self_obj);
    !l.is_null() && msg0::<bool>(l, "isOpaque")
}
unsafe extern "C" fn view_set_opaque(self_obj: Id, _cmd: Sel, o: bool) {
    let l = view_layer(self_obj);
    if !l.is_null() {
        msg1::<bool, ()>(l, "setOpaque:", o);
    }
}
unsafe extern "C" fn view_alpha(self_obj: Id, _cmd: Sel) -> f64 {
    let l = view_layer(self_obj);
    if l.is_null() {
        1.0
    } else {
        msg0::<f32>(l, "opacity") as f64
    }
}
unsafe extern "C" fn view_set_alpha(self_obj: Id, _cmd: Sel, a: f64) {
    let l = view_layer(self_obj);
    if !l.is_null() {
        msg1::<f32, ()>(l, "setOpacity:", a as f32);
    }
}
unsafe extern "C" fn view_autoresizing(self_obj: Id, _cmd: Sel) -> usize {
    get_view_flags(self_obj).autoresizing
}
unsafe extern "C" fn view_set_autoresizing(self_obj: Id, _cmd: Sel, m: usize) {
    let mut f = get_view_flags(self_obj);
    f.autoresizing = m;
    set_view_flags(self_obj, f);
}
unsafe extern "C" fn view_clears(self_obj: Id, _cmd: Sel) -> bool {
    get_view_flags(self_obj).clears_context
}
unsafe extern "C" fn view_set_clears(self_obj: Id, _cmd: Sel, b: bool) {
    let mut f = get_view_flags(self_obj);
    f.clears_context = b;
    set_view_flags(self_obj, f);
}
unsafe extern "C" fn view_clips(self_obj: Id, _cmd: Sel) -> bool {
    let l = view_layer(self_obj);
    !l.is_null() && msg0::<bool>(l, "masksToBounds")
}
unsafe extern "C" fn view_set_clips(self_obj: Id, _cmd: Sel, b: bool) {
    let l = view_layer(self_obj);
    if !l.is_null() {
        msg1::<bool, ()>(l, "setMasksToBounds:", b);
    }
}
unsafe extern "C" fn view_bg(self_obj: Id, _cmd: Sel) -> Id {
    assoc_get(self_obj, &VIEW_BG_KEY)
}
unsafe extern "C" fn view_set_bg(self_obj: Id, _cmd: Sel, color: Id) {
    assoc_set(self_obj, &VIEW_BG_KEY, color);
    let l = view_layer(self_obj);
    if !l.is_null() {
        let cg: *mut c_void = if color.is_null() {
            std::ptr::null_mut()
        } else {
            msg0(color, "CGColor")
        };
        msg1::<*mut c_void, ()>(l, "setBackgroundColor:", cg);
    }
}
unsafe extern "C" fn view_zero_int(_self: Id, _cmd: Sel) -> isize {
    0
}
unsafe extern "C" fn view_zero_u64(_self: Id, _cmd: Sel) -> u64 {
    0
}
unsafe extern "C" fn view_zero_insets(_self: Id, _cmd: Sel) -> UIEdgeInsets {
    UIEdgeInsets::default()
}
unsafe extern "C" fn view_layout_margins(_self: Id, _cmd: Sel) -> UIEdgeInsets {
    UIEdgeInsets {
        top: 8.0,
        left: 8.0,
        bottom: 8.0,
        right: 8.0,
    }
}
unsafe extern "C" fn view_trait_collection(_self: Id, _cmd: Sel) -> Id {
    msg0(cls("UITraitCollection"), "pear_current")
}
unsafe extern "C" fn view_empty_arr(_self: Id, _cmd: Sel) -> Id {
    msg0(cls("NSArray"), "array")
}
unsafe extern "C" fn view_add_gesture(_self: Id, _cmd: Sel, _g: Id) {
    let c1 = CString::new("UIView").unwrap();
    let c2 = CString::new("gesture recognizers are ignored").unwrap();
    PearMissingReport(1, c1.as_ptr(), c2.as_ptr(), std::ptr::null());
}
unsafe extern "C" fn view_point_inside(self_obj: Id, _cmd: Sel, pt: CGPoint, _ev: Id) -> bool {
    rect_contains(msg0(self_obj, "bounds"), pt)
}
unsafe extern "C" fn view_hit_test(self_obj: Id, _cmd: Sel, pt: CGPoint, ev: Id) -> Id {
    if msg0::<bool>(self_obj, "isHidden")
        || !msg0::<bool>(self_obj, "isUserInteractionEnabled")
        || msg0::<f64>(self_obj, "alpha") < 0.01
    {
        return NIL;
    }
    if !msg2::<CGPoint, Id, bool>(self_obj, "pointInside:withEvent:", pt, ev) {
        return NIL;
    }
    let subs = mut_array_for_key(self_obj, &VIEW_SUBVIEWS_KEY);
    let count = nsarray_count(subs);
    for idx in (0..count).rev() {
        let sub = nsarray_object_at(subs, idx);
        let conv: CGPoint = msg2(self_obj, "convertPoint:toView:", pt, sub);
        let hit: Id = msg2(sub, "hitTest:withEvent:", conv, ev);
        if !hit.is_null() {
            return hit;
        }
    }
    self_obj
}
unsafe extern "C" fn view_conv_pt_to(self_obj: Id, _cmd: Sel, pt: CGPoint, view: Id) -> CGPoint {
    let target = if !view.is_null() {
        msg0::<Id>(view, "layer")
    } else {
        let win = pear_objc::assoc_get_weak(self_obj, &VIEW_WINDOW_KEY);
        if win.is_null() {
            NIL
        } else {
            msg0(win, "layer")
        }
    };
    if target.is_null() {
        pt
    } else {
        msg2(view_layer(self_obj), "convertPoint:toLayer:", pt, target)
    }
}
unsafe extern "C" fn view_conv_pt_from(self_obj: Id, _cmd: Sel, pt: CGPoint, view: Id) -> CGPoint {
    let src = if !view.is_null() {
        msg0::<Id>(view, "layer")
    } else {
        let win = pear_objc::assoc_get_weak(self_obj, &VIEW_WINDOW_KEY);
        if win.is_null() {
            NIL
        } else {
            msg0(win, "layer")
        }
    };
    if src.is_null() {
        pt
    } else {
        msg2(view_layer(self_obj), "convertPoint:fromLayer:", pt, src)
    }
}
unsafe extern "C" fn view_conv_rect_to(self_obj: Id, _cmd: Sel, r: CGRect, view: Id) -> CGRect {
    let target = if !view.is_null() {
        msg0::<Id>(view, "layer")
    } else {
        let win = pear_objc::assoc_get_weak(self_obj, &VIEW_WINDOW_KEY);
        if win.is_null() {
            NIL
        } else {
            msg0(win, "layer")
        }
    };
    if target.is_null() {
        r
    } else {
        msg2(view_layer(self_obj), "convertRect:toLayer:", r, target)
    }
}
unsafe extern "C" fn view_conv_rect_from(self_obj: Id, _cmd: Sel, r: CGRect, view: Id) -> CGRect {
    let src = if !view.is_null() {
        msg0::<Id>(view, "layer")
    } else {
        let win = pear_objc::assoc_get_weak(self_obj, &VIEW_WINDOW_KEY);
        if win.is_null() {
            NIL
        } else {
            msg0(win, "layer")
        }
    };
    if src.is_null() {
        r
    } else {
        msg2(view_layer(self_obj), "convertRect:fromLayer:", r, src)
    }
}
unsafe extern "C" fn view_nil(_self: Id, _cmd: Sel) -> Id {
    NIL
}

// ============================================================================
// UIScrollView
// ============================================================================

static SV_STATE_KEY: u8 = 0;
static SV_DEL_KEY: u8 = 0;

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct ScrollState {
    offset: CGPoint,
    content_size: CGSize,
    inset: UIEdgeInsets,
    scroll_disabled: bool,
}

unsafe fn get_sv_state(v: Id) -> ScrollState {
    let d = assoc_get(v, &SV_STATE_KEY);
    if let Some(b) = nsdata_to_vec(d) {
        if b.len() == std::mem::size_of::<ScrollState>() {
            return std::ptr::read_unaligned(b.as_ptr() as *const ScrollState);
        }
    }
    ScrollState::default()
}
unsafe fn set_sv_state(v: Id, st: ScrollState) {
    let bytes = std::slice::from_raw_parts(&st as *const ScrollState as *const u8, std::mem::size_of::<ScrollState>());
    assoc_set(v, &SV_STATE_KEY, nsdata_from_bytes(bytes));
}
unsafe extern "C" fn sv_offset(self_obj: Id, _cmd: Sel) -> CGPoint {
    get_sv_state(self_obj).offset
}
unsafe extern "C" fn sv_set_offset(self_obj: Id, _cmd: Sel, off: CGPoint) {
    let mut st = get_sv_state(self_obj);
    st.offset = off;
    set_sv_state(self_obj, st);
    let mut b: CGRect = msg0(self_obj, "bounds");
    b.origin = off;
    msg1::<CGRect, ()>(self_obj, "setBounds:", b);
}
unsafe extern "C" fn sv_set_offset_anim(self_obj: Id, _cmd: Sel, off: CGPoint, _anim: bool) {
    sv_set_offset(self_obj, sel("setContentOffset:"), off);
}
unsafe extern "C" fn sv_content_size(self_obj: Id, _cmd: Sel) -> CGSize {
    get_sv_state(self_obj).content_size
}
unsafe extern "C" fn sv_set_content_size(self_obj: Id, _cmd: Sel, sz: CGSize) {
    let mut st = get_sv_state(self_obj);
    st.content_size = sz;
    set_sv_state(self_obj, st);
}
unsafe extern "C" fn sv_inset(self_obj: Id, _cmd: Sel) -> UIEdgeInsets {
    get_sv_state(self_obj).inset
}
unsafe extern "C" fn sv_set_inset(self_obj: Id, _cmd: Sel, ins: UIEdgeInsets) {
    let mut st = get_sv_state(self_obj);
    st.inset = ins;
    set_sv_state(self_obj, st);
}
unsafe extern "C" fn sv_scroll_enabled(self_obj: Id, _cmd: Sel) -> bool {
    !get_sv_state(self_obj).scroll_disabled
}
unsafe extern "C" fn sv_set_scroll_enabled(self_obj: Id, _cmd: Sel, en: bool) {
    let mut st = get_sv_state(self_obj);
    st.scroll_disabled = !en;
    set_sv_state(self_obj, st);
}
unsafe extern "C" fn sv_delegate(self_obj: Id, _cmd: Sel) -> Id {
    pear_objc::assoc_get_weak(self_obj, &SV_DEL_KEY)
}
unsafe extern "C" fn sv_set_delegate(self_obj: Id, _cmd: Sel, d: Id) {
    pear_objc::assoc_set_weak(self_obj, &SV_DEL_KEY, d);
}

// ============================================================================
// UIViewController
// ============================================================================

static VC_VIEW_KEY: u8 = 0;
static VC_TITLE_KEY: u8 = 0;
static VC_PRESENTED_KEY: u8 = 0;
static VC_PRESENTING_KEY: u8 = 0;
static VC_NIB_NAME_KEY: u8 = 0;
static VC_NIB_BUNDLE_KEY: u8 = 0;
static VC_MODAL_STYLE_KEY: u8 = 0;
static VC_EDGES_KEY: u8 = 0;

unsafe extern "C" fn vc_init(self_obj: Id, _cmd: Sel) -> Id {
    msg2(self_obj, "initWithNibName:bundle:", NIL, NIL)
}

unsafe extern "C" fn vc_init_nib(self_obj: Id, _cmd: Sel, nib_name: Id, bundle: Id) -> Id {
    let obj: Id = super0(self_obj, "UIViewController", "init");
    if !obj.is_null() {
        assoc_set(obj, &VC_EDGES_KEY, nsnumber_i64(15)); // all
        assoc_set(obj, &VC_NIB_NAME_KEY, nib_name);
        assoc_set(obj, &VC_NIB_BUNDLE_KEY, bundle);
    }
    obj
}

unsafe extern "C" fn vc_init_coder(self_obj: Id, _cmd: Sel, coder: Id) -> Id {
    let obj: Id = super0(self_obj, "UIViewController", "init");
    if obj.is_null() {
        return NIL;
    }
    assoc_set(obj, &VC_EDGES_KEY, nsnumber_i64(15));
    if !coder.is_null() {
        let nn: Id = msg1(coder, "decodeObjectForKey:", nsstring("UINibName"));
        if is_kind_of(nn, "NSString") {
            assoc_set(obj, &VC_NIB_NAME_KEY, nn);
        }
        if msg1::<Id, bool>(coder, "containsValueForKey:", nsstring("UIKeyEdgesForExtendedLayout")) {
            let e: isize = msg1(coder, "decodeIntegerForKey:", nsstring("UIKeyEdgesForExtendedLayout"));
            assoc_set(obj, &VC_EDGES_KEY, nsnumber_i64(e as i64));
        }
        let v: Id = msg1(coder, "decodeObjectForKey:", nsstring("UIView"));
        if is_kind_of(v, "UIView") {
            msg1::<Id, ()>(obj, "setView:", v);
        }
    }
    obj
}

unsafe extern "C" fn vc_nib_name(self_obj: Id, _cmd: Sel) -> Id {
    assoc_get(self_obj, &VC_NIB_NAME_KEY)
}

unsafe extern "C" fn vc_nib_bundle(self_obj: Id, _cmd: Sel) -> Id {
    let b = assoc_get(self_obj, &VC_NIB_BUNDLE_KEY);
    if b.is_null() {
        msg0(cls("NSBundle"), "mainBundle")
    } else {
        b
    }
}

unsafe extern "C" fn vc_load_view(self_obj: Id, _cmd: Sel) {
    let nn = assoc_get(self_obj, &VC_NIB_NAME_KEY);
    if !nn.is_null() && !nsstring_to_string(nn).unwrap_or_default().is_empty() {
        let b = vc_nib_bundle(self_obj, sel("nibBundle"));
        let nib: Id = msg2(cls("UINib"), "nibWithNibName:bundle:", nn, b);
        if !nib.is_null() {
            msg2::<Id, Id, Id>(nib, "instantiateWithOwner:options:", self_obj, NIL);
            if !assoc_get(self_obj, &VC_VIEW_KEY).is_null() {
                return;
            }
        }
    }
    let bounds: CGRect = msg0(msg0(cls("UIScreen"), "mainScreen"), "bounds");
    let v: Id = msg1(msg0(cls("UIView"), "alloc"), "initWithFrame:", bounds);
    msg1::<Id, ()>(self_obj, "setView:", v);
    objc_release(v);
}

unsafe extern "C" fn vc_view(self_obj: Id, _cmd: Sel) -> Id {
    let v = assoc_get(self_obj, &VC_VIEW_KEY);
    if !v.is_null() {
        return v;
    }
    msg0::<()>(self_obj, "loadView");
    if assoc_get(self_obj, &VC_VIEW_KEY).is_null() {
        let bounds: CGRect = msg0(msg0(cls("UIScreen"), "mainScreen"), "bounds");
        let fallback: Id = msg1(msg0(cls("UIView"), "alloc"), "initWithFrame:", bounds);
        msg1::<Id, ()>(self_obj, "setView:", fallback);
        objc_release(fallback);
    }
    msg0::<()>(self_obj, "viewDidLoad");
    assoc_get(self_obj, &VC_VIEW_KEY)
}

unsafe extern "C" fn vc_set_view(self_obj: Id, _cmd: Sel, new_v: Id) {
    let old = assoc_get(self_obj, &VC_VIEW_KEY);
    if !old.is_null() {
        msg1::<Id, ()>(old, "pear_setViewController:", NIL);
    }
    assoc_set(self_obj, &VC_VIEW_KEY, new_v);
    if !new_v.is_null() {
        msg1::<Id, ()>(new_v, "pear_setViewController:", self_obj);
    }
}

unsafe extern "C" fn vc_view_if_loaded(self_obj: Id, _cmd: Sel) -> Id {
    assoc_get(self_obj, &VC_VIEW_KEY)
}
unsafe extern "C" fn vc_is_view_loaded(self_obj: Id, _cmd: Sel) -> bool {
    !assoc_get(self_obj, &VC_VIEW_KEY).is_null()
}
unsafe extern "C" fn vc_load_view_if_needed(self_obj: Id, _cmd: Sel) {
    let _: Id = msg0(self_obj, "view");
}
unsafe extern "C" fn vc_next_responder(self_obj: Id, _cmd: Sel) -> Id {
    let v = assoc_get(self_obj, &VC_VIEW_KEY);
    if v.is_null() {
        return NIL;
    }
    let sv: Id = msg0(v, "superview");
    if !sv.is_null() {
        sv
    } else {
        msg0(v, "window")
    }
}
unsafe extern "C" fn vc_title(self_obj: Id, _cmd: Sel) -> Id {
    assoc_get(self_obj, &VC_TITLE_KEY)
}
unsafe extern "C" fn vc_set_title(self_obj: Id, _cmd: Sel, t: Id) {
    assoc_set(self_obj, &VC_TITLE_KEY, t);
}
unsafe extern "C" fn vc_modal_style(self_obj: Id, _cmd: Sel) -> isize {
    let n = assoc_get(self_obj, &VC_MODAL_STYLE_KEY);
    if n.is_null() {
        0
    } else {
        msg0(n, "integerValue")
    }
}
unsafe extern "C" fn vc_set_modal_style(self_obj: Id, _cmd: Sel, s: isize) {
    assoc_set(self_obj, &VC_MODAL_STYLE_KEY, nsnumber_i64(s as i64));
}
unsafe extern "C" fn vc_edges(self_obj: Id, _cmd: Sel) -> usize {
    let n = assoc_get(self_obj, &VC_EDGES_KEY);
    if n.is_null() {
        15
    } else {
        msg0::<isize>(n, "integerValue") as usize
    }
}
unsafe extern "C" fn vc_set_edges(self_obj: Id, _cmd: Sel, e: usize) {
    assoc_set(self_obj, &VC_EDGES_KEY, nsnumber_i64(e as i64));
}
unsafe extern "C" fn vc_orientations(_self: Id, _cmd: Sel) -> usize {
    (1 << 3) | (1 << 4) // landscape
}
unsafe extern "C" fn vc_pref_orientation(_self: Id, _cmd: Sel) -> isize {
    4 // landscapeRight
}
unsafe extern "C" fn vc_pref_edges(_self: Id, _cmd: Sel) -> usize {
    0
}
unsafe extern "C" fn vc_presented(self_obj: Id, _cmd: Sel) -> Id {
    assoc_get(self_obj, &VC_PRESENTED_KEY)
}
unsafe extern "C" fn vc_presenting(self_obj: Id, _cmd: Sel) -> Id {
    pear_objc::assoc_get_weak(self_obj, &VC_PRESENTING_KEY)
}
unsafe extern "C" fn vc_set_presenting(self_obj: Id, _cmd: Sel, p: Id) {
    pear_objc::assoc_set_weak(self_obj, &VC_PRESENTING_KEY, p);
}

unsafe extern "C" fn vc_present(self_obj: Id, _cmd: Sel, vc: Id, anim: bool, completion: Id) {
    if vc.is_null() {
        return;
    }
    if is_kind_of(vc, "UIAlertController") {
        msg1::<Id, ()>(vc, "pear_presentWithCompletion:", completion);
        return;
    }
    let cname = class_getName(object_getClass(vc));
    let m = CString::new("presentViewController: overlays the view only").unwrap();
    PearMissingReport(1, cname, m.as_ptr(), std::ptr::null());
    assoc_set(self_obj, &VC_PRESENTED_KEY, vc);
    msg1::<Id, ()>(vc, "pear_setPresenting:", self_obj);
    let my_v = assoc_get(self_obj, &VC_VIEW_KEY);
    if !my_v.is_null() {
        let w: Id = msg0(my_v, "window");
        if !w.is_null() {
            let v: Id = msg0(vc, "view");
            let wb: CGRect = msg0(w, "bounds");
            msg1::<CGRect, ()>(v, "setFrame:", wb);
            msg1::<bool, ()>(vc, "viewWillAppear:", anim);
            msg1::<Id, ()>(w, "addSubview:", v);
            msg1::<bool, ()>(vc, "viewDidAppear:", anim);
        }
    }
    dispatch_main_block(completion);
}

unsafe extern "C" fn vc_dismiss(self_obj: Id, _cmd: Sel, anim: bool, completion: Id) {
    let presented = assoc_get(self_obj, &VC_PRESENTED_KEY);
    let target = if !presented.is_null() { presented } else { self_obj };
    let presenter = if !presented.is_null() {
        self_obj
    } else {
        pear_objc::assoc_get_weak(self_obj, &VC_PRESENTING_KEY)
    };
    msg1::<bool, ()>(target, "viewWillDisappear:", anim);
    let tv: Id = msg0(target, "viewIfLoaded");
    if !tv.is_null() {
        msg0::<()>(tv, "removeFromSuperview");
    }
    msg1::<bool, ()>(target, "viewDidDisappear:", anim);
    msg1::<Id, ()>(target, "pear_setPresenting:", NIL);
    if !presenter.is_null() {
        assoc_set(presenter, &VC_PRESENTED_KEY, NIL);
    }
    dispatch_main_block(completion);
}

// ============================================================================
// UIWindow
// ============================================================================

static WIN_NATIVE_KEY: u8 = 0;
static WIN_ROOT_KEY: u8 = 0;
static WIN_SCREEN_KEY: u8 = 0;
static WIN_LEVEL_KEY: u8 = 0;

unsafe extern "C" fn win_init_frame(self_obj: Id, _cmd: Sel, frame: CGRect) -> Id {
    let obj: Id = super1(self_obj, "UIWindow", "initWithFrame:", frame);
    if !obj.is_null() {
        let scr: Id = msg0(cls("UIScreen"), "mainScreen");
        assoc_set(obj, &WIN_SCREEN_KEY, scr);
        msg1::<Id, ()>(obj, "pear_setWindow:", obj);
        PearUIKitRegisterWindow(obj);
    }
    obj
}

unsafe extern "C" fn win_init_coder(self_obj: Id, _cmd: Sel, coder: Id) -> Id {
    let obj: Id = super1(self_obj, "UIWindow", "initWithCoder:", coder);
    if !obj.is_null() {
        let scr: Id = msg0(cls("UIScreen"), "mainScreen");
        assoc_set(obj, &WIN_SCREEN_KEY, scr);
        let b: CGRect = msg0(scr, "bounds");
        msg1::<CGRect, ()>(obj, "setFrame:", b);
        msg1::<Id, ()>(obj, "pear_setWindow:", obj);
        PearUIKitRegisterWindow(obj);
    }
    obj
}

unsafe extern "C" fn win_init_scene(self_obj: Id, _cmd: Sel, _scene: Id) -> Id {
    let b: CGRect = msg0(msg0(cls("UIScreen"), "mainScreen"), "bounds");
    msg1(self_obj, "initWithFrame:", b)
}

unsafe extern "C" fn win_window(self_obj: Id, _cmd: Sel) -> Id {
    self_obj
}
unsafe extern "C" fn win_next_responder(_self: Id, _cmd: Sel) -> Id {
    msg0(cls("UIApplication"), "sharedApplication")
}
unsafe extern "C" fn win_screen(self_obj: Id, _cmd: Sel) -> Id {
    let s = assoc_get(self_obj, &WIN_SCREEN_KEY);
    if s.is_null() {
        msg0(cls("UIScreen"), "mainScreen")
    } else {
        s
    }
}
unsafe extern "C" fn win_set_screen(self_obj: Id, _cmd: Sel, s: Id) {
    assoc_set(self_obj, &WIN_SCREEN_KEY, s);
}
unsafe extern "C" fn win_level(self_obj: Id, _cmd: Sel) -> f64 {
    let n = assoc_get(self_obj, &WIN_LEVEL_KEY);
    if n.is_null() {
        0.0
    } else {
        msg0(n, "doubleValue")
    }
}
unsafe extern "C" fn win_set_level(self_obj: Id, _cmd: Sel, lvl: f64) {
    assoc_set(self_obj, &WIN_LEVEL_KEY, nsnumber_f64(lvl));
}
unsafe extern "C" fn win_is_key(self_obj: Id, _cmd: Sel) -> bool {
    PearUIKitKeyWindow() == self_obj
}
unsafe extern "C" fn win_root(self_obj: Id, _cmd: Sel) -> Id {
    assoc_get(self_obj, &WIN_ROOT_KEY)
}
unsafe extern "C" fn win_set_root(self_obj: Id, _cmd: Sel, vc: Id) {
    let old = assoc_get(self_obj, &WIN_ROOT_KEY);
    if old == vc {
        return;
    }
    if !old.is_null() {
        let ov: Id = msg0(old, "viewIfLoaded");
        if !ov.is_null() {
            msg0::<()>(ov, "removeFromSuperview");
        }
    }
    assoc_set(self_obj, &WIN_ROOT_KEY, vc);
    if !vc.is_null() {
        let v: Id = msg0(vc, "view");
        let b: CGRect = msg0(self_obj, "bounds");
        msg1::<CGRect, ()>(v, "setFrame:", b);
        msg1::<usize, ()>(v, "setAutoresizingMask:", (1 << 1) | (1 << 4));
        msg1::<bool, ()>(vc, "viewWillAppear:", false);
        msg1::<Id, ()>(self_obj, "addSubview:", v);
        msg1::<bool, ()>(vc, "viewDidAppear:", false);
    }
}

unsafe fn win_native_ptr(w: Id) -> *mut c_void {
    let n = assoc_get(w, &WIN_NATIVE_KEY);
    if n.is_null() {
        std::ptr::null_mut()
    } else {
        msg0::<*mut c_void>(n, "pointerValue")
    }
}

unsafe fn win_ensure_native(w: Id) -> *mut c_void {
    let existing = win_native_ptr(w);
    if !existing.is_null() {
        return existing;
    }
    let b: CGRect = msg0(w, "bounds");
    let native = pear_host::host()
        .create_window(pear_host::GuestObject(w), pear_host::Rect { x: b.origin.x, y: b.origin.y, width: b.size.width, height: b.size.height })
        .0;
    let val: Id = msg1(cls("NSValue"), "valueWithPointer:", native);
    assoc_set(w, &WIN_NATIVE_KEY, val);
    let l: Id = msg0(w, "layer");
    msg1::<CGPoint, ()>(l, "setAnchorPoint:", CGPoint::default());
    msg1::<CGPoint, ()>(l, "setPosition:", CGPoint::default());
    if !native.is_null() {
        pear_host::host().set_root_layer(pear_host::WindowHandle(native), pear_host::LayerRef(l));
    }
    native
}

unsafe extern "C" fn win_make_key(self_obj: Id, _cmd: Sel) {
    win_ensure_native(self_obj);
    PearUIKitSetKeyWindow(self_obj);
}
unsafe extern "C" fn win_make_key_visible(self_obj: Id, _cmd: Sel) {
    let native = win_ensure_native(self_obj);
    msg1::<bool, ()>(self_obj, "setHidden:", false);
    if !native.is_null() {
        pear_host::host().show_window(pear_host::WindowHandle(native), true);
    }
    PearUIKitSetKeyWindow(self_obj);
    msg0::<()>(self_obj, "setNeedsLayout");
}
unsafe extern "C" fn win_set_hidden(self_obj: Id, _cmd: Sel, hidden: bool) {
    super1::<bool, ()>(self_obj, "UIWindow", "setHidden:", hidden);
    if !hidden {
        let native = win_native_ptr(self_obj);
        if !native.is_null() {
            pear_host::host().show_window(pear_host::WindowHandle(native), false);
        }
    }
}

#[no_mangle]
pub extern "C" fn PearUIKitWindowDidResize(window: Id, point_size: CGSize, scale: f64) {
    unsafe {
        PearUIKitScreenDidChange(point_size, scale);
        if window.is_null() {
            return;
        }
        let r = CGRect {
            origin: CGPoint::default(),
            size: point_size,
        };
        msg1::<CGRect, ()>(window, "setFrame:", r);
        let l: Id = msg0(window, "layer");
        msg1::<CGPoint, ()>(l, "setAnchorPoint:", CGPoint::default());
        msg1::<CGPoint, ()>(l, "setPosition:", CGPoint::default());
        let w = point_size.width.round() as isize;
        let h = point_size.height.round() as isize;
        let tag = CString::new("host").unwrap();
        let msg = CString::new(format!("window resized to {w}x{h} pt @{scale:.1}x")).unwrap();
        PearHostLogMessage(tag.as_ptr(), msg.as_ptr());
    }
}

// ============================================================================
// UIBarItem / UIBarButtonItem / UINavigationController / UITabBarController
// ============================================================================

static BI_TITLE_KEY: u8 = 0;
static BI_IMAGE_KEY: u8 = 0;
static BI_TAG_KEY: u8 = 0;
static BI_DISABLED_KEY: u8 = 0;
static BBI_STYLE_KEY: u8 = 0;
static BBI_TARGET_KEY: u8 = 0;
static BBI_ACTION_KEY: u8 = 0;
static BBI_CUSTOM_KEY: u8 = 0;
static BBI_WIDTH_KEY: u8 = 0;

unsafe extern "C" fn bi_title(s: Id, _c: Sel) -> Id { assoc_get(s, &BI_TITLE_KEY) }
unsafe extern "C" fn bi_set_title(s: Id, _c: Sel, v: Id) { assoc_set(s, &BI_TITLE_KEY, v); }
unsafe extern "C" fn bi_image(s: Id, _c: Sel) -> Id { assoc_get(s, &BI_IMAGE_KEY) }
unsafe extern "C" fn bi_set_image(s: Id, _c: Sel, v: Id) { assoc_set(s, &BI_IMAGE_KEY, v); }
unsafe extern "C" fn bi_tag(s: Id, _c: Sel) -> isize {
    let n = assoc_get(s, &BI_TAG_KEY);
    if n.is_null() { 0 } else { msg0(n, "integerValue") }
}
unsafe extern "C" fn bi_set_tag(s: Id, _c: Sel, v: isize) { assoc_set(s, &BI_TAG_KEY, nsnumber_i64(v as i64)); }
unsafe extern "C" fn bi_enabled(s: Id, _c: Sel) -> bool {
    let n = assoc_get(s, &BI_DISABLED_KEY);
    n.is_null() || !msg0::<bool>(n, "boolValue")
}
unsafe extern "C" fn bi_set_enabled(s: Id, _c: Sel, en: bool) { assoc_set(s, &BI_DISABLED_KEY, nsnumber_bool(!en)); }

unsafe extern "C" fn bbi_init_title(s: Id, _c: Sel, title: Id, style: isize, target: Id, action: Sel) -> Id {
    let o: Id = super0(s, "UIBarButtonItem", "init");
    if !o.is_null() {
        assoc_set(o, &BI_TITLE_KEY, title);
        assoc_set(o, &BBI_STYLE_KEY, nsnumber_i64(style as i64));
        pear_objc::assoc_set_weak(o, &BBI_TARGET_KEY, target);
        if !action.is_null() {
            assoc_set(o, &BBI_ACTION_KEY, msg1(cls("NSValue"), "valueWithPointer:", action));
        }
    }
    o
}
unsafe extern "C" fn bbi_init_image(s: Id, _c: Sel, image: Id, style: isize, target: Id, action: Sel) -> Id {
    let o: Id = super0(s, "UIBarButtonItem", "init");
    if !o.is_null() {
        assoc_set(o, &BI_IMAGE_KEY, image);
        assoc_set(o, &BBI_STYLE_KEY, nsnumber_i64(style as i64));
        pear_objc::assoc_set_weak(o, &BBI_TARGET_KEY, target);
        if !action.is_null() {
            assoc_set(o, &BBI_ACTION_KEY, msg1(cls("NSValue"), "valueWithPointer:", action));
        }
    }
    o
}
unsafe extern "C" fn bbi_init_sys(s: Id, _c: Sel, _sys: isize, target: Id, action: Sel) -> Id {
    let o: Id = super0(s, "UIBarButtonItem", "init");
    if !o.is_null() {
        pear_objc::assoc_set_weak(o, &BBI_TARGET_KEY, target);
        if !action.is_null() {
            assoc_set(o, &BBI_ACTION_KEY, msg1(cls("NSValue"), "valueWithPointer:", action));
        }
    }
    o
}
unsafe extern "C" fn bbi_init_custom(s: Id, _c: Sel, cv: Id) -> Id {
    let o: Id = super0(s, "UIBarButtonItem", "init");
    if !o.is_null() {
        assoc_set(o, &BBI_CUSTOM_KEY, cv);
    }
    o
}
unsafe extern "C" fn bbi_style(s: Id, _c: Sel) -> isize {
    let n = assoc_get(s, &BBI_STYLE_KEY);
    if n.is_null() { 0 } else { msg0(n, "integerValue") }
}
unsafe extern "C" fn bbi_set_style(s: Id, _c: Sel, v: isize) { assoc_set(s, &BBI_STYLE_KEY, nsnumber_i64(v as i64)); }
unsafe extern "C" fn bbi_target(s: Id, _c: Sel) -> Id { pear_objc::assoc_get_weak(s, &BBI_TARGET_KEY) }
unsafe extern "C" fn bbi_set_target(s: Id, _c: Sel, v: Id) { pear_objc::assoc_set_weak(s, &BBI_TARGET_KEY, v); }
unsafe extern "C" fn bbi_action(s: Id, _c: Sel) -> Sel {
    let v = assoc_get(s, &BBI_ACTION_KEY);
    if v.is_null() { std::ptr::null_mut() } else { msg0(v, "pointerValue") }
}
unsafe extern "C" fn bbi_set_action(s: Id, _c: Sel, a: Sel) {
    let v = if a.is_null() { NIL } else { msg1(cls("NSValue"), "valueWithPointer:", a) };
    assoc_set(s, &BBI_ACTION_KEY, v);
}
unsafe extern "C" fn bbi_custom(s: Id, _c: Sel) -> Id { assoc_get(s, &BBI_CUSTOM_KEY) }
unsafe extern "C" fn bbi_set_custom(s: Id, _c: Sel, v: Id) { assoc_set(s, &BBI_CUSTOM_KEY, v); }
unsafe extern "C" fn bbi_width(s: Id, _c: Sel) -> f64 {
    let n = assoc_get(s, &BBI_WIDTH_KEY);
    if n.is_null() { 0.0 } else { msg0(n, "doubleValue") }
}
unsafe extern "C" fn bbi_set_width(s: Id, _c: Sel, v: f64) { assoc_set(s, &BBI_WIDTH_KEY, nsnumber_f64(v)); }

static NAV_VCS_KEY: u8 = 0;
static NAV_HIDDEN_KEY: u8 = 0;
static NAV_DEL_KEY: u8 = 0;

unsafe fn nav_show_top(nav: Id, anim: bool) {
    if !msg0::<bool>(nav, "isViewLoaded") {
        return;
    }
    let vcs = mut_array_for_key(nav, &NAV_VCS_KEY);
    let n = nsarray_count(vcs);
    if n == 0 {
        return;
    }
    let top = nsarray_object_at(vcs, n - 1);
    let top_v: Id = msg0(top, "view");
    let my_v: Id = msg0(nav, "view");
    let b: CGRect = msg0(my_v, "bounds");
    msg1::<CGRect, ()>(top_v, "setFrame:", b);
    msg1::<usize, ()>(top_v, "setAutoresizingMask:", (1 << 1) | (1 << 4));
    msg1::<bool, ()>(top, "viewWillAppear:", anim);
    msg1::<Id, ()>(my_v, "addSubview:", top_v);
    msg1::<bool, ()>(top, "viewDidAppear:", anim);
}

unsafe extern "C" fn nav_init_root(s: Id, _c: Sel, root: Id) -> Id {
    let o: Id = super2(s, "UINavigationController", "initWithNibName:bundle:", NIL, NIL);
    if !o.is_null() && !root.is_null() {
        let vcs = mut_array_for_key(o, &NAV_VCS_KEY);
        msg1::<Id, ()>(vcs, "addObject:", root);
    }
    o
}
unsafe extern "C" fn nav_top(s: Id, _c: Sel) -> Id {
    let vcs = mut_array_for_key(s, &NAV_VCS_KEY);
    let n = nsarray_count(vcs);
    if n == 0 { NIL } else { nsarray_object_at(vcs, n - 1) }
}
unsafe extern "C" fn nav_visible(s: Id, _c: Sel) -> Id {
    let p: Id = msg0(s, "presentedViewController");
    if !p.is_null() { p } else { nav_top(s, sel("topViewController")) }
}
unsafe extern "C" fn nav_hidden(s: Id, _c: Sel) -> bool {
    let n = assoc_get(s, &NAV_HIDDEN_KEY);
    !n.is_null() && msg0::<bool>(n, "boolValue")
}
unsafe extern "C" fn nav_set_hidden(s: Id, _c: Sel, h: bool) { assoc_set(s, &NAV_HIDDEN_KEY, nsnumber_bool(h)); }
unsafe extern "C" fn nav_set_hidden_anim(s: Id, _c: Sel, h: bool, _a: bool) { assoc_set(s, &NAV_HIDDEN_KEY, nsnumber_bool(h)); }
unsafe extern "C" fn nav_vcs(s: Id, _c: Sel) -> Id {
    msg1(cls("NSArray"), "arrayWithArray:", mut_array_for_key(s, &NAV_VCS_KEY))
}
unsafe extern "C" fn nav_set_vcs(s: Id, _c: Sel, arr: Id) { nav_set_vcs_anim(s, sel("setViewControllers:animated:"), arr, false); }
unsafe extern "C" fn nav_set_vcs_anim(s: Id, _c: Sel, arr: Id, anim: bool) {
    let vcs = mut_array_for_key(s, &NAV_VCS_KEY);
    let old = nav_top(s, sel("topViewController"));
    if !old.is_null() {
        let ov: Id = msg0(old, "viewIfLoaded");
        if !ov.is_null() { msg0::<()>(ov, "removeFromSuperview"); }
    }
    msg0::<()>(vcs, "removeAllObjects");
    if !arr.is_null() {
        msg1::<Id, ()>(vcs, "addObjectsFromArray:", arr);
    }
    nav_show_top(s, anim);
}
unsafe extern "C" fn nav_push(s: Id, _c: Sel, vc: Id, anim: bool) {
    if vc.is_null() { return; }
    let old = nav_top(s, sel("topViewController"));
    if !old.is_null() {
        let ov: Id = msg0(old, "viewIfLoaded");
        if !ov.is_null() { msg0::<()>(ov, "removeFromSuperview"); }
    }
    msg1::<Id, ()>(mut_array_for_key(s, &NAV_VCS_KEY), "addObject:", vc);
    nav_show_top(s, anim);
}
unsafe extern "C" fn nav_pop(s: Id, _c: Sel, anim: bool) -> Id {
    let vcs = mut_array_for_key(s, &NAV_VCS_KEY);
    let n = nsarray_count(vcs);
    if n <= 1 { return NIL; }
    let popped = nsarray_object_at(vcs, n - 1);
    objc_retain(popped);
    msg0::<()>(vcs, "removeLastObject");
    let pv: Id = msg0(popped, "viewIfLoaded");
    if !pv.is_null() { msg0::<()>(pv, "removeFromSuperview"); }
    nav_show_top(s, anim);
    pear_objc::objc_autorelease(popped)
}
unsafe extern "C" fn nav_pop_root(s: Id, _c: Sel, anim: bool) -> Id {
    let vcs = mut_array_for_key(s, &NAV_VCS_KEY);
    let n = nsarray_count(vcs);
    if n <= 1 { return NIL; }
    let mut popped = Vec::new();
    for i in 1..n {
        let vc = nsarray_object_at(vcs, i);
        let pv: Id = msg0(vc, "viewIfLoaded");
        if !pv.is_null() { msg0::<()>(pv, "removeFromSuperview"); }
        popped.push(vc);
    }
    let root = nsarray_object_at(vcs, 0);
    objc_retain(root);
    let ret = nsarray_from_ids(&popped);
    msg0::<()>(vcs, "removeAllObjects");
    msg1::<Id, ()>(vcs, "addObject:", root);
    objc_release(root);
    nav_show_top(s, anim);
    ret
}
unsafe extern "C" fn nav_view_did_load(s: Id, _c: Sel) {
    super0::<()>(s, "UINavigationController", "viewDidLoad");
    nav_show_top(s, false);
}
unsafe extern "C" fn nav_delegate(s: Id, _c: Sel) -> Id { pear_objc::assoc_get_weak(s, &NAV_DEL_KEY) }
unsafe extern "C" fn nav_set_delegate(s: Id, _c: Sel, d: Id) { pear_objc::assoc_set_weak(s, &NAV_DEL_KEY, d); }

static TAB_VCS_KEY: u8 = 0;
static TAB_IDX_KEY: u8 = 0;
static TAB_DEL_KEY: u8 = 0;

unsafe fn tab_show_selected(t: Id) {
    if !msg0::<bool>(t, "isViewLoaded") { return; }
    let sel_vc = tab_selected_vc(t, sel("selectedViewController"));
    if sel_vc.is_null() { return; }
    let cv: Id = msg0(sel_vc, "view");
    let mv: Id = msg0(t, "view");
    let b: CGRect = msg0(mv, "bounds");
    msg1::<CGRect, ()>(cv, "setFrame:", b);
    msg1::<usize, ()>(cv, "setAutoresizingMask:", (1 << 1) | (1 << 4));
    msg1::<Id, ()>(mv, "addSubview:", cv);
}
unsafe extern "C" fn tab_vcs(s: Id, _c: Sel) -> Id {
    msg1(cls("NSArray"), "arrayWithArray:", mut_array_for_key(s, &TAB_VCS_KEY))
}
unsafe extern "C" fn tab_set_vcs(s: Id, _c: Sel, arr: Id) { tab_set_vcs_anim(s, sel("setViewControllers:animated:"), arr, false); }
unsafe extern "C" fn tab_set_vcs_anim(s: Id, _c: Sel, arr: Id, _anim: bool) {
    let vcs = mut_array_for_key(s, &TAB_VCS_KEY);
    for i in 0..nsarray_count(vcs) {
        let cv: Id = msg0(nsarray_object_at(vcs, i), "viewIfLoaded");
        if !cv.is_null() { msg0::<()>(cv, "removeFromSuperview"); }
    }
    msg0::<()>(vcs, "removeAllObjects");
    if !arr.is_null() { msg1::<Id, ()>(vcs, "addObjectsFromArray:", arr); }
    let count = nsarray_count(vcs);
    let idx = if count == 0 { isize::MAX } else { tab_idx(s, sel("selectedIndex")).min((count - 1) as isize) };
    assoc_set(s, &TAB_IDX_KEY, nsnumber_i64(idx as i64));
    tab_show_selected(s);
}
unsafe extern "C" fn tab_idx(s: Id, _c: Sel) -> isize {
    let n = assoc_get(s, &TAB_IDX_KEY);
    if n.is_null() { 0 } else { msg0(n, "integerValue") }
}
unsafe extern "C" fn tab_set_idx(s: Id, _c: Sel, idx: isize) {
    let vcs = mut_array_for_key(s, &TAB_VCS_KEY);
    let count = nsarray_count(vcs) as isize;
    if idx < 0 || idx >= count { return; }
    let old = tab_selected_vc(s, sel("selectedViewController"));
    if !old.is_null() {
        let ov: Id = msg0(old, "viewIfLoaded");
        if !ov.is_null() { msg0::<()>(ov, "removeFromSuperview"); }
    }
    assoc_set(s, &TAB_IDX_KEY, nsnumber_i64(idx as i64));
    tab_show_selected(s);
}
unsafe extern "C" fn tab_selected_vc(s: Id, _c: Sel) -> Id {
    let vcs = mut_array_for_key(s, &TAB_VCS_KEY);
    let idx = tab_idx(s, sel("selectedIndex"));
    if idx >= 0 && (idx as usize) < nsarray_count(vcs) {
        nsarray_object_at(vcs, idx as usize)
    } else {
        NIL
    }
}
unsafe extern "C" fn tab_set_selected_vc(s: Id, _c: Sel, vc: Id) {
    let vcs = mut_array_for_key(s, &TAB_VCS_KEY);
    for i in 0..nsarray_count(vcs) {
        if nsarray_object_at(vcs, i) == vc {
            tab_set_idx(s, sel("setSelectedIndex:"), i as isize);
            break;
        }
    }
}
unsafe extern "C" fn tab_view_did_load(s: Id, _c: Sel) {
    super0::<()>(s, "UITabBarController", "viewDidLoad");
    tab_show_selected(s);
}
unsafe extern "C" fn tab_delegate(s: Id, _c: Sel) -> Id { pear_objc::assoc_get_weak(s, &TAB_DEL_KEY) }
unsafe extern "C" fn tab_set_delegate(s: Id, _c: Sel, d: Id) { pear_objc::assoc_set_weak(s, &TAB_DEL_KEY, d); }

// ============================================================================
// UIAlertAction / UIAlertController
// ============================================================================

static AA_TITLE_KEY: u8 = 0;
static AA_STYLE_KEY: u8 = 0;
static AA_HANDLER_KEY: u8 = 0;
static AA_DISABLED_KEY: u8 = 0;

unsafe extern "C" fn aa_action(self_cls: Id, _c: Sel, title: Id, style: isize, handler: Id) -> Id {
    let a: Id = msg0(self_cls, "new");
    assoc_set(a, &AA_TITLE_KEY, title);
    assoc_set(a, &AA_STYLE_KEY, nsnumber_i64(style as i64));
    if !handler.is_null() {
        let copied = _Block_copy(handler as *const c_void) as Id;
        assoc_set(a, &AA_HANDLER_KEY, copied);
        _Block_release(copied as *const c_void);
    }
    pear_objc::objc_autorelease(a)
}
unsafe extern "C" fn aa_title(s: Id, _c: Sel) -> Id { assoc_get(s, &AA_TITLE_KEY) }
unsafe extern "C" fn aa_style(s: Id, _c: Sel) -> isize {
    let n = assoc_get(s, &AA_STYLE_KEY);
    if n.is_null() { 0 } else { msg0(n, "integerValue") }
}
unsafe extern "C" fn aa_enabled(s: Id, _c: Sel) -> bool {
    let n = assoc_get(s, &AA_DISABLED_KEY);
    n.is_null() || !msg0::<bool>(n, "boolValue")
}
unsafe extern "C" fn aa_set_enabled(s: Id, _c: Sel, en: bool) { assoc_set(s, &AA_DISABLED_KEY, nsnumber_bool(!en)); }
unsafe extern "C" fn aa_fire(s: Id, _c: Sel) {
    let h = assoc_get(s, &AA_HANDLER_KEY);
    if !h.is_null() { invoke_id_block(h, s); }
}
unsafe extern "C" fn copy_self(s: Id, _c: Sel, _z: *mut c_void) -> Id { objc_retain(s) }

static AC_MSG_KEY: u8 = 0;
static AC_STYLE_KEY: u8 = 0;
static AC_ACTIONS_KEY: u8 = 0;
static AC_PREF_KEY: u8 = 0;

unsafe extern "C" fn ac_create(_cls: Id, _c: Sel, title: Id, message: Id, style: isize) -> Id {
    let c: Id = msg0(cls("UIAlertController"), "new");
    msg1::<Id, ()>(c, "setTitle:", title);
    assoc_set(c, &AC_MSG_KEY, message);
    assoc_set(c, &AC_STYLE_KEY, nsnumber_i64(style as i64));
    pear_objc::objc_autorelease(c)
}
unsafe extern "C" fn ac_msg(s: Id, _c: Sel) -> Id { assoc_get(s, &AC_MSG_KEY) }
unsafe extern "C" fn ac_set_msg(s: Id, _c: Sel, m: Id) { assoc_set(s, &AC_MSG_KEY, m); }
unsafe extern "C" fn ac_style(s: Id, _c: Sel) -> isize {
    let n = assoc_get(s, &AC_STYLE_KEY);
    if n.is_null() { 0 } else { msg0(n, "integerValue") }
}
unsafe extern "C" fn ac_actions(s: Id, _c: Sel) -> Id {
    msg1(cls("NSArray"), "arrayWithArray:", mut_array_for_key(s, &AC_ACTIONS_KEY))
}
unsafe extern "C" fn ac_pref(s: Id, _c: Sel) -> Id { assoc_get(s, &AC_PREF_KEY) }
unsafe extern "C" fn ac_set_pref(s: Id, _c: Sel, a: Id) { assoc_set(s, &AC_PREF_KEY, a); }
unsafe extern "C" fn ac_add_action(s: Id, _c: Sel, a: Id) {
    if !a.is_null() { msg1::<Id, ()>(mut_array_for_key(s, &AC_ACTIONS_KEY), "addObject:", a); }
}
unsafe extern "C" fn ac_add_tf(_s: Id, _c: Sel, _h: Id) {
    let c1 = CString::new("UIAlertController").unwrap();
    let c2 = CString::new("text fields in alerts").unwrap();
    PearMissingReport(1, c1.as_ptr(), c2.as_ptr(), std::ptr::null());
}

extern "C" fn ac_on_button(ctx: *mut c_void, index: isize) {
    unsafe {
        let arr = ctx as Id;
        if index >= 0 && (index as usize) < nsarray_count(arr) {
            let act = nsarray_object_at(arr, index as usize);
            msg0::<()>(act, "pear_fire");
        }
        objc_release(arr);
    }
}

unsafe extern "C" fn ac_present(s: Id, _c: Sel, completion: Id) {
    let actions = mut_array_for_key(s, &AC_ACTIONS_KEY);
    let snapshot: Id = msg1(cls("NSArray"), "arrayWithArray:", actions);
    objc_retain(snapshot);
    let mut titles_c = Vec::new();
    let mut titles_str = Vec::new();
    for i in 0..nsarray_count(snapshot) {
        let a = nsarray_object_at(snapshot, i);
        let t = nsstring_to_string(msg0(a, "title")).unwrap_or_else(|| "OK".to_string());
        titles_str.push(t.clone());
        titles_c.push(CString::new(t).unwrap_or_default());
    }
    let title_s = nsstring_to_string(msg0(s, "title")).unwrap_or_default();
    let msg_s = nsstring_to_string(assoc_get(s, &AC_MSG_KEY)).unwrap_or_default();
    let tag = CString::new("alert").unwrap();
    let log_msg = CString::new(format!("title=\"{title_s}\" message=\"{msg_s}\" actions=[{}]", titles_str.join(", "))).unwrap_or_default();
    PearHostLogMessage(tag.as_ptr(), log_msg.as_ptr());

    drop(titles_c);
    let actions_ptr = snapshot as usize;
    pear_host::host().present_alert(&title_s, &msg_s, &titles_str, Box::new(move |index| ac_on_button(actions_ptr as *mut c_void, index)));
    dispatch_main_block(completion);
}

// ============================================================================
// System Panels: UIActivityViewController, UIDocumentPickerViewController,
// UIImagePickerController, UIDocument, UIPopoverController
// ============================================================================

static ACT_ITEMS_KEY: u8 = 0;
static ACT_EXCL_KEY: u8 = 0;
static ACT_COMP_KEY: u8 = 0;

unsafe extern "C" fn act_init(s: Id, _c: Sel, items: Id, _app: Id) -> Id {
    let o: Id = super2(s, "UIActivityViewController", "initWithNibName:bundle:", NIL, NIL);
    if !o.is_null() { assoc_set(o, &ACT_ITEMS_KEY, items); }
    o
}
unsafe extern "C" fn act_init_coder(s: Id, _c: Sel, coder: Id) -> Id {
    super1(s, "UIActivityViewController", "initWithCoder:", coder)
}
unsafe extern "C" fn act_did_appear(s: Id, _c: Sel, anim: bool) {
    super1::<bool, ()>(s, "UIActivityViewController", "viewDidAppear:", anim);
    let items = assoc_get(s, &ACT_ITEMS_KEY);
    let picker: Id = msg1(msg0(cls("NSSharingServicePicker"), "alloc"), "initWithItems:", items);
    let ns_app = msg0::<Id>(cls("NSApplication"), "sharedApplication");
    let kw: Id = msg0(ns_app, "keyWindow");
    let cv: Id = if kw.is_null() { NIL } else { msg0(kw, "contentView") };
    if !cv.is_null() {
        let b: CGRect = msg0(cv, "bounds");
        let rect = CGRect {
            origin: CGPoint { x: b.origin.x + b.size.width * 0.5, y: b.origin.y + b.size.height * 0.5 },
            size: CGSize { width: 1.0, height: 1.0 },
        };
        msg3::<CGRect, Id, usize, ()>(picker, "showRelativeToRect:ofView:preferredEdge:", rect, cv, 1);
    }
    objc_release(picker);
}
unsafe extern "C" fn act_excl(s: Id, _c: Sel) -> Id { assoc_get(s, &ACT_EXCL_KEY) }
unsafe extern "C" fn act_set_excl(s: Id, _c: Sel, v: Id) { assoc_set(s, &ACT_EXCL_KEY, v); }
unsafe extern "C" fn act_comp(s: Id, _c: Sel) -> Id { assoc_get(s, &ACT_COMP_KEY) }
unsafe extern "C" fn act_set_comp(s: Id, _c: Sel, v: Id) {
    if v.is_null() {
        assoc_set(s, &ACT_COMP_KEY, NIL);
    } else {
        let c = _Block_copy(v as *const c_void) as Id;
        assoc_set(s, &ACT_COMP_KEY, c);
        _Block_release(c as *const c_void);
    }
}

static DP_DEL_KEY: u8 = 0;
static DP_MULTI_KEY: u8 = 0;
static DP_EXT_KEY: u8 = 0;
static DP_DIR_KEY: u8 = 0;
static DP_URLS_KEY: u8 = 0;
static DP_SAVE_KEY: u8 = 0;

unsafe extern "C" fn dp_init_open(s: Id, _c: Sel, _types: Id, _as_copy: bool) -> Id {
    super2(s, "UIDocumentPickerViewController", "initWithNibName:bundle:", NIL, NIL)
}
unsafe extern "C" fn dp_init_utis(s: Id, _c: Sel, _utis: Id, mode: isize) -> Id {
    let o: Id = super2(s, "UIDocumentPickerViewController", "initWithNibName:bundle:", NIL, NIL);
    if !o.is_null() { assoc_set(o, &DP_SAVE_KEY, nsnumber_bool(mode == 2 || mode == 3)); }
    o
}
unsafe extern "C" fn dp_init_export(s: Id, _c: Sel, urls: Id, _as_copy: bool) -> Id {
    let o: Id = super2(s, "UIDocumentPickerViewController", "initWithNibName:bundle:", NIL, NIL);
    if !o.is_null() {
        assoc_set(o, &DP_URLS_KEY, urls);
        assoc_set(o, &DP_SAVE_KEY, nsnumber_bool(true));
    }
    o
}
extern "C" fn dp_run_panel_async(ctx: *mut c_void) {
    unsafe {
        let s = ctx as Id;
        let is_save = !assoc_get(s, &DP_SAVE_KEY).is_null() && msg0::<bool>(assoc_get(s, &DP_SAVE_KEY), "boolValue");
        let dir = assoc_get(s, &DP_DIR_KEY);
        let del = pear_objc::assoc_get_weak(s, &DP_DEL_KEY);
        if is_save {
            let panel: Id = msg0(cls("NSSavePanel"), "savePanel");
            if !dir.is_null() { msg1::<Id, ()>(panel, "setDirectoryURL:", dir); }
            let urls = assoc_get(s, &DP_URLS_KEY);
            let name = if nsarray_count(urls) > 0 {
                msg0(nsarray_object_at(urls, 0), "lastPathComponent")
            } else {
                nsstring("Document")
            };
            msg1::<Id, ()>(panel, "setNameFieldStringValue:", name);
            let resp: isize = msg0(panel, "runModal");
            if resp == 1 {
                let dest: Id = msg0(panel, "URL");
                if !dest.is_null() && !del.is_null() && responds_to(del, "documentPicker:didPickDocumentsAtURLs:") {
                    msg2::<Id, Id, ()>(del, "documentPicker:didPickDocumentsAtURLs:", s, nsarray_from_ids(&[dest]));
                }
            } else if !del.is_null() && responds_to(del, "documentPickerWasCancelled:") {
                msg1::<Id, ()>(del, "documentPickerWasCancelled:", s);
            }
        } else {
            let panel: Id = msg0(cls("NSOpenPanel"), "openPanel");
            let multi = !assoc_get(s, &DP_MULTI_KEY).is_null() && msg0::<bool>(assoc_get(s, &DP_MULTI_KEY), "boolValue");
            msg1::<bool, ()>(panel, "setAllowsMultipleSelection:", multi);
            if !dir.is_null() { msg1::<Id, ()>(panel, "setDirectoryURL:", dir); }
            let resp: isize = msg0(panel, "runModal");
            if resp == 1 {
                let picked: Id = msg0(panel, "URLs");
                if !del.is_null() && responds_to(del, "documentPicker:didPickDocumentsAtURLs:") {
                    msg2::<Id, Id, ()>(del, "documentPicker:didPickDocumentsAtURLs:", s, picked);
                }
            } else if !del.is_null() && responds_to(del, "documentPickerWasCancelled:") {
                msg1::<Id, ()>(del, "documentPickerWasCancelled:", s);
            }
        }
        objc_release(s);
    }
}
unsafe extern "C" fn dp_did_appear(s: Id, _c: Sel, anim: bool) {
    super1::<bool, ()>(s, "UIDocumentPickerViewController", "viewDidAppear:", anim);
    objc_retain(s);
    dispatch_async_f(&_dispatch_main_q as *const c_void, s, dp_run_panel_async);
}
unsafe extern "C" fn dp_del(s: Id, _c: Sel) -> Id { pear_objc::assoc_get_weak(s, &DP_DEL_KEY) }
unsafe extern "C" fn dp_set_del(s: Id, _c: Sel, d: Id) { pear_objc::assoc_set_weak(s, &DP_DEL_KEY, d); }
unsafe extern "C" fn dp_multi(s: Id, _c: Sel) -> bool {
    let n = assoc_get(s, &DP_MULTI_KEY);
    !n.is_null() && msg0::<bool>(n, "boolValue")
}
unsafe extern "C" fn dp_set_multi(s: Id, _c: Sel, b: bool) { assoc_set(s, &DP_MULTI_KEY, nsnumber_bool(b)); }
unsafe extern "C" fn dp_ext(s: Id, _c: Sel) -> bool {
    let n = assoc_get(s, &DP_EXT_KEY);
    !n.is_null() && msg0::<bool>(n, "boolValue")
}
unsafe extern "C" fn dp_set_ext(s: Id, _c: Sel, b: bool) { assoc_set(s, &DP_EXT_KEY, nsnumber_bool(b)); }
unsafe extern "C" fn dp_dir(s: Id, _c: Sel) -> Id { assoc_get(s, &DP_DIR_KEY) }
unsafe extern "C" fn dp_set_dir(s: Id, _c: Sel, d: Id) { assoc_set(s, &DP_DIR_KEY, d); }

unsafe extern "C" fn ip_source_avail(_cls: Id, _c: Sel, ty: isize) -> bool { ty == 0 || ty == 2 }
unsafe extern "C" fn ip_media_types(c: Id, _c: Sel, ty: isize) -> Id {
    if ip_source_avail(c, sel("isSourceTypeAvailable:"), ty) {
        nsarray_from_ids(&[nsstring("public.image")])
    } else {
        NIL
    }
}

static DOC_URL_KEY: u8 = 0;
unsafe extern "C" fn doc_init(s: Id, _c: Sel, url: Id) -> Id {
    let o: Id = super0(s, "UIDocument", "init");
    if !o.is_null() { assoc_set(o, &DOC_URL_KEY, url); }
    o
}
unsafe extern "C" fn doc_url(s: Id, _c: Sel) -> Id { assoc_get(s, &DOC_URL_KEY) }
unsafe extern "C" fn doc_set_url(s: Id, _c: Sel, u: Id) { assoc_set(s, &DOC_URL_KEY, u); }
unsafe extern "C" fn doc_name(s: Id, _c: Sel) -> Id { msg0(assoc_get(s, &DOC_URL_KEY), "lastPathComponent") }
unsafe extern "C" fn doc_contents(_s: Id, _c: Sel, _t: Id, _e: *mut Id) -> Id { msg0(cls("NSData"), "data") }
unsafe extern "C" fn doc_load(_s: Id, _c: Sel, _d: Id, _t: Id, _e: *mut Id) -> bool { true }
unsafe extern "C" fn doc_open(s: Id, _c: Sel, handler: Id) {
    let url = assoc_get(s, &DOC_URL_KEY);
    let data: Id = msg1(cls("NSData"), "dataWithContentsOfURL:", url);
    let ok = if !data.is_null() {
        let ext: Id = msg0(url, "pathExtension");
        let mut err: Id = NIL;
        msg3::<Id, Id, *mut Id, bool>(s, "loadFromContents:ofType:error:", data, ext, &mut err)
    } else {
        false
    };
    if !handler.is_null() {
        let hdr = &*(handler as *const BlockHeader);
        let f: unsafe extern "C" fn(Id, bool) = std::mem::transmute(hdr.invoke);
        f(handler, ok);
    }
}
unsafe extern "C" fn doc_close(_s: Id, _c: Sel, handler: Id) {
    if !handler.is_null() {
        let hdr = &*(handler as *const BlockHeader);
        let f: unsafe extern "C" fn(Id, bool) = std::mem::transmute(hdr.invoke);
        f(handler, true);
    }
}
unsafe extern "C" fn doc_save(s: Id, _c: Sel, url: Id, _op: isize, handler: Id) {
    let ext: Id = msg0(url, "pathExtension");
    let mut err: Id = NIL;
    let data: Id = msg2(s, "contentsForType:error:", ext, &mut err);
    let ok = if is_kind_of(data, "NSData") {
        msg2::<Id, bool, bool>(data, "writeToURL:atomically:", url, true)
    } else {
        false
    };
    if !handler.is_null() {
        let hdr = &*(handler as *const BlockHeader);
        let f: unsafe extern "C" fn(Id, bool) = std::mem::transmute(hdr.invoke);
        f(handler, ok);
    }
}

static POP_VC_KEY: u8 = 0;
static POP_DEL_KEY: u8 = 0;
static POP_SIZE_KEY: u8 = 0;
static POP_VIS_KEY: u8 = 0;

unsafe extern "C" fn pop_init(s: Id, _c: Sel, vc: Id) -> Id {
    let o: Id = super0(s, "UIPopoverController", "init");
    if !o.is_null() { assoc_set(o, &POP_VC_KEY, vc); }
    o
}
unsafe extern "C" fn pop_vc(s: Id, _c: Sel) -> Id { assoc_get(s, &POP_VC_KEY) }
unsafe extern "C" fn pop_set_vc(s: Id, _c: Sel, v: Id) { assoc_set(s, &POP_VC_KEY, v); }
unsafe extern "C" fn pop_del(s: Id, _c: Sel) -> Id { pear_objc::assoc_get_weak(s, &POP_DEL_KEY) }
unsafe extern "C" fn pop_set_del(s: Id, _c: Sel, v: Id) { pear_objc::assoc_set_weak(s, &POP_DEL_KEY, v); }
unsafe extern "C" fn pop_size(s: Id, _c: Sel) -> CGSize {
    let d = assoc_get(s, &POP_SIZE_KEY);
    if let Some(b) = nsdata_to_vec(d) {
        if b.len() == std::mem::size_of::<CGSize>() {
            return std::ptr::read_unaligned(b.as_ptr() as *const CGSize);
        }
    }
    CGSize { width: 320.0, height: 480.0 }
}
unsafe extern "C" fn pop_set_size(s: Id, _c: Sel, sz: CGSize) {
    let bytes = std::slice::from_raw_parts(&sz as *const CGSize as *const u8, std::mem::size_of::<CGSize>());
    assoc_set(s, &POP_SIZE_KEY, nsdata_from_bytes(bytes));
}
unsafe extern "C" fn pop_vis(s: Id, _c: Sel) -> bool {
    let n = assoc_get(s, &POP_VIS_KEY);
    !n.is_null() && msg0::<bool>(n, "boolValue")
}
unsafe extern "C" fn pop_present(s: Id, _c: Sel, _r: CGRect, view: Id, _dirs: usize, anim: bool) {
    assoc_set(s, &POP_VIS_KEY, nsnumber_bool(true));
    let w: Id = if view.is_null() { NIL } else { msg0(view, "window") };
    let rvc: Id = if w.is_null() { NIL } else { msg0(w, "rootViewController") };
    if !rvc.is_null() {
        let vc = assoc_get(s, &POP_VC_KEY);
        msg3::<Id, bool, Id, ()>(rvc, "presentViewController:animated:completion:", vc, anim, NIL);
    }
}
unsafe extern "C" fn pop_dismiss(s: Id, _c: Sel, anim: bool) {
    if !pop_vis(s, sel("isPopoverVisible")) { return; }
    assoc_set(s, &POP_VIS_KEY, nsnumber_bool(false));
    let vc = assoc_get(s, &POP_VC_KEY);
    if !vc.is_null() {
        msg2::<bool, Id, ()>(vc, "dismissViewControllerAnimated:completion:", anim, NIL);
    }
}

// ---- UIImagePickerController properties ----
static IP_SOURCE_KEY: u8 = 0;
static IP_MEDIA_KEY: u8 = 0;
static IP_EDITING_KEY: u8 = 0;
static IP_CAMERA_CONTROLS_KEY: u8 = 0;

unsafe extern "C" fn ip_source(s: Id, _c: Sel) -> isize {
    pear_objc::assoc_i64(s, &IP_SOURCE_KEY, 0) as isize
}
unsafe extern "C" fn ip_set_source(s: Id, _c: Sel, v: isize) {
    pear_objc::assoc_set_i64(s, &IP_SOURCE_KEY, v as i64);
}
unsafe extern "C" fn ip_media(s: Id, _c: Sel) -> Id {
    let v = assoc_get(s, &IP_MEDIA_KEY);
    if v.is_null() {
        nsarray_from_ids(&[nsstring("public.image")])
    } else {
        v
    }
}
unsafe extern "C" fn ip_set_media(s: Id, _c: Sel, v: Id) {
    pear_objc::assoc_set_copy(s, &IP_MEDIA_KEY, v);
}
unsafe extern "C" fn ip_editing(s: Id, _c: Sel) -> bool {
    pear_objc::assoc_bool(s, &IP_EDITING_KEY, false)
}
unsafe extern "C" fn ip_set_editing(s: Id, _c: Sel, v: bool) {
    pear_objc::assoc_set_bool(s, &IP_EDITING_KEY, v);
}
unsafe extern "C" fn ip_camera_controls(s: Id, _c: Sel) -> bool {
    pear_objc::assoc_bool(s, &IP_CAMERA_CONTROLS_KEY, true)
}
unsafe extern "C" fn ip_set_camera_controls(s: Id, _c: Sel, v: bool) {
    pear_objc::assoc_set_bool(s, &IP_CAMERA_CONTROLS_KEY, v);
}

// ---- UIView interactions (UIInteraction protocol) ----
static VIEW_INTERACTIONS_KEY: u8 = 0;

unsafe extern "C" fn view_interactions(s: Id, _c: Sel) -> Id {
    let arr = assoc_get(s, &VIEW_INTERACTIONS_KEY);
    if arr.is_null() {
        msg0(cls("NSArray"), "array")
    } else {
        autoreleased_copy(arr)
    }
}
unsafe fn autoreleased_copy(o: Id) -> Id {
    pear_objc::autoreleased(msg0(o, "copy"))
}
unsafe extern "C" fn view_add_interaction(s: Id, _c: Sel, i: Id) {
    if i.is_null() {
        return;
    }
    let arr = mut_array_for_key(s, &VIEW_INTERACTIONS_KEY);
    if msg1::<Id, bool>(arr, "containsObject:", i) {
        return;
    }
    if responds_to(i, "willMoveToView:") {
        msg1::<Id, ()>(i, "willMoveToView:", s);
    }
    msg1::<Id, ()>(arr, "addObject:", i);
    if responds_to(i, "didMoveToView:") {
        msg1::<Id, ()>(i, "didMoveToView:", s);
    }
}
unsafe extern "C" fn view_remove_interaction(s: Id, _c: Sel, i: Id) {
    let arr = assoc_get(s, &VIEW_INTERACTIONS_KEY);
    if i.is_null() || arr.is_null() || !msg1::<Id, bool>(arr, "containsObject:", i) {
        return;
    }
    objc_retain(i);
    if responds_to(i, "willMoveToView:") {
        msg1::<Id, ()>(i, "willMoveToView:", NIL);
    }
    msg1::<Id, ()>(arr, "removeObject:", i);
    if responds_to(i, "didMoveToView:") {
        msg1::<Id, ()>(i, "didMoveToView:", NIL);
    }
    objc_release(i);
}

pub unsafe fn register_classes() {
    let v = cls("UIView");
    if !v.is_null() {
        add_class_method(v, "layerClass", view_layer_class as Imp, "#@:");
        add_method(v, "init", view_init as Imp, "@@:");
        add_method(v, "initWithFrame:", view_init_with_frame as Imp, "@@:{CGRect={CGPoint=dd}{CGSize=dd}}");
        add_method(v, "initWithCoder:", view_init_with_coder as Imp, "@@:@");
        add_method(v, "layer", view_get_layer as Imp, "@@:");
        add_method(v, "actionForLayer:forKey:", view_action_for_layer as Imp, "@@:@@");
        add_method(v, "frame", view_frame as Imp, "{CGRect={CGPoint=dd}{CGSize=dd}}@:");
        add_method(v, "setFrame:", view_set_frame as Imp, "v@:{CGRect={CGPoint=dd}{CGSize=dd}}");
        add_method(v, "bounds", view_bounds as Imp, "{CGRect={CGPoint=dd}{CGSize=dd}}@:");
        add_method(v, "setBounds:", view_set_bounds as Imp, "v@:{CGRect={CGPoint=dd}{CGSize=dd}}");
        add_method(v, "center", view_center as Imp, "{CGPoint=dd}@:");
        add_method(v, "setCenter:", view_set_center as Imp, "v@:{CGPoint=dd}");
        add_method(v, "superview", view_superview as Imp, "@@:");
        add_method(v, "subviews", view_subviews as Imp, "@@:");
        add_method(v, "window", view_window as Imp, "@@:");
        add_method(v, "nextResponder", view_next_responder as Imp, "@@:");
        add_method(v, "pear_setViewController:", view_set_vc as Imp, "v@:@");
        add_method(v, "pear_setWindow:", view_set_window as Imp, "v@:@");
        add_method(v, "insertSubview:atIndex:", view_insert_subview as Imp, "v@:@q");
        add_method(v, "addSubview:", view_add_subview as Imp, "v@:@");
        add_method(v, "bringSubviewToFront:", view_bring_to_front as Imp, "v@:@");
        add_method(v, "sendSubviewToBack:", view_send_to_back as Imp, "v@:@");
        add_method(v, "removeFromSuperview", view_remove_from_superview as Imp, "v@:");
        add_method(v, "interactions", view_interactions as Imp, "@@:");
        add_method(v, "addInteraction:", view_add_interaction as Imp, "v@:@");
        add_method(v, "removeInteraction:", view_remove_interaction as Imp, "v@:@");
        for s in ["didAddSubview:", "willRemoveSubview:", "willMoveToSuperview:", "willMoveToWindow:", "removeGestureRecognizer:"] {
            add_method(v, s, view_noop_id as Imp, "v@:@");
        }
        for s in ["didMoveToSuperview", "didMoveToWindow", "layoutSubviews"] {
            add_method(v, s, view_noop as Imp, "v@:");
        }
        add_method(v, "isDescendantOfView:", view_is_descendant as Imp, "B@:@");
        add_method(v, "viewWithTag:", view_with_tag as Imp, "@@:q");
        add_method(v, "tag", view_tag as Imp, "q@:");
        add_method(v, "setTag:", view_set_tag as Imp, "v@:q");
        add_method(v, "setNeedsLayout", view_set_needs_layout as Imp, "v@:");
        add_method(v, "layoutIfNeeded", view_layout_if_needed as Imp, "v@:");
        add_method(v, "setNeedsDisplay", view_set_needs_display as Imp, "v@:");
        add_method(v, "setNeedsDisplayInRect:", view_set_needs_display_rect as Imp, "v@:{CGRect={CGPoint=dd}{CGSize=dd}}");
        add_method(v, "drawRect:", view_noop_rect as Imp, "v@:{CGRect={CGPoint=dd}{CGSize=dd}}");
        add_method(v, "sizeThatFits:", view_size_that_fits as Imp, "{CGSize=dd}@:{CGSize=dd}");
        add_method(v, "sizeToFit", view_size_to_fit as Imp, "v@:");
        add_method(v, "contentScaleFactor", view_content_scale as Imp, "d@:");
        add_method(v, "setContentScaleFactor:", view_set_content_scale as Imp, "v@:d");
        add_method(v, "isMultipleTouchEnabled", view_is_multi_touch as Imp, "B@:");
        add_method(v, "setMultipleTouchEnabled:", view_set_multi_touch as Imp, "v@:B");
        add_method(v, "isExclusiveTouch", view_false as Imp, "B@:");
        add_method(v, "setExclusiveTouch:", view_noop_bool as Imp, "v@:B");
        add_method(v, "isUserInteractionEnabled", view_user_interaction as Imp, "B@:");
        add_method(v, "setUserInteractionEnabled:", view_set_user_interaction as Imp, "v@:B");
        add_method(v, "isHidden", view_is_hidden as Imp, "B@:");
        add_method(v, "setHidden:", view_set_hidden as Imp, "v@:B");
        add_method(v, "isOpaque", view_is_opaque as Imp, "B@:");
        add_method(v, "setOpaque:", view_set_opaque as Imp, "v@:B");
        add_method(v, "alpha", view_alpha as Imp, "d@:");
        add_method(v, "setAlpha:", view_set_alpha as Imp, "v@:d");
        add_method(v, "autoresizingMask", view_autoresizing as Imp, "Q@:");
        add_method(v, "setAutoresizingMask:", view_set_autoresizing as Imp, "v@:Q");
        add_method(v, "autoresizesSubviews", view_true as Imp, "B@:");
        add_method(v, "setAutoresizesSubviews:", view_noop_bool as Imp, "v@:B");
        add_method(v, "clearsContextBeforeDrawing", view_clears as Imp, "B@:");
        add_method(v, "setClearsContextBeforeDrawing:", view_set_clears as Imp, "v@:B");
        add_method(v, "clipsToBounds", view_clips as Imp, "B@:");
        add_method(v, "setClipsToBounds:", view_set_clips as Imp, "v@:B");
        add_method(v, "backgroundColor", view_bg as Imp, "@@:");
        add_method(v, "setBackgroundColor:", view_set_bg as Imp, "v@:@");
        add_method(v, "contentMode", view_zero_int as Imp, "q@:");
        add_method(v, "setContentMode:", view_noop_int as Imp, "v@:q");
        add_method(v, "safeAreaInsets", view_zero_insets as Imp, "{UIEdgeInsets=dddd}@:");
        add_method(v, "layoutMargins", view_layout_margins as Imp, "{UIEdgeInsets=dddd}@:");
        add_method(v, "traitCollection", view_trait_collection as Imp, "@@:");
        add_method(v, "gestureRecognizers", view_empty_arr as Imp, "@@:");
        add_method(v, "addGestureRecognizer:", view_add_gesture as Imp, "v@:@");
        add_method(v, "translatesAutoresizingMaskIntoConstraints", view_true as Imp, "B@:");
        add_method(v, "setTranslatesAutoresizingMaskIntoConstraints:", view_noop_bool as Imp, "v@:B");
        add_method(v, "pointInside:withEvent:", view_point_inside as Imp, "B@:{CGPoint=dd}@");
        add_method(v, "hitTest:withEvent:", view_hit_test as Imp, "@@:{CGPoint=dd}@");
        add_method(v, "convertPoint:toView:", view_conv_pt_to as Imp, "{CGPoint=dd}@:{CGPoint=dd}@");
        add_method(v, "convertPoint:fromView:", view_conv_pt_from as Imp, "{CGPoint=dd}@:{CGPoint=dd}@");
        add_method(v, "convertRect:toView:", view_conv_rect_to as Imp, "{CGRect={CGPoint=dd}{CGSize=dd}}@:{CGRect={CGPoint=dd}{CGSize=dd}}@");
        add_method(v, "convertRect:fromView:", view_conv_rect_from as Imp, "{CGRect={CGPoint=dd}{CGSize=dd}}@:{CGRect={CGPoint=dd}{CGSize=dd}}@");
        add_method(v, "isAccessibilityElement", view_false as Imp, "B@:");
        add_method(v, "setIsAccessibilityElement:", view_noop_bool as Imp, "v@:B");
        add_method(v, "accessibilityLabel", view_nil as Imp, "@@:");
        add_method(v, "setAccessibilityLabel:", view_noop_id as Imp, "v@:@");
        add_method(v, "accessibilityTraits", view_zero_u64 as Imp, "Q@:");
        add_method(v, "setAccessibilityTraits:", view_noop_u64 as Imp, "v@:Q");
        add_method(v, "accessibilityElements", view_nil as Imp, "@@:");
        add_method(v, "setAccessibilityElements:", view_noop_id as Imp, "v@:@");
        add_method(v, "endEditing:", view_true as Imp, "B@:B");
    }

    let sv = cls("UIScrollView");
    if !sv.is_null() {
        add_method(sv, "contentOffset", sv_offset as Imp, "{CGPoint=dd}@:");
        add_method(sv, "setContentOffset:", sv_set_offset as Imp, "v@:{CGPoint=dd}");
        add_method(sv, "setContentOffset:animated:", sv_set_offset_anim as Imp, "v@:{CGPoint=dd}B");
        add_method(sv, "contentSize", sv_content_size as Imp, "{CGSize=dd}@:");
        add_method(sv, "setContentSize:", sv_set_content_size as Imp, "v@:{CGSize=dd}");
        add_method(sv, "contentInset", sv_inset as Imp, "{UIEdgeInsets=dddd}@:");
        add_method(sv, "setContentInset:", sv_set_inset as Imp, "v@:{UIEdgeInsets=dddd}");
        add_method(sv, "isScrollEnabled", sv_scroll_enabled as Imp, "B@:");
        add_method(sv, "setScrollEnabled:", sv_set_scroll_enabled as Imp, "v@:B");
        add_method(sv, "delegate", sv_delegate as Imp, "@@:");
        add_method(sv, "setDelegate:", sv_set_delegate as Imp, "v@:@");
        add_method(sv, "showsVerticalScrollIndicator", view_false as Imp, "B@:");
        add_method(sv, "setShowsVerticalScrollIndicator:", view_noop_bool as Imp, "v@:B");
        add_method(sv, "showsHorizontalScrollIndicator", view_false as Imp, "B@:");
        add_method(sv, "setShowsHorizontalScrollIndicator:", view_noop_bool as Imp, "v@:B");
        add_method(sv, "bounces", view_false as Imp, "B@:");
        add_method(sv, "setBounces:", view_noop_bool as Imp, "v@:B");
    }

    let vc = cls("UIViewController");
    if !vc.is_null() {
        add_method(vc, "init", vc_init as Imp, "@@:");
        add_method(vc, "initWithNibName:bundle:", vc_init_nib as Imp, "@@:@@");
        add_method(vc, "initWithCoder:", vc_init_coder as Imp, "@@:@");
        add_method(vc, "nibName", vc_nib_name as Imp, "@@:");
        add_method(vc, "nibBundle", vc_nib_bundle as Imp, "@@:");
        add_method(vc, "loadView", vc_load_view as Imp, "v@:");
        add_method(vc, "view", vc_view as Imp, "@@:");
        add_method(vc, "setView:", vc_set_view as Imp, "v@:@");
        add_method(vc, "viewIfLoaded", vc_view_if_loaded as Imp, "@@:");
        add_method(vc, "isViewLoaded", vc_is_view_loaded as Imp, "B@:");
        add_method(vc, "loadViewIfNeeded", vc_load_view_if_needed as Imp, "v@:");
        add_method(vc, "nextResponder", vc_next_responder as Imp, "@@:");
        for s in ["viewDidLoad", "viewWillLayoutSubviews", "viewDidLayoutSubviews", "viewDidUnload", "didReceiveMemoryWarning",
                  "setNeedsUpdateOfPrefersPointerLocked", "setNeedsUpdateOfHomeIndicatorAutoHidden",
                  "setNeedsUpdateOfScreenEdgesDeferringSystemGestures", "setNeedsStatusBarAppearanceUpdate"] {
            add_method(vc, s, view_noop as Imp, "v@:");
        }
        for s in ["viewWillAppear:", "viewDidAppear:", "viewWillDisappear:", "viewDidDisappear:"] {
            add_method(vc, s, view_noop_bool as Imp, "v@:B");
        }
        add_method(vc, "viewWillTransitionToSize:withTransitionCoordinator:", view_noop as Imp, "v@:{CGSize=dd}@");
        add_method(vc, "traitCollectionDidChange:", view_noop_id as Imp, "v@:@");
        add_method(vc, "title", vc_title as Imp, "@@:");
        add_method(vc, "setTitle:", vc_set_title as Imp, "v@:@");
        add_method(vc, "modalPresentationStyle", vc_modal_style as Imp, "q@:");
        add_method(vc, "setModalPresentationStyle:", vc_set_modal_style as Imp, "v@:q");
        add_method(vc, "edgesForExtendedLayout", vc_edges as Imp, "Q@:");
        add_method(vc, "setEdgesForExtendedLayout:", vc_set_edges as Imp, "v@:Q");
        add_method(vc, "traitCollection", view_trait_collection as Imp, "@@:");
        add_method(vc, "parentViewController", view_nil as Imp, "@@:");
        add_method(vc, "childViewControllers", view_empty_arr as Imp, "@@:");
        add_method(vc, "navigationController", view_nil as Imp, "@@:");
        add_method(vc, "tabBarController", view_nil as Imp, "@@:");
        add_method(vc, "prefersStatusBarHidden", view_true as Imp, "B@:");
        add_method(vc, "prefersHomeIndicatorAutoHidden", view_true as Imp, "B@:");
        add_method(vc, "prefersPointerLocked", view_false as Imp, "B@:");
        add_method(vc, "shouldAutorotate", view_true as Imp, "B@:");
        add_method(vc, "supportedInterfaceOrientations", vc_orientations as Imp, "Q@:");
        add_method(vc, "preferredInterfaceOrientationForPresentation", vc_pref_orientation as Imp, "q@:");
        add_method(vc, "preferredScreenEdgesDeferringSystemGestures", vc_pref_edges as Imp, "Q@:");
        add_method(vc, "presentedViewController", vc_presented as Imp, "@@:");
        add_method(vc, "presentingViewController", vc_presenting as Imp, "@@:");
        add_method(vc, "pear_setPresenting:", vc_set_presenting as Imp, "v@:@");
        add_method(vc, "presentViewController:animated:completion:", vc_present as Imp, "v@:@B@?");
        add_method(vc, "dismissViewControllerAnimated:completion:", vc_dismiss as Imp, "v@:B@?");
    }

    let w = cls("UIWindow");
    if !w.is_null() {
        add_method(w, "initWithFrame:", win_init_frame as Imp, "@@:{CGRect={CGPoint=dd}{CGSize=dd}}");
        add_method(w, "initWithCoder:", win_init_coder as Imp, "@@:@");
        add_method(w, "initWithWindowScene:", win_init_scene as Imp, "@@:@");
        add_method(w, "window", win_window as Imp, "@@:");
        add_method(w, "nextResponder", win_next_responder as Imp, "@@:");
        add_method(w, "screen", win_screen as Imp, "@@:");
        add_method(w, "setScreen:", win_set_screen as Imp, "v@:@");
        add_method(w, "windowLevel", win_level as Imp, "d@:");
        add_method(w, "setWindowLevel:", win_set_level as Imp, "v@:d");
        add_method(w, "windowScene", view_nil as Imp, "@@:");
        add_method(w, "setWindowScene:", view_noop_id as Imp, "v@:@");
        add_method(w, "isKeyWindow", win_is_key as Imp, "B@:");
        add_method(w, "rootViewController", win_root as Imp, "@@:");
        add_method(w, "setRootViewController:", win_set_root as Imp, "v@:@");
        add_method(w, "makeKeyWindow", win_make_key as Imp, "v@:");
        add_method(w, "makeKeyAndVisible", win_make_key_visible as Imp, "v@:");
        add_method(w, "setHidden:", win_set_hidden as Imp, "v@:B");
        add_method(w, "becomeKeyWindow", view_noop as Imp, "v@:");
        add_method(w, "resignKeyWindow", view_noop as Imp, "v@:");
        add_method(w, "sendEvent:", view_noop_id as Imp, "v@:@");
    }

    let bi = cls("UIBarItem");
    if !bi.is_null() {
        add_method(bi, "title", bi_title as Imp, "@@:");
        add_method(bi, "setTitle:", bi_set_title as Imp, "v@:@");
        add_method(bi, "image", bi_image as Imp, "@@:");
        add_method(bi, "setImage:", bi_set_image as Imp, "v@:@");
        add_method(bi, "tag", bi_tag as Imp, "q@:");
        add_method(bi, "setTag:", bi_set_tag as Imp, "v@:q");
        add_method(bi, "isEnabled", bi_enabled as Imp, "B@:");
        add_method(bi, "setEnabled:", bi_set_enabled as Imp, "v@:B");
    }

    let bbi = cls("UIBarButtonItem");
    if !bbi.is_null() {
        add_method(bbi, "initWithTitle:style:target:action:", bbi_init_title as Imp, "@@:@q@:");
        add_method(bbi, "initWithImage:style:target:action:", bbi_init_image as Imp, "@@:@q@:");
        add_method(bbi, "initWithBarButtonSystemItem:target:action:", bbi_init_sys as Imp, "@@:q@:");
        add_method(bbi, "initWithCustomView:", bbi_init_custom as Imp, "@@:@");
        add_method(bbi, "style", bbi_style as Imp, "q@:");
        add_method(bbi, "setStyle:", bbi_set_style as Imp, "v@:q");
        add_method(bbi, "target", bbi_target as Imp, "@@:");
        add_method(bbi, "setTarget:", bbi_set_target as Imp, "v@:@");
        add_method(bbi, "action", bbi_action as Imp, ":@:");
        add_method(bbi, "setAction:", bbi_set_action as Imp, "v@::");
        add_method(bbi, "customView", bbi_custom as Imp, "@@:");
        add_method(bbi, "setCustomView:", bbi_set_custom as Imp, "v@:@");
        add_method(bbi, "width", bbi_width as Imp, "d@:");
        add_method(bbi, "setWidth:", bbi_set_width as Imp, "v@:d");
    }

    let nav = cls("UINavigationController");
    if !nav.is_null() {
        add_method(nav, "initWithRootViewController:", nav_init_root as Imp, "@@:@");
        add_method(nav, "topViewController", nav_top as Imp, "@@:");
        add_method(nav, "visibleViewController", nav_visible as Imp, "@@:");
        add_method(nav, "navigationBar", view_nil as Imp, "@@:");
        add_method(nav, "isNavigationBarHidden", nav_hidden as Imp, "B@:");
        add_method(nav, "setNavigationBarHidden:", nav_set_hidden as Imp, "v@:B");
        add_method(nav, "setNavigationBarHidden:animated:", nav_set_hidden_anim as Imp, "v@:BB");
        add_method(nav, "viewControllers", nav_vcs as Imp, "@@:");
        add_method(nav, "setViewControllers:", nav_set_vcs as Imp, "v@:@");
        add_method(nav, "setViewControllers:animated:", nav_set_vcs_anim as Imp, "v@:@B");
        add_method(nav, "pushViewController:animated:", nav_push as Imp, "v@:@B");
        add_method(nav, "popViewControllerAnimated:", nav_pop as Imp, "@@:B");
        add_method(nav, "popToRootViewControllerAnimated:", nav_pop_root as Imp, "@@:B");
        add_method(nav, "viewDidLoad", nav_view_did_load as Imp, "v@:");
        add_method(nav, "delegate", nav_delegate as Imp, "@@:");
        add_method(nav, "setDelegate:", nav_set_delegate as Imp, "v@:@");
    }

    let tab = cls("UITabBarController");
    if !tab.is_null() {
        add_method(tab, "viewControllers", tab_vcs as Imp, "@@:");
        add_method(tab, "setViewControllers:", tab_set_vcs as Imp, "v@:@");
        add_method(tab, "setViewControllers:animated:", tab_set_vcs_anim as Imp, "v@:@B");
        add_method(tab, "selectedIndex", tab_idx as Imp, "q@:");
        add_method(tab, "setSelectedIndex:", tab_set_idx as Imp, "v@:q");
        add_method(tab, "selectedViewController", tab_selected_vc as Imp, "@@:");
        add_method(tab, "setSelectedViewController:", tab_set_selected_vc as Imp, "v@:@");
        add_method(tab, "tabBar", view_nil as Imp, "@@:");
        add_method(tab, "viewDidLoad", tab_view_did_load as Imp, "v@:");
        add_method(tab, "delegate", tab_delegate as Imp, "@@:");
        add_method(tab, "setDelegate:", tab_set_delegate as Imp, "v@:@");
    }

    let aa = cls("UIAlertAction");
    if !aa.is_null() {
        add_class_method(aa, "actionWithTitle:style:handler:", aa_action as Imp, "@@:@q@?");
        add_method(aa, "title", aa_title as Imp, "@@:");
        add_method(aa, "style", aa_style as Imp, "q@:");
        add_method(aa, "isEnabled", aa_enabled as Imp, "B@:");
        add_method(aa, "setEnabled:", aa_set_enabled as Imp, "v@:B");
        add_method(aa, "pear_fire", aa_fire as Imp, "v@:");
        add_method(aa, "copyWithZone:", copy_self as Imp, "@@:^{_NSZone=}");
    }

    let ac = cls("UIAlertController");
    if !ac.is_null() {
        add_class_method(ac, "alertControllerWithTitle:message:preferredStyle:", ac_create as Imp, "@@:@@q");
        add_method(ac, "message", ac_msg as Imp, "@@:");
        add_method(ac, "setMessage:", ac_set_msg as Imp, "v@:@");
        add_method(ac, "preferredStyle", ac_style as Imp, "q@:");
        add_method(ac, "actions", ac_actions as Imp, "@@:");
        add_method(ac, "preferredAction", ac_pref as Imp, "@@:");
        add_method(ac, "setPreferredAction:", ac_set_pref as Imp, "v@:@");
        add_method(ac, "addAction:", ac_add_action as Imp, "v@:@");
        add_method(ac, "textFields", view_empty_arr as Imp, "@@:");
        add_method(ac, "addTextFieldWithConfigurationHandler:", ac_add_tf as Imp, "v@:@?");
        add_method(ac, "popoverPresentationController", view_nil as Imp, "@@:");
        add_method(ac, "pear_presentWithCompletion:", ac_present as Imp, "v@:@?");
    }

    let act = cls("UIActivityViewController");
    if !act.is_null() {
        add_method(act, "initWithActivityItems:applicationActivities:", act_init as Imp, "@@:@@");
        add_method(act, "initWithCoder:", act_init_coder as Imp, "@@:@");
        add_method(act, "viewDidAppear:", act_did_appear as Imp, "v@:B");
        add_method(act, "excludedActivityTypes", act_excl as Imp, "@@:");
        add_method(act, "setExcludedActivityTypes:", act_set_excl as Imp, "v@:@");
        add_method(act, "completionWithItemsHandler", act_comp as Imp, "@?@:");
        add_method(act, "setCompletionWithItemsHandler:", act_set_comp as Imp, "v@:@?");
    }

    let dp = cls("UIDocumentPickerViewController");
    if !dp.is_null() {
        add_method(dp, "initForOpeningContentTypes:asCopy:", dp_init_open as Imp, "@@:@B");
        add_method(dp, "initWithDocumentTypes:inMode:", dp_init_utis as Imp, "@@:@q");
        add_method(dp, "initForExportingURLs:asCopy:", dp_init_export as Imp, "@@:@B");
        add_method(dp, "viewDidAppear:", dp_did_appear as Imp, "v@:B");
        add_method(dp, "delegate", dp_del as Imp, "@@:");
        add_method(dp, "setDelegate:", dp_set_del as Imp, "v@:@");
        add_method(dp, "allowsMultipleSelection", dp_multi as Imp, "B@:");
        add_method(dp, "setAllowsMultipleSelection:", dp_set_multi as Imp, "v@:B");
        add_method(dp, "shouldShowFileExtensions", dp_ext as Imp, "B@:");
        add_method(dp, "setShouldShowFileExtensions:", dp_set_ext as Imp, "v@:B");
        add_method(dp, "directoryURL", dp_dir as Imp, "@@:");
        add_method(dp, "setDirectoryURL:", dp_set_dir as Imp, "v@:@");
    }

    let ip = cls("UIImagePickerController");
    if !ip.is_null() {
        add_class_method(ip, "isSourceTypeAvailable:", ip_source_avail as Imp, "B@:q");
        add_class_method(ip, "availableMediaTypesForSourceType:", ip_media_types as Imp, "@@:q");
        add_method(ip, "sourceType", ip_source as Imp, "q@:");
        add_method(ip, "setSourceType:", ip_set_source as Imp, "v@:q");
        add_method(ip, "mediaTypes", ip_media as Imp, "@@:");
        add_method(ip, "setMediaTypes:", ip_set_media as Imp, "v@:@");
        add_method(ip, "allowsEditing", ip_editing as Imp, "B@:");
        add_method(ip, "setAllowsEditing:", ip_set_editing as Imp, "v@:B");
        add_method(ip, "showsCameraControls", ip_camera_controls as Imp, "B@:");
        add_method(ip, "setShowsCameraControls:", ip_set_camera_controls as Imp, "v@:B");
    }

    let doc = cls("UIDocument");
    if !doc.is_null() {
        add_method(doc, "initWithFileURL:", doc_init as Imp, "@@:@");
        add_method(doc, "fileURL", doc_url as Imp, "@@:");
        add_method(doc, "setFileURL:", doc_set_url as Imp, "v@:@");
        add_method(doc, "localizedName", doc_name as Imp, "@@:");
        add_method(doc, "documentState", view_zero_u64 as Imp, "Q@:");
        add_method(doc, "contentsForType:error:", doc_contents as Imp, "@@:@^@");
        add_method(doc, "loadFromContents:ofType:error:", doc_load as Imp, "B@:@@^@");
        add_method(doc, "openWithCompletionHandler:", doc_open as Imp, "v@:@?");
        add_method(doc, "closeWithCompletionHandler:", doc_close as Imp, "v@:@?");
        add_method(doc, "saveToURL:forSaveOperation:completionHandler:", doc_save as Imp, "v@:@q@?");
    }

    let pop = cls("UIPopoverController");
    if !pop.is_null() {
        add_method(pop, "initWithContentViewController:", pop_init as Imp, "@@:@");
        add_method(pop, "contentViewController", pop_vc as Imp, "@@:");
        add_method(pop, "setContentViewController:", pop_set_vc as Imp, "v@:@");
        add_method(pop, "delegate", pop_del as Imp, "@@:");
        add_method(pop, "setDelegate:", pop_set_del as Imp, "v@:@");
        add_method(pop, "popoverContentSize", pop_size as Imp, "{CGSize=dd}@:");
        add_method(pop, "setPopoverContentSize:", pop_set_size as Imp, "v@:{CGSize=dd}");
        add_method(pop, "isPopoverVisible", pop_vis as Imp, "B@:");
        add_method(pop, "presentPopoverFromRect:inView:permittedArrowDirections:animated:", pop_present as Imp, "v@:{CGRect={CGPoint=dd}{CGSize=dd}}@QB");
        add_method(pop, "dismissPopoverAnimated:", pop_dismiss as Imp, "v@:B");
    }
}
