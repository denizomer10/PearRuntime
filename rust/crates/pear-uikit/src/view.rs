//! UIView, UIScrollView, UIViewController, UIWindow, UINavigationController,
//! UITabBarController, UIBarItem, UIBarButtonItem, UIAlertAction, UIAlertController,
//! UIActivityViewController, UIDocumentPickerViewController, UIImagePickerController,
//! UIDocument, UIPopoverController, and PearUIKitWindowDidResize.

use crate::graphics::CGAffineTransform;

mod pointer_gestures;
pub(crate) use pointer_gestures::deliver_pointer_gestures;
use pear_objc::{
    class_respondsToSelector, cls, is_kind_of, msg0, msg1, msg2, msg3, nsarray_count,
    nsarray_from_ids, nsarray_object_at, nsdata_from_bytes, nsdata_to_vec, nsnumber_bool,
    nsnumber_f64, nsnumber_i64, nsstring, nsstring_to_string, objc_autorelease,
    objc_getAssociatedObject, objc_msgSend, objc_msgSendSuper, objc_release, objc_retain,
    objc_setAssociatedObject, replace_method as add_method, responds_to, sel, sel_getName,
    CGPoint, CGRect, CGSize, Class, Id, Imp, NSRange, Sel, UIEdgeInsets, NIL, OBJC_ASSOCIATION_RETAIN,
};
use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

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
    fn dispatch_after_f(when: u64, queue: *const c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
    fn dispatch_time(when: u64, delta: i64) -> u64;
    fn _Block_copy(aBlock: *const c_void) -> *mut c_void;
    fn _Block_release(aBlock: *const c_void);
    fn object_getClass(obj: Id) -> Class;
    fn class_getSuperclass(cls: Class) -> Class;
    fn class_getName(cls: Class) -> *const c_char;
    fn class_getInstanceMethod(cls: Class, name: Sel) -> *mut c_void;
    fn class_getMethodImplementation(cls: Class, name: Sel) -> Imp;
    fn method_getImplementation(method: *mut c_void) -> Imp;
    fn PearMissingReport(kind: c_int, class_name: *const c_char, selector: *const c_char, caller: *const c_void);
    fn PearHostLogMessage(tag: *const c_char, message: *const c_char);
    fn PearUIKitRegisterWindow(window: Id);
    fn PearUIKitSetKeyWindow(window: Id);
    fn PearUIKitKeyWindow() -> Id;
    fn PearUIKitFirstResponder() -> Id;
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
static VIEW_NATIVE_SUBVIEWS_KEY: u8 = 0;
static HAS_NATIVE_SUBVIEWS: AtomicBool = AtomicBool::new(false);
static VIEW_WINDOW_KEY: u8 = 0;
static VIEW_VC_KEY: u8 = 0;
static VIEW_BG_KEY: u8 = 0;
static VIEW_TINT_KEY: u8 = 0;
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
    translates_autoresizing: bool,
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
            translates_autoresizing: true,
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

unsafe fn view_sync_native_subviews(view: Id) {
    let native = assoc_get(view, &VIEW_NATIVE_SUBVIEWS_KEY);
    if native.is_null() { return; }
    let window = pear_objc::assoc_get_weak(view, &VIEW_WINDOW_KEY);
    let frame: CGRect = msg2(view, "convertRect:toView:", msg0::<CGRect>(view, "bounds"), window);
    let mut ancestor = view;
    let mut hidden = false;
    while !ancestor.is_null() {
        hidden |= msg0::<bool>(ancestor, "isHidden");
        ancestor = pear_objc::assoc_get_weak(ancestor, &VIEW_SUPERVIEW_KEY);
    }
    for i in 0..nsarray_count(native) {
        let child = nsarray_object_at(native, i);
        msg3::<Id, CGRect, bool, ()>(child, "pear_mountInHostWindow:frame:hidden:", win_native_ptr(window) as Id, frame, hidden);
    }
}

unsafe fn view_sync_native_tree(view: Id) {
    if !HAS_NATIVE_SUBVIEWS.load(Ordering::Relaxed) { return; }
    view_sync_native_subviews(view);
    let subs = assoc_get(view, &VIEW_SUBVIEWS_KEY);
    for i in 0..nsarray_count(subs) {
        view_sync_native_tree(nsarray_object_at(subs, i));
    }
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
    if old_b.size != msg0::<CGRect>(l, "bounds").size {
        if layout_controls_imp::is_solving_layout() {
            msg0::<()>(self_obj, "setNeedsLayout");
        } else {
            view_bounds_size_changed(self_obj, old_b.size);
        }
    }
    view_sync_native_tree(self_obj);
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
    view_sync_native_tree(self_obj);
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
    view_sync_native_tree(self_obj);
}

unsafe extern "C" fn view_transform(self_obj: Id, _cmd: Sel) -> CGAffineTransform {
    msg0(view_layer(self_obj), "affineTransform")
}

unsafe extern "C" fn view_set_transform(self_obj: Id, _cmd: Sel, transform: CGAffineTransform) {
    let ca = cls("CATransaction");
    msg0::<()>(ca, "begin");
    msg1::<bool, ()>(ca, "setDisableActions:", true);
    msg1::<CGAffineTransform, ()>(view_layer(self_obj), "setAffineTransform:", transform);
    msg0::<()>(ca, "commit");
    view_sync_native_tree(self_obj);
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
    view_sync_native_subviews(self_obj);
    msg0::<()>(self_obj, "didMoveToWindow");
}

unsafe extern "C" fn view_insert_subview(self_obj: Id, _cmd: Sel, view: Id, index: isize) {
    if view.is_null() || view == self_obj {
        return;
    }
    if responds_to(view, "pear_mountInHostWindow:frame:hidden:") {
        let native = mut_array_for_key(self_obj, &VIEW_NATIVE_SUBVIEWS_KEY);
        if !msg1::<Id, bool>(native, "containsObject:", view) {
            msg1::<Id, ()>(native, "addObject:", view);
        }
        HAS_NATIVE_SUBVIEWS.store(true, Ordering::Relaxed);
        view_sync_native_subviews(self_obj);
        return;
    }
    objc_retain(view);
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
    objc_release(view);
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
    objc_retain(view);
    let subs = mut_array_for_key(self_obj, &VIEW_SUBVIEWS_KEY);
    msg1::<Id, ()>(subs, "removeObjectIdenticalTo:", view);
    msg1::<Id, ()>(subs, "addObject:", view);
    let sub_l = view_layer(view);
    let l = view_layer(self_obj);
    if !sub_l.is_null() && !l.is_null() {
        msg0::<()>(sub_l, "removeFromSuperlayer");
        msg1::<Id, ()>(l, "addSublayer:", sub_l);
    }
    objc_release(view);
}

unsafe extern "C" fn view_send_to_back(self_obj: Id, _cmd: Sel, view: Id) {
    if view.is_null() || pear_objc::assoc_get_weak(view, &VIEW_SUPERVIEW_KEY) != self_obj {
        return;
    }
    objc_retain(view);
    let subs = mut_array_for_key(self_obj, &VIEW_SUBVIEWS_KEY);
    msg1::<Id, ()>(subs, "removeObjectIdenticalTo:", view);
    msg2::<Id, usize, ()>(subs, "insertObject:atIndex:", view, 0);
    let sub_l = view_layer(view);
    let l = view_layer(self_obj);
    if !sub_l.is_null() && !l.is_null() {
        msg0::<()>(sub_l, "removeFromSuperlayer");
        msg2::<Id, u32, ()>(l, "insertSublayer:atIndex:", sub_l, 0);
    }
    objc_release(view);
}

unsafe extern "C" fn view_remove_from_superview(self_obj: Id, _cmd: Sel) {
    let parent = pear_objc::assoc_get_weak(self_obj, &VIEW_SUPERVIEW_KEY);
    if parent.is_null() {
        return;
    }
    objc_retain(self_obj);
    objc_retain(parent);
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
    objc_release(parent);
    objc_release(self_obj);
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
    if !layout_controls_imp::is_solving_layout() {
        layout_controls_imp::solve_autolayout_subtree(self_obj);
    }
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
    view_sync_native_tree(self_obj);
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
unsafe extern "C" fn view_tint_color(self_obj: Id, _cmd: Sel) -> Id {
    let mut cur = self_obj;
    while !cur.is_null() {
        let c = assoc_get(cur, &VIEW_TINT_KEY);
        if !c.is_null() {
            return c;
        }
        cur = pear_objc::assoc_get_weak(cur, &VIEW_SUPERVIEW_KEY);
    }
    let f: unsafe extern "C" fn(Id, Sel, f64, f64, f64, f64) -> Id =
        std::mem::transmute(objc_msgSend as *const c_void);
    f(cls("UIColor") as Id, sel("colorWithRed:green:blue:alpha:"), 0.0, 0.478, 1.0, 1.0)
}
unsafe extern "C" fn view_set_tint_color(self_obj: Id, _cmd: Sel, color: Id) {
    assoc_set(self_obj, &VIEW_TINT_KEY, color);
    msg0::<()>(self_obj, "tintColorDidChange");
}
unsafe extern "C" fn view_translates_autoresizing(self_obj: Id, _cmd: Sel) -> bool {
    get_view_flags(self_obj).translates_autoresizing
}
unsafe extern "C" fn view_set_translates_autoresizing(self_obj: Id, _cmd: Sel, b: bool) {
    let mut f = get_view_flags(self_obj);
    f.translates_autoresizing = b;
    set_view_flags(self_obj, f);
}
unsafe extern "C" fn view_snapshot(self_obj: Id, _cmd: Sel, _after_updates: bool) -> Id {
    let b: CGRect = msg0(self_obj, "bounds");
    let snap: Id = msg1(msg0(cls("UIView"), "alloc"), "initWithFrame:", b);
    if snap.is_null() {
        return NIL;
    }
    let bg = msg0::<Id>(self_obj, "backgroundColor");
    if !bg.is_null() {
        msg1::<Id, ()>(snap, "setBackgroundColor:", bg);
    }
    msg1::<f64, ()>(snap, "setAlpha:", msg0::<f64>(self_obj, "alpha"));
    let src_l = view_layer(self_obj);
    let dst_l = view_layer(snap);
    if !src_l.is_null() && !dst_l.is_null() {
        let contents: Id = msg0(src_l, "contents");
        if !contents.is_null() {
            msg1::<Id, ()>(dst_l, "setContents:", contents);
        }
    }
    objc_autorelease(snap)
}
unsafe extern "C" fn view_end_editing(self_obj: Id, _cmd: Sel, force: bool) -> bool {
    let fr = PearUIKitFirstResponder();
    if !fr.is_null() && is_kind_of(fr, "UIView") && msg1::<Id, bool>(fr, "isDescendantOfView:", self_obj) {
        let resigned: bool = msg0(fr, "resignFirstResponder");
        return resigned || force;
    }
    true
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
static VIEW_GESTURES_KEY: u8 = 0;
static GR_STATE_KEY: u8 = 0;
static GR_VIEW_KEY: u8 = 0;
static GR_DELEGATE_KEY: u8 = 0;
static GR_TARGETS_KEY: u8 = 0;
static GR_ACTION_TARGET_KEY: u8 = 0;
static GR_ACTION_SELECTOR_KEY: u8 = 0;
static GR_NAME_KEY: u8 = 0;
static GR_TOUCH_TYPES_KEY: u8 = 0;
static GR_PRESS_TYPES_KEY: u8 = 0;

#[repr(C)]
#[derive(Copy, Clone)]
struct GestureState {
    state: isize,
    enabled: bool,
    cancels_touches: bool,
    delays_began: bool,
    delays_ended: bool,
    requires_exclusive_touch_type: bool,
    reserved: [u8; 3],
    min_press_duration: f64,
    allowable_movement: f64,
    touches_required: usize,
    taps_required: isize,
    location: CGPoint,
}

impl Default for GestureState {
    fn default() -> Self {
        Self {
            state: 0,
            enabled: true,
            cancels_touches: true,
            delays_began: false,
            delays_ended: false,
            requires_exclusive_touch_type: true,
            reserved: [0; 3],
            min_press_duration: 0.5,
            allowable_movement: 10.0,
            touches_required: 1,
            taps_required: -1,
            location: CGPoint::default(),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
struct GestureTarget {
    target: usize,
    action: usize,
}

unsafe fn get_gr_state(g: Id) -> GestureState {
    let data = assoc_get(g, &GR_STATE_KEY);
    if let Some(bytes) = nsdata_to_vec(data) {
        if bytes.len() == std::mem::size_of::<GestureState>() {
            return std::ptr::read_unaligned(bytes.as_ptr() as *const GestureState);
        }
    }
    GestureState::default()
}

unsafe fn set_gr_state(g: Id, st: GestureState) {
    let bytes = std::slice::from_raw_parts(
        &st as *const GestureState as *const u8,
        std::mem::size_of::<GestureState>(),
    );
    assoc_set(g, &GR_STATE_KEY, nsdata_from_bytes(bytes));
}

unsafe fn get_gr_targets(g: Id) -> Vec<GestureTarget> {
    let entries = assoc_get(g, &GR_TARGETS_KEY);
    (0..nsarray_count(entries)).filter_map(|index| {
        let entry = nsarray_object_at(entries, index);
        let target = pear_objc::assoc_get_weak(entry, &GR_ACTION_TARGET_KEY);
        let action = pear_objc::assoc_i64(entry, &GR_ACTION_SELECTOR_KEY, 0);
        (!target.is_null()).then_some(GestureTarget { target: target as usize, action: action as usize })
    }).collect()
}

unsafe fn set_gr_targets(g: Id, targets: &[GestureTarget]) {
    let entries: Id = msg0(cls("NSMutableArray"), "array");
    for target in targets {
        let entry: Id = msg0(cls("NSObject"), "new");
        pear_objc::assoc_set_weak(entry, &GR_ACTION_TARGET_KEY, target.target as Id);
        pear_objc::assoc_set_i64(entry, &GR_ACTION_SELECTOR_KEY, target.action as i64);
        msg1::<Id, ()>(entries, "addObject:", entry);
        objc_release(entry);
    }
    assoc_set(g, &GR_TARGETS_KEY, entries);
}

unsafe extern "C" fn view_empty_arr(_self: Id, _cmd: Sel) -> Id {
    msg0(cls("NSArray"), "array")
}
unsafe extern "C" fn view_gestures(self_obj: Id, _cmd: Sel) -> Id {
    let arr = mut_array_for_key(self_obj, &VIEW_GESTURES_KEY);
    msg1(cls("NSArray"), "arrayWithArray:", arr)
}
unsafe extern "C" fn view_add_gesture(self_obj: Id, _cmd: Sel, g: Id) {
    if g.is_null() {
        return;
    }
    let prev = pear_objc::assoc_get_weak(g, &GR_VIEW_KEY);
    if !prev.is_null() && prev != self_obj {
        msg1::<Id, ()>(prev, "removeGestureRecognizer:", g);
    }
    let arr = mut_array_for_key(self_obj, &VIEW_GESTURES_KEY);
    if !msg1::<Id, bool>(arr, "containsObject:", g) {
        msg1::<Id, ()>(arr, "addObject:", g);
    }
    pear_objc::assoc_set_weak(g, &GR_VIEW_KEY, self_obj);
}
unsafe extern "C" fn view_remove_gesture(self_obj: Id, _cmd: Sel, g: Id) {
    if g.is_null() {
        return;
    }
    objc_retain(g);
    let arr = mut_array_for_key(self_obj, &VIEW_GESTURES_KEY);
    msg1::<Id, ()>(arr, "removeObjectIdenticalTo:", g);
    if pear_objc::assoc_get_weak(g, &GR_VIEW_KEY) == self_obj {
        pear_objc::assoc_set_weak(g, &GR_VIEW_KEY, NIL);
    }
    objc_release(g);
}
unsafe extern "C" fn view_set_gestures(self_obj: Id, _cmd: Sel, new_arr: Id) {
    let cur = view_gestures(self_obj, sel("gestureRecognizers"));
    for i in 0..nsarray_count(cur) {
        let g = nsarray_object_at(cur, i);
        view_remove_gesture(self_obj, sel("removeGestureRecognizer:"), g);
    }
    if !new_arr.is_null() {
        for i in 0..nsarray_count(new_arr) {
            let g = nsarray_object_at(new_arr, i);
            view_add_gesture(self_obj, sel("addGestureRecognizer:"), g);
        }
    }
}

unsafe extern "C" fn gr_init(s: Id, _c: Sel) -> Id {
    let o: Id = super0(s, "UIGestureRecognizer", "init");
    if !o.is_null() {
        set_gr_state(o, GestureState::default());
    }
    o
}
unsafe extern "C" fn gr_init_target_action(s: Id, _c: Sel, target: Id, action: Sel) -> Id {
    let o: Id = super0(s, "UIGestureRecognizer", "init");
    if !o.is_null() {
        set_gr_state(o, GestureState::default());
        if !target.is_null() && !action.is_null() {
            set_gr_targets(
                o,
                &[GestureTarget {
                    target: target as usize,
                    action: action as usize,
                }],
            );
        }
    }
    o
}
unsafe extern "C" fn gr_init_coder(s: Id, _c: Sel, _coder: Id) -> Id {
    gr_init(s, sel("init"))
}
unsafe extern "C" fn gr_add_target_action(s: Id, _c: Sel, target: Id, action: Sel) {
    if target.is_null() || action.is_null() {
        return;
    }
    let pair = GestureTarget {
        target: target as usize,
        action: action as usize,
    };
    let mut pairs = get_gr_targets(s);
    if !pairs.contains(&pair) {
        pairs.push(pair);
        set_gr_targets(s, &pairs);
    }
}
unsafe extern "C" fn gr_remove_target_action(s: Id, _c: Sel, target: Id, action: Sel) {
    let t = target as usize;
    let a = action as usize;
    let mut pairs = get_gr_targets(s);
    if t == 0 && a == 0 {
        pairs.clear();
    } else {
        pairs.retain(|p| {
            let match_t = t == 0 || p.target == t;
            let match_a = a == 0 || p.action == a;
            !(match_t && match_a)
        });
    }
    set_gr_targets(s, &pairs);
}
unsafe extern "C" fn gr_view(s: Id, _c: Sel) -> Id {
    pear_objc::assoc_get_weak(s, &GR_VIEW_KEY)
}
unsafe extern "C" fn gr_delegate(s: Id, _c: Sel) -> Id {
    pear_objc::assoc_get_weak(s, &GR_DELEGATE_KEY)
}
unsafe extern "C" fn gr_set_delegate(s: Id, _c: Sel, d: Id) {
    pear_objc::assoc_set_weak(s, &GR_DELEGATE_KEY, d);
}
unsafe extern "C" fn gr_state(s: Id, _c: Sel) -> isize {
    get_gr_state(s).state
}
unsafe extern "C" fn gr_set_state(s: Id, _c: Sel, state: isize) {
    let mut st = get_gr_state(s);
    st.state = state;
    set_gr_state(s, st);
}
unsafe extern "C" fn gr_enabled(s: Id, _c: Sel) -> bool {
    get_gr_state(s).enabled
}
unsafe extern "C" fn gr_set_enabled(s: Id, _c: Sel, v: bool) {
    let mut st = get_gr_state(s);
    st.enabled = v;
    set_gr_state(s, st);
}
unsafe extern "C" fn gr_cancels(s: Id, _c: Sel) -> bool {
    get_gr_state(s).cancels_touches
}
unsafe extern "C" fn gr_set_cancels(s: Id, _c: Sel, v: bool) {
    let mut st = get_gr_state(s);
    st.cancels_touches = v;
    set_gr_state(s, st);
}
unsafe extern "C" fn gr_delays_began(s: Id, _c: Sel) -> bool {
    get_gr_state(s).delays_began
}
unsafe extern "C" fn gr_set_delays_began(s: Id, _c: Sel, v: bool) {
    let mut st = get_gr_state(s);
    st.delays_began = v;
    set_gr_state(s, st);
}
unsafe extern "C" fn gr_delays_ended(s: Id, _c: Sel) -> bool {
    get_gr_state(s).delays_ended
}
unsafe extern "C" fn gr_set_delays_ended(s: Id, _c: Sel, v: bool) {
    let mut st = get_gr_state(s);
    st.delays_ended = v;
    set_gr_state(s, st);
}
unsafe extern "C" fn gr_exclusive_touch(s: Id, _c: Sel) -> bool {
    get_gr_state(s).requires_exclusive_touch_type
}
unsafe extern "C" fn gr_set_exclusive_touch(s: Id, _c: Sel, v: bool) {
    let mut st = get_gr_state(s);
    st.requires_exclusive_touch_type = v;
    set_gr_state(s, st);
}
unsafe extern "C" fn gr_num_touches(_s: Id, _c: Sel) -> usize {
    0
}
unsafe extern "C" fn gr_location(s: Id, _c: Sel, _view: Id) -> CGPoint {
    get_gr_state(s).location
}
unsafe extern "C" fn gr_location_touch(_s: Id, _c: Sel, _idx: usize, _view: Id) -> CGPoint {
    CGPoint::default()
}
unsafe extern "C" fn gr_require_fail(_s: Id, _c: Sel, _other: Id) {}
unsafe extern "C" fn gr_name(s: Id, _c: Sel) -> Id {
    assoc_get(s, &GR_NAME_KEY)
}
unsafe extern "C" fn gr_set_name(s: Id, _c: Sel, v: Id) {
    let copied = if v.is_null() { NIL } else { msg0(v, "copy") };
    assoc_set(s, &GR_NAME_KEY, copied);
    if !copied.is_null() {
        objc_release(copied);
    }
}
unsafe extern "C" fn gr_allowed_touch_types(s: Id, _c: Sel) -> Id {
    let arr = assoc_get(s, &GR_TOUCH_TYPES_KEY);
    if !arr.is_null() {
        return arr;
    }
    nsarray_from_ids(&[nsnumber_i64(0), nsnumber_i64(1), nsnumber_i64(2)])
}
unsafe extern "C" fn gr_set_allowed_touch_types(s: Id, _c: Sel, arr: Id) {
    let copied = if arr.is_null() { NIL } else { msg1(cls("NSArray"), "arrayWithArray:", arr) };
    assoc_set(s, &GR_TOUCH_TYPES_KEY, copied);
}
unsafe extern "C" fn gr_allowed_press_types(s: Id, _c: Sel) -> Id {
    let arr = assoc_get(s, &GR_PRESS_TYPES_KEY);
    if !arr.is_null() {
        arr
    } else {
        msg0(cls("NSArray"), "array")
    }
}
unsafe extern "C" fn gr_set_allowed_press_types(s: Id, _c: Sel, arr: Id) {
    let copied = if arr.is_null() { NIL } else { msg1(cls("NSArray"), "arrayWithArray:", arr) };
    assoc_set(s, &GR_PRESS_TYPES_KEY, copied);
}
unsafe extern "C" fn gr_reset(s: Id, _c: Sel) {
    let mut st = get_gr_state(s);
    st.state = 0;
    set_gr_state(s, st);
}
unsafe extern "C" fn gr_noop_touches(_s: Id, _c: Sel, _touches: Id, _event: Id) {}

unsafe extern "C" fn lp_min_duration(s: Id, _c: Sel) -> f64 {
    get_gr_state(s).min_press_duration
}
unsafe extern "C" fn lp_set_min_duration(s: Id, _c: Sel, d: f64) {
    let mut st = get_gr_state(s);
    st.min_press_duration = d;
    set_gr_state(s, st);
}
unsafe extern "C" fn lp_allowable_movement(s: Id, _c: Sel) -> f64 {
    get_gr_state(s).allowable_movement
}
unsafe extern "C" fn lp_set_allowable_movement(s: Id, _c: Sel, m: f64) {
    let mut st = get_gr_state(s);
    st.allowable_movement = m;
    set_gr_state(s, st);
}
unsafe extern "C" fn gr_touches_required(s: Id, _c: Sel) -> usize {
    get_gr_state(s).touches_required
}
unsafe extern "C" fn gr_set_touches_required(s: Id, _c: Sel, n: usize) {
    let mut st = get_gr_state(s);
    st.touches_required = n;
    set_gr_state(s, st);
}
unsafe extern "C" fn lp_taps_required(s: Id, _c: Sel) -> usize {
    get_gr_state(s).taps_required.max(0) as usize
}
unsafe extern "C" fn tap_taps_required(s: Id, _c: Sel) -> usize {
    let t = get_gr_state(s).taps_required;
    if t < 0 { 1 } else { t as usize }
}
unsafe extern "C" fn gr_set_taps_required(s: Id, _c: Sel, n: usize) {
    let mut st = get_gr_state(s);
    st.taps_required = n as isize;
    set_gr_state(s, st);
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
static VC_MODAL_TRANSITION_KEY: u8 = 0;
static VC_EDGES_KEY: u8 = 0;
static VC_WANTS_FULL_SCREEN_KEY: u8 = 0;
static VC_NAV_ITEM_KEY: u8 = 0;

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

unsafe fn try_load_nib_view(self_obj: Id, nib_name: Id, bundle: Id) -> bool {
    let nib: Id = msg2(cls("UINib"), "nibWithNibName:bundle:", nib_name, bundle);
    if nib.is_null() {
        return false;
    }
    msg2::<Id, Id, Id>(nib, "instantiateWithOwner:options:", self_obj, NIL);
    !assoc_get(self_obj, &VC_VIEW_KEY).is_null()
}

unsafe extern "C" fn vc_load_view(self_obj: Id, _cmd: Sel) {
    let b = vc_nib_bundle(self_obj, sel("nibBundle"));
    let nn = assoc_get(self_obj, &VC_NIB_NAME_KEY);
    if let Some(explicit) = nsstring_to_string(nn).filter(|s| !s.is_empty()) {
        if try_load_nib_view(self_obj, nsstring(&explicit), b) {
            return;
        }
    } else {
        let cname_ptr = class_getName(object_getClass(self_obj));
        if !cname_ptr.is_null() {
            let full = CStr::from_ptr(cname_ptr).to_string_lossy();
            let is_base = matches!(
                full.as_ref(),
                "UIViewController"
                    | "UINavigationController"
                    | "UITabBarController"
                    | "UISplitViewController"
                    | "UIPageViewController"
                    | "UICollectionViewController"
                    | "UITableViewController"
            );
            if !is_base {
                let mut candidates = vec![full.to_string()];
                let unqual = match full.rsplit_once('.') {
                    Some((_, tail)) if !tail.is_empty() => {
                        candidates.push(tail.to_string());
                        tail
                    }
                    _ => full.as_ref(),
                };
                if let Some(stripped) = unqual.strip_suffix("Controller") {
                    if !stripped.is_empty() {
                        candidates.push(stripped.to_string());
                    }
                }
                for cand in candidates {
                    if try_load_nib_view(self_obj, nsstring(&cand), b) {
                        return;
                    }
                }
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

unsafe extern "C" fn vc_modal_transition(controller: Id, _cmd: Sel) -> isize {
    pear_objc::assoc_i64(controller, &VC_MODAL_TRANSITION_KEY, 0) as isize
}

unsafe extern "C" fn vc_set_modal_transition(controller: Id, _cmd: Sel, style: isize) {
    pear_objc::assoc_set_i64(controller, &VC_MODAL_TRANSITION_KEY, style as i64);
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

unsafe extern "C" fn vc_wants_full_screen_layout(controller: Id, _cmd: Sel) -> bool {
    pear_objc::assoc_bool(controller, &VC_WANTS_FULL_SCREEN_KEY, false)
}

unsafe extern "C" fn vc_set_wants_full_screen_layout(controller: Id, _cmd: Sel, full_screen: bool) {
    // PearHost has no in-window status bar, so the root already uses the
    // entire content area. Keep the legacy controller property readable.
    pear_objc::assoc_set_bool(controller, &VC_WANTS_FULL_SCREEN_KEY, full_screen);
}

unsafe extern "C" fn vc_navigation_item(self_obj: Id, _cmd: Sel) -> Id {
    let mut item = assoc_get(self_obj, &VC_NAV_ITEM_KEY);
    if item.is_null() {
        let ni_cls = cls("UINavigationItem");
        if !ni_cls.is_null() {
            item = msg0(ni_cls as Id, "new");
            assoc_set(self_obj, &VC_NAV_ITEM_KEY, item);
            objc_release(item);
        }
    }
    item
}

static ACT_IND_STYLE_KEY: u8 = 0;
static ACT_IND_ANIM_KEY: u8 = 0;
static ACT_IND_HIDES_KEY: u8 = 0;
static ACT_IND_COLOR_KEY: u8 = 0;

unsafe extern "C" fn aiv_init_style(self_obj: Id, _cmd: Sel, style: isize) -> Id {
    let frame = CGRect {
        origin: CGPoint::default(),
        size: CGSize { width: 37.0, height: 37.0 },
    };
    let obj: Id = super1(self_obj, "UIActivityIndicatorView", "initWithFrame:", frame);
    if !obj.is_null() {
        assoc_set(obj, &ACT_IND_STYLE_KEY, nsnumber_i64(style as i64));
        assoc_set(obj, &ACT_IND_ANIM_KEY, nsnumber_bool(false));
        assoc_set(obj, &ACT_IND_HIDES_KEY, nsnumber_bool(true));
        msg1::<bool, ()>(obj, "setHidden:", true);
    }
    obj
}
unsafe extern "C" fn aiv_start(self_obj: Id, _cmd: Sel) {
    assoc_set(self_obj, &ACT_IND_ANIM_KEY, nsnumber_bool(true));
    msg1::<bool, ()>(self_obj, "setHidden:", false);
}
unsafe extern "C" fn aiv_stop(self_obj: Id, _cmd: Sel) {
    assoc_set(self_obj, &ACT_IND_ANIM_KEY, nsnumber_bool(false));
    if aiv_hides(self_obj, sel("hidesWhenStopped")) {
        msg1::<bool, ()>(self_obj, "setHidden:", true);
    }
}
unsafe extern "C" fn aiv_is_animating(self_obj: Id, _cmd: Sel) -> bool {
    let n = assoc_get(self_obj, &ACT_IND_ANIM_KEY);
    !n.is_null() && msg0::<bool>(n, "boolValue")
}
unsafe extern "C" fn aiv_hides(self_obj: Id, _cmd: Sel) -> bool {
    let n = assoc_get(self_obj, &ACT_IND_HIDES_KEY);
    n.is_null() || msg0::<bool>(n, "boolValue")
}
unsafe extern "C" fn aiv_set_hides(self_obj: Id, _cmd: Sel, h: bool) {
    assoc_set(self_obj, &ACT_IND_HIDES_KEY, nsnumber_bool(h));
    if h && !aiv_is_animating(self_obj, sel("isAnimating")) {
        msg1::<bool, ()>(self_obj, "setHidden:", true);
    }
}
unsafe extern "C" fn aiv_color(self_obj: Id, _cmd: Sel) -> Id {
    assoc_get(self_obj, &ACT_IND_COLOR_KEY)
}
unsafe extern "C" fn aiv_set_color(self_obj: Id, _cmd: Sel, c: Id) {
    assoc_set(self_obj, &ACT_IND_COLOR_KEY, c);
}
unsafe extern "C" fn aiv_style(self_obj: Id, _cmd: Sel) -> isize {
    let n = assoc_get(self_obj, &ACT_IND_STYLE_KEY);
    if n.is_null() { 0 } else { msg0(n, "integerValue") }
}
unsafe extern "C" fn aiv_set_style(self_obj: Id, _cmd: Sel, style: isize) {
    assoc_set(self_obj, &ACT_IND_STYLE_KEY, nsnumber_i64(style as i64));
}
unsafe extern "C" fn vc_orientations(_self: Id, _cmd: Sel) -> usize {
    (1 << 3) | (1 << 4) // landscape
}
unsafe extern "C" fn vc_pref_orientation(_self: Id, _cmd: Sel) -> isize {
    4 // landscapeRight
}
unsafe extern "C" fn vc_interface_orientation(_self: Id, _cmd: Sel) -> isize {
    let view: Id = msg0(_self, "viewIfLoaded");
    let window: Id = msg0(view, "window");
    window_orientation(if window.is_null() { PearUIKitKeyWindow() } else { window })
}

static VC_ORIENTATION_UPDATE_KEY: u8 = 0;

unsafe extern "C" fn vc_needs_orientation_update(controller: Id, _cmd: Sel) {
    if !assoc_get(controller, &VC_ORIENTATION_UPDATE_KEY).is_null() {
        return;
    }
    assoc_set(controller, &VC_ORIENTATION_UPDATE_KEY, nsnumber_bool(true));
    dispatch_async_f(&raw const _dispatch_main_q, objc_retain(controller) as *mut c_void, vc_update_orientation);
}

extern "C" fn vc_update_orientation(context: *mut c_void) {
    unsafe {
        let controller = context as Id;
        assoc_set(controller, &VC_ORIENTATION_UPDATE_KEY, NIL);
        let view: Id = msg0(controller, "viewIfLoaded");
        let window: Id = msg0(view, "window");
        if !window.is_null() {
            let mut active: Id = msg0(window, "rootViewController");
            while !active.is_null() {
                let presented: Id = msg0(active, "presentedViewController");
                if presented.is_null() || presented == active { break; }
                active = presented;
            }
            if active == controller {
                update_window_orientation(window, controller);
            }
        }
        objc_release(controller);
    }
}

unsafe fn update_window_orientation(window: Id, controller: Id) {
    let application: Id = msg0(cls("UIApplication"), "sharedApplication");
    let application_mask: usize = msg1(application, "supportedInterfaceOrientationsForWindow:", window);
    let controller_mask: usize = msg0(controller, "supportedInterfaceOrientations");
    let supported = application_mask & controller_mask;
    let current = window_orientation(window);
    if supported & (1 << current) != 0 {
        return;
    }
    let preferred: isize = msg0(controller, "preferredInterfaceOrientationForPresentation");
    let Some(orientation) = [preferred, 4, 3, 1, 2].into_iter().find(|&value| {
        (1..=4).contains(&value) && supported & (1 << value) != 0
    }) else { return };

    let native = win_native_ptr(window);
    if native.is_null() {
        return;
    }
    pear_objc::assoc_set_i64(window, &WIN_ORIENTATION_KEY, orientation as i64);
    if (orientation >= 3) != (current >= 3) {
        let bounds: CGRect = msg0(window, "bounds");
        let size = pear_host::Size { width: bounds.size.height, height: bounds.size.width };
        pear_host::host().orient_window(pear_host::WindowHandle(native), size);
    }
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

static WIN_SCENE_KEY: u8 = 0;
static WIN_ORIENTATION_KEY: u8 = 0;

pub(crate) unsafe fn window_orientation(window: Id) -> isize {
    if window.is_null() {
        return 4;
    }
    let bounds: CGRect = msg0(window, "bounds");
    let landscape = bounds.size.width >= bounds.size.height;
    let stored = pear_objc::assoc_i64(window, &WIN_ORIENTATION_KEY, 0) as isize;
    if stored != 0 && (stored >= 3) == landscape { stored } else if landscape { 4 } else { 1 }
}

unsafe extern "C" fn win_init_scene(self_obj: Id, _cmd: Sel, scene: Id) -> Id {
    let b: CGRect = msg0(msg0(cls("UIScreen"), "mainScreen"), "bounds");
    let obj: Id = msg1(self_obj, "initWithFrame:", b);
    if !obj.is_null() {
        pear_objc::assoc_set_weak(obj, &WIN_SCENE_KEY, scene);
    }
    obj
}
unsafe extern "C" fn win_scene(self_obj: Id, _cmd: Sel) -> Id {
    pear_objc::assoc_get_weak(self_obj, &WIN_SCENE_KEY)
}
unsafe extern "C" fn win_set_scene(self_obj: Id, _cmd: Sel, scene: Id) {
    pear_objc::assoc_set_weak(self_obj, &WIN_SCENE_KEY, scene);
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
    if !native.is_null() {
        pear_host::host().set_root_layer(pear_host::WindowHandle(native), pear_host::LayerRef(l));
        view_sync_native_tree(w);
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
        msg0::<()>(window, "layoutIfNeeded");
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
static NAV_ITEM_TITLE_KEY: u8 = 0;
unsafe extern "C" fn nav_item_title(s: Id, _c: Sel) -> Id { assoc_get(s, &NAV_ITEM_TITLE_KEY) }
unsafe extern "C" fn nav_item_set_title(s: Id, _c: Sel, t: Id) { assoc_set(s, &NAV_ITEM_TITLE_KEY, t); }
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
    let mut shared = Vec::new();
    for i in 0..nsarray_count(items) {
        let o = nsarray_object_at(items, i);
        if is_kind_of(o, "NSString") {
            shared.push(pear_host::PasteboardItem::Text(nsstring_to_string(o).unwrap_or_default()));
        } else if is_kind_of(o, "NSURL") {
            shared.push(pear_host::PasteboardItem::Url(nsstring_to_string(msg0(o, "absoluteString")).unwrap_or_default()));
        }
    }
    pear_host::host().share(&shared);
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
        let directory = if dir.is_null() { None } else { nsstring_to_string(msg0(dir, "absoluteString")) };
        let request = if is_save {
            let urls = assoc_get(s, &DP_URLS_KEY);
            let name = if nsarray_count(urls) > 0 {
                nsstring_to_string(msg0(nsarray_object_at(urls, 0), "lastPathComponent")).unwrap_or_default()
            } else {
                "Document".to_string()
            };
            pear_host::DocumentRequest::Save { suggested_name: name, directory }
        } else {
            let multiple = !assoc_get(s, &DP_MULTI_KEY).is_null() && msg0::<bool>(assoc_get(s, &DP_MULTI_KEY), "boolValue");
            pear_host::DocumentRequest::Open { multiple, directory }
        };
        let picker = s as usize;
        pear_host::host().pick_documents(
            request,
            Box::new(move |chosen| {
                let s = picker as Id;
                let del = pear_objc::assoc_get_weak(s, &DP_DEL_KEY);
                match chosen {
                    Some(urls) if !urls.is_empty() => {
                        let urls: Vec<Id> = urls.iter().map(|u| msg1(cls("NSURL"), "URLWithString:", nsstring(u))).filter(|u: &Id| !u.is_null()).collect();
                        if !del.is_null() && responds_to(del, "documentPicker:didPickDocumentsAtURLs:") {
                            msg2::<Id, Id, ()>(del, "documentPicker:didPickDocumentsAtURLs:", s, nsarray_from_ids(&urls));
                        }
                    }
                    _ => {
                        if !del.is_null() && responds_to(del, "documentPickerWasCancelled:") {
                            msg1::<Id, ()>(del, "documentPickerWasCancelled:", s);
                        }
                    }
                }
                objc_release(s);
            }),
        );
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
        add_method(v, "transform", view_transform as Imp, "{CGAffineTransform=dddddd}@:");
        add_method(v, "setTransform:", view_set_transform as Imp, "v@:{CGAffineTransform=dddddd}");
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
        for s in ["didAddSubview:", "willRemoveSubview:", "willMoveToSuperview:", "willMoveToWindow:"] {
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
        add_method(v, "tintColor", view_tint_color as Imp, "@@:");
        add_method(v, "setTintColor:", view_set_tint_color as Imp, "v@:@");
        add_method(v, "tintColorDidChange", view_noop as Imp, "v@:");
        add_method(v, "snapshotViewAfterScreenUpdates:", view_snapshot as Imp, "@@:B");
        add_method(v, "contentMode", view_zero_int as Imp, "q@:");
        add_method(v, "setContentMode:", view_noop_int as Imp, "v@:q");
        add_method(v, "safeAreaInsets", view_zero_insets as Imp, "{UIEdgeInsets=dddd}@:");
        add_method(v, "layoutMargins", view_layout_margins as Imp, "{UIEdgeInsets=dddd}@:");
        add_method(v, "traitCollection", view_trait_collection as Imp, "@@:");
        add_method(v, "gestureRecognizers", view_gestures as Imp, "@@:");
        add_method(v, "setGestureRecognizers:", view_set_gestures as Imp, "v@:@");
        add_method(v, "addGestureRecognizer:", view_add_gesture as Imp, "v@:@");
        add_method(v, "removeGestureRecognizer:", view_remove_gesture as Imp, "v@:@");
        add_method(v, "translatesAutoresizingMaskIntoConstraints", view_translates_autoresizing as Imp, "B@:");
        add_method(v, "setTranslatesAutoresizingMaskIntoConstraints:", view_set_translates_autoresizing as Imp, "v@:B");
        layout_controls_imp::register_view(v);
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
        add_method(v, "endEditing:", view_end_editing as Imp, "B@:B");
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
        add_method(vc, "modalTransitionStyle", vc_modal_transition as Imp, "q@:");
        add_method(vc, "setModalTransitionStyle:", vc_set_modal_transition as Imp, "v@:q");
        add_method(vc, "edgesForExtendedLayout", vc_edges as Imp, "Q@:");
        add_method(vc, "setEdgesForExtendedLayout:", vc_set_edges as Imp, "v@:Q");
        add_method(vc, "wantsFullScreenLayout", vc_wants_full_screen_layout as Imp, "B@:");
        add_method(vc, "setWantsFullScreenLayout:", vc_set_wants_full_screen_layout as Imp, "v@:B");
        add_method(vc, "traitCollection", view_trait_collection as Imp, "@@:");
        add_method(vc, "parentViewController", view_nil as Imp, "@@:");
        add_method(vc, "childViewControllers", view_empty_arr as Imp, "@@:");
        add_method(vc, "navigationController", view_nil as Imp, "@@:");
        add_method(vc, "navigationItem", vc_navigation_item as Imp, "@@:");
        add_method(vc, "tabBarController", view_nil as Imp, "@@:");
        add_method(vc, "prefersStatusBarHidden", view_true as Imp, "B@:");
        add_method(vc, "prefersHomeIndicatorAutoHidden", view_true as Imp, "B@:");
        add_method(vc, "prefersPointerLocked", view_false as Imp, "B@:");
        add_method(vc, "shouldAutorotate", view_true as Imp, "B@:");
        add_method(vc, "supportedInterfaceOrientations", vc_orientations as Imp, "Q@:");
        add_method(vc, "setNeedsUpdateOfSupportedInterfaceOrientations", vc_needs_orientation_update as Imp, "v@:");
        add_method(vc, "interfaceOrientation", vc_interface_orientation as Imp, "q@:");
        add_method(vc, "preferredInterfaceOrientationForPresentation", vc_pref_orientation as Imp, "q@:");
        add_method(vc, "preferredScreenEdgesDeferringSystemGestures", vc_pref_edges as Imp, "Q@:");
        add_method(vc, "presentedViewController", vc_presented as Imp, "@@:");
        add_method(vc, "presentingViewController", vc_presenting as Imp, "@@:");
        add_method(vc, "pear_setPresenting:", vc_set_presenting as Imp, "v@:@");
        add_method(vc, "presentViewController:animated:completion:", vc_present as Imp, "v@:@B@?");
        add_method(vc, "dismissViewControllerAnimated:completion:", vc_dismiss as Imp, "v@:B@?");
        add_method(vc, "isBeingPresented", view_false as Imp, "B@:");
        add_method(vc, "isBeingDismissed", view_false as Imp, "B@:");
        add_method(vc, "isMovingToParentViewController", view_false as Imp, "B@:");
        add_method(vc, "isMovingFromParentViewController", view_false as Imp, "B@:");
        add_method(object_getClass(vc as Id), "attemptRotationToDeviceOrientation", view_noop as Imp, "v@:");
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
        add_method(w, "windowScene", win_scene as Imp, "@@:");
        add_method(w, "setWindowScene:", win_set_scene as Imp, "v@:@");
        add_method(w, "isKeyWindow", win_is_key as Imp, "B@:");
        add_method(w, "canBecomeKeyWindow", view_true as Imp, "B@:");
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

    let ni = cls("UINavigationItem");
    if !ni.is_null() {
        add_method(ni, "title", nav_item_title as Imp, "@@:");
        add_method(ni, "setTitle:", nav_item_set_title as Imp, "v@:@");
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

    let aiv = cls("UIActivityIndicatorView");
    if !aiv.is_null() {
        add_method(aiv, "initWithActivityIndicatorStyle:", aiv_init_style as Imp, "@@:q");
        add_method(aiv, "startAnimating", aiv_start as Imp, "v@:");
        add_method(aiv, "stopAnimating", aiv_stop as Imp, "v@:");
        add_method(aiv, "isAnimating", aiv_is_animating as Imp, "B@:");
        add_method(aiv, "hidesWhenStopped", aiv_hides as Imp, "B@:");
        add_method(aiv, "setHidesWhenStopped:", aiv_set_hides as Imp, "v@:B");
        add_method(aiv, "color", aiv_color as Imp, "@@:");
        add_method(aiv, "setColor:", aiv_set_color as Imp, "v@:@");
        add_method(aiv, "activityIndicatorViewStyle", aiv_style as Imp, "q@:");
        add_method(aiv, "setActivityIndicatorViewStyle:", aiv_set_style as Imp, "v@:q");
    }

    let gr = cls("UIGestureRecognizer");
    if !gr.is_null() {
        add_method(gr, "init", gr_init as Imp, "@@:");
        add_method(gr, "initWithTarget:action:", gr_init_target_action as Imp, "@@:@:");
        add_method(gr, "initWithCoder:", gr_init_coder as Imp, "@@:@");
        add_method(gr, "addTarget:action:", gr_add_target_action as Imp, "v@:@:");
        add_method(gr, "removeTarget:action:", gr_remove_target_action as Imp, "v@:@:");
        add_method(gr, "view", gr_view as Imp, "@@:");
        add_method(gr, "delegate", gr_delegate as Imp, "@@:");
        add_method(gr, "setDelegate:", gr_set_delegate as Imp, "v@:@");
        add_method(gr, "state", gr_state as Imp, "q@:");
        add_method(gr, "setState:", gr_set_state as Imp, "v@:q");
        add_method(gr, "isEnabled", gr_enabled as Imp, "B@:");
        add_method(gr, "enabled", gr_enabled as Imp, "B@:");
        add_method(gr, "setEnabled:", gr_set_enabled as Imp, "v@:B");
        add_method(gr, "cancelsTouchesInView", gr_cancels as Imp, "B@:");
        add_method(gr, "setCancelsTouchesInView:", gr_set_cancels as Imp, "v@:B");
        add_method(gr, "delaysTouchesBegan", gr_delays_began as Imp, "B@:");
        add_method(gr, "setDelaysTouchesBegan:", gr_set_delays_began as Imp, "v@:B");
        add_method(gr, "delaysTouchesEnded", gr_delays_ended as Imp, "B@:");
        add_method(gr, "setDelaysTouchesEnded:", gr_set_delays_ended as Imp, "v@:B");
        add_method(gr, "requiresExclusiveTouchType", gr_exclusive_touch as Imp, "B@:");
        add_method(gr, "setRequiresExclusiveTouchType:", gr_set_exclusive_touch as Imp, "v@:B");
        add_method(gr, "numberOfTouches", gr_num_touches as Imp, "Q@:");
        add_method(gr, "locationInView:", gr_location as Imp, "{CGPoint=dd}@:@");
        add_method(gr, "locationOfTouch:inView:", gr_location_touch as Imp, "{CGPoint=dd}@:Q@");
        add_method(gr, "requireGestureRecognizerToFail:", gr_require_fail as Imp, "v@:@");
        add_method(gr, "name", gr_name as Imp, "@@:");
        add_method(gr, "setName:", gr_set_name as Imp, "v@:@");
        add_method(gr, "allowedTouchTypes", gr_allowed_touch_types as Imp, "@@:");
        add_method(gr, "setAllowedTouchTypes:", gr_set_allowed_touch_types as Imp, "v@:@");
        add_method(gr, "allowedPressTypes", gr_allowed_press_types as Imp, "@@:");
        add_method(gr, "setAllowedPressTypes:", gr_set_allowed_press_types as Imp, "v@:@");
        add_method(gr, "reset", gr_reset as Imp, "v@:");
        for s in [
            "touchesBegan:withEvent:",
            "touchesMoved:withEvent:",
            "touchesEnded:withEvent:",
            "touchesCancelled:withEvent:",
        ] {
            add_method(gr, s, gr_noop_touches as Imp, "v@:@@");
        }
    }

    let lp = cls("UILongPressGestureRecognizer");
    if !lp.is_null() {
        add_method(lp, "minimumPressDuration", lp_min_duration as Imp, "d@:");
        add_method(lp, "setMinimumPressDuration:", lp_set_min_duration as Imp, "v@:d");
        add_method(lp, "allowableMovement", lp_allowable_movement as Imp, "d@:");
        add_method(lp, "setAllowableMovement:", lp_set_allowable_movement as Imp, "v@:d");
        add_method(lp, "numberOfTouchesRequired", gr_touches_required as Imp, "Q@:");
        add_method(lp, "setNumberOfTouchesRequired:", gr_set_touches_required as Imp, "v@:Q");
        add_method(lp, "numberOfTapsRequired", lp_taps_required as Imp, "Q@:");
        add_method(lp, "setNumberOfTapsRequired:", gr_set_taps_required as Imp, "v@:Q");
    }

    let tap = cls("UITapGestureRecognizer");
    if !tap.is_null() {
        add_method(tap, "numberOfTapsRequired", tap_taps_required as Imp, "Q@:");
        add_method(tap, "setNumberOfTapsRequired:", gr_set_taps_required as Imp, "v@:Q");
        add_method(tap, "numberOfTouchesRequired", gr_touches_required as Imp, "Q@:");
        add_method(tap, "setNumberOfTouchesRequired:", gr_set_touches_required as Imp, "v@:Q");
    }

    let scribble = cls("UIIndirectScribbleInteraction");
    if !scribble.is_null() {
        unsafe extern "C" fn scribble_init_del(self_obj: Id, _cmd: Sel, _del: Id) -> Id {
            msg0(self_obj, "init")
        }
        add_method(scribble, "initWithDelegate:", scribble_init_del as Imp, "@@:@");
    }

    pointer_gestures::register();
    layout_controls_imp::register_classes();
}

mod layout_controls_imp {
    use super::*;

    unsafe fn get_pod<T: Copy + Default>(obj: Id, key: &u8) -> T {
        let data = assoc_get(obj, key);
        if let Some(bytes) = nsdata_to_vec(data) {
            if bytes.len() == std::mem::size_of::<T>() {
                return std::ptr::read_unaligned(bytes.as_ptr() as *const T);
            }
        }
        T::default()
    }

    unsafe fn set_pod<T: Copy>(obj: Id, key: &u8, val: T) {
        let bytes = std::slice::from_raw_parts(&val as *const T as *const u8, std::mem::size_of::<T>());
        assoc_set(obj, key, nsdata_from_bytes(bytes));
    }

    unsafe fn set_copied(obj: Id, key: &u8, val: Id) {
        let copied = if val.is_null() { NIL } else { msg0(val, "copy") };
        assoc_set(obj, key, copied);
        if !copied.is_null() {
            objc_release(copied);
        }
    }

    // ---- UIView animations + constraints ----
    static G_ANIMATIONS_ENABLED: AtomicBool = AtomicBool::new(true);
    static SOLVING_LAYOUT: AtomicBool = AtomicBool::new(false);
    static VIEW_CONSTRAINTS_KEY: u8 = 0;

    pub(crate) fn is_solving_layout() -> bool {
        SOLVING_LAYOUT.load(Ordering::Acquire)
    }

    unsafe fn view_set_anim(_cls: Id, _c: Sel, enabled: bool) {
        G_ANIMATIONS_ENABLED.store(enabled, Ordering::Release);
    }
    unsafe fn view_are_anim(_cls: Id, _c: Sel) -> bool {
        G_ANIMATIONS_ENABLED.load(Ordering::Acquire)
    }
    extern "C" fn finish_view_animation(context: *mut c_void) {
        unsafe {
            let block = &*context.cast::<BlockHeader>();
            let finish: unsafe extern "C" fn(Id, bool) = std::mem::transmute(block.invoke);
            finish(context as Id, true);
            _Block_release(context);
        }
    }
    unsafe extern "C" fn view_animate(
        _cls: Id, _c: Sel, duration: f64, delay: f64, _options: usize, animations: Id, completion: Id,
    ) {
        // Update the view state immediately, as UIKit does for model values,
        // and deliver the completion on the main queue when the transition ends.
        invoke_void_block(animations);
        if completion.is_null() { return; }
        let copied = _Block_copy(completion as *const c_void);
        let seconds = if G_ANIMATIONS_ENABLED.load(Ordering::Acquire) {
            delay.max(0.0) + duration.max(0.0)
        } else {
            0.0
        };
        dispatch_after_f(
            dispatch_time(0, (seconds * 1e9) as i64),
            &_dispatch_main_q,
            copied,
            finish_view_animation,
        );
    }
    unsafe extern "C" fn view_constraints(self_obj: Id, _c: Sel) -> Id {
        let arr = mut_array_for_key(self_obj, &VIEW_CONSTRAINTS_KEY);
        msg1(cls("NSArray"), "arrayWithArray:", arr)
    }
    unsafe extern "C" fn view_add_constraint(self_obj: Id, _c: Sel, c: Id) {
        if c.is_null() {
            return;
        }
        let arr = mut_array_for_key(self_obj, &VIEW_CONSTRAINTS_KEY);
        if !msg1::<Id, bool>(arr, "containsObject:", c) {
            msg1::<Id, ()>(arr, "addObject:", c);
        }
        if is_kind_of(c, "PearLayoutConstraint") {
            let mut d: ConstraintData = get_pod(c, &PLC_DATA_KEY);
            d.active = true;
            set_pod(c, &PLC_DATA_KEY, d);
            pear_objc::assoc_set_weak(c, &PLC_OWNER_KEY, self_obj);
        }
        msg0::<()>(self_obj, "setNeedsLayout");
        let sv = msg0::<Id>(self_obj, "superview");
        if !sv.is_null() {
            msg0::<()>(sv, "setNeedsLayout");
        }
    }
    unsafe extern "C" fn view_add_constraints(self_obj: Id, _c: Sel, cs: Id) {
        if cs.is_null() {
            return;
        }
        for i in 0..nsarray_count(cs) {
            view_add_constraint(self_obj, sel("addConstraint:"), nsarray_object_at(cs, i));
        }
    }
    unsafe extern "C" fn view_remove_constraint(self_obj: Id, _c: Sel, c: Id) {
        if c.is_null() {
            return;
        }
        let arr = mut_array_for_key(self_obj, &VIEW_CONSTRAINTS_KEY);
        msg1::<Id, ()>(arr, "removeObjectIdenticalTo:", c);
        if is_kind_of(c, "PearLayoutConstraint") {
            let mut d: ConstraintData = get_pod(c, &PLC_DATA_KEY);
            d.active = false;
            set_pod(c, &PLC_DATA_KEY, d);
            if pear_objc::assoc_get_weak(c, &PLC_OWNER_KEY) == self_obj {
                pear_objc::assoc_set_weak(c, &PLC_OWNER_KEY, NIL);
            }
        }
        msg0::<()>(self_obj, "setNeedsLayout");
    }
    unsafe extern "C" fn view_remove_constraints(self_obj: Id, _c: Sel, cs: Id) {
        if cs.is_null() {
            return;
        }
        for i in 0..nsarray_count(cs) {
            view_remove_constraint(self_obj, sel("removeConstraint:"), nsarray_object_at(cs, i));
        }
    }

    unsafe extern "C" fn view_animate_simple(
        cls_obj: Id, _c: Sel, duration: f64, animations: Id, completion: Id,
    ) {
        view_animate(
            cls_obj,
            sel("animateWithDuration:delay:options:animations:completion:"),
            duration,
            0.0,
            0,
            animations,
            completion,
        );
    }

    pub unsafe fn register_view(v: Class) {
        add_class_method(v, "setAnimationsEnabled:", view_set_anim as Imp, "v@:B");
        add_class_method(v, "areAnimationsEnabled", view_are_anim as Imp, "B@:");
        add_class_method(v, "animateWithDuration:delay:options:animations:completion:", view_animate as Imp, "v@:ddQ@?@?");
        add_class_method(v, "animateWithDuration:animations:completion:", view_animate_simple as Imp, "v@:d@?@?");
        add_method(v, "constraints", view_constraints as Imp, "@@:");
        add_method(v, "addConstraint:", view_add_constraint as Imp, "v@:@");
        add_method(v, "addConstraints:", view_add_constraints as Imp, "v@:@");
        add_method(v, "removeConstraint:", view_remove_constraint as Imp, "v@:@");
        add_method(v, "removeConstraints:", view_remove_constraints as Imp, "v@:@");
        for s in ["setNeedsUpdateConstraints", "updateConstraintsIfNeeded", "updateConstraints"] {
            add_method(v, s, view_noop as Imp, "v@:");
        }
        for name in ANCHOR_SELECTORS {
            add_method(v, name, view_anchor as Imp, "@@:");
        }
        add_method(v, "safeAreaLayoutGuide", view_layout_guide as Imp, "@@:");
        add_method(v, "layoutMarginsGuide", view_layout_guide as Imp, "@@:");
        add_method(v, "keyboardLayoutGuide", view_layout_guide as Imp, "@@:");
        add_method(v, "layoutGuideForLayoutRegion:", view_layout_guide_for_region as Imp, "@@:@");
    }

    static GUIDE_OWNER_KEY: u8 = 0;
    static GUIDE_IDENT_KEY: u8 = 0;
    static VIEW_SAFE_GUIDE_KEY: u8 = 0;

    unsafe extern "C" fn region_factory(c: Id, _cmd: Sel, _adapt: isize) -> Id {
        let obj = msg0::<Id>(c, "alloc");
        let init = msg0::<Id>(obj, "init");
        objc_autorelease(init)
    }
    unsafe extern "C" fn region_simple(c: Id, _cmd: Sel) -> Id {
        region_factory(c, _cmd, 0)
    }

    unsafe extern "C" fn view_layout_guide(self_obj: Id, _cmd: Sel) -> Id {
        if self_obj.is_null() {
            return NIL;
        }
        let existing = assoc_get(self_obj, &VIEW_SAFE_GUIDE_KEY);
        if !existing.is_null() {
            return existing;
        }
        let g_cls = cls("UILayoutGuide");
        if g_cls.is_null() {
            return NIL;
        }
        let guide = msg0::<Id>(msg0(g_cls as Id, "alloc"), "init");
        if !guide.is_null() {
            objc_setAssociatedObject(
                guide,
                &GUIDE_OWNER_KEY as *const u8 as *const c_void,
                self_obj,
                0, // OBJC_ASSOCIATION_ASSIGN: avoid retain cycle with owning UIView
            );
            assoc_set(self_obj, &VIEW_SAFE_GUIDE_KEY, guide);
            objc_release(guide);
        }
        guide
    }
    unsafe extern "C" fn view_layout_guide_for_region(self_obj: Id, _cmd: Sel, _region: Id) -> Id {
        view_layout_guide(self_obj, _cmd)
    }

    unsafe extern "C" fn guide_owning_view(self_obj: Id, _cmd: Sel) -> Id {
        assoc_get(self_obj, &GUIDE_OWNER_KEY)
    }
    unsafe extern "C" fn guide_set_owning_view(self_obj: Id, _cmd: Sel, view: Id) {
        objc_setAssociatedObject(
            self_obj,
            &GUIDE_OWNER_KEY as *const u8 as *const c_void,
            view,
            0,
        );
    }
    unsafe extern "C" fn guide_identifier(self_obj: Id, _cmd: Sel) -> Id {
        assoc_get(self_obj, &GUIDE_IDENT_KEY)
    }
    unsafe extern "C" fn guide_set_identifier(self_obj: Id, _cmd: Sel, ident: Id) {
        assoc_set(self_obj, &GUIDE_IDENT_KEY, ident);
    }
    unsafe extern "C" fn guide_layout_frame(self_obj: Id, _cmd: Sel) -> CGRect {
        let owner = assoc_get(self_obj, &GUIDE_OWNER_KEY);
        if !owner.is_null() {
            super::view_bounds(owner, sel("bounds"))
        } else {
            CGRect {
                origin: CGPoint { x: 0.0, y: 0.0 },
                size: CGSize { width: 0.0, height: 0.0 },
            }
        }
    }

    // ---- PearLayoutConstraint + NSLayoutConstraint ----
    #[repr(C)]
    #[derive(Copy, Clone)]
    struct ConstraintData {
        first_attr: isize,
        second_attr: isize,
        relation: isize,
        multiplier: f64,
        constant: f64,
        priority: f32,
        active: bool,
    }
    impl Default for ConstraintData {
        fn default() -> Self {
            Self {
                first_attr: 0,
                second_attr: 0,
                relation: 0,
                multiplier: 1.0,
                constant: 0.0,
                priority: 1000.0,
                active: false,
            }
        }
    }

    static PLC_DATA_KEY: u8 = 0;
    static PLC_FIRST_KEY: u8 = 0;
    static PLC_SECOND_KEY: u8 = 0;
    static PLC_OWNER_KEY: u8 = 0;
    static PLC_IDENT_KEY: u8 = 0;
    static ORIG_WITH_ITEM: AtomicUsize = AtomicUsize::new(0);
    static ORIG_ACTIVATE: AtomicUsize = AtomicUsize::new(0);
    static ORIG_DEACTIVATE: AtomicUsize = AtomicUsize::new(0);
    static ORIG_VFL: AtomicUsize = AtomicUsize::new(0);
    static ANCHOR_KEYS: [u8; 12] = [0; 12];
    static ANCHOR_ITEM_KEY: u8 = 0;
    static ANCHOR_ATTRIBUTE_KEY: u8 = 0;
    const ANCHOR_SELECTORS: [&str; 12] = [
        "leftAnchor", "rightAnchor", "topAnchor", "bottomAnchor", "leadingAnchor",
        "trailingAnchor", "widthAnchor", "heightAnchor", "centerXAnchor", "centerYAnchor",
        "lastBaselineAnchor", "firstBaselineAnchor",
    ];

    unsafe extern "C" fn view_anchor(item: Id, cmd: Sel) -> Id {
        let Some(index) = ANCHOR_SELECTORS.iter().position(|name| sel(name) == cmd) else {
            return NIL;
        };
        let key = &ANCHOR_KEYS[index];
        let existing = assoc_get(item, key);
        if !existing.is_null() { return existing; }
        let anchor: Id = msg0(cls("PearLayoutAnchor"), "new");
        pear_objc::assoc_set_weak(anchor, &ANCHOR_ITEM_KEY, item);
        assoc_set(anchor, &ANCHOR_ATTRIBUTE_KEY, nsnumber_i64(index as i64 + 1));
        assoc_set(item, key, anchor);
        objc_release(anchor);
        anchor
    }

    unsafe fn anchor_constraint(anchor: Id, cmd: Sel, other: Id, multiplier: f64, constant: f64) -> Id {
        let relation = match CStr::from_ptr(sel_getName(cmd)).to_bytes() {
            s if s.starts_with(b"constraintLessThanOrEqualTo") => -1,
            s if s.starts_with(b"constraintGreaterThanOrEqualTo") => 1,
            _ => 0,
        };
        let item = pear_objc::assoc_get_weak(anchor, &ANCHOR_ITEM_KEY);
        let other_item = pear_objc::assoc_get_weak(other, &ANCHOR_ITEM_KEY);
        let attribute = msg0(assoc_get(anchor, &ANCHOR_ATTRIBUTE_KEY), "integerValue");
        let other_attribute = msg0(assoc_get(other, &ANCHOR_ATTRIBUTE_KEY), "integerValue");
        let command = sel("constraintWithItem:attribute:relatedBy:toItem:attribute:multiplier:constant:");
        nslc_with_item(cls("NSLayoutConstraint") as Id, command, item, attribute, relation,
                       other_item, other_attribute, multiplier, constant)
    }

    unsafe extern "C" fn anchor_to(anchor: Id, cmd: Sel, other: Id) -> Id {
        anchor_constraint(anchor, cmd, other, 1.0, 0.0)
    }
    unsafe extern "C" fn anchor_to_constant(anchor: Id, cmd: Sel, other: Id, constant: f64) -> Id {
        anchor_constraint(anchor, cmd, other, 1.0, constant)
    }
    unsafe extern "C" fn anchor_constant(anchor: Id, cmd: Sel, constant: f64) -> Id {
        anchor_constraint(anchor, cmd, NIL, 1.0, constant)
    }
    unsafe extern "C" fn anchor_to_multiplier(anchor: Id, cmd: Sel, other: Id, multiplier: f64) -> Id {
        anchor_constraint(anchor, cmd, other, multiplier, 0.0)
    }
    unsafe extern "C" fn anchor_to_multiplier_constant(
        anchor: Id, cmd: Sel, other: Id, multiplier: f64, constant: f64,
    ) -> Id {
        anchor_constraint(anchor, cmd, other, multiplier, constant)
    }

    unsafe fn is_pear_layout_item(obj: Id) -> bool {
        !obj.is_null() && (is_kind_of(obj, "UIView") || is_kind_of(obj, "UILayoutGuide"))
    }

    unsafe fn resolve_layout_view(item: Id) -> Id {
        if item.is_null() {
            NIL
        } else if is_kind_of(item, "UILayoutGuide") {
            guide_owning_view(item, sel("owningView"))
        } else if is_kind_of(item, "UIView") {
            item
        } else {
            NIL
        }
    }

    unsafe fn common_ancestor_view(v1: Id, v2: Id) -> Id {
        if v1.is_null() {
            return v2;
        }
        if v2.is_null() || v1 == v2 {
            return v1;
        }
        let mut ancestors = Vec::new();
        let mut cur = v1;
        while !cur.is_null() {
            ancestors.push(cur);
            cur = msg0(cur, "superview");
        }
        cur = v2;
        while !cur.is_null() {
            if ancestors.contains(&cur) {
                return cur;
            }
            cur = msg0(cur, "superview");
        }
        NIL
    }

    unsafe fn invalidate_constraint_layout(c: Id) {
        let owner = pear_objc::assoc_get_weak(c, &PLC_OWNER_KEY);
        if !owner.is_null() {
            msg0::<()>(owner, "setNeedsLayout");
        }
        let v1 = resolve_layout_view(pear_objc::assoc_get_weak(c, &PLC_FIRST_KEY));
        if !v1.is_null() {
            msg0::<()>(v1, "setNeedsLayout");
            let sv = msg0::<Id>(v1, "superview");
            if !sv.is_null() {
                msg0::<()>(sv, "setNeedsLayout");
            }
        }
        let v2 = resolve_layout_view(pear_objc::assoc_get_weak(c, &PLC_SECOND_KEY));
        if !v2.is_null() && v2 != v1 {
            msg0::<()>(v2, "setNeedsLayout");
            let sv = msg0::<Id>(v2, "superview");
            if !sv.is_null() {
                msg0::<()>(sv, "setNeedsLayout");
            }
        }
    }

    #[derive(Copy, Clone, PartialEq, Eq)]
    enum NormAttr {
        Left,
        Right,
        Top,
        Bottom,
        Width,
        Height,
        CenterX,
        CenterY,
    }

    fn normalize_attr(attr: isize) -> Option<(NormAttr, f64)> {
        match attr {
            1 | 5 => Some((NormAttr::Left, 0.0)),
            13 | 17 => Some((NormAttr::Left, 8.0)),
            2 | 6 => Some((NormAttr::Right, 0.0)),
            14 | 18 => Some((NormAttr::Right, -8.0)),
            3 | 12 => Some((NormAttr::Top, 0.0)),
            15 => Some((NormAttr::Top, 8.0)),
            4 | 11 => Some((NormAttr::Bottom, 0.0)),
            16 => Some((NormAttr::Bottom, -8.0)),
            7 => Some((NormAttr::Width, 0.0)),
            8 => Some((NormAttr::Height, 0.0)),
            9 | 19 => Some((NormAttr::CenterX, 0.0)),
            10 | 20 => Some((NormAttr::CenterY, 0.0)),
            _ => None,
        }
    }

    struct LayoutNode {
        view: Id,
        parent: Option<usize>,
        children: Vec<usize>,
        init_frame: CGRect,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        is_window: bool,
        constrained: bool,
        has_left: bool,
        has_right: bool,
        has_center_x: bool,
        has_width: bool,
        has_top: bool,
        has_bottom: bool,
        has_center_y: bool,
        has_height: bool,
        intrinsic_w: f64,
        intrinsic_h: f64,
        is_multiline_label: bool,
    }

    struct ResolvedConstraint {
        idx1: usize,
        attr1: NormAttr,
        margin1: f64,
        idx2: Option<usize>,
        attr2: Option<NormAttr>,
        margin2: f64,
        relation: isize,
        multiplier: f64,
        constant: f64,
        priority: f32,
        seq: usize,
    }

    fn shift_subtree(nodes: &mut [LayoutNode], idx: usize, dx: f64, dy: f64) {
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        let children = nodes[idx].children.clone();
        for c in children {
            nodes[c].x += dx;
            nodes[c].y += dy;
            shift_subtree(nodes, c, dx, dy);
        }
    }

    fn read_node_attr(n: &LayoutNode, attr: NormAttr, margin: f64) -> f64 {
        let base = match attr {
            NormAttr::Left => n.x,
            NormAttr::Right => n.x + n.w,
            NormAttr::Top => n.y,
            NormAttr::Bottom => n.y + n.h,
            NormAttr::Width => n.w,
            NormAttr::Height => n.h,
            NormAttr::CenterX => n.x + n.w * 0.5,
            NormAttr::CenterY => n.y + n.h * 0.5,
        };
        base + margin
    }

    fn apply_node_attr(nodes: &mut [LayoutNode], idx: usize, attr: NormAttr, margin: f64, raw_val: f64) {
        let val = raw_val - margin;
        if !val.is_finite() {
            return;
        }
        let old_x = nodes[idx].x;
        let old_y = nodes[idx].y;
        match attr {
            NormAttr::Width => {
                nodes[idx].w = val.max(0.0);
            }
            NormAttr::Height => {
                nodes[idx].h = val.max(0.0);
            }
            NormAttr::Left => {
                if nodes[idx].has_right && !nodes[idx].has_width {
                    let right = nodes[idx].x + nodes[idx].w;
                    nodes[idx].x = val;
                    nodes[idx].w = (right - val).max(0.0);
                } else {
                    nodes[idx].x = val;
                }
            }
            NormAttr::Right => {
                if nodes[idx].has_left && !nodes[idx].has_width {
                    nodes[idx].w = (val - nodes[idx].x).max(0.0);
                } else {
                    nodes[idx].x = val - nodes[idx].w;
                }
            }
            NormAttr::CenterX => {
                nodes[idx].x = val - nodes[idx].w * 0.5;
            }
            NormAttr::Top => {
                if nodes[idx].has_bottom && !nodes[idx].has_height {
                    let bottom = nodes[idx].y + nodes[idx].h;
                    nodes[idx].y = val;
                    nodes[idx].h = (bottom - val).max(0.0);
                } else {
                    nodes[idx].y = val;
                }
            }
            NormAttr::Bottom => {
                if nodes[idx].has_top && !nodes[idx].has_height {
                    nodes[idx].h = (val - nodes[idx].y).max(0.0);
                } else {
                    nodes[idx].y = val - nodes[idx].h;
                }
            }
            NormAttr::CenterY => {
                nodes[idx].y = val - nodes[idx].h * 0.5;
            }
        }
        let dx = nodes[idx].x - old_x;
        let dy = nodes[idx].y - old_y;
        shift_subtree(nodes, idx, dx, dy);
    }

    fn is_ancestor(nodes: &[LayoutNode], maybe_anc: usize, mut cur: usize) -> bool {
        while let Some(p) = nodes[cur].parent {
            if p == maybe_anc {
                return true;
            }
            cur = p;
        }
        false
    }

    unsafe fn subtree_has_constraints(v: Id) -> bool {
        let cs = assoc_get(v, &VIEW_CONSTRAINTS_KEY);
        if !cs.is_null() && nsarray_count(cs) > 0 {
            return true;
        }
        let subs = assoc_get(v, &VIEW_SUBVIEWS_KEY);
        if !subs.is_null() {
            for i in 0..nsarray_count(subs) {
                if subtree_has_constraints(nsarray_object_at(subs, i)) {
                    return true;
                }
            }
        }
        false
    }

    pub(crate) unsafe fn solve_autolayout_subtree(start_view: Id) {
        if start_view.is_null() {
            return;
        }
        let mut root = start_view;
        loop {
            let sv = msg0::<Id>(root, "superview");
            if sv.is_null() || !get_view_flags(sv).needs_layout {
                break;
            }
            root = sv;
        }
        if !subtree_has_constraints(root) {
            return;
        }
        if SOLVING_LAYOUT
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        struct SolveGuard;
        impl Drop for SolveGuard {
            fn drop(&mut self) {
                SOLVING_LAYOUT.store(false, Ordering::Release);
            }
        }
        let _guard = SolveGuard;

        let mut nodes: Vec<LayoutNode> = Vec::new();
        let mut view_map: HashMap<usize, usize> = HashMap::new();

        let root_frame: CGRect = msg0(root, "frame");
        let root_bounds: CGRect = msg0(root, "bounds");
        nodes.push(LayoutNode {
            view: root,
            parent: None,
            children: Vec::new(),
            init_frame: root_frame,
            x: 0.0,
            y: 0.0,
            w: root_bounds.size.width,
            h: root_bounds.size.height,
            is_window: is_kind_of(root, "UIWindow"),
            constrained: false,
            has_left: false,
            has_right: false,
            has_center_x: false,
            has_width: false,
            has_top: false,
            has_bottom: false,
            has_center_y: false,
            has_height: false,
            intrinsic_w: -1.0,
            intrinsic_h: -1.0,
            is_multiline_label: false,
        });
        view_map.insert(root as usize, 0);

        let mut head = 0;
        while head < nodes.len() {
            let parent_view = nodes[head].view;
            let px = nodes[head].x;
            let py = nodes[head].y;
            let subs = assoc_get(parent_view, &VIEW_SUBVIEWS_KEY);
            if !subs.is_null() {
                for i in 0..nsarray_count(subs) {
                    let sub = nsarray_object_at(subs, i);
                    if sub.is_null() || view_map.contains_key(&(sub as usize)) {
                        continue;
                    }
                    let f: CGRect = msg0(sub, "frame");
                    let idx = nodes.len();
                    nodes.push(LayoutNode {
                        view: sub,
                        parent: Some(head),
                        children: Vec::new(),
                        init_frame: f,
                        x: px + f.origin.x,
                        y: py + f.origin.y,
                        w: f.size.width,
                        h: f.size.height,
                        is_window: is_kind_of(sub, "UIWindow"),
                        constrained: false,
                        has_left: false,
                        has_right: false,
                        has_center_x: false,
                        has_width: false,
                        has_top: false,
                        has_bottom: false,
                        has_center_y: false,
                        has_height: false,
                        intrinsic_w: -1.0,
                        intrinsic_h: -1.0,
                        is_multiline_label: is_kind_of(sub, "UILabel")
                            && msg0::<isize>(sub, "numberOfLines") == 0,
                    });
                    nodes[head].children.push(idx);
                    view_map.insert(sub as usize, idx);
                }
            }
            head += 1;
        }

        let mut seen_constraints: HashMap<usize, ()> = HashMap::new();
        let mut constraints: Vec<ResolvedConstraint> = Vec::new();
        for idx in 0..nodes.len() {
            let cs = assoc_get(nodes[idx].view, &VIEW_CONSTRAINTS_KEY);
            if cs.is_null() {
                continue;
            }
            for i in 0..nsarray_count(cs) {
                let c = nsarray_object_at(cs, i);
                if c.is_null()
                    || !is_kind_of(c, "PearLayoutConstraint")
                    || seen_constraints.insert(c as usize, ()).is_some()
                {
                    continue;
                }
                let d: ConstraintData = get_pod(c, &PLC_DATA_KEY);
                if !d.active {
                    continue;
                }
                let v1 = resolve_layout_view(pear_objc::assoc_get_weak(c, &PLC_FIRST_KEY));
                let Some(&idx1) = view_map.get(&(v1 as usize)) else {
                    continue;
                };
                let Some((attr1, margin1)) = normalize_attr(d.first_attr) else {
                    continue;
                };
                let item2 = pear_objc::assoc_get_weak(c, &PLC_SECOND_KEY);
                let (idx2, attr2, margin2) = if item2.is_null() || d.second_attr == 0 {
                    (None, None, 0.0)
                } else {
                    let v2 = resolve_layout_view(item2);
                    let Some(&i2) = view_map.get(&(v2 as usize)) else {
                        continue;
                    };
                    let Some((a2, m2)) = normalize_attr(d.second_attr) else {
                        continue;
                    };
                    (Some(i2), Some(a2), m2)
                };
                if d.multiplier == 0.0 && idx2.is_some() {
                    continue;
                }
                let n1 = &mut nodes[idx1];
                n1.constrained = true;
                match attr1 {
                    NormAttr::Left => n1.has_left = true,
                    NormAttr::Right => n1.has_right = true,
                    NormAttr::CenterX => n1.has_center_x = true,
                    NormAttr::Width => n1.has_width = true,
                    NormAttr::Top => n1.has_top = true,
                    NormAttr::Bottom => n1.has_bottom = true,
                    NormAttr::CenterY => n1.has_center_y = true,
                    NormAttr::Height => n1.has_height = true,
                }
                let seq = constraints.len();
                constraints.push(ResolvedConstraint {
                    idx1,
                    attr1,
                    margin1,
                    idx2,
                    attr2,
                    margin2,
                    relation: d.relation,
                    multiplier: d.multiplier,
                    constant: d.constant,
                    priority: d.priority,
                    seq,
                });
            }
        }

        if constraints.is_empty() {
            return;
        }

        // Seed intrinsic sizes for constrained views.
        for n in &mut nodes {
            if !n.constrained || n.is_window {
                continue;
            }
            if responds_to(n.view, "intrinsicContentSize") {
                let sz: CGSize = msg0(n.view, "intrinsicContentSize");
                if sz.width.is_finite() && sz.width > 0.0 {
                    n.intrinsic_w = sz.width;
                    if !n.has_width && !(n.has_left && n.has_right) && n.w <= 0.01 {
                        n.w = sz.width;
                    }
                }
                if sz.height.is_finite() && sz.height > 0.0 {
                    n.intrinsic_h = sz.height;
                    if !n.has_height && !(n.has_top && n.has_bottom) && n.h <= 0.01 {
                        n.h = sz.height;
                    }
                }
            }
        }

        constraints.sort_by(|a, b| {
            a.priority
                .partial_cmp(&b.priority)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.seq.cmp(&b.seq))
        });

        for _ in 0..16 {
            for c in &constraints {
                let target = match (c.idx2, c.attr2) {
                    (Some(i2), Some(a2)) => read_node_attr(&nodes[i2], a2, c.margin2) * c.multiplier + c.constant,
                    _ => c.constant,
                };
                if !target.is_finite() {
                    continue;
                }
                let cur = read_node_attr(&nodes[c.idx1], c.attr1, c.margin1);
                let violated = match c.relation {
                    -1 => cur > target + 0.01,
                    1 => cur < target - 0.01,
                    _ => (cur - target).abs() > 0.01,
                };
                if !violated {
                    continue;
                }

                // If a child's bottom/right pins to an unconstrained wrapping parent's bottom/right,
                // grow/size the wrapping parent instead of squashing the child.
                if let (Some(i2), Some(a2)) = (c.idx2, c.attr2) {
                    if is_ancestor(&nodes, i2, c.idx1) && !nodes[i2].is_window {
                        if c.attr1 == NormAttr::Bottom
                            && a2 == NormAttr::Bottom
                            && !nodes[i2].has_height
                            && !(nodes[i2].has_top && nodes[i2].has_bottom)
                            && (nodes[c.idx1].has_top && (nodes[c.idx1].has_height || nodes[c.idx1].intrinsic_h > 0.0))
                            && c.multiplier != 0.0
                        {
                            let desired_bottom2 = (cur - c.constant) / c.multiplier - c.margin2;
                            if desired_bottom2.is_finite() {
                                nodes[i2].h = (desired_bottom2 - nodes[i2].y).max(0.0);
                            }
                            continue;
                        }
                        if c.attr1 == NormAttr::Right
                            && a2 == NormAttr::Right
                            && !nodes[i2].has_width
                            && !(nodes[i2].has_left && nodes[i2].has_right)
                            && (nodes[c.idx1].has_left && (nodes[c.idx1].has_width || nodes[c.idx1].intrinsic_w > 0.0))
                            && c.multiplier != 0.0
                        {
                            let desired_right2 = (cur - c.constant) / c.multiplier - c.margin2;
                            if desired_right2.is_finite() {
                                nodes[i2].w = (desired_right2 - nodes[i2].x).max(0.0);
                            }
                            continue;
                        }
                    }
                }

                apply_node_attr(&mut nodes, c.idx1, c.attr1, c.margin1, target);
            }

            // Re-evaluate multiline UILabel intrinsic heights after width settles.
            for n in &mut nodes {
                if n.is_multiline_label
                    && n.constrained
                    && !n.has_height
                    && !(n.has_top && n.has_bottom)
                    && n.w > 1.0
                {
                    let fit: CGSize = msg1(n.view, "sizeThatFits:", CGSize { width: n.w, height: 1.0e6 });
                    if fit.height.is_finite() && fit.height > 0.0 {
                        n.h = fit.height;
                    }
                }
            }
        }

        for i in 0..nodes.len() {
            if nodes[i].is_window || !nodes[i].constrained {
                continue;
            }
            if i == 0 && !nodes[i].has_width && !nodes[i].has_height {
                continue;
            }
            let (px, py) = match nodes[i].parent {
                Some(p) => (nodes[p].x, nodes[p].y),
                None => (nodes[i].x - nodes[i].init_frame.origin.x, nodes[i].y - nodes[i].init_frame.origin.y),
            };
            let nx = nodes[i].x - px;
            let ny = nodes[i].y - py;
            let nw = nodes[i].w;
            let nh = nodes[i].h;
            if !nx.is_finite() || !ny.is_finite() || !nw.is_finite() || !nh.is_finite() {
                continue;
            }
            let f = nodes[i].init_frame;
            if (nx - f.origin.x).abs() > 0.01
                || (ny - f.origin.y).abs() > 0.01
                || (nw - f.size.width).abs() > 0.01
                || (nh - f.size.height).abs() > 0.01
            {
                let new_frame = CGRect {
                    origin: CGPoint { x: nx, y: ny },
                    size: CGSize { width: nw.max(0.0), height: nh.max(0.0) },
                };
                msg1::<CGRect, ()>(nodes[i].view, "setFrame:", new_frame);
            }
        }
    }

    unsafe extern "C" fn plc_first(s: Id, _c: Sel) -> Id { pear_objc::assoc_get_weak(s, &PLC_FIRST_KEY) }
    unsafe extern "C" fn plc_second(s: Id, _c: Sel) -> Id { pear_objc::assoc_get_weak(s, &PLC_SECOND_KEY) }
    unsafe extern "C" fn plc_first_attr(s: Id, _c: Sel) -> isize { get_pod::<ConstraintData>(s, &PLC_DATA_KEY).first_attr }
    unsafe extern "C" fn plc_second_attr(s: Id, _c: Sel) -> isize { get_pod::<ConstraintData>(s, &PLC_DATA_KEY).second_attr }
    unsafe extern "C" fn plc_relation(s: Id, _c: Sel) -> isize { get_pod::<ConstraintData>(s, &PLC_DATA_KEY).relation }
    unsafe extern "C" fn plc_multiplier(s: Id, _c: Sel) -> f64 { get_pod::<ConstraintData>(s, &PLC_DATA_KEY).multiplier }
    unsafe extern "C" fn plc_constant(s: Id, _c: Sel) -> f64 { get_pod::<ConstraintData>(s, &PLC_DATA_KEY).constant }
    unsafe extern "C" fn plc_set_constant(s: Id, _c: Sel, v: f64) {
        let mut d: ConstraintData = get_pod(s, &PLC_DATA_KEY);
        d.constant = v;
        set_pod(s, &PLC_DATA_KEY, d);
        invalidate_constraint_layout(s);
    }
    unsafe extern "C" fn plc_priority(s: Id, _c: Sel) -> f32 { get_pod::<ConstraintData>(s, &PLC_DATA_KEY).priority }
    unsafe extern "C" fn plc_set_priority(s: Id, _c: Sel, v: f32) {
        let mut d: ConstraintData = get_pod(s, &PLC_DATA_KEY);
        d.priority = v;
        set_pod(s, &PLC_DATA_KEY, d);
        invalidate_constraint_layout(s);
    }
    unsafe extern "C" fn plc_active(s: Id, _c: Sel) -> bool { get_pod::<ConstraintData>(s, &PLC_DATA_KEY).active }
    unsafe extern "C" fn plc_set_active(s: Id, _c: Sel, v: bool) {
        let mut d: ConstraintData = get_pod(s, &PLC_DATA_KEY);
        if v {
            d.active = true;
            set_pod(s, &PLC_DATA_KEY, d);
            let v1 = resolve_layout_view(pear_objc::assoc_get_weak(s, &PLC_FIRST_KEY));
            let v2 = resolve_layout_view(pear_objc::assoc_get_weak(s, &PLC_SECOND_KEY));
            let owner = common_ancestor_view(v1, v2);
            if !owner.is_null() {
                let old_owner = pear_objc::assoc_get_weak(s, &PLC_OWNER_KEY);
                if !old_owner.is_null() && old_owner != owner {
                    let old_arr = assoc_get(old_owner, &VIEW_CONSTRAINTS_KEY);
                    if !old_arr.is_null() {
                        msg1::<Id, ()>(old_arr, "removeObjectIdenticalTo:", s);
                    }
                }
                view_add_constraint(owner, sel("addConstraint:"), s);
            } else {
                invalidate_constraint_layout(s);
            }
        } else {
            d.active = false;
            set_pod(s, &PLC_DATA_KEY, d);
            let old_owner = pear_objc::assoc_get_weak(s, &PLC_OWNER_KEY);
            if !old_owner.is_null() {
                view_remove_constraint(old_owner, sel("removeConstraint:"), s);
            }
        }
    }
    unsafe extern "C" fn plc_auto_install(s: Id, cmd: Sel) {
        // Preserve the app's PureLayout category, including batched installs.
        let method = class_getInstanceMethod(cls("NSLayoutConstraint"), cmd);
        if !method.is_null() {
            let install: unsafe extern "C" fn(Id, Sel) = std::mem::transmute(method_getImplementation(method));
            install(s, cmd);
        } else {
            plc_set_active(s, sel("setActive:"), true);
        }
    }
    unsafe extern "C" fn plc_ident(s: Id, _c: Sel) -> Id { assoc_get(s, &PLC_IDENT_KEY) }
    unsafe extern "C" fn plc_set_ident(s: Id, _c: Sel, v: Id) { set_copied(s, &PLC_IDENT_KEY, v); }

    unsafe extern "C" fn nslc_with_item(
        cls_obj: Id,
        cmd: Sel,
        v1: Id,
        a1: isize,
        rel: isize,
        v2: Id,
        a2: isize,
        mul: f64,
        c: f64,
    ) -> Id {
        if is_pear_layout_item(v1) || is_pear_layout_item(v2) {
            let obj: Id = msg0(cls("PearLayoutConstraint"), "new");
            if obj.is_null() {
                return NIL;
            }
            pear_objc::assoc_set_weak(obj, &PLC_FIRST_KEY, v1);
            pear_objc::assoc_set_weak(obj, &PLC_SECOND_KEY, v2);
            set_pod(
                obj,
                &PLC_DATA_KEY,
                ConstraintData {
                    first_attr: a1,
                    second_attr: a2,
                    relation: rel,
                    multiplier: mul,
                    constant: c,
                    priority: 1000.0,
                    active: false,
                },
            );
            return objc_autorelease(obj);
        }
        let orig = ORIG_WITH_ITEM.load(Ordering::Acquire);
        if orig != 0 {
            let f: unsafe extern "C" fn(Id, Sel, Id, isize, isize, Id, isize, f64, f64) -> Id =
                std::mem::transmute(orig);
            return f(cls_obj, cmd, v1, a1, rel, v2, a2, mul, c);
        }
        NIL
    }

    unsafe fn toggle_constraints(cls_obj: Id, cmd: Sel, arr: Id, activate: bool, slot: &AtomicUsize) {
        if arr.is_null() {
            return;
        }
        let mut rest = Vec::new();
        for i in 0..nsarray_count(arr) {
            let item = nsarray_object_at(arr, i);
            if is_kind_of(item, "PearLayoutConstraint") {
                plc_set_active(item, sel("setActive:"), activate);
            } else if !item.is_null() {
                rest.push(item);
            }
        }
        if !rest.is_empty() {
            let orig = slot.load(Ordering::Acquire);
            if orig != 0 {
                let f: unsafe extern "C" fn(Id, Sel, Id) = std::mem::transmute(orig);
                f(cls_obj, cmd, nsarray_from_ids(&rest));
            }
        }
    }

    unsafe extern "C" fn nslc_activate(cls_obj: Id, cmd: Sel, arr: Id) {
        toggle_constraints(cls_obj, cmd, arr, true, &ORIG_ACTIVATE);
    }
    unsafe extern "C" fn nslc_deactivate(cls_obj: Id, cmd: Sel, arr: Id) {
        toggle_constraints(cls_obj, cmd, arr, false, &ORIG_DEACTIVATE);
    }
    unsafe extern "C" fn nslc_vfl(
        cls_obj: Id,
        cmd: Sel,
        fmt: Id,
        opts: usize,
        metrics: Id,
        views: Id,
    ) -> Id {
        if !views.is_null() && is_kind_of(views, "NSDictionary") {
            let vals: Id = msg0(views, "allValues");
            for i in 0..nsarray_count(vals) {
                if is_pear_layout_item(nsarray_object_at(vals, i)) {
                    return msg0(cls("NSArray"), "array");
                }
            }
        }
        let orig = ORIG_VFL.load(Ordering::Acquire);
        if orig != 0 {
            let f: unsafe extern "C" fn(Id, Sel, Id, usize, Id, Id) -> Id = std::mem::transmute(orig);
            return f(cls_obj, cmd, fmt, opts, metrics, views);
        }
        msg0(cls("NSArray"), "array")
    }

    unsafe fn replace_class_hook(meta: Class, name: &str, imp: Imp, types: &str, slot: &AtomicUsize) {
        if slot.load(Ordering::Acquire) != 0 {
            return;
        }
        let prev = class_getMethodImplementation(meta, sel(name));
        let replaced = add_method(meta, name, imp, types);
        let orig = if !replaced.is_null() { replaced } else { prev };
        slot.store(orig as usize, Ordering::Release);
    }

    // ---- UIControl ----
    #[repr(C)]
    #[derive(Copy, Clone)]
    struct ControlFlags {
        enabled: bool,
        selected: bool,
        highlighted: bool,
        h_align: isize,
        v_align: isize,
    }
    impl Default for ControlFlags {
        fn default() -> Self {
            Self {
                enabled: true,
                selected: false,
                highlighted: false,
                h_align: 0,
                v_align: 0,
            }
        }
    }

    #[repr(C)]
    #[derive(Copy, Clone, PartialEq, Eq)]
    struct ControlTarget {
        target: usize,
        action: usize,
        events: usize,
    }

    static CTL_FLAGS_KEY: u8 = 0;
    static CTL_TARGETS_KEY: u8 = 0;

    unsafe fn get_ctl_targets(c: Id) -> Vec<ControlTarget> {
        let sz = std::mem::size_of::<ControlTarget>();
        if let Some(bytes) = nsdata_to_vec(assoc_get(c, &CTL_TARGETS_KEY)) {
            return bytes
                .chunks_exact(sz)
                .map(|ch| std::ptr::read_unaligned(ch.as_ptr() as *const ControlTarget))
                .collect();
        }
        Vec::new()
    }
    unsafe fn set_ctl_targets(c: Id, targets: &[ControlTarget]) {
        let sz = std::mem::size_of::<ControlTarget>();
        let bytes = std::slice::from_raw_parts(targets.as_ptr() as *const u8, targets.len() * sz);
        assoc_set(c, &CTL_TARGETS_KEY, nsdata_from_bytes(bytes));
    }

    unsafe extern "C" fn ctl_add_target(s: Id, _c: Sel, target: Id, action: Sel, events: usize) {
        if action.is_null() || events == 0 {
            return;
        }
        let t = target as usize;
        let a = action as usize;
        let mut list = get_ctl_targets(s);
        if let Some(item) = list.iter_mut().find(|p| p.target == t && p.action == a) {
            item.events |= events;
        } else {
            list.push(ControlTarget { target: t, action: a, events });
        }
        set_ctl_targets(s, &list);
    }
    unsafe extern "C" fn ctl_remove_target(s: Id, _c: Sel, target: Id, action: Sel, events: usize) {
        let t = target as usize;
        let a = action as usize;
        let mut list = get_ctl_targets(s);
        list.retain_mut(|p| {
            let match_t = t == 0 || p.target == t;
            let match_a = a == 0 || p.action == a;
            if match_t && match_a {
                p.events &= !events;
                p.events != 0
            } else {
                true
            }
        });
        set_ctl_targets(s, &list);
    }
    unsafe extern "C" fn ctl_all_targets(s: Id, _c: Sel) -> Id {
        let ids: Vec<Id> = get_ctl_targets(s)
            .into_iter()
            .filter_map(|p| (p.target != 0).then_some(p.target as Id))
            .collect();
        msg1(cls("NSSet"), "setWithArray:", nsarray_from_ids(&ids))
    }
    unsafe extern "C" fn ctl_all_events(s: Id, _c: Sel) -> usize {
        get_ctl_targets(s).into_iter().fold(0, |acc, p| acc | p.events)
    }
    unsafe extern "C" fn ctl_actions_for_target(s: Id, _c: Sel, target: Id, event: usize) -> Id {
        let t = target as usize;
        let names: Vec<Id> = get_ctl_targets(s)
            .into_iter()
            .filter(|p| p.target == t && (p.events & event) != 0 && p.action != 0)
            .filter_map(|p| {
                let ptr = sel_getName(p.action as Sel);
                (!ptr.is_null()).then(|| nsstring(&CStr::from_ptr(ptr).to_string_lossy()))
            })
            .collect();
        nsarray_from_ids(&names)
    }
    unsafe extern "C" fn ctl_send_actions(s: Id, _c: Sel, events: usize) {
        let list = get_ctl_targets(s);
        for p in list {
            if (p.events & events) == 0 || p.target == 0 || p.action == 0 {
                continue;
            }
            let target = p.target as Id;
            let action = p.action as Sel;
            if class_respondsToSelector(object_getClass(target), action) {
                let f: unsafe extern "C" fn(Id, Sel, Id, Id) =
                    std::mem::transmute(objc_msgSend as *const c_void);
                f(target, action, s, NIL);
            }
        }
    }
    unsafe extern "C" fn ctl_enabled(s: Id, _c: Sel) -> bool { get_pod::<ControlFlags>(s, &CTL_FLAGS_KEY).enabled }
    unsafe extern "C" fn ctl_set_enabled(s: Id, _c: Sel, v: bool) {
        let mut f: ControlFlags = get_pod(s, &CTL_FLAGS_KEY);
        if f.enabled != v {
            f.enabled = v;
            set_pod(s, &CTL_FLAGS_KEY, f);
            msg0::<()>(s, "setNeedsLayout");
        }
    }
    unsafe extern "C" fn ctl_selected(s: Id, _c: Sel) -> bool { get_pod::<ControlFlags>(s, &CTL_FLAGS_KEY).selected }
    unsafe extern "C" fn ctl_set_selected(s: Id, _c: Sel, v: bool) {
        let mut f: ControlFlags = get_pod(s, &CTL_FLAGS_KEY);
        if f.selected != v {
            f.selected = v;
            set_pod(s, &CTL_FLAGS_KEY, f);
            msg0::<()>(s, "setNeedsLayout");
        }
    }
    unsafe extern "C" fn ctl_highlighted(s: Id, _c: Sel) -> bool { get_pod::<ControlFlags>(s, &CTL_FLAGS_KEY).highlighted }
    unsafe extern "C" fn ctl_set_highlighted(s: Id, _c: Sel, v: bool) {
        let mut f: ControlFlags = get_pod(s, &CTL_FLAGS_KEY);
        if f.highlighted != v {
            f.highlighted = v;
            set_pod(s, &CTL_FLAGS_KEY, f);
            msg0::<()>(s, "setNeedsLayout");
        }
    }
    unsafe extern "C" fn ctl_h_align(s: Id, _c: Sel) -> isize { get_pod::<ControlFlags>(s, &CTL_FLAGS_KEY).h_align }
    unsafe extern "C" fn ctl_set_h_align(s: Id, _c: Sel, v: isize) {
        let mut f: ControlFlags = get_pod(s, &CTL_FLAGS_KEY);
        f.h_align = v;
        set_pod(s, &CTL_FLAGS_KEY, f);
    }
    unsafe extern "C" fn ctl_v_align(s: Id, _c: Sel) -> isize { get_pod::<ControlFlags>(s, &CTL_FLAGS_KEY).v_align }
    unsafe extern "C" fn ctl_set_v_align(s: Id, _c: Sel, v: isize) {
        let mut f: ControlFlags = get_pod(s, &CTL_FLAGS_KEY);
        f.v_align = v;
        set_pod(s, &CTL_FLAGS_KEY, f);
    }

    static BUTTON_IMAGES_KEY: u8 = 0;
    static BUTTON_IMAGE_VIEW_KEY: u8 = 0;
    static BUTTON_IMAGE_INSETS_KEY: u8 = 0;
    static BUTTON_TITLES_KEY: u8 = 0;
    static BUTTON_ATTR_TITLES_KEY: u8 = 0;
    static BUTTON_TITLE_COLORS_KEY: u8 = 0;
    static BUTTON_BG_IMAGES_KEY: u8 = 0;
    static BUTTON_TITLE_LABEL_KEY: u8 = 0;
    static BUTTON_BG_IMAGE_VIEW_KEY: u8 = 0;
    static BUTTON_TITLE_INSETS_KEY: u8 = 0;
    static BUTTON_CONTENT_INSETS_KEY: u8 = 0;
    static BUTTON_TYPE_KEY: u8 = 0;

    unsafe fn state_dict_get(obj: Id, dict_key: &u8, state: usize) -> Id {
        let dict = assoc_get(obj, dict_key);
        if dict.is_null() {
            return NIL;
        }
        msg1(dict, "objectForKey:", nsnumber_i64(state as i64))
    }

    unsafe fn state_dict_get_resolved(obj: Id, dict_key: &u8, state: usize) -> Id {
        let v = state_dict_get(obj, dict_key, state);
        if v.is_null() && state != 0 {
            state_dict_get(obj, dict_key, 0)
        } else {
            v
        }
    }

    unsafe fn state_dict_set(obj: Id, dict_key: &u8, val: Id, state: usize) {
        let mut dict = assoc_get(obj, dict_key);
        if dict.is_null() {
            dict = msg0(cls("NSMutableDictionary"), "dictionary");
            assoc_set(obj, dict_key, dict);
        }
        let key = nsnumber_i64(state as i64);
        if val.is_null() {
            msg1::<Id, ()>(dict, "removeObjectForKey:", key);
        } else {
            msg2::<Id, Id, ()>(dict, "setObject:forKey:", val, key);
        }
        msg0::<()>(obj, "setNeedsLayout");
    }

    unsafe fn button_current_state(s: Id) -> usize {
        let flags: ControlFlags = get_pod(s, &CTL_FLAGS_KEY);
        flags.highlighted as usize | ((!flags.enabled as usize) << 1) | ((flags.selected as usize) << 2)
    }

    unsafe extern "C" fn button_with_type(cls_obj: Id, _c: Sel, btype: isize) -> Id {
        let btn: Id = msg1(msg0(cls_obj, "alloc"), "initWithFrame:", CGRect::default());
        if !btn.is_null() {
            pear_objc::assoc_set_i64(btn, &BUTTON_TYPE_KEY, btype as i64);
            objc_autorelease(btn)
        } else {
            NIL
        }
    }

    unsafe extern "C" fn button_type(s: Id, _c: Sel) -> isize {
        pear_objc::assoc_i64(s, &BUTTON_TYPE_KEY, 0) as isize
    }

    unsafe extern "C" fn button_image(s: Id, _c: Sel, state: usize) -> Id {
        state_dict_get(s, &BUTTON_IMAGES_KEY, state)
    }

    unsafe extern "C" fn button_set_image(s: Id, _c: Sel, image: Id, state: usize) {
        state_dict_set(s, &BUTTON_IMAGES_KEY, image, state);
    }

    unsafe extern "C" fn button_current_image(s: Id, _c: Sel) -> Id {
        state_dict_get_resolved(s, &BUTTON_IMAGES_KEY, button_current_state(s))
    }

    unsafe extern "C" fn button_bg_image(s: Id, _c: Sel, state: usize) -> Id {
        state_dict_get(s, &BUTTON_BG_IMAGES_KEY, state)
    }

    unsafe extern "C" fn button_set_bg_image(s: Id, _c: Sel, image: Id, state: usize) {
        state_dict_set(s, &BUTTON_BG_IMAGES_KEY, image, state);
    }

    unsafe extern "C" fn button_current_bg_image(s: Id, _c: Sel) -> Id {
        state_dict_get_resolved(s, &BUTTON_BG_IMAGES_KEY, button_current_state(s))
    }

    unsafe extern "C" fn button_title(s: Id, _c: Sel, state: usize) -> Id {
        state_dict_get(s, &BUTTON_TITLES_KEY, state)
    }

    unsafe extern "C" fn button_set_title(s: Id, _c: Sel, title: Id, state: usize) {
        let copy = if title.is_null() { NIL } else { msg0::<Id>(title, "copy") };
        state_dict_set(s, &BUTTON_TITLES_KEY, copy, state);
        if !copy.is_null() {
            objc_release(copy);
        }
        // Setting a plain title clears any attributed title for that state.
        state_dict_set(s, &BUTTON_ATTR_TITLES_KEY, NIL, state);
    }

    unsafe extern "C" fn button_current_title(s: Id, _c: Sel) -> Id {
        let state = button_current_state(s);
        let t = state_dict_get_resolved(s, &BUTTON_TITLES_KEY, state);
        if !t.is_null() {
            return t;
        }
        let attr = state_dict_get_resolved(s, &BUTTON_ATTR_TITLES_KEY, state);
        if attr.is_null() { NIL } else { msg0(attr, "string") }
    }

    unsafe extern "C" fn button_attr_title(s: Id, _c: Sel, state: usize) -> Id {
        state_dict_get(s, &BUTTON_ATTR_TITLES_KEY, state)
    }

    unsafe extern "C" fn button_set_attr_title(s: Id, _c: Sel, attr: Id, state: usize) {
        let copy = if attr.is_null() { NIL } else { msg0::<Id>(attr, "copy") };
        state_dict_set(s, &BUTTON_ATTR_TITLES_KEY, copy, state);
        if !copy.is_null() {
            objc_release(copy);
        }
    }

    unsafe extern "C" fn button_current_attr_title(s: Id, _c: Sel) -> Id {
        state_dict_get_resolved(s, &BUTTON_ATTR_TITLES_KEY, button_current_state(s))
    }

    unsafe extern "C" fn button_title_color(s: Id, _c: Sel, state: usize) -> Id {
        state_dict_get(s, &BUTTON_TITLE_COLORS_KEY, state)
    }

    unsafe extern "C" fn button_set_title_color(s: Id, _c: Sel, color: Id, state: usize) {
        state_dict_set(s, &BUTTON_TITLE_COLORS_KEY, color, state);
    }

    unsafe extern "C" fn button_current_title_color(s: Id, _c: Sel) -> Id {
        let c = state_dict_get_resolved(s, &BUTTON_TITLE_COLORS_KEY, button_current_state(s));
        if !c.is_null() {
            c
        } else {
            msg0(cls("UIColor"), "blackColor")
        }
    }

    unsafe fn button_ensure_image_view(s: Id) -> Id {
        let mut view = assoc_get(s, &BUTTON_IMAGE_VIEW_KEY);
        if view.is_null() {
            view = msg0(cls("UIImageView"), "new");
            msg1::<isize, ()>(view, "setContentMode:", 4); // center
            msg1::<bool, ()>(view, "setUserInteractionEnabled:", false);
            assoc_set(s, &BUTTON_IMAGE_VIEW_KEY, view);
            msg1::<Id, ()>(s, "addSubview:", view);
            objc_release(view);
        }
        view
    }

    unsafe extern "C" fn button_image_view(s: Id, _c: Sel) -> Id {
        button_ensure_image_view(s)
    }

    unsafe extern "C" fn button_title_label(s: Id, _c: Sel) -> Id {
        let mut lbl = assoc_get(s, &BUTTON_TITLE_LABEL_KEY);
        if lbl.is_null() {
            lbl = msg0(cls("UILabel"), "new");
            msg1::<bool, ()>(lbl, "setUserInteractionEnabled:", false);
            msg1::<isize, ()>(lbl, "setTextAlignment:", 1); // center
            let black: Id = msg0(cls("UIColor"), "blackColor");
            msg1::<Id, ()>(lbl, "setTextColor:", black);
            assoc_set(s, &BUTTON_TITLE_LABEL_KEY, lbl);
            msg1::<Id, ()>(s, "addSubview:", lbl);
            objc_release(lbl);
        }
        lbl
    }

    unsafe extern "C" fn button_image_insets(s: Id, _c: Sel) -> UIEdgeInsets {
        get_pod(s, &BUTTON_IMAGE_INSETS_KEY)
    }

    unsafe extern "C" fn button_set_image_insets(s: Id, _c: Sel, insets: UIEdgeInsets) {
        set_pod(s, &BUTTON_IMAGE_INSETS_KEY, insets);
        msg0::<()>(s, "setNeedsLayout");
    }

    unsafe extern "C" fn button_title_insets(s: Id, _c: Sel) -> UIEdgeInsets {
        get_pod(s, &BUTTON_TITLE_INSETS_KEY)
    }

    unsafe extern "C" fn button_set_title_insets(s: Id, _c: Sel, insets: UIEdgeInsets) {
        set_pod(s, &BUTTON_TITLE_INSETS_KEY, insets);
        msg0::<()>(s, "setNeedsLayout");
    }

    unsafe extern "C" fn button_content_insets(s: Id, _c: Sel) -> UIEdgeInsets {
        get_pod(s, &BUTTON_CONTENT_INSETS_KEY)
    }

    unsafe extern "C" fn button_set_content_insets(s: Id, _c: Sel, insets: UIEdgeInsets) {
        set_pod(s, &BUTTON_CONTENT_INSETS_KEY, insets);
        msg0::<()>(s, "setNeedsLayout");
    }

    unsafe fn button_sync_title_label(s: Id) -> (Id, bool) {
        let state = button_current_state(s);
        let attr_title = state_dict_get_resolved(s, &BUTTON_ATTR_TITLES_KEY, state);
        let title = state_dict_get_resolved(s, &BUTTON_TITLES_KEY, state);
        let has_title = (!attr_title.is_null() && msg0::<usize>(attr_title, "length") > 0)
            || (!title.is_null() && msg0::<usize>(title, "length") > 0);
        let existing_lbl = assoc_get(s, &BUTTON_TITLE_LABEL_KEY);
        if !has_title && existing_lbl.is_null() {
            return (NIL, false);
        }
        let lbl = button_title_label(s, sel("titleLabel"));
        let color = state_dict_get_resolved(s, &BUTTON_TITLE_COLORS_KEY, state);
        if !color.is_null() {
            msg1::<Id, ()>(lbl, "setTextColor:", color);
        } else {
            let colors_dict = assoc_get(s, &BUTTON_TITLE_COLORS_KEY);
            if !colors_dict.is_null() && msg0::<usize>(colors_dict, "count") > 0 {
                let black: Id = msg0(cls("UIColor"), "blackColor");
                msg1::<Id, ()>(lbl, "setTextColor:", black);
            }
        }
        if !attr_title.is_null() {
            msg1::<Id, ()>(lbl, "setAttributedText:", attr_title);
        } else {
            msg1::<Id, ()>(lbl, "setText:", title);
        }
        (lbl, has_title)
    }

    unsafe fn inset_rect(r: CGRect, i: UIEdgeInsets) -> CGRect {
        CGRect {
            origin: CGPoint { x: r.origin.x + i.left, y: r.origin.y + i.top },
            size: CGSize {
                width: (r.size.width - i.left - i.right).max(0.0),
                height: (r.size.height - i.top - i.bottom).max(0.0),
            },
        }
    }

    unsafe extern "C" fn button_size_that_fits(s: Id, _c: Sel, fit: CGSize) -> CGSize {
        let state = button_current_state(s);
        let image = state_dict_get_resolved(s, &BUTTON_IMAGES_KEY, state);
        let bg_image = state_dict_get_resolved(s, &BUTTON_BG_IMAGES_KEY, state);
        let (lbl, has_title) = button_sync_title_label(s);
        let img_sz: CGSize = if image.is_null() { CGSize::default() } else { msg0(image, "size") };
        let bg_sz: CGSize = if bg_image.is_null() { CGSize::default() } else { msg0(bg_image, "size") };
        let lbl_sz: CGSize = if has_title && !lbl.is_null() {
            let max_w = if fit.width > 0.0 { fit.width } else { 1.0e6 };
            msg1(lbl, "sizeThatFits:", CGSize { width: max_w, height: 1.0e6 })
        } else {
            CGSize::default()
        };
        let ci = button_content_insets(s, sel("contentEdgeInsets"));
        let content_w = if !image.is_null() && has_title {
            img_sz.width + lbl_sz.width
        } else {
            img_sz.width.max(lbl_sz.width)
        };
        let content_h = img_sz.height.max(lbl_sz.height);
        let w = (content_w + ci.left + ci.right).max(bg_sz.width);
        let h = (content_h + ci.top + ci.bottom).max(bg_sz.height);
        CGSize { width: w, height: h }
    }

    unsafe extern "C" fn button_intrinsic_size(s: Id, c: Sel) -> CGSize {
        button_size_that_fits(s, c, CGSize::default())
    }

    unsafe extern "C" fn button_layout(s: Id, _c: Sel) {
        let state = button_current_state(s);
        let bounds: CGRect = msg0(s, "bounds");

        // 1. Background image view at the very back.
        let bg_image = state_dict_get_resolved(s, &BUTTON_BG_IMAGES_KEY, state);
        let mut bg_view = assoc_get(s, &BUTTON_BG_IMAGE_VIEW_KEY);
        if !bg_image.is_null() {
            if bg_view.is_null() {
                bg_view = msg0(cls("UIImageView"), "new");
                msg1::<isize, ()>(bg_view, "setContentMode:", 0); // scaleToFill
                msg1::<bool, ()>(bg_view, "setUserInteractionEnabled:", false);
                assoc_set(s, &BUTTON_BG_IMAGE_VIEW_KEY, bg_view);
                msg2::<Id, isize, ()>(s, "insertSubview:atIndex:", bg_view, 0);
                objc_release(bg_view);
            }
            msg1::<CGRect, ()>(bg_view, "setFrame:", bounds);
            msg1::<Id, ()>(bg_view, "setImage:", bg_image);
            msg1::<bool, ()>(bg_view, "setHidden:", false);
        } else if !bg_view.is_null() {
            msg1::<Id, ()>(bg_view, "setImage:", NIL);
            msg1::<bool, ()>(bg_view, "setHidden:", true);
        }

        // 2. Sync foreground image & title label.
        let image = state_dict_get_resolved(s, &BUTTON_IMAGES_KEY, state);
        let (lbl, has_title) = button_sync_title_label(s);
        let ci = button_content_insets(s, sel("contentEdgeInsets"));
        let ii = button_image_insets(s, sel("imageEdgeInsets"));
        let ti = button_title_insets(s, sel("titleEdgeInsets"));
        let content = inset_rect(bounds, ci);

        if !image.is_null() && has_title && !lbl.is_null() {
            let iv = button_ensure_image_view(s);
            let img_sz: CGSize = msg0(image, "size");
            let lbl_fit: CGSize = msg1(lbl, "sizeThatFits:", CGSize { width: content.size.width, height: content.size.height });
            let total_w = (img_sz.width + lbl_fit.width).min(content.size.width);
            let start_x = content.origin.x + ((content.size.width - total_w) * 0.5).max(0.0);
            let img_w = img_sz.width.min(content.size.width);
            let img_rect = inset_rect(
                CGRect {
                    origin: CGPoint { x: start_x, y: content.origin.y },
                    size: CGSize { width: img_w, height: content.size.height },
                },
                ii,
            );
            let lbl_w = (total_w - img_w).max(0.0);
            let lbl_rect = inset_rect(
                CGRect {
                    origin: CGPoint { x: start_x + img_w, y: content.origin.y },
                    size: CGSize { width: lbl_w, height: content.size.height },
                },
                ti,
            );
            msg1::<CGRect, ()>(iv, "setFrame:", img_rect);
            msg1::<Id, ()>(iv, "setImage:", image);
            msg1::<bool, ()>(iv, "setHidden:", false);
            msg1::<CGRect, ()>(lbl, "setFrame:", lbl_rect);
            msg1::<bool, ()>(lbl, "setHidden:", false);
        } else {
            let existing_iv = assoc_get(s, &BUTTON_IMAGE_VIEW_KEY);
            if !image.is_null() || !existing_iv.is_null() {
                let iv = if !image.is_null() { button_ensure_image_view(s) } else { existing_iv };
                if !iv.is_null() {
                    msg1::<CGRect, ()>(iv, "setFrame:", inset_rect(content, ii));
                    msg1::<Id, ()>(iv, "setImage:", image);
                    msg1::<bool, ()>(iv, "setHidden:", image.is_null());
                }
            }
            if !lbl.is_null() {
                msg1::<CGRect, ()>(lbl, "setFrame:", inset_rect(content, ti));
                msg1::<bool, ()>(lbl, "setHidden:", !has_title);
            }
        }
    }

    unsafe extern "C" fn button_touch_began(s: Id, _c: Sel, _touches: Id, _event: Id) {
        if ctl_enabled(s, sel("isEnabled")) {
            ctl_set_highlighted(s, sel("setHighlighted:"), true);
        }
    }

    unsafe extern "C" fn button_touch_moved(s: Id, _c: Sel, touches: Id, event: Id) {
        let touch: Id = msg0(touches, "anyObject");
        if !touch.is_null() && ctl_enabled(s, sel("isEnabled")) {
            let point: CGPoint = msg1(touch, "locationInView:", s);
            let inside: bool = msg2(s, "pointInside:withEvent:", point, event);
            ctl_set_highlighted(s, sel("setHighlighted:"), inside);
        }
    }

    unsafe extern "C" fn button_touch_cancelled(s: Id, _c: Sel, _touches: Id, _event: Id) {
        ctl_set_highlighted(s, sel("setHighlighted:"), false);
    }

    unsafe extern "C" fn button_touch_ended(s: Id, _c: Sel, touches: Id, event: Id) {
        ctl_set_highlighted(s, sel("setHighlighted:"), false);
        let touch: Id = msg0(touches, "anyObject");
        if !touch.is_null() && ctl_enabled(s, sel("isEnabled")) {
            let point: CGPoint = msg1(touch, "locationInView:", s);
            let inside: bool = msg2(s, "pointInside:withEvent:", point, event);
            msg1::<usize, ()>(s, "sendActionsForControlEvents:", if inside { 1 << 6 } else { 1 << 7 });
        }
    }

    // ---- UITextField ----
    #[repr(C)]
    #[derive(Copy, Clone)]
    struct TextFieldState {
        align: isize,
        border: isize,
        clear_mode: isize,
        right_view_mode: isize,
        left_view_mode: isize,
        kb_type: isize,
        kb_app: isize,
        ret_key: isize,
        autocorrect: isize,
        autocap: isize,
        spell: isize,
        secure: bool,
        clears_begin: bool,
        adjusts_fit: bool,
        editing: bool,
        min_font: f64,
    }
    impl Default for TextFieldState {
        fn default() -> Self {
            Self {
                align: 0,
                border: 0,
                clear_mode: 0,
                right_view_mode: 0,
                left_view_mode: 0,
                kb_type: 0,
                kb_app: 0,
                ret_key: 0,
                autocorrect: 0,
                autocap: 0,
                spell: 0,
                secure: false,
                clears_begin: false,
                adjusts_fit: false,
                editing: false,
                min_font: 0.0,
            }
        }
    }

    static TF_DEL_KEY: u8 = 0;
    static TF_TEXT_KEY: u8 = 0;
    static TF_ATTR_TEXT_KEY: u8 = 0;
    static TF_PLACEHOLDER_KEY: u8 = 0;
    static TF_ATTR_PLACEHOLDER_KEY: u8 = 0;
    static TF_FONT_KEY: u8 = 0;
    static TF_COLOR_KEY: u8 = 0;
    static TF_INPUT_ACC_KEY: u8 = 0;
    static TF_INPUT_VIEW_KEY: u8 = 0;
    static TF_RIGHT_VIEW_KEY: u8 = 0;
    static TF_LEFT_VIEW_KEY: u8 = 0;
    static TF_DISPLAY_LABEL_KEY: u8 = 0;
    static TF_CARET_VIEW_KEY: u8 = 0;
    static TF_STATE_KEY: u8 = 0;

    unsafe fn tf_ensure_display_label(s: Id) -> Id {
        let mut lbl = assoc_get(s, &TF_DISPLAY_LABEL_KEY);
        if lbl.is_null() {
            lbl = msg0(cls("UILabel"), "new");
            msg1::<bool, ()>(lbl, "setUserInteractionEnabled:", false);
            msg1::<isize, ()>(lbl, "setNumberOfLines:", 1);
            assoc_set(s, &TF_DISPLAY_LABEL_KEY, lbl);
            msg2::<Id, isize, ()>(s, "insertSubview:atIndex:", lbl, 0);
            objc_release(lbl);
        }
        lbl
    }

    unsafe fn tf_sync_display(s: Id) {
        let st: TextFieldState = get_pod(s, &TF_STATE_KEY);
        let lbl = tf_ensure_display_label(s);
        let font = assoc_get(s, &TF_FONT_KEY);
        let eff_font = if !font.is_null() {
            font
        } else {
            msg1::<f64, Id>(cls("UIFont") as Id, "systemFontOfSize:", 15.0)
        };
        msg1::<Id, ()>(lbl, "setFont:", eff_font);
        msg1::<isize, ()>(lbl, "setTextAlignment:", st.align);

        let text = assoc_get(s, &TF_TEXT_KEY);
        let text_len = if text.is_null() { 0 } else { msg0::<usize>(text, "length") };
        if text_len > 0 {
            let color = assoc_get(s, &TF_COLOR_KEY);
            let eff_color = if !color.is_null() { color } else { msg0(cls("UIColor"), "blackColor") };
            msg1::<Id, ()>(lbl, "setTextColor:", eff_color);
            if st.secure {
                let dots = "•".repeat(text_len);
                msg1::<Id, ()>(lbl, "setText:", nsstring(&dots));
            } else {
                let attr = assoc_get(s, &TF_ATTR_TEXT_KEY);
                if !attr.is_null() && msg0::<usize>(attr, "length") > 0 {
                    msg1::<Id, ()>(lbl, "setAttributedText:", attr);
                } else {
                    msg1::<Id, ()>(lbl, "setText:", text);
                }
            }
        } else {
            let placeholder_color: Id = msg2::<f64, f64, Id>(cls("UIColor"), "colorWithWhite:alpha:", 0.6, 1.0);
            msg1::<Id, ()>(lbl, "setTextColor:", placeholder_color);
            let attr_ph = assoc_get(s, &TF_ATTR_PLACEHOLDER_KEY);
            if !attr_ph.is_null() && msg0::<usize>(attr_ph, "length") > 0 {
                msg1::<Id, ()>(lbl, "setAttributedText:", attr_ph);
            } else {
                let ph = assoc_get(s, &TF_PLACEHOLDER_KEY);
                msg1::<Id, ()>(lbl, "setText:", ph);
            }
        }

        let mut caret = assoc_get(s, &TF_CARET_VIEW_KEY);
        if st.editing {
            if caret.is_null() {
                caret = msg0(cls("UIView"), "new");
                msg1::<bool, ()>(caret, "setUserInteractionEnabled:", false);
                assoc_set(s, &TF_CARET_VIEW_KEY, caret);
                msg1::<Id, ()>(s, "addSubview:", caret);
                objc_release(caret);
            }
            let tint = view_tint_color(s, sel("tintColor"));
            msg1::<Id, ()>(caret, "setBackgroundColor:", tint);
            msg1::<bool, ()>(caret, "setHidden:", false);
        } else if !caret.is_null() {
            msg1::<bool, ()>(caret, "setHidden:", true);
        }
    }

    unsafe fn tf_accessory_visible(mode: isize, editing: bool) -> bool {
        match mode {
            1 => editing,  // UITextFieldViewModeWhileEditing
            2 => !editing, // UITextFieldViewModeUnlessEditing
            3 => true,     // UITextFieldViewModeAlways
            _ => false,    // UITextFieldViewModeNever
        }
    }

    unsafe extern "C" fn tf_del(s: Id, _c: Sel) -> Id { pear_objc::assoc_get_weak(s, &TF_DEL_KEY) }
    unsafe extern "C" fn tf_set_del(s: Id, _c: Sel, v: Id) { pear_objc::assoc_set_weak(s, &TF_DEL_KEY, v); }
    unsafe extern "C" fn tf_text(s: Id, _c: Sel) -> Id {
        let t = assoc_get(s, &TF_TEXT_KEY);
        if t.is_null() { nsstring("") } else { t }
    }
    unsafe extern "C" fn tf_set_text(s: Id, _c: Sel, v: Id) {
        set_copied(s, &TF_TEXT_KEY, v);
        assoc_set(s, &TF_ATTR_TEXT_KEY, NIL);
        tf_sync_display(s);
        msg0::<()>(s, "setNeedsLayout");
    }
    unsafe extern "C" fn tf_attr_text(s: Id, _c: Sel) -> Id { assoc_get(s, &TF_ATTR_TEXT_KEY) }
    unsafe extern "C" fn tf_set_attr_text(s: Id, _c: Sel, v: Id) {
        set_copied(s, &TF_ATTR_TEXT_KEY, v);
        let plain = if v.is_null() { NIL } else { msg0::<Id>(v, "string") };
        set_copied(s, &TF_TEXT_KEY, plain);
        tf_sync_display(s);
        msg0::<()>(s, "setNeedsLayout");
    }
    unsafe extern "C" fn tf_placeholder(s: Id, _c: Sel) -> Id { assoc_get(s, &TF_PLACEHOLDER_KEY) }
    unsafe extern "C" fn tf_set_placeholder(s: Id, _c: Sel, v: Id) {
        set_copied(s, &TF_PLACEHOLDER_KEY, v);
        assoc_set(s, &TF_ATTR_PLACEHOLDER_KEY, NIL);
        tf_sync_display(s);
        msg0::<()>(s, "setNeedsLayout");
    }
    unsafe extern "C" fn tf_attr_placeholder(s: Id, _c: Sel) -> Id { assoc_get(s, &TF_ATTR_PLACEHOLDER_KEY) }
    unsafe extern "C" fn tf_set_attr_placeholder(s: Id, _c: Sel, v: Id) {
        set_copied(s, &TF_ATTR_PLACEHOLDER_KEY, v);
        let plain = if v.is_null() { NIL } else { msg0::<Id>(v, "string") };
        set_copied(s, &TF_PLACEHOLDER_KEY, plain);
        tf_sync_display(s);
        msg0::<()>(s, "setNeedsLayout");
    }
    unsafe extern "C" fn tf_font(s: Id, _c: Sel) -> Id { assoc_get(s, &TF_FONT_KEY) }
    unsafe extern "C" fn tf_set_font(s: Id, _c: Sel, v: Id) {
        assoc_set(s, &TF_FONT_KEY, v);
        tf_sync_display(s);
        msg0::<()>(s, "setNeedsLayout");
    }
    unsafe extern "C" fn tf_color(s: Id, _c: Sel) -> Id { assoc_get(s, &TF_COLOR_KEY) }
    unsafe extern "C" fn tf_set_color(s: Id, _c: Sel, v: Id) {
        assoc_set(s, &TF_COLOR_KEY, v);
        tf_sync_display(s);
    }
    unsafe extern "C" fn tf_input_acc(s: Id, _c: Sel) -> Id { assoc_get(s, &TF_INPUT_ACC_KEY) }
    unsafe extern "C" fn tf_set_input_acc(s: Id, _c: Sel, v: Id) { assoc_set(s, &TF_INPUT_ACC_KEY, v); }
    unsafe extern "C" fn tf_input_view(s: Id, _c: Sel) -> Id { assoc_get(s, &TF_INPUT_VIEW_KEY) }
    unsafe extern "C" fn tf_set_input_view(s: Id, _c: Sel, v: Id) { assoc_set(s, &TF_INPUT_VIEW_KEY, v); }
    unsafe extern "C" fn tf_right_view(s: Id, _c: Sel) -> Id { assoc_get(s, &TF_RIGHT_VIEW_KEY) }
    unsafe extern "C" fn tf_set_right_view(s: Id, _c: Sel, v: Id) {
        let old = assoc_get(s, &TF_RIGHT_VIEW_KEY);
        if old == v {
            return;
        }
        if !old.is_null() && msg0::<Id>(old, "superview") == s {
            msg0::<()>(old, "removeFromSuperview");
        }
        assoc_set(s, &TF_RIGHT_VIEW_KEY, v);
        if !v.is_null() && v != s {
            msg1::<Id, ()>(s, "addSubview:", v);
        }
        msg0::<()>(s, "setNeedsLayout");
    }
    unsafe extern "C" fn tf_right_view_mode(s: Id, _c: Sel) -> isize { get_pod::<TextFieldState>(s, &TF_STATE_KEY).right_view_mode }
    unsafe extern "C" fn tf_set_right_view_mode(s: Id, _c: Sel, v: isize) {
        let mut st: TextFieldState = get_pod(s, &TF_STATE_KEY);
        st.right_view_mode = v;
        set_pod(s, &TF_STATE_KEY, st);
        msg0::<()>(s, "setNeedsLayout");
    }
    unsafe extern "C" fn tf_left_view(s: Id, _c: Sel) -> Id { assoc_get(s, &TF_LEFT_VIEW_KEY) }
    unsafe extern "C" fn tf_set_left_view(s: Id, _c: Sel, v: Id) {
        let old = assoc_get(s, &TF_LEFT_VIEW_KEY);
        if old == v {
            return;
        }
        if !old.is_null() && msg0::<Id>(old, "superview") == s {
            msg0::<()>(old, "removeFromSuperview");
        }
        assoc_set(s, &TF_LEFT_VIEW_KEY, v);
        if !v.is_null() && v != s {
            msg1::<Id, ()>(s, "addSubview:", v);
        }
        msg0::<()>(s, "setNeedsLayout");
    }
    unsafe extern "C" fn tf_left_view_mode(s: Id, _c: Sel) -> isize { get_pod::<TextFieldState>(s, &TF_STATE_KEY).left_view_mode }
    unsafe extern "C" fn tf_set_left_view_mode(s: Id, _c: Sel, v: isize) {
        let mut st: TextFieldState = get_pod(s, &TF_STATE_KEY);
        st.left_view_mode = v;
        set_pod(s, &TF_STATE_KEY, st);
        msg0::<()>(s, "setNeedsLayout");
    }
    unsafe extern "C" fn tf_editing(s: Id, _c: Sel) -> bool { get_pod::<TextFieldState>(s, &TF_STATE_KEY).editing }

    unsafe extern "C" fn tf_left_view_rect(s: Id, _c: Sel, bounds: CGRect) -> CGRect {
        let lv = assoc_get(s, &TF_LEFT_VIEW_KEY);
        if lv.is_null() {
            return CGRect::default();
        }
        let mut sz = msg0::<CGRect>(lv, "frame").size;
        if sz.width <= 0.0 || sz.height <= 0.0 {
            let fit: CGSize = msg1(lv, "sizeThatFits:", bounds.size);
            if sz.width <= 0.0 { sz.width = if fit.width > 0.0 { fit.width } else { bounds.size.height }; }
            if sz.height <= 0.0 { sz.height = if fit.height > 0.0 { fit.height } else { bounds.size.height }; }
        }
        let h = sz.height.min(bounds.size.height);
        CGRect {
            origin: CGPoint { x: bounds.origin.x, y: bounds.origin.y + (bounds.size.height - h) * 0.5 },
            size: CGSize { width: sz.width, height: h },
        }
    }

    unsafe extern "C" fn tf_right_view_rect(s: Id, _c: Sel, bounds: CGRect) -> CGRect {
        let rv = assoc_get(s, &TF_RIGHT_VIEW_KEY);
        if rv.is_null() {
            return CGRect::default();
        }
        let mut sz = msg0::<CGRect>(rv, "frame").size;
        if sz.width <= 0.0 || sz.height <= 0.0 {
            let fit: CGSize = msg1(rv, "sizeThatFits:", bounds.size);
            if sz.width <= 0.0 { sz.width = if fit.width > 0.0 { fit.width } else { bounds.size.height }; }
            if sz.height <= 0.0 { sz.height = if fit.height > 0.0 { fit.height } else { bounds.size.height }; }
        }
        let h = sz.height.min(bounds.size.height);
        CGRect {
            origin: CGPoint {
                x: (bounds.origin.x + bounds.size.width - sz.width).max(bounds.origin.x),
                y: bounds.origin.y + (bounds.size.height - h) * 0.5,
            },
            size: CGSize { width: sz.width, height: h },
        }
    }

    unsafe extern "C" fn tf_text_rect(s: Id, _c: Sel, bounds: CGRect) -> CGRect {
        let st: TextFieldState = get_pod(s, &TF_STATE_KEY);
        let pad_x = if st.border != 0 { 8.0 } else { 0.0 };
        let mut x1 = bounds.origin.x + pad_x;
        let mut x2 = bounds.origin.x + bounds.size.width - pad_x;
        let lv = assoc_get(s, &TF_LEFT_VIEW_KEY);
        if !lv.is_null() && tf_accessory_visible(st.left_view_mode, st.editing) {
            let lr: CGRect = msg1(s, "leftViewRectForBounds:", bounds);
            x1 = x1.max(lr.origin.x + lr.size.width + 4.0);
        }
        let rv = assoc_get(s, &TF_RIGHT_VIEW_KEY);
        if !rv.is_null() && tf_accessory_visible(st.right_view_mode, st.editing) {
            let rr: CGRect = msg1(s, "rightViewRectForBounds:", bounds);
            x2 = x2.min(rr.origin.x - 4.0);
        }
        CGRect {
            origin: CGPoint { x: x1, y: bounds.origin.y },
            size: CGSize { width: (x2 - x1).max(0.0), height: bounds.size.height },
        }
    }

    unsafe extern "C" fn tf_size_that_fits(s: Id, _c: Sel, fit: CGSize) -> CGSize {
        tf_sync_display(s);
        let lbl = assoc_get(s, &TF_DISPLAY_LABEL_KEY);
        let mut sz: CGSize = if lbl.is_null() {
            CGSize { width: 100.0, height: 34.0 }
        } else {
            msg1(lbl, "sizeThatFits:", fit)
        };
        sz.height = sz.height.max(30.0);
        if sz.width <= 0.0 {
            sz.width = 100.0;
        }
        sz
    }

    unsafe extern "C" fn tf_intrinsic_size(s: Id, c: Sel) -> CGSize {
        tf_size_that_fits(s, c, CGSize::default())
    }

    unsafe extern "C" fn tf_layout(s: Id, _c: Sel) {
        let bounds: CGRect = msg0(s, "bounds");
        let st: TextFieldState = get_pod(s, &TF_STATE_KEY);
        tf_sync_display(s);

        let lbl = assoc_get(s, &TF_DISPLAY_LABEL_KEY);
        let text_rect: CGRect = if st.editing {
            msg1(s, "editingRectForBounds:", bounds)
        } else {
            msg1(s, "textRectForBounds:", bounds)
        };
        if !lbl.is_null() {
            msg1::<CGRect, ()>(lbl, "setFrame:", text_rect);
        }

        let lv = assoc_get(s, &TF_LEFT_VIEW_KEY);
        if !lv.is_null() {
            if msg0::<Id>(lv, "superview") != s {
                msg1::<Id, ()>(s, "addSubview:", lv);
            }
            let vis = tf_accessory_visible(st.left_view_mode, st.editing);
            msg1::<bool, ()>(lv, "setHidden:", !vis);
            if vis {
                let r: CGRect = msg1(s, "leftViewRectForBounds:", bounds);
                msg1::<CGRect, ()>(lv, "setFrame:", r);
                msg1::<Id, ()>(s, "bringSubviewToFront:", lv);
            }
        }
        let rv = assoc_get(s, &TF_RIGHT_VIEW_KEY);
        if !rv.is_null() {
            if msg0::<Id>(rv, "superview") != s {
                msg1::<Id, ()>(s, "addSubview:", rv);
            }
            let vis = tf_accessory_visible(st.right_view_mode, st.editing);
            msg1::<bool, ()>(rv, "setHidden:", !vis);
            if vis {
                let r: CGRect = msg1(s, "rightViewRectForBounds:", bounds);
                msg1::<CGRect, ()>(rv, "setFrame:", r);
                msg1::<Id, ()>(s, "bringSubviewToFront:", rv);
            }
        }

        let caret = assoc_get(s, &TF_CARET_VIEW_KEY);
        if st.editing && !caret.is_null() && !lbl.is_null() {
            let text = assoc_get(s, &TF_TEXT_KEY);
            let mut text_w = 0.0f64;
            if !text.is_null() && msg0::<usize>(text, "length") > 0 {
                let fit: CGSize = msg1(lbl, "sizeThatFits:", CGSize { width: text_rect.size.width, height: text_rect.size.height });
                text_w = fit.width.min(text_rect.size.width);
            }
            let caret_x = match st.align {
                1 => text_rect.origin.x + (text_rect.size.width + text_w) * 0.5,
                2 => text_rect.origin.x + text_rect.size.width - 1.5,
                _ => text_rect.origin.x + text_w + 1.0,
            };
            let ch = (text_rect.size.height * 0.6).clamp(14.0, 24.0);
            msg1::<CGRect, ()>(caret, "setFrame:", CGRect {
                origin: CGPoint {
                    x: caret_x.min(text_rect.origin.x + text_rect.size.width - 2.0).max(text_rect.origin.x),
                    y: text_rect.origin.y + (text_rect.size.height - ch) * 0.5,
                },
                size: CGSize { width: 1.5, height: ch },
            });
            msg1::<Id, ()>(s, "bringSubviewToFront:", caret);
        }
    }

    unsafe extern "C" fn tf_can_become_first(s: Id, _c: Sel) -> bool {
        ctl_enabled(s, sel("isEnabled"))
    }

    unsafe extern "C" fn tf_become_first(s: Id, _c: Sel) -> bool {
        let st: TextFieldState = get_pod(s, &TF_STATE_KEY);
        if st.editing {
            return true;
        }
        let d = pear_objc::assoc_get_weak(s, &TF_DEL_KEY);
        if !d.is_null() && responds_to(d, "textFieldShouldBeginEditing:") && !msg1::<Id, bool>(d, "textFieldShouldBeginEditing:", s) {
            return false;
        }
        if !super0::<bool>(s, "UITextField", "becomeFirstResponder") {
            return false;
        }
        let mut st = get_pod::<TextFieldState>(s, &TF_STATE_KEY);
        st.editing = true;
        set_pod(s, &TF_STATE_KEY, st);
        if st.clears_begin {
            assoc_set(s, &TF_TEXT_KEY, NIL);
            assoc_set(s, &TF_ATTR_TEXT_KEY, NIL);
        }
        tf_sync_display(s);
        msg0::<()>(s, "setNeedsLayout");
        if !d.is_null() && responds_to(d, "textFieldDidBeginEditing:") {
            msg1::<Id, ()>(d, "textFieldDidBeginEditing:", s);
        }
        let nc: Id = msg0(cls("NSNotificationCenter"), "defaultCenter");
        msg2::<Id, Id, ()>(nc, "postNotificationName:object:", nsstring("UITextFieldTextDidBeginEditingNotification"), s);
        msg1::<usize, ()>(s, "sendActionsForControlEvents:", 1 << 16); // EditingDidBegin
        true
    }

    unsafe extern "C" fn tf_resign_first(s: Id, _c: Sel) -> bool {
        let st: TextFieldState = get_pod(s, &TF_STATE_KEY);
        if !st.editing {
            return super0(s, "UITextField", "resignFirstResponder");
        }
        let d = pear_objc::assoc_get_weak(s, &TF_DEL_KEY);
        if !d.is_null() && responds_to(d, "textFieldShouldEndEditing:") && !msg1::<Id, bool>(d, "textFieldShouldEndEditing:", s) {
            return false;
        }
        let _: bool = super0(s, "UITextField", "resignFirstResponder");
        let mut st = get_pod::<TextFieldState>(s, &TF_STATE_KEY);
        st.editing = false;
        set_pod(s, &TF_STATE_KEY, st);
        tf_sync_display(s);
        msg0::<()>(s, "setNeedsLayout");
        if !d.is_null() {
            if responds_to(d, "textFieldDidEndEditing:reason:") {
                msg2::<Id, isize, ()>(d, "textFieldDidEndEditing:reason:", s, 0);
            } else if responds_to(d, "textFieldDidEndEditing:") {
                msg1::<Id, ()>(d, "textFieldDidEndEditing:", s);
            }
        }
        let nc: Id = msg0(cls("NSNotificationCenter"), "defaultCenter");
        msg2::<Id, Id, ()>(nc, "postNotificationName:object:", nsstring("UITextFieldTextDidEndEditingNotification"), s);
        msg1::<usize, ()>(s, "sendActionsForControlEvents:", 1 << 18); // EditingDidEnd
        true
    }

    unsafe extern "C" fn tf_touch_ended(s: Id, _c: Sel, touches: Id, event: Id) {
        let touch: Id = msg0(touches, "anyObject");
        if !touch.is_null() && ctl_enabled(s, sel("isEnabled")) {
            let point: CGPoint = msg1(touch, "locationInView:", s);
            if msg2::<CGPoint, Id, bool>(s, "pointInside:withEvent:", point, event) {
                let _: bool = msg0(s, "becomeFirstResponder");
            }
        }
    }

    unsafe extern "C" fn tf_has_text(s: Id, _c: Sel) -> bool {
        let t = assoc_get(s, &TF_TEXT_KEY);
        !t.is_null() && msg0::<usize>(t, "length") > 0
    }

    unsafe fn tf_replace_range(s: Id, range: NSRange, rep: Id) {
        let cur = tf_text(s, sel("text"));
        let d = pear_objc::assoc_get_weak(s, &TF_DEL_KEY);
        if !d.is_null() && responds_to(d, "textField:shouldChangeCharactersInRange:replacementString:") {
            if !msg3::<Id, NSRange, Id, bool>(d, "textField:shouldChangeCharactersInRange:replacementString:", s, range, rep) {
                return;
            }
        }
        let new_str: Id = msg2(cur, "stringByReplacingCharactersInRange:withString:", range, rep);
        set_copied(s, &TF_TEXT_KEY, new_str);
        assoc_set(s, &TF_ATTR_TEXT_KEY, NIL);
        tf_sync_display(s);
        msg0::<()>(s, "setNeedsLayout");
        let nc: Id = msg0(cls("NSNotificationCenter"), "defaultCenter");
        msg2::<Id, Id, ()>(nc, "postNotificationName:object:", nsstring("UITextFieldTextDidChangeNotification"), s);
        msg1::<usize, ()>(s, "sendActionsForControlEvents:", 1 << 17); // EditingChanged
    }

    unsafe extern "C" fn tf_insert_text(s: Id, _c: Sel, text: Id) {
        let ins = if text.is_null() { nsstring("") } else { text };
        if let Some(rust_s) = nsstring_to_string(ins) {
            if rust_s == "\n" || rust_s == "\r" {
                let d = pear_objc::assoc_get_weak(s, &TF_DEL_KEY);
                if !d.is_null() && responds_to(d, "textFieldShouldReturn:") {
                    let _: bool = msg1(d, "textFieldShouldReturn:", s);
                }
                msg1::<usize, ()>(s, "sendActionsForControlEvents:", 1 << 19); // EditingDidEndOnExit
                return;
            }
        }
        let cur = tf_text(s, sel("text"));
        let len: usize = msg0(cur, "length");
        tf_replace_range(s, NSRange { location: len, length: 0 }, ins);
    }

    unsafe extern "C" fn tf_delete_backward(s: Id, _c: Sel) {
        let cur = tf_text(s, sel("text"));
        let len: usize = msg0(cur, "length");
        if len == 0 {
            return;
        }
        let range: NSRange = msg1(cur, "rangeOfComposedCharacterSequenceAtIndex:", len - 1);
        tf_replace_range(s, range, nsstring(""));
    }

    unsafe extern "C" fn tf_align(s: Id, _c: Sel) -> isize { get_pod::<TextFieldState>(s, &TF_STATE_KEY).align }
    unsafe extern "C" fn tf_set_align(s: Id, _c: Sel, v: isize) {
        let mut st: TextFieldState = get_pod(s, &TF_STATE_KEY);
        st.align = v;
        set_pod(s, &TF_STATE_KEY, st);
        tf_sync_display(s);
    }
    unsafe extern "C" fn tf_border(s: Id, _c: Sel) -> isize { get_pod::<TextFieldState>(s, &TF_STATE_KEY).border }
    unsafe extern "C" fn tf_set_border(s: Id, _c: Sel, v: isize) {
        let mut st: TextFieldState = get_pod(s, &TF_STATE_KEY);
        st.border = v;
        set_pod(s, &TF_STATE_KEY, st);
    }
    unsafe extern "C" fn tf_clear_mode(s: Id, _c: Sel) -> isize { get_pod::<TextFieldState>(s, &TF_STATE_KEY).clear_mode }
    unsafe extern "C" fn tf_set_clear_mode(s: Id, _c: Sel, v: isize) {
        let mut st: TextFieldState = get_pod(s, &TF_STATE_KEY);
        st.clear_mode = v;
        set_pod(s, &TF_STATE_KEY, st);
    }
    unsafe extern "C" fn tf_kb_type(s: Id, _c: Sel) -> isize { get_pod::<TextFieldState>(s, &TF_STATE_KEY).kb_type }
    unsafe extern "C" fn tf_set_kb_type(s: Id, _c: Sel, v: isize) {
        let mut st: TextFieldState = get_pod(s, &TF_STATE_KEY);
        st.kb_type = v;
        set_pod(s, &TF_STATE_KEY, st);
    }
    unsafe extern "C" fn tf_kb_app(s: Id, _c: Sel) -> isize { get_pod::<TextFieldState>(s, &TF_STATE_KEY).kb_app }
    unsafe extern "C" fn tf_set_kb_app(s: Id, _c: Sel, v: isize) {
        let mut st: TextFieldState = get_pod(s, &TF_STATE_KEY);
        st.kb_app = v;
        set_pod(s, &TF_STATE_KEY, st);
    }
    unsafe extern "C" fn tf_ret_key(s: Id, _c: Sel) -> isize { get_pod::<TextFieldState>(s, &TF_STATE_KEY).ret_key }
    unsafe extern "C" fn tf_set_ret_key(s: Id, _c: Sel, v: isize) {
        let mut st: TextFieldState = get_pod(s, &TF_STATE_KEY);
        st.ret_key = v;
        set_pod(s, &TF_STATE_KEY, st);
    }
    unsafe extern "C" fn tf_autocorrect(s: Id, _c: Sel) -> isize { get_pod::<TextFieldState>(s, &TF_STATE_KEY).autocorrect }
    unsafe extern "C" fn tf_set_autocorrect(s: Id, _c: Sel, v: isize) {
        let mut st: TextFieldState = get_pod(s, &TF_STATE_KEY);
        st.autocorrect = v;
        set_pod(s, &TF_STATE_KEY, st);
    }
    unsafe extern "C" fn tf_autocap(s: Id, _c: Sel) -> isize { get_pod::<TextFieldState>(s, &TF_STATE_KEY).autocap }
    unsafe extern "C" fn tf_set_autocap(s: Id, _c: Sel, v: isize) {
        let mut st: TextFieldState = get_pod(s, &TF_STATE_KEY);
        st.autocap = v;
        set_pod(s, &TF_STATE_KEY, st);
    }
    unsafe extern "C" fn tf_spell(s: Id, _c: Sel) -> isize { get_pod::<TextFieldState>(s, &TF_STATE_KEY).spell }
    unsafe extern "C" fn tf_set_spell(s: Id, _c: Sel, v: isize) {
        let mut st: TextFieldState = get_pod(s, &TF_STATE_KEY);
        st.spell = v;
        set_pod(s, &TF_STATE_KEY, st);
    }
    unsafe extern "C" fn tf_secure(s: Id, _c: Sel) -> bool { get_pod::<TextFieldState>(s, &TF_STATE_KEY).secure }
    unsafe extern "C" fn tf_set_secure(s: Id, _c: Sel, v: bool) {
        let mut st: TextFieldState = get_pod(s, &TF_STATE_KEY);
        st.secure = v;
        set_pod(s, &TF_STATE_KEY, st);
        tf_sync_display(s);
    }
    unsafe extern "C" fn tf_clears_begin(s: Id, _c: Sel) -> bool { get_pod::<TextFieldState>(s, &TF_STATE_KEY).clears_begin }
    unsafe extern "C" fn tf_set_clears_begin(s: Id, _c: Sel, v: bool) {
        let mut st: TextFieldState = get_pod(s, &TF_STATE_KEY);
        st.clears_begin = v;
        set_pod(s, &TF_STATE_KEY, st);
    }
    unsafe extern "C" fn tf_adjusts_fit(s: Id, _c: Sel) -> bool { get_pod::<TextFieldState>(s, &TF_STATE_KEY).adjusts_fit }
    unsafe extern "C" fn tf_set_adjusts_fit(s: Id, _c: Sel, v: bool) {
        let mut st: TextFieldState = get_pod(s, &TF_STATE_KEY);
        st.adjusts_fit = v;
        set_pod(s, &TF_STATE_KEY, st);
    }
    unsafe extern "C" fn tf_min_font(s: Id, _c: Sel) -> f64 { get_pod::<TextFieldState>(s, &TF_STATE_KEY).min_font }
    unsafe extern "C" fn tf_set_min_font(s: Id, _c: Sel, v: f64) {
        let mut st: TextFieldState = get_pod(s, &TF_STATE_KEY);
        st.min_font = v;
        set_pod(s, &TF_STATE_KEY, st);
    }

    // ---- UIToolbar ----
    #[repr(C)]
    #[derive(Copy, Clone)]
    struct ToolbarState {
        bar_style: isize,
        translucent: bool,
    }
    impl Default for ToolbarState {
        fn default() -> Self {
            Self { bar_style: 0, translucent: true }
        }
    }

    static TB_ITEMS_KEY: u8 = 0;
    static TB_TINT_KEY: u8 = 0;
    static TB_STATE_KEY: u8 = 0;

    unsafe extern "C" fn tb_items(s: Id, _c: Sel) -> Id {
        let arr = assoc_get(s, &TB_ITEMS_KEY);
        if arr.is_null() { msg0(cls("NSArray"), "array") } else { arr }
    }
    unsafe extern "C" fn tb_set_items(s: Id, _c: Sel, items: Id) {
        let copied = if items.is_null() { NIL } else { msg1(cls("NSArray"), "arrayWithArray:", items) };
        assoc_set(s, &TB_ITEMS_KEY, copied);
    }
    unsafe extern "C" fn tb_set_items_anim(s: Id, _c: Sel, items: Id, _anim: bool) {
        tb_set_items(s, sel("setItems:"), items);
    }
    unsafe extern "C" fn tb_style(s: Id, _c: Sel) -> isize { get_pod::<ToolbarState>(s, &TB_STATE_KEY).bar_style }
    unsafe extern "C" fn tb_set_style(s: Id, _c: Sel, v: isize) {
        let mut st: ToolbarState = get_pod(s, &TB_STATE_KEY);
        st.bar_style = v;
        set_pod(s, &TB_STATE_KEY, st);
    }
    unsafe extern "C" fn tb_translucent(s: Id, _c: Sel) -> bool { get_pod::<ToolbarState>(s, &TB_STATE_KEY).translucent }
    unsafe extern "C" fn tb_set_translucent(s: Id, _c: Sel, v: bool) {
        let mut st: ToolbarState = get_pod(s, &TB_STATE_KEY);
        st.translucent = v;
        set_pod(s, &TB_STATE_KEY, st);
    }
    unsafe extern "C" fn tb_tint(s: Id, _c: Sel) -> Id { assoc_get(s, &TB_TINT_KEY) }
    unsafe extern "C" fn tb_set_tint(s: Id, _c: Sel, v: Id) { assoc_set(s, &TB_TINT_KEY, v); }

    pub unsafe fn register_classes() {
        let lr = cls("UIViewLayoutRegion");
        if !lr.is_null() {
            add_class_method(lr, "marginsLayoutRegionWithCornerAdaptation:", region_factory as Imp, "@@:q");
            add_class_method(lr, "safeAreaLayoutRegionWithCornerAdaptation:", region_factory as Imp, "@@:q");
            add_class_method(lr, "marginsLayoutRegion", region_simple as Imp, "@@:");
            add_class_method(lr, "safeAreaLayoutRegion", region_simple as Imp, "@@:");
        }
        let lg = cls("UILayoutGuide");
        if !lg.is_null() {
            add_method(lg, "owningView", guide_owning_view as Imp, "@@:");
            add_method(lg, "setOwningView:", guide_set_owning_view as Imp, "v@:@");
            add_method(lg, "identifier", guide_identifier as Imp, "@@:");
            add_method(lg, "setIdentifier:", guide_set_identifier as Imp, "v@:@");
            add_method(lg, "layoutFrame", guide_layout_frame as Imp, "{CGRect={CGPoint=dd}{CGSize=dd}}@:");
            for name in ANCHOR_SELECTORS {
                add_method(lg, name, view_anchor as Imp, "@@:");
            }
        }
        let anchor = cls("PearLayoutAnchor");
        for relation in ["EqualTo", "LessThanOrEqualTo", "GreaterThanOrEqualTo"] {
            for (suffix, implementation, types) in [
                ("Anchor:", anchor_to as Imp, "@@:@"),
                ("Anchor:constant:", anchor_to_constant as Imp, "@@:@d"),
                ("Constant:", anchor_constant as Imp, "@@:d"),
                ("Anchor:multiplier:", anchor_to_multiplier as Imp, "@@:@d"),
                ("Anchor:multiplier:constant:", anchor_to_multiplier_constant as Imp, "@@:@dd"),
            ] {
                add_method(anchor, &format!("constraint{relation}{suffix}"), implementation, types);
            }
        }
        let plc = cls("PearLayoutConstraint");
        if !plc.is_null() {
            add_method(plc, "firstItem", plc_first as Imp, "@@:");
            add_method(plc, "secondItem", plc_second as Imp, "@@:");
            add_method(plc, "firstAttribute", plc_first_attr as Imp, "q@:");
            add_method(plc, "secondAttribute", plc_second_attr as Imp, "q@:");
            add_method(plc, "relation", plc_relation as Imp, "q@:");
            add_method(plc, "multiplier", plc_multiplier as Imp, "d@:");
            add_method(plc, "constant", plc_constant as Imp, "d@:");
            add_method(plc, "setConstant:", plc_set_constant as Imp, "v@:d");
            add_method(plc, "priority", plc_priority as Imp, "f@:");
            add_method(plc, "setPriority:", plc_set_priority as Imp, "v@:f");
            add_method(plc, "isActive", plc_active as Imp, "B@:");
            add_method(plc, "active", plc_active as Imp, "B@:");
            add_method(plc, "setActive:", plc_set_active as Imp, "v@:B");
            add_method(plc, "autoInstall", plc_auto_install as Imp, "v@:");
            add_method(plc, "identifier", plc_ident as Imp, "@@:");
            add_method(plc, "setIdentifier:", plc_set_ident as Imp, "v@:@");
        }

        let nslc = cls("NSLayoutConstraint");
        if !nslc.is_null() {
            let meta = object_getClass(nslc as Id);
            replace_class_hook(
                meta,
                "constraintWithItem:attribute:relatedBy:toItem:attribute:multiplier:constant:",
                nslc_with_item as Imp,
                "@@:@qq@qdd",
                &ORIG_WITH_ITEM,
            );
            replace_class_hook(meta, "activateConstraints:", nslc_activate as Imp, "v@:@", &ORIG_ACTIVATE);
            replace_class_hook(meta, "deactivateConstraints:", nslc_deactivate as Imp, "v@:@", &ORIG_DEACTIVATE);
            replace_class_hook(
                meta,
                "constraintsWithVisualFormat:options:metrics:views:",
                nslc_vfl as Imp,
                "@@:@Q@@",
                &ORIG_VFL,
            );
        }

        let ctl = cls("UIControl");
        if !ctl.is_null() {
            add_method(ctl, "addTarget:action:forControlEvents:", ctl_add_target as Imp, "v@:@:Q");
            add_method(ctl, "removeTarget:action:forControlEvents:", ctl_remove_target as Imp, "v@:@:Q");
            add_method(ctl, "allTargets", ctl_all_targets as Imp, "@@:");
            add_method(ctl, "allControlEvents", ctl_all_events as Imp, "Q@:");
            add_method(ctl, "actionsForTarget:forControlEvent:", ctl_actions_for_target as Imp, "@@:@Q");
            add_method(ctl, "sendActionsForControlEvents:", ctl_send_actions as Imp, "v@:Q");
            add_method(ctl, "isEnabled", ctl_enabled as Imp, "B@:");
            add_method(ctl, "enabled", ctl_enabled as Imp, "B@:");
            add_method(ctl, "setEnabled:", ctl_set_enabled as Imp, "v@:B");
            add_method(ctl, "isSelected", ctl_selected as Imp, "B@:");
            add_method(ctl, "selected", ctl_selected as Imp, "B@:");
            add_method(ctl, "setSelected:", ctl_set_selected as Imp, "v@:B");
            add_method(ctl, "isHighlighted", ctl_highlighted as Imp, "B@:");
            add_method(ctl, "highlighted", ctl_highlighted as Imp, "B@:");
            add_method(ctl, "setHighlighted:", ctl_set_highlighted as Imp, "v@:B");
            add_method(ctl, "contentHorizontalAlignment", ctl_h_align as Imp, "q@:");
            add_method(ctl, "setContentHorizontalAlignment:", ctl_set_h_align as Imp, "v@:q");
            add_method(ctl, "contentVerticalAlignment", ctl_v_align as Imp, "q@:");
            add_method(ctl, "setContentVerticalAlignment:", ctl_set_v_align as Imp, "v@:q");
        }

        let button = cls("UIButton");
        if !button.is_null() {
            add_class_method(button, "buttonWithType:", button_with_type as Imp, "@@:q");
            add_method(button, "buttonType", button_type as Imp, "q@:");
            add_method(button, "imageForState:", button_image as Imp, "@@:Q");
            add_method(button, "setImage:forState:", button_set_image as Imp, "v@:@Q");
            add_method(button, "currentImage", button_current_image as Imp, "@@:");
            add_method(button, "backgroundImageForState:", button_bg_image as Imp, "@@:Q");
            add_method(button, "setBackgroundImage:forState:", button_set_bg_image as Imp, "v@:@Q");
            add_method(button, "currentBackgroundImage", button_current_bg_image as Imp, "@@:");
            add_method(button, "titleForState:", button_title as Imp, "@@:Q");
            add_method(button, "setTitle:forState:", button_set_title as Imp, "v@:@Q");
            add_method(button, "currentTitle", button_current_title as Imp, "@@:");
            add_method(button, "attributedTitleForState:", button_attr_title as Imp, "@@:Q");
            add_method(button, "setAttributedTitle:forState:", button_set_attr_title as Imp, "v@:@Q");
            add_method(button, "currentAttributedTitle", button_current_attr_title as Imp, "@@:");
            add_method(button, "titleColorForState:", button_title_color as Imp, "@@:Q");
            add_method(button, "setTitleColor:forState:", button_set_title_color as Imp, "v@:@Q");
            add_method(button, "currentTitleColor", button_current_title_color as Imp, "@@:");
            add_method(button, "imageView", button_image_view as Imp, "@@:");
            add_method(button, "titleLabel", button_title_label as Imp, "@@:");
            add_method(button, "imageEdgeInsets", button_image_insets as Imp, "{UIEdgeInsets=dddd}@:");
            add_method(button, "setImageEdgeInsets:", button_set_image_insets as Imp, "v@:{UIEdgeInsets=dddd}");
            add_method(button, "titleEdgeInsets", button_title_insets as Imp, "{UIEdgeInsets=dddd}@:");
            add_method(button, "setTitleEdgeInsets:", button_set_title_insets as Imp, "v@:{UIEdgeInsets=dddd}");
            add_method(button, "contentEdgeInsets", button_content_insets as Imp, "{UIEdgeInsets=dddd}@:");
            add_method(button, "setContentEdgeInsets:", button_set_content_insets as Imp, "v@:{UIEdgeInsets=dddd}");
            add_method(button, "sizeThatFits:", button_size_that_fits as Imp, "{CGSize=dd}@:{CGSize=dd}");
            add_method(button, "intrinsicContentSize", button_intrinsic_size as Imp, "{CGSize=dd}@:");
            add_method(button, "layoutSubviews", button_layout as Imp, "v@:");
            add_method(button, "touchesBegan:withEvent:", button_touch_began as Imp, "v@:@@");
            add_method(button, "touchesMoved:withEvent:", button_touch_moved as Imp, "v@:@@");
            add_method(button, "touchesCancelled:withEvent:", button_touch_cancelled as Imp, "v@:@@");
            add_method(button, "touchesEnded:withEvent:", button_touch_ended as Imp, "v@:@@");
        }

        let tf = cls("UITextField");
        if !tf.is_null() {
            add_method(tf, "delegate", tf_del as Imp, "@@:");
            add_method(tf, "setDelegate:", tf_set_del as Imp, "v@:@");
            add_method(tf, "text", tf_text as Imp, "@@:");
            add_method(tf, "setText:", tf_set_text as Imp, "v@:@");
            add_method(tf, "attributedText", tf_attr_text as Imp, "@@:");
            add_method(tf, "setAttributedText:", tf_set_attr_text as Imp, "v@:@");
            add_method(tf, "placeholder", tf_placeholder as Imp, "@@:");
            add_method(tf, "setPlaceholder:", tf_set_placeholder as Imp, "v@:@");
            add_method(tf, "attributedPlaceholder", tf_attr_placeholder as Imp, "@@:");
            add_method(tf, "setAttributedPlaceholder:", tf_set_attr_placeholder as Imp, "v@:@");
            add_method(tf, "font", tf_font as Imp, "@@:");
            add_method(tf, "setFont:", tf_set_font as Imp, "v@:@");
            add_method(tf, "textColor", tf_color as Imp, "@@:");
            add_method(tf, "setTextColor:", tf_set_color as Imp, "v@:@");
            add_method(tf, "rightView", tf_right_view as Imp, "@@:");
            add_method(tf, "setRightView:", tf_set_right_view as Imp, "v@:@");
            add_method(tf, "rightViewMode", tf_right_view_mode as Imp, "q@:");
            add_method(tf, "setRightViewMode:", tf_set_right_view_mode as Imp, "v@:q");
            add_method(tf, "leftView", tf_left_view as Imp, "@@:");
            add_method(tf, "setLeftView:", tf_set_left_view as Imp, "v@:@");
            add_method(tf, "leftViewMode", tf_left_view_mode as Imp, "q@:");
            add_method(tf, "setLeftViewMode:", tf_set_left_view_mode as Imp, "v@:q");
            add_method(tf, "isEditing", tf_editing as Imp, "B@:");
            add_method(tf, "editing", tf_editing as Imp, "B@:");
            add_method(tf, "textRectForBounds:", tf_text_rect as Imp, "{CGRect={CGPoint=dd}{CGSize=dd}}@:{CGRect={CGPoint=dd}{CGSize=dd}}");
            add_method(tf, "editingRectForBounds:", tf_text_rect as Imp, "{CGRect={CGPoint=dd}{CGSize=dd}}@:{CGRect={CGPoint=dd}{CGSize=dd}}");
            add_method(tf, "placeholderRectForBounds:", tf_text_rect as Imp, "{CGRect={CGPoint=dd}{CGSize=dd}}@:{CGRect={CGPoint=dd}{CGSize=dd}}");
            add_method(tf, "leftViewRectForBounds:", tf_left_view_rect as Imp, "{CGRect={CGPoint=dd}{CGSize=dd}}@:{CGRect={CGPoint=dd}{CGSize=dd}}");
            add_method(tf, "rightViewRectForBounds:", tf_right_view_rect as Imp, "{CGRect={CGPoint=dd}{CGSize=dd}}@:{CGRect={CGPoint=dd}{CGSize=dd}}");
            add_method(tf, "sizeThatFits:", tf_size_that_fits as Imp, "{CGSize=dd}@:{CGSize=dd}");
            add_method(tf, "intrinsicContentSize", tf_intrinsic_size as Imp, "{CGSize=dd}@:");
            add_method(tf, "layoutSubviews", tf_layout as Imp, "v@:");
            add_method(tf, "canBecomeFirstResponder", tf_can_become_first as Imp, "B@:");
            add_method(tf, "becomeFirstResponder", tf_become_first as Imp, "B@:");
            add_method(tf, "resignFirstResponder", tf_resign_first as Imp, "B@:");
            add_method(tf, "touchesEnded:withEvent:", tf_touch_ended as Imp, "v@:@@");
            add_method(tf, "hasText", tf_has_text as Imp, "B@:");
            add_method(tf, "insertText:", tf_insert_text as Imp, "v@:@");
            add_method(tf, "deleteBackward", tf_delete_backward as Imp, "v@:");
            add_method(tf, "textAlignment", tf_align as Imp, "q@:");
            add_method(tf, "setTextAlignment:", tf_set_align as Imp, "v@:q");
            add_method(tf, "borderStyle", tf_border as Imp, "q@:");
            add_method(tf, "setBorderStyle:", tf_set_border as Imp, "v@:q");
            add_method(tf, "clearButtonMode", tf_clear_mode as Imp, "q@:");
            add_method(tf, "setClearButtonMode:", tf_set_clear_mode as Imp, "v@:q");
            add_method(tf, "inputAccessoryView", tf_input_acc as Imp, "@@:");
            add_method(tf, "setInputAccessoryView:", tf_set_input_acc as Imp, "v@:@");
            add_method(tf, "inputView", tf_input_view as Imp, "@@:");
            add_method(tf, "setInputView:", tf_set_input_view as Imp, "v@:@");
            add_method(tf, "keyboardType", tf_kb_type as Imp, "q@:");
            add_method(tf, "setKeyboardType:", tf_set_kb_type as Imp, "v@:q");
            add_method(tf, "keyboardAppearance", tf_kb_app as Imp, "q@:");
            add_method(tf, "setKeyboardAppearance:", tf_set_kb_app as Imp, "v@:q");
            add_method(tf, "returnKeyType", tf_ret_key as Imp, "q@:");
            add_method(tf, "setReturnKeyType:", tf_set_ret_key as Imp, "v@:q");
            add_method(tf, "autocorrectionType", tf_autocorrect as Imp, "q@:");
            add_method(tf, "setAutocorrectionType:", tf_set_autocorrect as Imp, "v@:q");
            add_method(tf, "autocapitalizationType", tf_autocap as Imp, "q@:");
            add_method(tf, "setAutocapitalizationType:", tf_set_autocap as Imp, "v@:q");
            add_method(tf, "spellCheckingType", tf_spell as Imp, "q@:");
            add_method(tf, "setSpellCheckingType:", tf_set_spell as Imp, "v@:q");
            add_method(tf, "isSecureTextEntry", tf_secure as Imp, "B@:");
            add_method(tf, "secureTextEntry", tf_secure as Imp, "B@:");
            add_method(tf, "setSecureTextEntry:", tf_set_secure as Imp, "v@:B");
            add_method(tf, "clearsOnBeginEditing", tf_clears_begin as Imp, "B@:");
            add_method(tf, "setClearsOnBeginEditing:", tf_set_clears_begin as Imp, "v@:B");
            add_method(tf, "adjustsFontSizeToFitWidth", tf_adjusts_fit as Imp, "B@:");
            add_method(tf, "setAdjustsFontSizeToFitWidth:", tf_set_adjusts_fit as Imp, "v@:B");
            add_method(tf, "minimumFontSize", tf_min_font as Imp, "d@:");
            add_method(tf, "setMinimumFontSize:", tf_set_min_font as Imp, "v@:d");
        }

        let tb = cls("UIToolbar");
        if !tb.is_null() {
            add_method(tb, "items", tb_items as Imp, "@@:");
            add_method(tb, "setItems:", tb_set_items as Imp, "v@:@");
            add_method(tb, "setItems:animated:", tb_set_items_anim as Imp, "v@:@B");
            add_method(tb, "barStyle", tb_style as Imp, "q@:");
            add_method(tb, "setBarStyle:", tb_set_style as Imp, "v@:q");
            add_method(tb, "isTranslucent", tb_translucent as Imp, "B@:");
            add_method(tb, "translucent", tb_translucent as Imp, "B@:");
            add_method(tb, "setTranslucent:", tb_set_translucent as Imp, "v@:B");
            add_method(tb, "barTintColor", tb_tint as Imp, "@@:");
            add_method(tb, "setBarTintColor:", tb_set_tint as Imp, "v@:@");
        }
    }
}
