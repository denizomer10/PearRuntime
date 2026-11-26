//! Clean-room: unified PearRuntime facade tying together `pear-macho`,
//! `pear-dyld`, `pear-libobjc`, `pear-foundation`, `pear-host` (+ OS backend),
//! and `pear-image` with zero external crates.io dependencies.

pub use pear_dyld;
pub use pear_foundation;
pub use pear_graphics;
pub use pear_host;
pub use pear_image;
pub use pear_jit;
pub use pear_legacy_host;
pub use pear_libobjc;
pub use pear_macho;
pub use pear_metal_vk;
pub use pear_swift_rt;
pub mod unwind;

pub mod accelerate;
pub mod frameworks;
pub mod security;

#[cfg(target_os = "linux")]
pub use pear_host_linux;

use std::cell::UnsafeCell;
use std::ffi::{c_char, c_void};
use std::sync::Once;
use pear_libobjc::abi::{ClassT, Id, Imp, Sel};
use pear_libobjc::object::{alloc, EMPTY_CACHE, NSOBJECT_CLASS, NSOBJECT_META, StaticClass};
use pear_libobjc::runtime as objc_rt;

macro_rules! guest_class {
    ($cls:ident, $meta:ident) => {
        static $meta: StaticClass = StaticClass(UnsafeCell::new(ClassT {
            isa: &raw const NSOBJECT_META as *const ClassT,
            superclass: &raw const NSOBJECT_META as *const ClassT,
            cache: &raw const EMPTY_CACHE as *const c_void,
            vtable: std::ptr::null(),
            bits: 0,
        }));
        static $cls: StaticClass = StaticClass(UnsafeCell::new(ClassT {
            isa: &raw const $meta as *const ClassT,
            superclass: &raw const NSOBJECT_CLASS as *const ClassT,
            cache: &raw const EMPTY_CACHE as *const c_void,
            vtable: std::ptr::null(),
            bits: 0,
        }));
    };
}

guest_class!(CALAYER_CLASS, CALAYER_META);
guest_class!(CAMETALLAYER_CLASS, CAMETALLAYER_META);
guest_class!(CAEAGLLAYER_CLASS, CAEAGLLAYER_META);
guest_class!(CATRANSACTION_CLASS, CATRANSACTION_META);
guest_class!(CADISPLAYLINK_CLASS, CADISPLAYLINK_META);
guest_class!(UIAPPLICATION_CLASS, UIAPPLICATION_META);
guest_class!(UIWINDOW_CLASS, UIWINDOW_META);
guest_class!(UIVIEW_CLASS, UIVIEW_META);
guest_class!(UISCREEN_CLASS, UISCREEN_META);
guest_class!(UICOLOR_CLASS, UICOLOR_META);
guest_class!(UIIMAGE_CLASS, UIIMAGE_META);
guest_class!(UILABEL_CLASS, UILABEL_META);
guest_class!(UIFONT_CLASS, UIFONT_META);
guest_class!(UIDEVICE_CLASS, UIDEVICE_META);
guest_class!(UIRESPONDER_CLASS, UIRESPONDER_META);
guest_class!(UIVIEWCONTROLLER_CLASS, UIVIEWCONTROLLER_META);
guest_class!(UIGESTURERECOGNIZER_CLASS, UIGESTURERECOGNIZER_META);
guest_class!(UILONGPRESSGESTURERECOGNIZER_CLASS, UILONGPRESSGESTURERECOGNIZER_META);
guest_class!(UITAPGESTURERECOGNIZER_CLASS, UITAPGESTURERECOGNIZER_META);
guest_class!(UIPANGESTURERECOGNIZER_CLASS, UIPANGESTURERECOGNIZER_META);
guest_class!(UISTORYBOARD_CLASS, UISTORYBOARD_META);
guest_class!(UICONTROL_CLASS, UICONTROL_META);
guest_class!(UIBUTTON_CLASS, UIBUTTON_META);
guest_class!(GLKVIEW_CLASS, GLKVIEW_META);
guest_class!(GLKVIEWCONTROLLER_CLASS, GLKVIEWCONTROLLER_META);
guest_class!(MTKVIEW_CLASS, MTKVIEW_META);
guest_class!(EAGLCONTEXT_CLASS, EAGLCONTEXT_META);
guest_class!(UIDOCUMENT_CLASS, UIDOCUMENT_META);
guest_class!(UIACCESSIBILITYELEMENT_CLASS, UIACCESSIBILITYELEMENT_META);
guest_class!(UITEXTPOSITION_CLASS, UITEXTPOSITION_META);
guest_class!(UITEXTRANGE_CLASS, UITEXTRANGE_META);
guest_class!(UITEXTFIELD_CLASS, UITEXTFIELD_META);
guest_class!(UITEXTVIEW_CLASS, UITEXTVIEW_META);
guest_class!(UISCROLLVIEW_CLASS, UISCROLLVIEW_META);
guest_class!(UINAVIGATIONCONTROLLER_CLASS, UINAVIGATIONCONTROLLER_META);
guest_class!(UIALERTCONTROLLER_CLASS, UIALERTCONTROLLER_META);
guest_class!(SKREQUEST_CLASS, SKREQUEST_META);
guest_class!(SKPRODUCTSREQUEST_CLASS, SKPRODUCTSREQUEST_META);
guest_class!(SKPAYMENT_CLASS, SKPAYMENT_META);
guest_class!(SKPAYMENTQUEUE_CLASS, SKPAYMENTQUEUE_META);
guest_class!(SKSTOREREVIEWCONTROLLER_CLASS, SKSTOREREVIEWCONTROLLER_META);
guest_class!(UIACCELEROMETER_CLASS, UIACCELEROMETER_META);
guest_class!(UIACTIVITYVIEWCONTROLLER_CLASS, UIACTIVITYVIEWCONTROLLER_META);
guest_class!(UIALERTVIEW_CLASS, UIALERTVIEW_META);
guest_class!(UIPASTEBOARD_CLASS, UIPASTEBOARD_META);
guest_class!(UIWINDOWSCENE_CLASS, UIWINDOWSCENE_META);
guest_class!(UITOUCH_CLASS, UITOUCH_META);
guest_class!(UIEVENT_CLASS, UIEVENT_META);
guest_class!(UIVIEWLAYOUTREGION_CLASS, UIVIEWLAYOUTREGION_META);
guest_class!(UILAYOUTGUIDE_CLASS, UILAYOUTGUIDE_META);
guest_class!(UISCREENMODE_CLASS, UISCREENMODE_META);
guest_class!(AVAUDIOSESSION_CLASS, AVAUDIOSESSION_META);
guest_class!(AVAUDIOSESSIONROUTEDESCRIPTION_CLASS, AVAUDIOSESSIONROUTEDESCRIPTION_META);
guest_class!(AVAUDIOPLAYER_CLASS, AVAUDIOPLAYER_META);
guest_class!(AVPLAYER_CLASS, AVPLAYER_META);
guest_class!(AVPLAYERITEM_CLASS, AVPLAYERITEM_META);
guest_class!(AVURLASSET_CLASS, AVURLASSET_META);
guest_class!(AVASSETREADER_CLASS, AVASSETREADER_META);
guest_class!(AVASSETREADERTRACKOUTPUT_CLASS, AVASSETREADERTRACKOUTPUT_META);
guest_class!(AVPLAYERITEMLEGIBLEOUTPUT_CLASS, AVPLAYERITEMLEGIBLEOUTPUT_META);
guest_class!(AVPLAYERITEMVIDEOOUTPUT_CLASS, AVPLAYERITEMVIDEOOUTPUT_META);
guest_class!(CKCONTAINER_CLASS, CKCONTAINER_META);
guest_class!(CKDATABASE_CLASS, CKDATABASE_META);
guest_class!(CKMODIFYRECORDSOPERATION_CLASS, CKMODIFYRECORDSOPERATION_META);
guest_class!(CKQUERY_CLASS, CKQUERY_META);
guest_class!(CKQUERYOPERATION_CLASS, CKQUERYOPERATION_META);
guest_class!(CKRECORD_CLASS, CKRECORD_META);
guest_class!(CKRECORDID_CLASS, CKRECORDID_META);
guest_class!(GCCONTROLLER_CLASS, GCCONTROLLER_META);
guest_class!(OS_DISPATCH_SEMAPHORE_CLASS, OS_DISPATCH_SEMAPHORE_META);
guest_class!(OS_DISPATCH_GROUP_CLASS, OS_DISPATCH_GROUP_META);
guest_class!(UISCENESESSION_CLASS, UISCENESESSION_META);
guest_class!(UISCENECONNECTIONOPTIONS_CLASS, UISCENECONNECTIONOPTIONS_META);

extern "C" fn shared_app_stub(_self: Id, _cmd: Sel) -> Id {
    static APP: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *APP.get_or_init(|| unsafe { alloc(UIAPPLICATION_CLASS.ptr()) } as usize) as Id
}

extern "C" fn app_begin_bg_task(_self: Id, _cmd: Sel, _handler: *const c_void) -> usize {
    static NEXT_TASK: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(1);
    NEXT_TASK.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

extern "C" fn app_begin_bg_task2(
    self_obj: Id,
    cmd: Sel,
    _name: Id,
    handler: *const c_void,
) -> usize {
    app_begin_bg_task(self_obj, cmd, handler)
}

extern "C" fn main_screen_stub(_self: Id, _cmd: Sel) -> Id {
    static SCR: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *SCR.get_or_init(|| unsafe { alloc(UISCREEN_CLASS.ptr()) } as usize) as Id
}

fn nsarray_with_object(obj: Id) -> Id {
    unsafe {
        let cls = objc_rt::class_named("NSArray").unwrap_or(std::ptr::null());
        if cls.is_null() {
            return std::ptr::null_mut();
        }
        let s = objc_rt::sel("arrayWithObject:");
        let d = pear_libobjc::message::lookup_for_send(cls as Id, s);
        let f: extern "C" fn(Id, Sel, Id) -> Id = std::mem::transmute(d.imp);
        f(d.receiver, s, obj)
    }
}

extern "C" fn screen_screens_stub(_self: Id, _cmd: Sel) -> Id {
    nsarray_with_object(main_screen_stub(std::ptr::null_mut(), std::ptr::null()))
}

extern "C" fn screen_current_mode(_self: Id, _cmd: Sel) -> Id {
    static MODE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *MODE.get_or_init(|| unsafe { alloc(UISCREENMODE_CLASS.ptr()) } as usize) as Id
}

extern "C" fn screen_available_modes(_self: Id, _cmd: Sel) -> Id {
    nsarray_with_object(screen_current_mode(std::ptr::null_mut(), std::ptr::null()))
}

extern "C" fn screen_mode_size(_self: Id, _cmd: Sel) -> pear_graphics::CGSize {
    let s = pear_host::host().screen();
    pear_graphics::CGSize {
        width: s.points.width * s.scale,
        height: s.points.height * s.scale,
    }
}

extern "C" fn screen_mode_aspect(_self: Id, _cmd: Sel) -> f64 {
    1.0
}

extern "C" fn screen_scale_stub(_self: Id, _cmd: Sel) -> f64 {
    pear_host::host().screen().scale
}

extern "C" fn current_device_stub(_self: Id, _cmd: Sel) -> Id {
    static DEV: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *DEV.get_or_init(|| unsafe { alloc(UIDEVICE_CLASS.ptr()) } as usize) as Id
}

extern "C" fn device_system_name(_self: Id, _cmd: Sel) -> Id {
    pear_foundation::nsstring_from_rust("iOS")
}
extern "C" fn device_system_version(_self: Id, _cmd: Sel) -> Id {
    pear_foundation::nsstring_from_rust("17.0")
}
extern "C" fn device_model(_self: Id, _cmd: Sel) -> Id {
    pear_foundation::nsstring_from_rust("iPhone")
}
extern "C" fn device_name(_self: Id, _cmd: Sel) -> Id {
    pear_foundation::nsstring_from_rust("PearRuntime")
}
extern "C" fn device_idiom(_self: Id, _cmd: Sel) -> isize { 0 }
extern "C" fn device_multitasking(_self: Id, _cmd: Sel) -> bool { true }
extern "C" fn device_battery_level(_self: Id, _cmd: Sel) -> f32 { 1.0 }
extern "C" fn device_battery_state(_self: Id, _cmd: Sel) -> isize { 2 }
extern "C" fn device_orientation(_self: Id, _cmd: Sel) -> isize { 1 }
extern "C" fn device_vendor_uuid(_self: Id, _cmd: Sel) -> Id {
    unsafe {
        let cls = objc_rt::class_named("NSUUID").unwrap_or(std::ptr::null());
        if cls.is_null() { return std::ptr::null_mut(); }
        let d = pear_libobjc::message::lookup_for_send(cls as Id, objc_rt::sel("UUID"));
        let f: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d.imp);
        f(d.receiver, objc_rt::sel("UUID"))
    }
}

extern "C" fn screen_bounds(_self: Id, _cmd: Sel) -> pear_graphics::CGRect {
    let s = pear_host::host().screen();
    pear_graphics::CGRect {
        origin: pear_graphics::CGPoint { x: 0.0, y: 0.0 },
        size: pear_graphics::CGSize { width: s.points.width, height: s.points.height },
    }
}
extern "C" fn screen_native_bounds(_self: Id, _cmd: Sel) -> pear_graphics::CGRect {
    let s = pear_host::host().screen();
    pear_graphics::CGRect {
        origin: pear_graphics::CGPoint { x: 0.0, y: 0.0 },
        size: pear_graphics::CGSize {
            width: s.points.width * s.scale,
            height: s.points.height * s.scale,
        },
    }
}
extern "C" fn screen_max_fps(_self: Id, _cmd: Sel) -> isize {
    pear_host::host().screen().max_fps as isize
}
extern "C" fn screen_brightness(_self: Id, _cmd: Sel) -> f64 { 1.0 }

static APP_DELEGATE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
extern "C" fn app_delegate(_self: Id, _cmd: Sel) -> Id {
    APP_DELEGATE.load(std::sync::atomic::Ordering::Acquire) as Id
}
extern "C" fn app_set_delegate(_self: Id, _cmd: Sel, d: Id) {
    APP_DELEGATE.store(d as usize, std::sync::atomic::Ordering::Release);
}
extern "C" fn app_windows(_self: Id, _cmd: Sel) -> Id {
    unsafe {
        let cls = objc_rt::class_named("NSArray").unwrap_or(std::ptr::null());
        if cls.is_null() { return std::ptr::null_mut(); }
        let d = pear_libobjc::message::lookup_for_send(cls as Id, objc_rt::sel("array"));
        let f: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d.imp);
        f(d.receiver, objc_rt::sel("array"))
    }
}
extern "C" fn app_key_window(_self: Id, _cmd: Sel) -> Id {
    HOST_WINDOWS
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.keys().next().copied())
        .unwrap_or(0) as Id
}
extern "C" fn app_state(_self: Id, _cmd: Sel) -> isize { 0 }
extern "C" fn app_orientation(_self: Id, _cmd: Sel) -> isize { 1 }
extern "C" fn app_true(_self: Id, _cmd: Sel) -> bool { true }
extern "C" fn app_false(_self: Id, _cmd: Sel) -> bool { false }
extern "C" fn app_false1(_self: Id, _cmd: Sel, _arg: Id) -> bool { false }
extern "C" fn app_noop1(_self: Id, _cmd: Sel, _arg: isize) {}

extern "C" fn uicolor_rgba(cls: Id, _cmd: Sel, _r: f64, _g: f64, _b: f64, _a: f64) -> Id {
    unsafe { alloc(cls as *const ClassT) }
}
extern "C" fn uicolor_wa(cls: Id, _cmd: Sel, _w: f64, _a: f64) -> Id {
    unsafe { alloc(cls as *const ClassT) }
}
extern "C" fn uicolor_preset(cls: Id, _cmd: Sel) -> Id {
    unsafe { alloc(cls as *const ClassT) }
}

static VC_VIEWS: std::sync::Mutex<Option<std::collections::HashMap<usize, usize>>> =
    std::sync::Mutex::new(None);
static VC_LOADING: std::sync::Mutex<Option<std::collections::HashSet<usize>>> =
    std::sync::Mutex::new(None);
static WINDOW_ROOT_VC: std::sync::Mutex<Option<std::collections::HashMap<usize, usize>>> =
    std::sync::Mutex::new(None);
static VIEW_LAYERS: std::sync::Mutex<Option<std::collections::HashMap<usize, usize>>> =
    std::sync::Mutex::new(None);
static DISPLAY_LINKS: std::sync::Mutex<Option<std::collections::HashMap<usize, (usize, usize, bool, bool)>>> =
    std::sync::Mutex::new(None);
static VIEW_SUBVIEWS: std::sync::Mutex<Option<std::collections::HashMap<usize, Vec<usize>>>> =
    std::sync::Mutex::new(None);

#[derive(Clone, Copy)]
struct TouchState {
    x: f64,
    y: f64,
    prev_x: f64,
    prev_y: f64,
    timestamp: f64,
    phase: isize,
    tap_count: isize,
    view: usize,
    window: usize,
}

static TOUCH_DATA: std::sync::Mutex<Option<std::collections::HashMap<usize, TouchState>>> =
    std::sync::Mutex::new(None);
static EVENT_TOUCHES: std::sync::Mutex<Option<std::collections::HashMap<usize, (usize, f64)>>> =
    std::sync::Mutex::new(None);
static ACTIVE_TOUCH_OBJ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

extern "C" fn touches_noop(_self: Id, _cmd: Sel, _touches: Id, _event: Id) {}

extern "C" fn trigger_layout_recursive(view: Id) {
    if view.is_null() {
        return;
    }
    unsafe {
        let sel = objc_rt::sel("layoutSubviews");
        let d = pear_libobjc::message::lookup_for_send(view, sel);
        if !d.imp.is_null() {
            let f: extern "C" fn(Id, Sel) = std::mem::transmute(d.imp);
            f(d.receiver, sel);
        }
    }
    let children: Vec<usize> = VIEW_SUBVIEWS
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(&(view as usize)).cloned())
        .unwrap_or_default();
    for ch in children {
        trigger_layout_recursive(ch as Id);
    }
}

extern "C" fn uiview_add_subview(self_obj: Id, _cmd: Sel, child: Id) {
    if self_obj.is_null() || child.is_null() || self_obj == child {
        return;
    }
    let inserted = {
        let mut g = VIEW_SUBVIEWS.lock().unwrap();
        let map = g.get_or_insert_with(std::collections::HashMap::new);
        let list = map.entry(self_obj as usize).or_default();
        if !list.contains(&(child as usize)) {
            pear_libobjc::object::retain(child);
            list.push(child as usize);
            true
        } else {
            false
        }
    };
    if inserted {
        trigger_layout_recursive(child);
    }
}

extern "C" fn uiview_remove_from_superview(self_obj: Id, _cmd: Sel) {
    if self_obj.is_null() {
        return;
    }
    let mut removed = false;
    {
        let mut g = VIEW_SUBVIEWS.lock().unwrap();
        if let Some(map) = g.as_mut() {
            for list in map.values_mut() {
                let before = list.len();
                list.retain(|&p| p != self_obj as usize);
                if list.len() != before {
                    removed = true;
                }
            }
        }
    }
    if removed {
        pear_libobjc::object::release(self_obj);
    }
}

extern "C" fn uiview_subviews(self_obj: Id, _cmd: Sel) -> Id {
    let children: Vec<usize> = if self_obj.is_null() {
        Vec::new()
    } else {
        VIEW_SUBVIEWS
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|m| m.get(&(self_obj as usize)).cloned())
            .unwrap_or_default()
    };
    unsafe {
        let cls = objc_rt::class_named("NSMutableArray").unwrap_or(std::ptr::null());
        if cls.is_null() {
            return std::ptr::null_mut();
        }
        let d_arr = pear_libobjc::message::lookup_for_send(cls as Id, objc_rt::sel("array"));
        let f_arr: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d_arr.imp);
        let arr = f_arr(d_arr.receiver, objc_rt::sel("array"));
        if !arr.is_null() {
            let sel_add = objc_rt::sel("addObject:");
            for ch in children {
                let d_add = pear_libobjc::message::lookup_for_send(arr, sel_add);
                let f_add: extern "C" fn(Id, Sel, Id) = std::mem::transmute(d_add.imp);
                f_add(d_add.receiver, sel_add, ch as Id);
            }
        }
        arr
    }
}

extern "C" fn uiview_convert_point(
    _self: Id,
    _cmd: Sel,
    pt: pear_graphics::CGPoint,
    _other: Id,
) -> pear_graphics::CGPoint {
    pt
}

extern "C" fn uitouch_location_in_view(self_obj: Id, _cmd: Sel, _view: Id) -> pear_graphics::CGPoint {
    let st = TOUCH_DATA
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(&(self_obj as usize)).copied());
    match st {
        Some(t) => pear_graphics::CGPoint { x: t.x, y: t.y },
        None => pear_graphics::CGPoint { x: 0.0, y: 0.0 },
    }
}

extern "C" fn uitouch_prev_location_in_view(
    self_obj: Id,
    _cmd: Sel,
    _view: Id,
) -> pear_graphics::CGPoint {
    let st = TOUCH_DATA
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(&(self_obj as usize)).copied());
    match st {
        Some(t) => pear_graphics::CGPoint { x: t.prev_x, y: t.prev_y },
        None => pear_graphics::CGPoint { x: 0.0, y: 0.0 },
    }
}

extern "C" fn uitouch_timestamp(self_obj: Id, _cmd: Sel) -> f64 {
    TOUCH_DATA
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(&(self_obj as usize)).map(|t| t.timestamp))
        .unwrap_or(0.0)
}

extern "C" fn uitouch_phase(self_obj: Id, _cmd: Sel) -> isize {
    TOUCH_DATA
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(&(self_obj as usize)).map(|t| t.phase))
        .unwrap_or(0)
}

extern "C" fn uitouch_tap_count(self_obj: Id, _cmd: Sel) -> isize {
    TOUCH_DATA
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(&(self_obj as usize)).map(|t| t.tap_count))
        .unwrap_or(1)
}

extern "C" fn uitouch_view(self_obj: Id, _cmd: Sel) -> Id {
    TOUCH_DATA
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(&(self_obj as usize)).map(|t| t.view))
        .unwrap_or(0) as Id
}

extern "C" fn uitouch_window(self_obj: Id, _cmd: Sel) -> Id {
    TOUCH_DATA
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(&(self_obj as usize)).map(|t| t.window))
        .unwrap_or(0) as Id
}

extern "C" fn uitouch_dealloc(self_obj: Id, _cmd: Sel) {
    if let Some(m) = TOUCH_DATA.lock().unwrap().as_mut() {
        m.remove(&(self_obj as usize));
    }
}

extern "C" fn uievent_all_touches(self_obj: Id, _cmd: Sel) -> Id {
    EVENT_TOUCHES
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(&(self_obj as usize)).map(|p| p.0))
        .unwrap_or(0) as Id
}

extern "C" fn uievent_touches_for(self_obj: Id, _cmd: Sel, _arg: Id) -> Id {
    uievent_all_touches(self_obj, _cmd)
}

extern "C" fn uievent_timestamp(self_obj: Id, _cmd: Sel) -> f64 {
    EVENT_TOUCHES
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(&(self_obj as usize)).map(|p| p.1))
        .unwrap_or(0.0)
}

extern "C" fn uievent_dealloc(self_obj: Id, _cmd: Sel) {
    if let Some(m) = EVENT_TOUCHES.lock().unwrap().as_mut() {
        m.remove(&(self_obj as usize));
    }
}

extern "C" fn uiview_init_frame(self_obj: Id, _cmd: Sel, _frame: pear_graphics::CGRect) -> Id {
    self_obj
}
extern "C" fn uiview_bounds(_self: Id, _cmd: Sel) -> pear_graphics::CGRect {
    screen_bounds(_self, _cmd)
}
extern "C" fn uiview_layer(self_obj: Id, _cmd: Sel) -> Id {
    if self_obj.is_null() {
        return std::ptr::null_mut();
    }
    let mut g = VIEW_LAYERS.lock().unwrap();
    let map = g.get_or_insert_with(std::collections::HashMap::new);
    if let Some(&layer) = map.get(&(self_obj as usize)) {
        return layer as Id;
    }
    let cls = unsafe { objc_rt::object_class(self_obj) };
    let layer_cls = if !cls.is_null() {
        let s_lc = objc_rt::sel("layerClass");
        let d = pear_libobjc::message::lookup_for_send(cls as Id, s_lc);
        let f: unsafe extern "C" fn(Id, Sel) -> Id = unsafe { std::mem::transmute(d.imp) };
        let res = unsafe { f(d.receiver, s_lc) } as *const ClassT;
        if res.is_null() { CAEAGLLAYER_CLASS.ptr() } else { res }
    } else {
        CAEAGLLAYER_CLASS.ptr()
    };
    let layer = unsafe { alloc(layer_cls) };
    map.insert(self_obj as usize, layer as usize);
    layer
}
extern "C" fn uiview_layer_class(_cls: Id, _cmd: Sel) -> Id {
    CAEAGLLAYER_CLASS.ptr() as Id
}
extern "C" fn uiview_scale(_self: Id, _cmd: Sel) -> f64 {
    pear_host::host().screen().scale
}
extern "C" fn uivc_view(self_obj: Id, _cmd: Sel) -> Id {
    if self_obj.is_null() {
        return std::ptr::null_mut();
    }
    if let Some(&v) = VC_VIEWS
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(&(self_obj as usize)))
    {
        return v as Id;
    }
    let already_loading = {
        let mut g = VC_LOADING.lock().unwrap();
        !g.get_or_insert_with(std::collections::HashSet::new)
            .insert(self_obj as usize)
    };
    if !already_loading {
        unsafe {
            let sel_load = objc_rt::sel("loadView");
            let d_load = pear_libobjc::message::lookup_for_send(self_obj, sel_load);
            let f_load: extern "C" fn(Id, Sel) = std::mem::transmute(d_load.imp);
            f_load(d_load.receiver, sel_load);
        }
        if let Some(s) = VC_LOADING.lock().unwrap().as_mut() {
            s.remove(&(self_obj as usize));
        }
        let existing = VC_VIEWS
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|m| m.get(&(self_obj as usize)).copied());
        let v = existing.unwrap_or_else(|| {
            let created = unsafe { alloc(UIVIEW_CLASS.ptr()) } as usize;
            VC_VIEWS
                .lock()
                .unwrap()
                .get_or_insert_with(std::collections::HashMap::new)
                .insert(self_obj as usize, created);
            created
        });
        unsafe {
            let sel_vdl = objc_rt::sel("viewDidLoad");
            let d_vdl = pear_libobjc::message::lookup_for_send(self_obj, sel_vdl);
            let f_vdl: extern "C" fn(Id, Sel) = std::mem::transmute(d_vdl.imp);
            f_vdl(d_vdl.receiver, sel_vdl);
        }
        return v as Id;
    }
    let v = unsafe { alloc(UIVIEW_CLASS.ptr()) };
    VC_VIEWS
        .lock()
        .unwrap()
        .get_or_insert_with(std::collections::HashMap::new)
        .insert(self_obj as usize, v as usize);
    v
}
extern "C" fn uivc_set_view(self_obj: Id, _cmd: Sel, view: Id) {
    if !view.is_null() {
        pear_libobjc::object::retain(view);
    }
    let old = {
        let mut g = VC_VIEWS.lock().unwrap();
        let map = g.get_or_insert_with(std::collections::HashMap::new);
        if view.is_null() {
            map.remove(&(self_obj as usize))
        } else {
            map.insert(self_obj as usize, view as usize)
        }
    };
    if let Some(prev) = old {
        if prev != 0 && prev != view as usize {
            pear_libobjc::object::release(prev as Id);
        }
    }
}
extern "C" fn uivc_is_view_loaded(self_obj: Id, _cmd: Sel) -> bool {
    if self_obj.is_null() {
        return false;
    }
    VC_VIEWS
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|m| m.contains_key(&(self_obj as usize)))
}
extern "C" fn uivc_view_if_loaded(self_obj: Id, _cmd: Sel) -> Id {
    if self_obj.is_null() {
        return std::ptr::null_mut();
    }
    VC_VIEWS
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(&(self_obj as usize)).copied())
        .unwrap_or(0) as Id
}
extern "C" fn uivc_nil(_self: Id, _cmd: Sel) -> Id {
    std::ptr::null_mut()
}
extern "C" fn uivc_present(_self: Id, _cmd: Sel, _vc: Id, _anim: bool, _comp: Id) {}
extern "C" fn uivc_dismiss(_self: Id, _cmd: Sel, _anim: bool, _comp: Id) {}
extern "C" fn ui_window_set_root_vc(self_obj: Id, _cmd: Sel, vc: Id) {
    if !vc.is_null() {
        pear_libobjc::object::retain(vc);
    }
    let old = {
        let mut g = WINDOW_ROOT_VC.lock().unwrap();
        let map = g.get_or_insert_with(std::collections::HashMap::new);
        if vc.is_null() {
            map.remove(&(self_obj as usize))
        } else {
            map.insert(self_obj as usize, vc as usize)
        }
    };
    if let Some(prev) = old {
        if prev != 0 && prev != vc as usize {
            pear_libobjc::object::release(prev as Id);
        }
    }
    if !vc.is_null() {
        let v = uivc_view(vc, objc_rt::sel("view"));
        trigger_layout_recursive(v);
    }
}
extern "C" fn ui_window_root_vc(self_obj: Id, _cmd: Sel) -> Id {
    WINDOW_ROOT_VC
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(&(self_obj as usize)).copied())
        .unwrap_or(0) as Id
}
extern "C" fn dl_create(cls: Id, _cmd: Sel, target: Id, sel_ptr: Sel) -> Id {
    let link = unsafe { alloc(cls as *const ClassT) };
    if !target.is_null() {
        pear_libobjc::object::retain(target);
    }
    let mut g = DISPLAY_LINKS.lock().unwrap();
    g.get_or_insert_with(std::collections::HashMap::new)
        .insert(link as usize, (target as usize, sel_ptr as usize, false, false));
    link
}
extern "C" fn dl_add_runloop(self_obj: Id, _cmd: Sel, _rl: Id, _mode: Id) {
    if let Some(map) = DISPLAY_LINKS.lock().unwrap().as_mut() {
        if let Some(entry) = map.get_mut(&(self_obj as usize)) {
            entry.3 = true;
        }
    }
}
extern "C" fn dl_set_paused(self_obj: Id, _cmd: Sel, paused: bool) {
    if let Some(map) = DISPLAY_LINKS.lock().unwrap().as_mut() {
        if let Some(entry) = map.get_mut(&(self_obj as usize)) {
            entry.2 = paused;
        }
    }
}
extern "C" fn dl_invalidate(self_obj: Id, _cmd: Sel) {
    if let Some(map) = DISPLAY_LINKS.lock().unwrap().as_mut() {
        map.remove(&(self_obj as usize));
    }
}
extern "C" fn dl_timestamp(_self: Id, _cmd: Sel) -> f64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64()
}
extern "C" fn dl_duration(_self: Id, _cmd: Sel) -> f64 {
    1.0 / 60.0
}
extern "C" fn pump_display_links() {
    let items: Vec<(usize, usize, usize)> = DISPLAY_LINKS
        .lock()
        .unwrap()
        .as_ref()
        .map(|m| {
            m.iter()
                .filter(|(_, (_, _, paused, scheduled))| !*paused && *scheduled)
                .map(|(&link, &(target, sel_ptr, _, _))| (link, target, sel_ptr))
                .collect()
        })
        .unwrap_or_default();
    for (link, target, sel_ptr) in items {
        if target != 0 && sel_ptr != 0 {
            unsafe {
                let d = pear_libobjc::message::lookup_for_send(target as Id, sel_ptr as Sel);
                let f: extern "C" fn(Id, Sel, Id) = std::mem::transmute(d.imp);
                f(d.receiver, sel_ptr as Sel, link as Id);
            }
        }
    }
}

extern "C" fn pump_frame() {
    let _ = pear_libobjc::object::drain_delayed_performs();
    pump_display_links();
}

extern "C" fn noop_method(_self: Id, _cmd: Sel) {}

extern "C" fn ui_application_main(
    _argc: i32,
    _argv: *const *const c_char,
    _principal: Id,
    delegate: Id,
) -> i32 {
    let delegate_name = if !delegate.is_null() {
        pear_foundation::rust_str_from_nsstring(delegate)
    } else {
        None
    };
    eprintln!("[PearRuntime][boot] UIApplicationMain delegate={delegate_name:?}");
    #[cfg(target_os = "linux")]
    pear_host_linux::set_frame_tick(pump_frame);
    pear_host::host().run(Box::new(move || {
        if let Some(name) = delegate_name {
            if let Some(cls) = objc_rt::class_named(&name) {
                unsafe {
                    let app = shared_app_stub(std::ptr::null_mut(), objc_rt::sel("sharedApplication"));
                    let del = alloc(cls);
                    let sel_init = objc_rt::sel("init");
                    let d_init = pear_libobjc::message::lookup_for_send(del, sel_init);
                    let f_init: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d_init.imp);
                    let del = f_init(d_init.receiver, sel_init);
                    app_set_delegate(app, objc_rt::sel("setDelegate:"), del);
                    eprintln!("[PearRuntime][boot] calling -[{name} application:didFinishLaunchingWithOptions:]");
                    let sel_did = objc_rt::sel("application:didFinishLaunchingWithOptions:");
                    let d_did = pear_libobjc::message::lookup_for_send(del, sel_did);
                    let f_did: extern "C" fn(Id, Sel, Id, Id) -> bool = std::mem::transmute(d_did.imp);
                    let _ = f_did(d_did.receiver, sel_did, app, std::ptr::null_mut());
                    let mut became_active_via_scene = false;
                    if let Some(scene_del_name) = scene_delegate_class_from_bundle() {
                        if let Some(scene_del_cls) = objc_rt::class_named(&scene_del_name) {
                            let scene = alloc(UIWINDOWSCENE_CLASS.ptr());
                            CONNECTED_SCENE.store(scene as usize, std::sync::atomic::Ordering::Release);
                            let session = alloc(UISCENESESSION_CLASS.ptr());
                            let opts = alloc(UISCENECONNECTIONOPTIONS_CLASS.ptr());
                            let s_del = alloc(scene_del_cls);
                            let d_sinit = pear_libobjc::message::lookup_for_send(s_del, sel_init);
                            let f_sinit: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d_sinit.imp);
                            let s_del = f_sinit(d_sinit.receiver, sel_init);
                            SCENE_DELEGATE.store(s_del as usize, std::sync::atomic::Ordering::Release);
                            eprintln!("[PearRuntime][boot] connecting UIWindowScene to {scene_del_name}");
                            let sel_conn = objc_rt::sel("scene:willConnectToSession:options:");
                            if pear_libobjc::runtime::lookup(scene_del_cls, sel_conn).is_some() {
                                let d_conn = pear_libobjc::message::lookup_for_send(s_del, sel_conn);
                                let f_conn: extern "C" fn(Id, Sel, Id, Id, Id) = std::mem::transmute(d_conn.imp);
                                f_conn(d_conn.receiver, sel_conn, scene, session, opts);
                            }
                            let sel_fg = objc_rt::sel("sceneWillEnterForeground:");
                            if pear_libobjc::runtime::lookup(scene_del_cls, sel_fg).is_some() {
                                let d_fg = pear_libobjc::message::lookup_for_send(s_del, sel_fg);
                                let f_fg: extern "C" fn(Id, Sel, Id) = std::mem::transmute(d_fg.imp);
                                f_fg(d_fg.receiver, sel_fg, scene);
                            }
                            let sel_sact = objc_rt::sel("sceneDidBecomeActive:");
                            if pear_libobjc::runtime::lookup(scene_del_cls, sel_sact).is_some() {
                                eprintln!("[PearRuntime][boot] calling -[{scene_del_name} sceneDidBecomeActive:]");
                                let d_sact = pear_libobjc::message::lookup_for_send(s_del, sel_sact);
                                let f_sact: extern "C" fn(Id, Sel, Id) = std::mem::transmute(d_sact.imp);
                                f_sact(d_sact.receiver, sel_sact, scene);
                                became_active_via_scene = true;
                            }
                        }
                    }
                    if !became_active_via_scene {
                        let sel_active = objc_rt::sel("applicationDidBecomeActive:");
                        if pear_libobjc::runtime::lookup(cls, sel_active).is_some() {
                            eprintln!("[PearRuntime][boot] calling -[{name} applicationDidBecomeActive:]");
                            let d_act = pear_libobjc::message::lookup_for_send(del, sel_active);
                            let f_act: extern "C" fn(Id, Sel, Id) = std::mem::transmute(d_act.imp);
                            f_act(d_act.receiver, sel_active, app);
                        }
                    }
                    let headless = cfg!(not(target_os = "linux"))
                        || std::env::var("PEAR_HEADLESS").as_deref() == Ok("1");
                    if headless {
                        let max_frames: usize = std::env::var("PEAR_HEADLESS_FRAMES")
                            .ok()
                            .and_then(|v| v.parse().ok())
                            .unwrap_or(3);
                        for frame in 0..max_frames {
                            let fired = pear_libobjc::object::drain_delayed_performs();
                            pump_display_links();
                            if frame == 0 && fired > 0 {
                                eprintln!(
                                    "[PearRuntime][boot] pumped frame {frame} (delayed_performs={fired})"
                                );
                            }
                            let has_links = DISPLAY_LINKS
                                .lock()
                                .unwrap()
                                .as_ref()
                                .is_some_and(|m| !m.is_empty());
                            if fired == 0 && !has_links {
                                break;
                            }
                            std::thread::sleep(std::time::Duration::from_millis(16));
                        }
                    }
                }
            } else {
                eprintln!("[PearRuntime][boot] UIApplicationMain delegate class '{name}' not registered");
            }
        }
    }));
    0
}

static HOST_WINDOWS: std::sync::Mutex<Option<std::collections::HashMap<usize, (usize, usize)>>> =
    std::sync::Mutex::new(None);
static CURRENT_EAGL_OBJ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static ACTIVE_EGL_SURFACE: std::sync::Mutex<Option<usize>> = std::sync::Mutex::new(None);
static EAGL_CTX_API: std::sync::Mutex<Option<std::collections::HashMap<usize, usize>>> =
    std::sync::Mutex::new(None);
static EAGL_LAST_BOUND_FBO: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
static EAGL_WINDOW_FBO: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(u32::MAX);
static EAGL_SURFACE_SIZE: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new((1280u64 << 32) | 720u64);
static CONNECTED_SCENE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static SCENE_DELEGATE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

extern "C" fn scene_delegate_get(_self: Id, _cmd: Sel) -> Id {
    SCENE_DELEGATE.load(std::sync::atomic::Ordering::Acquire) as Id
}

extern "C" fn scene_delegate_set(_self: Id, _cmd: Sel, del: Id) {
    SCENE_DELEGATE.store(del as usize, std::sync::atomic::Ordering::Release);
}

extern "C" fn scene_session_role(_self: Id, _cmd: Sel) -> Id {
    pear_foundation::nsstring_from_rust("UIWindowSceneSessionRoleApplication")
}

extern "C" fn ret_empty_set(_self: Id, _cmd: Sel) -> Id {
    unsafe {
        let cls = objc_rt::class_named("NSSet").unwrap_or(std::ptr::null());
        if cls.is_null() {
            return std::ptr::null_mut();
        }
        let s = objc_rt::sel("set");
        let d = pear_libobjc::message::lookup_for_send(cls as Id, s);
        let f: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d.imp);
        f(d.receiver, s)
    }
}

extern "C" fn app_connected_scenes(_self: Id, _cmd: Sel) -> Id {
    let sc = CONNECTED_SCENE.load(std::sync::atomic::Ordering::Acquire) as Id;
    if sc.is_null() {
        return ret_empty_set(_self, _cmd);
    }
    unsafe {
        let cls = objc_rt::class_named("NSSet").unwrap_or(std::ptr::null());
        if cls.is_null() {
            return std::ptr::null_mut();
        }
        let s = objc_rt::sel("setWithObject:");
        let d = pear_libobjc::message::lookup_for_send(cls as Id, s);
        let f: extern "C" fn(Id, Sel, Id) -> Id = std::mem::transmute(d.imp);
        f(d.receiver, s, sc)
    }
}

fn scene_delegate_class_from_bundle() -> Option<String> {
    fn dict_get(dict: Id, key: &str) -> Id {
        if dict.is_null() {
            return std::ptr::null_mut();
        }
        unsafe {
            let k = pear_foundation::nsstring_from_rust(key);
            let s = objc_rt::sel("objectForKey:");
            let d = pear_libobjc::message::lookup_for_send(dict, s);
            let f: extern "C" fn(Id, Sel, Id) -> Id = std::mem::transmute(d.imp);
            f(d.receiver, s, k)
        }
    }
    unsafe {
        let b_cls = objc_rt::class_named("NSBundle").unwrap_or(std::ptr::null());
        if b_cls.is_null() {
            return None;
        }
        let s_main = objc_rt::sel("mainBundle");
        let d_main = pear_libobjc::message::lookup_for_send(b_cls as Id, s_main);
        let f_main: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d_main.imp);
        let bundle = f_main(d_main.receiver, s_main);
        let s_info = objc_rt::sel("infoDictionary");
        let d_info = pear_libobjc::message::lookup_for_send(bundle, s_info);
        let f_info: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d_info.imp);
        let info = f_info(d_info.receiver, s_info);
        let manifest = dict_get(info, "UIApplicationSceneManifest");
        let configs = dict_get(manifest, "UISceneConfigurations");
        let roles = dict_get(configs, "UIWindowSceneSessionRoleApplication");
        if roles.is_null() {
            return None;
        }
        let s_cnt = objc_rt::sel("count");
        let d_cnt = pear_libobjc::message::lookup_for_send(roles, s_cnt);
        let f_cnt: extern "C" fn(Id, Sel) -> usize = std::mem::transmute(d_cnt.imp);
        if f_cnt(d_cnt.receiver, s_cnt) == 0 {
            return None;
        }
        let s_at = objc_rt::sel("objectAtIndex:");
        let d_at = pear_libobjc::message::lookup_for_send(roles, s_at);
        let f_at: extern "C" fn(Id, Sel, usize) -> Id = std::mem::transmute(d_at.imp);
        let first = f_at(d_at.receiver, s_at, 0);
        let name_ns = dict_get(first, "UISceneDelegateClassName");
        pear_foundation::rust_str_from_nsstring(name_ns)
    }
}

extern "C" fn ui_window_init_with_scene(self_obj: Id, cmd: Sel, _scene: Id) -> Id {
    let bounds = screen_bounds(self_obj, cmd);
    ui_window_init_with_frame(self_obj, cmd, bounds)
}

extern "C" fn ui_window_scene_stub(_self: Id, _cmd: Sel) -> Id {
    CONNECTED_SCENE.load(std::sync::atomic::Ordering::Acquire) as Id
}

extern "C" fn ui_window_init_with_frame(
    self_obj: Id,
    _cmd: Sel,
    frame: pear_graphics::CGRect,
) -> Id {
    let rect = pear_host::Rect {
        x: frame.origin.x,
        y: frame.origin.y,
        width: frame.size.width.max(64.0),
        height: frame.size.height.max(64.0),
    };
    let host = pear_host::host();
    let win = host.create_window(pear_host::GuestObject(self_obj as *mut c_void), rect);
    let layer = pear_host::LayerRef(self_obj as *mut c_void);
    host.set_root_layer(win, layer);
    let mut guard = HOST_WINDOWS.lock().unwrap();
    guard
        .get_or_insert_with(std::collections::HashMap::new)
        .insert(self_obj as usize, (win.0 as usize, layer.0 as usize));
    self_obj
}

extern "C" fn ui_window_make_key_and_visible(self_obj: Id, _cmd: Sel) {
    let pair = HOST_WINDOWS
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(&(self_obj as usize)).copied());
    let (win, _layer) = match pair {
        Some(p) => (pear_host::WindowHandle(p.0 as *mut c_void), p.1),
        None => {
            let _ = ui_window_init_with_frame(
                self_obj,
                _cmd,
                pear_graphics::CGRect {
                    origin: pear_graphics::CGPoint { x: 80.0, y: 60.0 },
                    size: pear_graphics::CGSize { width: 960.0, height: 600.0 },
                },
            );
            let p = HOST_WINDOWS
                .lock()
                .unwrap()
                .as_ref()
                .and_then(|m| m.get(&(self_obj as usize)).copied())
                .unwrap_or((0, 0));
            (pear_host::WindowHandle(p.0 as *mut c_void), p.1)
        }
    };
    if !win.0.is_null() {
        pear_host::host().show_window(win, true);
    }
    let vc = ui_window_root_vc(self_obj, objc_rt::sel("rootViewController"));
    if !vc.is_null() {
        let v = uivc_view_if_loaded(vc, objc_rt::sel("viewIfLoaded"));
        trigger_layout_recursive(v);
    }
    trigger_layout_recursive(self_obj);
}

fn ensure_egl_surface(drawable: Option<Id>, api: usize) -> bool {
    if ACTIVE_EGL_SURFACE.lock().unwrap().is_some() {
        return true;
    }
    let layer = drawable
        .and_then(|d| pear_host::host().surface(pear_host::LayerRef(d as *mut c_void)))
        .or_else(|| {
            let root = HOST_WINDOWS
                .lock()
                .unwrap()
                .as_ref()
                .and_then(|m| m.values().next().copied())
                .map(|(_, l)| l)?;
            pear_host::host().surface(pear_host::LayerRef(root as *mut c_void))
        });
    if let Some(info) = layer {
        let w = (info.pixels.width.max(1.0)) as u32;
        let h = (info.pixels.height.max(1.0)) as u32;
        EAGL_SURFACE_SIZE.store(((w as u64) << 32) | (h as u64), std::sync::atomic::Ordering::Release);
        if let pear_host::NativeSurface::Xlib { display, window } = info.surface {
            if let Some(egl) = egl_bridge::EglSurfaceCtx::create_for_api(display, window, api) {
                let boxed = Box::into_raw(Box::new(egl)) as usize;
                *ACTIVE_EGL_SURFACE.lock().unwrap() = Some(boxed);
                return true;
            }
        }
    }
    false
}

extern "C" fn eagl_init_with_api(self_obj: Id, _cmd: Sel, api: usize) -> Id {
    EAGL_CTX_API
        .lock()
        .unwrap()
        .get_or_insert_with(std::collections::HashMap::new)
        .insert(self_obj as usize, api);
    self_obj
}

extern "C" fn eagl_current_context(_cls: Id, _cmd: Sel) -> Id {
    CURRENT_EAGL_OBJ.load(std::sync::atomic::Ordering::Acquire) as Id
}

extern "C" fn eagl_set_current_context(_self_cls: Id, _cmd: Sel, ctx: Id) -> bool {
    CURRENT_EAGL_OBJ.store(ctx as usize, std::sync::atomic::Ordering::Release);
    if !ctx.is_null() {
        let api = EAGL_CTX_API
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|m| m.get(&(ctx as usize)).copied())
            .unwrap_or(2);
        let _ = ensure_egl_surface(None, api);
    }
    true
}

extern "C" fn eagl_renderbuffer_storage(
    self_obj: Id,
    _cmd: Sel,
    _target: usize,
    drawable: Id,
) -> bool {
    let api = EAGL_CTX_API
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|m| m.get(&(self_obj as usize)).copied())
        .unwrap_or(2);
    let _ = ensure_egl_surface(Some(drawable), api);
    let cur_fbo = EAGL_LAST_BOUND_FBO.load(std::sync::atomic::Ordering::Acquire);
    if cur_fbo != 0 {
        EAGL_WINDOW_FBO.store(cur_fbo, std::sync::atomic::Ordering::Release);
        eagl_real_bind_fbo(0x8D40, 0);
    }
    true
}

extern "C" fn eagl_present_renderbuffer(_self_obj: Id, _cmd: Sel, _target: usize) -> bool {
    if let Some(ptr) = *ACTIVE_EGL_SURFACE.lock().unwrap() {
        if ptr > 0x10000 {
            let egl = unsafe { &*(ptr as *const egl_bridge::EglSurfaceCtx) };
            egl.swap_buffers();
        }
    }
    true
}

extern "C" fn ns_string_from_cgrect(r: pear_graphics::CGRect) -> Id {
    pear_foundation::nsstring_from_rust(&format!(
        "{{{{{}, {}}}, {{{}, {}}}}}",
        r.origin.x, r.origin.y, r.size.width, r.size.height
    ))
}

extern "C" fn ns_string_from_cgsize(s: pear_graphics::CGSize) -> Id {
    pear_foundation::nsstring_from_rust(&format!("{{{}, {}}}", s.width, s.height))
}

extern "C" fn ns_string_from_cgpoint(p: pear_graphics::CGPoint) -> Id {
    pear_foundation::nsstring_from_rust(&format!("{{{}, {}}}", p.x, p.y))
}

#[derive(Clone, Copy)]
struct CaMetalLayerState {
    device: usize,
    pixel_format: usize,
    drawable_size: pear_graphics::CGSize,
    framebuffer_only: bool,
}

impl Default for CaMetalLayerState {
    fn default() -> Self {
        Self {
            device: 0,
            pixel_format: 80, // MTLPixelFormatBGRA8Unorm
            drawable_size: pear_graphics::CGSize { width: 1280.0, height: 720.0 },
            framebuffer_only: true,
        }
    }
}

static CAMETAL_LAYERS: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<usize, CaMetalLayerState>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

extern "C" fn cametal_layer(_cls: Id, _cmd: Sel) -> Id {
    unsafe { alloc(CAMETALLAYER_CLASS.ptr()) }
}

extern "C" fn cametal_device(self_obj: Id, _cmd: Sel) -> Id {
    let map = CAMETAL_LAYERS.lock().unwrap();
    let dev = map.get(&(self_obj as usize)).map(|s| s.device).unwrap_or(0);
    if dev != 0 {
        dev as Id
    } else {
        pear_metal_vk::create_system_default_device()
    }
}

extern "C" fn cametal_set_device(self_obj: Id, _cmd: Sel, dev: Id) {
    let mut map = CAMETAL_LAYERS.lock().unwrap();
    map.entry(self_obj as usize).or_default().device = dev as usize;
}

extern "C" fn cametal_pixel_format(self_obj: Id, _cmd: Sel) -> usize {
    CAMETAL_LAYERS
        .lock()
        .unwrap()
        .get(&(self_obj as usize))
        .map(|s| s.pixel_format)
        .unwrap_or(80)
}

extern "C" fn cametal_set_pixel_format(self_obj: Id, _cmd: Sel, fmt: usize) {
    let mut map = CAMETAL_LAYERS.lock().unwrap();
    map.entry(self_obj as usize).or_default().pixel_format = fmt;
}

extern "C" fn cametal_drawable_size(self_obj: Id, _cmd: Sel) -> pear_graphics::CGSize {
    CAMETAL_LAYERS
        .lock()
        .unwrap()
        .get(&(self_obj as usize))
        .map(|s| s.drawable_size)
        .unwrap_or(pear_graphics::CGSize { width: 1280.0, height: 720.0 })
}

extern "C" fn cametal_set_drawable_size(self_obj: Id, _cmd: Sel, sz: pear_graphics::CGSize) {
    let mut map = CAMETAL_LAYERS.lock().unwrap();
    map.entry(self_obj as usize).or_default().drawable_size = sz;
}

extern "C" fn cametal_framebuffer_only(self_obj: Id, _cmd: Sel) -> bool {
    CAMETAL_LAYERS
        .lock()
        .unwrap()
        .get(&(self_obj as usize))
        .map(|s| s.framebuffer_only)
        .unwrap_or(true)
}

extern "C" fn cametal_set_framebuffer_only(self_obj: Id, _cmd: Sel, flag: bool) {
    let mut map = CAMETAL_LAYERS.lock().unwrap();
    map.entry(self_obj as usize).or_default().framebuffer_only = flag;
}

extern "C" fn cametal_next_drawable(self_obj: Id, _cmd: Sel) -> Id {
    let state = CAMETAL_LAYERS
        .lock()
        .unwrap()
        .get(&(self_obj as usize))
        .copied()
        .unwrap_or_default();
    let w = (state.drawable_size.width.max(1.0)) as usize;
    let h = (state.drawable_size.height.max(1.0)) as usize;
    pear_metal_vk::next_drawable_for_layer(self_obj, w, h, state.pixel_format)
}

fn init_guest_frameworks() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let i = |p: *const ()| p as Imp;
        for (cls, meta, name) in [
            (&CATRANSACTION_CLASS, &CATRANSACTION_META, "CATransaction"),
            (&UIIMAGE_CLASS, &UIIMAGE_META, "UIImage"),
            (&UILABEL_CLASS, &UILABEL_META, "UILabel"),
            (&UIFONT_CLASS, &UIFONT_META, "UIFont"),
            (&UIRESPONDER_CLASS, &UIRESPONDER_META, "UIResponder"),
            (&UISTORYBOARD_CLASS, &UISTORYBOARD_META, "UIStoryboard"),
            (&UICONTROL_CLASS, &UICONTROL_META, "UIControl"),
            (&UIBUTTON_CLASS, &UIBUTTON_META, "UIButton"),
            (&GLKVIEW_CLASS, &GLKVIEW_META, "GLKView"),
            (&GLKVIEWCONTROLLER_CLASS, &GLKVIEWCONTROLLER_META, "GLKViewController"),
            (&MTKVIEW_CLASS, &MTKVIEW_META, "MTKView"),
            (&UIDOCUMENT_CLASS, &UIDOCUMENT_META, "UIDocument"),
            (&UIACCESSIBILITYELEMENT_CLASS, &UIACCESSIBILITYELEMENT_META, "UIAccessibilityElement"),
            (&UITEXTPOSITION_CLASS, &UITEXTPOSITION_META, "UITextPosition"),
            (&UITEXTRANGE_CLASS, &UITEXTRANGE_META, "UITextRange"),
            (&UITEXTFIELD_CLASS, &UITEXTFIELD_META, "UITextField"),
            (&UITEXTVIEW_CLASS, &UITEXTVIEW_META, "UITextView"),
            (&UISCROLLVIEW_CLASS, &UISCROLLVIEW_META, "UIScrollView"),
            (&UINAVIGATIONCONTROLLER_CLASS, &UINAVIGATIONCONTROLLER_META, "UINavigationController"),
            (&UIALERTCONTROLLER_CLASS, &UIALERTCONTROLLER_META, "UIAlertController"),
            (&SKREQUEST_CLASS, &SKREQUEST_META, "SKRequest"),
            (&SKPRODUCTSREQUEST_CLASS, &SKPRODUCTSREQUEST_META, "SKProductsRequest"),
            (&SKPAYMENT_CLASS, &SKPAYMENT_META, "SKPayment"),
            (&SKPAYMENTQUEUE_CLASS, &SKPAYMENTQUEUE_META, "SKPaymentQueue"),
            (&SKSTOREREVIEWCONTROLLER_CLASS, &SKSTOREREVIEWCONTROLLER_META, "SKStoreReviewController"),
            (&UIACCELEROMETER_CLASS, &UIACCELEROMETER_META, "UIAccelerometer"),
            (&UIACTIVITYVIEWCONTROLLER_CLASS, &UIACTIVITYVIEWCONTROLLER_META, "UIActivityViewController"),
            (&UIALERTVIEW_CLASS, &UIALERTVIEW_META, "UIAlertView"),
            (&UIPASTEBOARD_CLASS, &UIPASTEBOARD_META, "UIPasteboard"),
            (&AVAUDIOSESSION_CLASS, &AVAUDIOSESSION_META, "AVAudioSession"),
            (&AVAUDIOPLAYER_CLASS, &AVAUDIOPLAYER_META, "AVAudioPlayer"),
            (&AVPLAYER_CLASS, &AVPLAYER_META, "AVPlayer"),
            (&AVPLAYERITEM_CLASS, &AVPLAYERITEM_META, "AVPlayerItem"),
            (&AVURLASSET_CLASS, &AVURLASSET_META, "AVURLAsset"),
            (&AVASSETREADER_CLASS, &AVASSETREADER_META, "AVAssetReader"),
            (&AVASSETREADERTRACKOUTPUT_CLASS, &AVASSETREADERTRACKOUTPUT_META, "AVAssetReaderTrackOutput"),
            (&AVPLAYERITEMLEGIBLEOUTPUT_CLASS, &AVPLAYERITEMLEGIBLEOUTPUT_META, "AVPlayerItemLegibleOutput"),
            (&AVPLAYERITEMVIDEOOUTPUT_CLASS, &AVPLAYERITEMVIDEOOUTPUT_META, "AVPlayerItemVideoOutput"),
            (&CKCONTAINER_CLASS, &CKCONTAINER_META, "CKContainer"),
            (&CKDATABASE_CLASS, &CKDATABASE_META, "CKDatabase"),
            (&CKMODIFYRECORDSOPERATION_CLASS, &CKMODIFYRECORDSOPERATION_META, "CKModifyRecordsOperation"),
            (&CKQUERY_CLASS, &CKQUERY_META, "CKQuery"),
            (&CKQUERYOPERATION_CLASS, &CKQUERYOPERATION_META, "CKQueryOperation"),
            (&CKRECORD_CLASS, &CKRECORD_META, "CKRecord"),
            (&CKRECORDID_CLASS, &CKRECORDID_META, "CKRecordID"),
            (&GCCONTROLLER_CLASS, &GCCONTROLLER_META, "GCController"),
        ] {
            objc_rt::register_builtin(cls.ptr(), meta.ptr(), name, 32, &[], &[]);
        }
        objc_rt::register_builtin(
            UIAPPLICATION_CLASS.ptr(),
            UIAPPLICATION_META.ptr(),
            "UIApplication",
            32,
            &[
                ("delegate", i(app_delegate as *const ())),
                ("setDelegate:", i(app_set_delegate as *const ())),
                ("windows", i(app_windows as *const ())),
                ("keyWindow", i(app_key_window as *const ())),
                ("connectedScenes", i(app_connected_scenes as *const ())),
                ("applicationState", i(app_state as *const ())),
                ("statusBarOrientation", i(app_orientation as *const ())),
                ("isStatusBarHidden", i(app_true as *const ())),
                ("setIdleTimerDisabled:", i(app_noop1 as *const ())),
                ("canOpenURL:", i(app_false1 as *const ())),
                ("openURL:", i(app_false1 as *const ())),
                ("isRegisteredForRemoteNotifications", i(app_false as *const ())),
                ("beginBackgroundTaskWithExpirationHandler:", i(app_begin_bg_task as *const ())),
                ("beginBackgroundTaskWithName:expirationHandler:", i(app_begin_bg_task2 as *const ())),
                ("endBackgroundTask:", i(app_noop1 as *const ())),
            ],
            &[("sharedApplication", i(shared_app_stub as *const ()))],
        );
        objc_rt::register_builtin(
            UISCREEN_CLASS.ptr(),
            UISCREEN_META.ptr(),
            "UIScreen",
            32,
            &[
                ("scale", i(screen_scale_stub as *const ())),
                ("nativeScale", i(screen_scale_stub as *const ())),
                ("bounds", i(screen_bounds as *const ())),
                ("nativeBounds", i(screen_native_bounds as *const ())),
                ("applicationFrame", i(screen_bounds as *const ())),
                ("maximumFramesPerSecond", i(screen_max_fps as *const ())),
                ("brightness", i(screen_brightness as *const ())),
                ("preferredMode", i(screen_current_mode as *const ())),
                ("currentMode", i(screen_current_mode as *const ())),
                ("setCurrentMode:", i(app_noop1 as *const ())),
                ("availableModes", i(screen_available_modes as *const ())),
                ("overscanCompensation", i(app_state as *const ())),
                ("setOverscanCompensation:", i(app_noop1 as *const ())),
            ],
            &[
                ("mainScreen", i(main_screen_stub as *const ())),
                ("screens", i(screen_screens_stub as *const ())),
            ],
        );
        objc_rt::register_builtin(
            UISCREENMODE_CLASS.ptr(),
            UISCREENMODE_META.ptr(),
            "UIScreenMode",
            16,
            &[
                ("size", i(screen_mode_size as *const ())),
                ("pixelAspectRatio", i(screen_mode_aspect as *const ())),
            ],
            &[],
        );
        objc_rt::register_builtin(
            UIDEVICE_CLASS.ptr(),
            UIDEVICE_META.ptr(),
            "UIDevice",
            32,
            &[
                ("systemName", i(device_system_name as *const ())),
                ("systemVersion", i(device_system_version as *const ())),
                ("model", i(device_model as *const ())),
                ("localizedModel", i(device_model as *const ())),
                ("name", i(device_name as *const ())),
                ("userInterfaceIdiom", i(device_idiom as *const ())),
                ("identifierForVendor", i(device_vendor_uuid as *const ())),
                ("isMultitaskingSupported", i(device_multitasking as *const ())),
                ("batteryLevel", i(device_battery_level as *const ())),
                ("batteryState", i(device_battery_state as *const ())),
                ("orientation", i(device_orientation as *const ())),
                ("setBatteryMonitoringEnabled:", i(app_noop1 as *const ())),
                ("beginGeneratingDeviceOrientationNotifications", i(noop_method as *const ())),
                ("endGeneratingDeviceOrientationNotifications", i(noop_method as *const ())),
                ("isGeneratingDeviceOrientationNotifications", i(app_false as *const ())),
            ],
            &[("currentDevice", i(current_device_stub as *const ()))],
        );
        objc_rt::register_builtin(
            UICOLOR_CLASS.ptr(),
            UICOLOR_META.ptr(),
            "UIColor",
            32,
            &[],
            &[
                ("colorWithRed:green:blue:alpha:", i(uicolor_rgba as *const ())),
                ("colorWithWhite:alpha:", i(uicolor_wa as *const ())),
                ("clearColor", i(uicolor_preset as *const ())),
                ("blackColor", i(uicolor_preset as *const ())),
                ("whiteColor", i(uicolor_preset as *const ())),
                ("grayColor", i(uicolor_preset as *const ())),
                ("redColor", i(uicolor_preset as *const ())),
                ("greenColor", i(uicolor_preset as *const ())),
                ("blueColor", i(uicolor_preset as *const ())),
            ],
        );
        objc_rt::register_builtin(
            UIVIEW_CLASS.ptr(),
            UIVIEW_META.ptr(),
            "UIView",
            64,
            &[
                ("initWithFrame:", i(uiview_init_frame as *const ())),
                ("bounds", i(uiview_bounds as *const ())),
                ("frame", i(uiview_bounds as *const ())),
                ("setFrame:", i(uiview_init_frame as *const ())),
                ("layer", i(uiview_layer as *const ())),
                ("addSubview:", i(uiview_add_subview as *const ())),
                ("subviews", i(uiview_subviews as *const ())),
                ("layoutSubviews", i(noop_method as *const ())),
                ("convertPoint:fromView:", i(uiview_convert_point as *const ())),
                ("convertPoint:toView:", i(uiview_convert_point as *const ())),
                ("convertPoint:fromView:", i(uiview_convert_point as *const ())),
                ("convertPoint:toView:", i(uiview_convert_point as *const ())),
                ("setBackgroundColor:", i(app_noop1 as *const ())),
                ("setOpaque:", i(app_noop1 as *const ())),
                ("setMultipleTouchEnabled:", i(app_noop1 as *const ())),
                ("setUserInteractionEnabled:", i(app_noop1 as *const ())),
                ("setContentScaleFactor:", i(app_noop1 as *const ())),
                ("contentScaleFactor", i(uiview_scale as *const ())),
                ("window", i(app_key_window as *const ())),
                ("layoutSubviews", i(noop_method as *const ())),
                ("addGestureRecognizer:", i(app_noop1 as *const ())),
                ("removeGestureRecognizer:", i(app_noop1 as *const ())),
                ("setAutoresizingMask:", i(app_noop1 as *const ())),
                ("setClipsToBounds:", i(app_noop1 as *const ())),
                ("setHidden:", i(app_noop1 as *const ())),
                ("isHidden", i(app_false as *const ())),
                ("setExclusiveTouch:", i(app_noop1 as *const ())),
                ("setIsAccessibilityElement:", i(app_noop1 as *const ())),
                ("setAccessibilityTraits:", i(app_noop1 as *const ())),
                ("layoutGuideForLayoutRegion:", i(uiview_layout_guide as *const ())),
                ("removeFromSuperview", i(uiview_remove_from_superview as *const ())),
                ("touchesBegan:withEvent:", i(touches_noop as *const ())),
                ("touchesMoved:withEvent:", i(touches_noop as *const ())),
                ("touchesEnded:withEvent:", i(touches_noop as *const ())),
                ("touchesCancelled:withEvent:", i(touches_noop as *const ())),
            ],
            &[("layerClass", i(uiview_layer_class as *const ()))],
        );
        extern "C" fn gr_init_target_action(self_obj: Id, _cmd: Sel, _t: Id, _a: Sel) -> Id {
            self_obj
        }
        extern "C" fn gr_state(_self: Id, _cmd: Sel) -> isize { 0 }
        extern "C" fn gr_loc(_self: Id, _cmd: Sel, _v: Id) -> pear_graphics::CGPoint {
            pear_graphics::CGPoint { x: 0.0, y: 0.0 }
        }
        extern "C" fn gr_noop_f64(_self: Id, _cmd: Sel, _v: f64) {}
        for (cls, meta, name) in [
            (&UIGESTURERECOGNIZER_CLASS, &UIGESTURERECOGNIZER_META, "UIGestureRecognizer"),
            (&UILONGPRESSGESTURERECOGNIZER_CLASS, &UILONGPRESSGESTURERECOGNIZER_META, "UILongPressGestureRecognizer"),
            (&UITAPGESTURERECOGNIZER_CLASS, &UITAPGESTURERECOGNIZER_META, "UITapGestureRecognizer"),
            (&UIPANGESTURERECOGNIZER_CLASS, &UIPANGESTURERECOGNIZER_META, "UIPanGestureRecognizer"),
        ] {
            objc_rt::register_builtin(
                cls.ptr(),
                meta.ptr(),
                name,
                32,
                &[
                    ("initWithTarget:action:", i(gr_init_target_action as *const ())),
                    ("addTarget:action:", i(gr_init_target_action as *const ())),
                    ("removeTarget:action:", i(gr_init_target_action as *const ())),
                    ("setDelegate:", i(app_noop1 as *const ())),
                    ("setEnabled:", i(app_noop1 as *const ())),
                    ("isEnabled", i(app_true as *const ())),
                    ("setMinimumPressDuration:", i(gr_noop_f64 as *const ())),
                    ("setAllowableMovement:", i(gr_noop_f64 as *const ())),
                    ("setNumberOfTapsRequired:", i(app_noop1 as *const ())),
                    ("setNumberOfTouchesRequired:", i(app_noop1 as *const ())),
                    ("setCancelsTouchesInView:", i(app_noop1 as *const ())),
                    ("cancelsTouchesInView", i(app_false as *const ())),
                    ("setDelaysTouchesBegan:", i(app_noop1 as *const ())),
                    ("setDelaysTouchesEnded:", i(app_noop1 as *const ())),
                    ("setRequiresExclusiveTouchType:", i(app_noop1 as *const ())),
                    ("state", i(gr_state as *const ())),
                    ("locationInView:", i(gr_loc as *const ())),
                ],
                &[],
            );
        }
        objc_rt::register_builtin(
            UIVIEWCONTROLLER_CLASS.ptr(),
            UIVIEWCONTROLLER_META.ptr(),
            "UIViewController",
            32,
            &[
                ("view", i(uivc_view as *const ())),
                ("setView:", i(uivc_set_view as *const ())),
                ("isViewLoaded", i(uivc_is_view_loaded as *const ())),
                ("viewIfLoaded", i(uivc_view_if_loaded as *const ())),
                ("loadView", i(noop_method as *const ())),
                ("viewDidLoad", i(noop_method as *const ())),
                ("viewWillAppear:", i(app_noop1 as *const ())),
                ("viewDidAppear:", i(app_noop1 as *const ())),
                ("viewWillDisappear:", i(app_noop1 as *const ())),
                ("viewDidDisappear:", i(app_noop1 as *const ())),
                ("viewWillLayoutSubviews", i(noop_method as *const ())),
                ("viewDidLayoutSubviews", i(noop_method as *const ())),
                ("interfaceOrientation", i(app_orientation as *const ())),
                ("preferredInterfaceOrientationForPresentation", i(app_orientation as *const ())),
                ("shouldAutorotate", i(app_true as *const ())),
                ("supportedInterfaceOrientations", i(app_orientation as *const ())),
                ("prefersStatusBarHidden", i(app_true as *const ())),
                ("prefersHomeIndicatorAutoHidden", i(app_true as *const ())),
                ("preferredStatusBarStyle", i(app_state as *const ())),
                ("setNeedsStatusBarAppearanceUpdate", i(noop_method as *const ())),
                ("setNeedsUpdateOfScreenEdgesDeferringSystemGestures", i(noop_method as *const ())),
                ("childViewControllers", i(ret_empty_array as *const ())),
                ("parentViewController", i(uivc_nil as *const ())),
                ("presentedViewController", i(uivc_nil as *const ())),
                ("presentingViewController", i(uivc_nil as *const ())),
                ("presentViewController:animated:completion:", i(uivc_present as *const ())),
                ("dismissViewControllerAnimated:completion:", i(uivc_dismiss as *const ())),
                ("touchesBegan:withEvent:", i(touches_noop as *const ())),
                ("touchesMoved:withEvent:", i(touches_noop as *const ())),
                ("touchesEnded:withEvent:", i(touches_noop as *const ())),
                ("touchesCancelled:withEvent:", i(touches_noop as *const ())),
            ],
            &[],
        );
        objc_rt::register_builtin(
            UITOUCH_CLASS.ptr(),
            UITOUCH_META.ptr(),
            "UITouch",
            32,
            &[
                ("locationInView:", i(uitouch_location_in_view as *const ())),
                ("previousLocationInView:", i(uitouch_prev_location_in_view as *const ())),
                ("timestamp", i(uitouch_timestamp as *const ())),
                ("phase", i(uitouch_phase as *const ())),
                ("tapCount", i(uitouch_tap_count as *const ())),
                ("view", i(uitouch_view as *const ())),
                ("window", i(uitouch_window as *const ())),
                ("dealloc", i(uitouch_dealloc as *const ())),
            ],
            &[],
        );
        objc_rt::register_builtin(
            UIEVENT_CLASS.ptr(),
            UIEVENT_META.ptr(),
            "UIEvent",
            32,
            &[
                ("allTouches", i(uievent_all_touches as *const ())),
                ("touchesForView:", i(uievent_touches_for as *const ())),
                ("touchesForWindow:", i(uievent_touches_for as *const ())),
                ("touchesForGestureRecognizer:", i(uievent_touches_for as *const ())),
                ("timestamp", i(uievent_timestamp as *const ())),
                ("dealloc", i(uievent_dealloc as *const ())),
            ],
            &[],
        );
        objc_rt::register_builtin(
            UILAYOUTGUIDE_CLASS.ptr(),
            UILAYOUTGUIDE_META.ptr(),
            "UILayoutGuide",
            32,
            &[
                ("layoutFrame", i(uiview_bounds as *const ())),
                ("owningView", i(uivc_nil as *const ())),
                ("identifier", i(uivc_nil as *const ())),
            ],
            &[],
        );
        objc_rt::register_builtin(
            UIVIEWLAYOUTREGION_CLASS.ptr(),
            UIVIEWLAYOUTREGION_META.ptr(),
            "UIViewLayoutRegion",
            16,
            &[],
            &[("marginsLayoutRegionWithCornerAdaptation:", i(ret_alloc1 as *const ()))],
        );
        objc_rt::register_builtin(
            CALAYER_CLASS.ptr(),
            CALAYER_META.ptr(),
            "CALayer",
            32,
            &[
                ("bounds", i(uiview_bounds as *const ())),
                ("frame", i(uiview_bounds as *const ())),
                ("contentsScale", i(uiview_scale as *const ())),
                ("setContentsScale:", i(app_noop1 as *const ())),
                ("setOpaque:", i(app_noop1 as *const ())),
                ("setBackgroundColor:", i(app_noop1 as *const ())),
            ],
            &[],
        );
        objc_rt::register_builtin(
            CAEAGLLAYER_CLASS.ptr(),
            CAEAGLLAYER_META.ptr(),
            "CAEAGLLayer",
            32,
            &[
                ("bounds", i(uiview_bounds as *const ())),
                ("frame", i(uiview_bounds as *const ())),
                ("contentsScale", i(uiview_scale as *const ())),
                ("setContentsScale:", i(app_noop1 as *const ())),
                ("setOpaque:", i(app_noop1 as *const ())),
                ("setBackgroundColor:", i(app_noop1 as *const ())),
                ("setContentsGravity:", i(app_noop1 as *const ())),
                ("setDrawableProperties:", i(app_noop1 as *const ())),
                ("device", i(cametal_device as *const ())),
                ("setDevice:", i(cametal_set_device as *const ())),
                ("pixelFormat", i(cametal_pixel_format as *const ())),
                ("setPixelFormat:", i(cametal_set_pixel_format as *const ())),
                ("drawableSize", i(cametal_drawable_size as *const ())),
                ("setDrawableSize:", i(cametal_set_drawable_size as *const ())),
                ("framebufferOnly", i(cametal_framebuffer_only as *const ())),
                ("setFramebufferOnly:", i(cametal_set_framebuffer_only as *const ())),
                ("nextDrawable", i(cametal_next_drawable as *const ())),
            ],
            &[],
        );
        objc_rt::register_builtin(
            CAMETALLAYER_CLASS.ptr(),
            CAMETALLAYER_META.ptr(),
            "CAMetalLayer",
            32,
            &[
                ("bounds", i(uiview_bounds as *const ())),
                ("frame", i(uiview_bounds as *const ())),
                ("contentsScale", i(uiview_scale as *const ())),
                ("setContentsScale:", i(app_noop1 as *const ())),
                ("setOpaque:", i(app_noop1 as *const ())),
                ("setBackgroundColor:", i(app_noop1 as *const ())),
                ("setContentsGravity:", i(app_noop1 as *const ())),
                ("setPresentsWithTransaction:", i(app_noop1 as *const ())),
                ("presentsWithTransaction", i(app_false as *const ())),
                ("setDrawsAsynchronously:", i(app_noop1 as *const ())),
                ("drawsAsynchronously", i(app_true as *const ())),
                ("device", i(cametal_device as *const ())),
                ("setDevice:", i(cametal_set_device as *const ())),
                ("pixelFormat", i(cametal_pixel_format as *const ())),
                ("setPixelFormat:", i(cametal_set_pixel_format as *const ())),
                ("drawableSize", i(cametal_drawable_size as *const ())),
                ("setDrawableSize:", i(cametal_set_drawable_size as *const ())),
                ("framebufferOnly", i(cametal_framebuffer_only as *const ())),
                ("setFramebufferOnly:", i(cametal_set_framebuffer_only as *const ())),
                ("nextDrawable", i(cametal_next_drawable as *const ())),
            ],
            &[("layer", i(cametal_layer as *const ()))],
        );
        objc_rt::register_builtin(
            CADISPLAYLINK_CLASS.ptr(),
            CADISPLAYLINK_META.ptr(),
            "CADisplayLink",
            32,
            &[
                ("addToRunLoop:forMode:", i(dl_add_runloop as *const ())),
                ("invalidate", i(dl_invalidate as *const ())),
                ("setPaused:", i(dl_set_paused as *const ())),
                ("setPreferredFramesPerSecond:", i(app_noop1 as *const ())),
                ("setFrameInterval:", i(app_noop1 as *const ())),
                ("frameInterval", i(app_orientation as *const ())),
                ("timestamp", i(dl_timestamp as *const ())),
                ("duration", i(dl_duration as *const ())),
            ],
            &[("displayLinkWithTarget:selector:", i(dl_create as *const ()))],
        );
        objc_rt::register_builtin(
            UIWINDOW_CLASS.ptr(),
            UIWINDOW_META.ptr(),
            "UIWindow",
            64,
            &[
                ("initWithFrame:", i(ui_window_init_with_frame as *const ())),
                ("initWithWindowScene:", i(ui_window_init_with_scene as *const ())),
                ("windowScene", i(ui_window_scene_stub as *const ())),
                ("screen", i(main_screen_stub as *const ())),
                ("makeKeyAndVisible", i(ui_window_make_key_and_visible as *const ())),
                ("bounds", i(uiview_bounds as *const ())),
                ("frame", i(uiview_bounds as *const ())),
                ("rootViewController", i(ui_window_root_vc as *const ())),
                ("setRootViewController:", i(ui_window_set_root_vc as *const ())),
                ("setBackgroundColor:", i(app_noop1 as *const ())),
                ("addSubview:", i(uiview_add_subview as *const ())),
                ("subviews", i(uiview_subviews as *const ())),
                ("layoutSubviews", i(noop_method as *const ())),
                ("convertPoint:fromView:", i(uiview_convert_point as *const ())),
                ("convertPoint:toView:", i(uiview_convert_point as *const ())),
                ("touchesBegan:withEvent:", i(touches_noop as *const ())),
                ("touchesMoved:withEvent:", i(touches_noop as *const ())),
                ("touchesEnded:withEvent:", i(touches_noop as *const ())),
                ("touchesCancelled:withEvent:", i(touches_noop as *const ())),
            ],
            &[],
        );
        objc_rt::register_builtin(
            UIWINDOWSCENE_CLASS.ptr(),
            UIWINDOWSCENE_META.ptr(),
            "UIWindowScene",
            32,
            &[
                ("screen", i(main_screen_stub as *const ())),
                ("windows", i(app_windows as *const ())),
                ("coordinateSpace", i(main_screen_stub as *const ())),
                ("bounds", i(screen_bounds as *const ())),
                ("delegate", i(scene_delegate_get as *const ())),
                ("setDelegate:", i(scene_delegate_set as *const ())),
                ("activationState", i(app_state as *const ())),
                ("interfaceOrientation", i(app_orientation as *const ())),
                ("sizeRestrictions", i(uivc_nil as *const ())),
                ("statusBarManager", i(uivc_nil as *const ())),
            ],
            &[],
        );
        objc_rt::register_builtin(
            UISCENESESSION_CLASS.ptr(),
            UISCENESESSION_META.ptr(),
            "UISceneSession",
            16,
            &[("role", i(scene_session_role as *const ()))],
            &[],
        );
        objc_rt::register_builtin(
            UISCENECONNECTIONOPTIONS_CLASS.ptr(),
            UISCENECONNECTIONOPTIONS_META.ptr(),
            "UISceneConnectionOptions",
            16,
            &[
                ("URLContexts", i(ret_empty_set as *const ())),
                ("userActivities", i(ret_empty_set as *const ())),
            ],
            &[],
        );
extern "C" fn ret_self1(self_obj: Id, _cmd: Sel, _arg: Id) -> Id {
    self_obj
}
extern "C" fn uiview_layout_guide(_self_obj: Id, _cmd: Sel, _region: Id) -> Id {
    unsafe { alloc(UILAYOUTGUIDE_CLASS.ptr()) }
}
extern "C" fn ret_alloc1(cls: Id, _cmd: Sel, _arg: Id) -> Id {
    unsafe { alloc(cls as *const ClassT) }
}
extern "C" fn ret_singleton(cls: Id, _cmd: Sel) -> Id {
    unsafe { alloc(cls as *const ClassT) }
}
extern "C" fn ret_empty_array(_self: Id, _cmd: Sel) -> Id {
    unsafe {
        let cls = objc_rt::class_named("NSArray").unwrap_or(std::ptr::null());
        if cls.is_null() { return std::ptr::null_mut(); }
        let d = pear_libobjc::message::lookup_for_send(cls as Id, objc_rt::sel("array"));
        let f: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d.imp);
        f(d.receiver, objc_rt::sel("array"))
    }
}
extern "C" fn noop3(_self: Id, _cmd: Sel, _a: isize, _b: isize, _c: isize) -> bool { true }
extern "C" fn noop4(_self: Id, _cmd: Sel, _a: isize, _b: isize, _c: isize, _d: isize) -> bool { true }

        objc_rt::register_builtin(
            EAGLCONTEXT_CLASS.ptr(),
            EAGLCONTEXT_META.ptr(),
            "EAGLContext",
            64,
            &[
                ("initWithAPI:", i(eagl_init_with_api as *const ())),
                ("renderbufferStorage:fromDrawable:", i(eagl_renderbuffer_storage as *const ())),
                ("presentRenderbuffer:", i(eagl_present_renderbuffer as *const ())),
            ],
            &[
                ("setCurrentContext:", i(eagl_set_current_context as *const ())),
                ("currentContext", i(eagl_current_context as *const ())),
            ],
        );
        objc_rt::register_builtin(
            SKPAYMENTQUEUE_CLASS.ptr(),
            SKPAYMENTQUEUE_META.ptr(),
            "SKPaymentQueue",
            32,
            &[
                ("addTransactionObserver:", i(app_noop1 as *const ())),
                ("removeTransactionObserver:", i(app_noop1 as *const ())),
                ("addPayment:", i(app_noop1 as *const ())),
                ("finishTransaction:", i(app_noop1 as *const ())),
                ("restoreCompletedTransactions", i(noop_method as *const ())),
                ("transactions", i(ret_empty_array as *const ())),
            ],
            &[
                ("defaultQueue", i(ret_singleton as *const ())),
                ("canMakePayments", i(app_false as *const ())),
            ],
        );
        objc_rt::register_builtin(
            SKPRODUCTSREQUEST_CLASS.ptr(),
            SKPRODUCTSREQUEST_META.ptr(),
            "SKProductsRequest",
            32,
            &[
                ("initWithProductIdentifiers:", i(ret_self1 as *const ())),
                ("setDelegate:", i(app_noop1 as *const ())),
                ("start", i(noop_method as *const ())),
                ("cancel", i(noop_method as *const ())),
            ],
            &[],
        );
        objc_rt::register_builtin(
            SKREQUEST_CLASS.ptr(),
            SKREQUEST_META.ptr(),
            "SKRequest",
            32,
            &[
                ("setDelegate:", i(app_noop1 as *const ())),
                ("start", i(noop_method as *const ())),
                ("cancel", i(noop_method as *const ())),
            ],
            &[],
        );
        objc_rt::register_builtin(
            SKPAYMENT_CLASS.ptr(),
            SKPAYMENT_META.ptr(),
            "SKPayment",
            32,
            &[("productIdentifier", i(ret_singleton as *const ()))],
            &[
                ("paymentWithProduct:", i(ret_self1 as *const ())),
                ("paymentWithProductIdentifier:", i(ret_self1 as *const ())),
            ],
        );
        objc_rt::register_builtin(
            AVAUDIOSESSIONROUTEDESCRIPTION_CLASS.ptr(),
            AVAUDIOSESSIONROUTEDESCRIPTION_META.ptr(),
            "AVAudioSessionRouteDescription",
            32,
            &[
                ("outputs", i(ret_empty_array as *const ())),
                ("inputs", i(ret_empty_array as *const ())),
            ],
            &[],
        );
        objc_rt::register_builtin(
            AVAUDIOSESSION_CLASS.ptr(),
            AVAUDIOSESSION_META.ptr(),
            "AVAudioSession",
            32,
            &[
                ("setCategory:error:", i(noop3 as *const ())),
                ("setCategory:withOptions:error:", i(noop4 as *const ())),
                ("setCategory:mode:options:error:", i(noop4 as *const ())),
                ("setActive:error:", i(noop3 as *const ())),
                ("setActive:withOptions:error:", i(noop4 as *const ())),
                ("isOtherAudioPlaying", i(app_false as *const ())),
                ("secondaryAudioShouldBeSilencedHint", i(app_false as *const ())),
                ("currentRoute", i(avaudiosession_current_route as *const ())),
            ],
            &[("sharedInstance", i(ret_singleton as *const ()))],
        );
        objc_rt::register_builtin(
            UIACCELEROMETER_CLASS.ptr(),
            UIACCELEROMETER_META.ptr(),
            "UIAccelerometer",
            32,
            &[
                ("setUpdateInterval:", i(app_noop1 as *const ())),
                ("setDelegate:", i(app_noop1 as *const ())),
            ],
            &[("sharedAccelerometer", i(ret_singleton as *const ()))],
        );
        objc_rt::register_builtin(
            CATRANSACTION_CLASS.ptr(),
            CATRANSACTION_META.ptr(),
            "CATransaction",
            16,
            &[],
            &[
                ("begin", i(noop_method as *const ())),
                ("commit", i(noop_method as *const ())),
                ("flush", i(noop_method as *const ())),
                ("setDisableActions:", i(app_noop1 as *const ())),
            ],
        );
        extern "C" fn alert_init(
            self_obj: Id,
            _cmd: Sel,
            title: Id,
            message: Id,
            _del: Id,
            _cancel: Id,
            _other: Id,
        ) -> Id {
            let t = pear_foundation::rust_str_from_nsstring(title).unwrap_or_default();
            let m = pear_foundation::rust_str_from_nsstring(message).unwrap_or_default();
            eprintln!("[UIAlertView] title={t:?} message={m:?}");
            self_obj
        }
        extern "C" fn alert_add_btn(_self: Id, _cmd: Sel, _t: Id) -> isize { 0 }
        extern "C" fn alert_dismiss(_self: Id, _cmd: Sel, _idx: isize, _anim: bool) {}
        objc_rt::register_builtin(
            UIALERTVIEW_CLASS.ptr(),
            UIALERTVIEW_META.ptr(),
            "UIAlertView",
            32,
            &[
                ("initWithTitle:message:delegate:cancelButtonTitle:otherButtonTitles:", i(alert_init as *const ())),
                ("setTitle:", i(app_noop1 as *const ())),
                ("setMessage:", i(app_noop1 as *const ())),
                ("setDelegate:", i(app_noop1 as *const ())),
                ("addButtonWithTitle:", i(alert_add_btn as *const ())),
                ("show", i(noop_method as *const ())),
                ("dismissWithClickedButtonIndex:animated:", i(alert_dismiss as *const ())),
            ],
            &[],
        );
        objc_rt::register_builtin(
            OS_DISPATCH_SEMAPHORE_CLASS.ptr(),
            OS_DISPATCH_SEMAPHORE_META.ptr(),
            "OS_dispatch_semaphore",
            16,
            &[("dealloc", i(dispatch_semaphore_dealloc as *const ()))],
            &[],
        );
        objc_rt::register_builtin(
            OS_DISPATCH_GROUP_CLASS.ptr(),
            OS_DISPATCH_GROUP_META.ptr(),
            "OS_dispatch_group",
            16,
            &[("dealloc", i(dispatch_group_dealloc as *const ()))],
            &[],
        );

        fn register_stub_class(name: &str, super_cls: *const ClassT, instance_size: usize) {
            if objc_rt::class_named(name).is_some() {
                return;
            }
            let sup = if super_cls.is_null() { NSOBJECT_CLASS.ptr() } else { super_cls };
            let sup_meta = unsafe { (*sup).isa };
            let meta = Box::leak(Box::new(ClassT {
                isa: NSOBJECT_META.ptr(),
                superclass: if sup_meta.is_null() { NSOBJECT_META.ptr() } else { sup_meta },
                cache: &raw const EMPTY_CACHE as *const c_void,
                vtable: std::ptr::null(),
                bits: 0,
            }));
            let cls = Box::leak(Box::new(ClassT {
                isa: meta,
                superclass: sup,
                cache: &raw const EMPTY_CACHE as *const c_void,
                vtable: std::ptr::null(),
                bits: 0,
            }));
            objc_rt::register_builtin(cls, meta, name, instance_size, &[], &[]);
        }

        let ns_obj = NSOBJECT_CLASS.ptr();
        let ui_view = UIVIEW_CLASS.ptr();
        let ui_scroll = UISCROLLVIEW_CLASS.ptr();
        let ui_ctrl = UICONTROL_CLASS.ptr();
        let ui_vc = UIVIEWCONTROLLER_CLASS.ptr();
        let ui_gr = UIGESTURERECOGNIZER_CLASS.ptr();
        let ca_layer = CALAYER_CLASS.ptr();

        frameworks::register_foundation_collection_classes();

        // Foundation / CFNetwork base classes
        for name in [
            "NSProxy", "NSOperation", "NSHTTPURLResponse", "NSURLResponse",
            "NSURLCache", "NSCachedURLResponse", "NSURLProtocol", "NSURLConnection",
            "NSURLCredential", "NSURLSessionTask", "NSHTTPCookie", "NSHTTPCookieStorage",
            "NSNetService", "NSInputStream", "NSOutputStream", "NSInvocation",
            "NSMethodSignature", "NSCache", "NSDateFormatter", "NSISO8601DateFormatter",
            "NSByteCountFormatter", "NSDecimalNumberHandler", "NSFileHandle",
            "NSHashTable", "NSMapTable", "NSPointerArray", "NSIndexSet",
            "NSCountedSet", "NSAttributedString", "NSCharacterSet",
            "NSKeyedArchiver", "NSLayoutConstraint", "NSLock", "NSRecursiveLock",
            "NSMachPort", "NSNotification", "NSNotificationQueue", "NSProgress",
            "NSPropertyListSerialization", "NSRegularExpression", "NSScanner",
            "NSSortDescriptor", "NSTextCheckingResult", "NSURLComponents",
            "NSURLQueryItem", "NSUserActivity", "NSValue", "NSExpression", "NSTimeZone",
        ] {
            register_stub_class(name, ns_obj, 32);
        }
        register_stub_class("NSBlockOperation", objc_rt::class_named("NSOperation").unwrap_or(ns_obj), 32);
        register_stub_class("NSInvocationOperation", objc_rt::class_named("NSOperation").unwrap_or(ns_obj), 32);
        register_stub_class("NSURLSessionDataTask", objc_rt::class_named("NSURLSessionTask").unwrap_or(ns_obj), 32);
        register_stub_class("NSMutableIndexSet", objc_rt::class_named("NSIndexSet").unwrap_or(ns_obj), 32);
        register_stub_class("NSMutableAttributedString", objc_rt::class_named("NSAttributedString").unwrap_or(ns_obj), 32);
        register_stub_class("NSMutableCharacterSet", objc_rt::class_named("NSCharacterSet").unwrap_or(ns_obj), 32);
        register_stub_class("NSMutableOrderedSet", objc_rt::class_named("NSSet").unwrap_or(ns_obj), 32);
        register_stub_class("NSDecimalNumber", objc_rt::class_named("NSNumber").unwrap_or(ns_obj), 32);
        register_stub_class("NSConstantIntegerNumber", objc_rt::class_named("NSNumber").unwrap_or(ns_obj), 32);

        // UIKit / WebKit / QuartzCore base classes
        for name in [
            "UIImageView", "UICollectionViewCell", "UITableViewCell",
            "UIActivityIndicatorView", "UIProgressView", "UIStackView",
            "UIVisualEffectView", "UIToolbar", "WKWebView",
        ] {
            register_stub_class(name, ui_view, 64);
        }
        for name in ["UICollectionView", "UITableView"] {
            register_stub_class(name, ui_scroll, 64);
        }
        for name in ["UIDatePicker", "UISwitch", "UIPickerView"] {
            register_stub_class(name, ui_ctrl, 64);
        }
        for name in ["UIImagePickerController", "UIPageViewController", "UITabBarController"] {
            register_stub_class(name, ui_vc, 32);
        }
        register_stub_class("UISwipeGestureRecognizer", ui_gr, 32);
        for name in ["CAReplicatorLayer", "CAShapeLayer"] {
            register_stub_class(name, ca_layer, 32);
        }
        for name in [
            "CABasicAnimation", "CAKeyframeAnimation", "CAMediaTimingFunction",
            "UICollectionViewFlowLayout", "UIActivity", "UIAlertAction",
            "UIBarButtonItem", "UIBezierPath", "UIGraphicsImageRenderer",
            "UIGraphicsImageRendererFormat", "UILocalNotification", "UIPopoverController",
            "UIScene", "UISceneConfiguration", "UIUserNotificationSettings", "UIViewLayoutRegion",
            "NSLayoutManager", "NSMutableParagraphStyle", "NSTextAttachment",
            "NSTextContainer", "NSTextStorage",
            "WKWebViewConfiguration", "WKUserContentController", "WKUserScript",
            "WKNavigationAction", "WKNavigationResponse", "WKWebsiteDataStore",
            // UserNotifications, StoreKit, CoreImage, CoreTelephony, MediaPlayer, JSC, Safari, etc.
            "UNUserNotificationCenter", "UNMutableNotificationContent", "UNNotificationRequest",
            "UNNotificationSound", "UNNotificationAttachment",
            "UNTimeIntervalNotificationTrigger", "UNCalendarNotificationTrigger", "UNPushNotificationTrigger",
            "SKAdNetwork", "SKMutablePayment", "SKPaymentTransaction",
            "SKProductSubscriptionPeriod", "SKReceiptRefreshRequest",
            "CIColor", "CIContext", "CIFilter", "CIImage",
            "CTCallCenter", "CTCarrier", "CTTelephonyNetworkInfo",
            "MPMediaItemArtwork", "MPMoviePlayerController", "MPNowPlayingInfoCenter", "MPRemoteCommandCenter",
            "JSContext", "JSValue", "SFSafariViewController", "ASIdentifierManager",
            "ASWebAuthenticationSession", "GKLocalPlayer", "SLComposeViewController",
            // Photos, PhotosUI, CoreHaptics, AVFoundation, AuthenticationServices, GameController, MetalFX, MetricKit, CoreMotion, AdSupport/AdServices
            "PHAsset", "PHAssetChangeRequest", "PHAssetCollection", "PHAssetCollectionChangeRequest",
            "PHAssetResource", "PHAssetResourceManager", "PHFetchOptions", "PHImageManager",
            "PHImageRequestOptions", "PHLivePhoto", "PHPhotoLibrary", "PHVideoRequestOptions",
            "PHPickerConfiguration", "PHPickerFilter", "PHPickerViewController",
            "CHHapticEngine", "CHHapticEvent", "CHHapticEventParameter", "CHHapticPattern",
            "AVAssetImageGenerator", "AVCaptureDevice", "AVMutableAudioMix",
            "AVMutableAudioMixInputParameters", "AVPlayerLayer",
            "ASAuthorizationAppleIDCredential", "ASAuthorizationAppleIDProvider", "ASAuthorizationController",
            "ATTrackingManager", "CMMotionManager", "AAAttribution",
            "GCDualShockGamepad", "GCXboxGamepad", "UNLocationNotificationTrigger",
            "MTLFXSpatialScalerDescriptor", "MTLFXTemporalScalerDescriptor",
            "MXDiagnosticPayload", "MXMetricManager", "MXMetricPayload",
        ] {
            register_stub_class(name, ns_obj, 32);
        }
    });
}

extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn calloc(count: usize, size: usize) -> *mut c_void;
    fn realloc(p: *mut c_void, size: usize) -> *mut c_void;
    fn free(p: *mut c_void);
    fn memcpy(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void;
    fn memset(dst: *mut c_void, c: i32, n: usize) -> *mut c_void;
    fn strlen(s: *const c_char) -> usize;
    fn strcmp(a: *const c_char, b: *const c_char) -> i32;
    fn getenv(name: *const c_char) -> *const c_char;
    fn abort() -> !;
    fn dlopen(filename: *const c_char, flags: i32) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dladdr(addr: *const c_void, info: *mut DlInfo) -> i32;
    fn posix_memalign(memptr: *mut *mut c_void, alignment: usize, size: usize) -> i32;
    fn srand(seed: u32);
    #[cfg(target_os = "macos")]
    fn malloc_size(ptr: *const c_void) -> usize;
    #[cfg(target_os = "linux")]
    fn malloc_usable_size(ptr: *mut c_void) -> usize;
    fn signal(sig: i32, handler: usize) -> usize;
    fn backtrace(array: *mut *mut c_void, size: i32) -> i32;
    fn backtrace_symbols_fd(array: *const *mut c_void, size: i32, fd: i32);
    fn _exit(status: i32) -> !;
}

fn dump_guest_backtrace(reason: &str) {
    eprintln!("[PearRuntime][crash] {reason}");
    let mut frames = [std::ptr::null_mut::<c_void>(); 32];
    let n = unsafe { backtrace(frames.as_mut_ptr(), 32) };
    if let Ok(guard) = LOADED_IMAGES.try_lock() {
        for (idx, &pc) in frames.iter().take(n.max(0) as usize).enumerate() {
            let addr = pc as usize;
            let mut matched = false;
            for img in guard.iter() {
                if addr >= img.addr_start && addr < img.addr_end {
                    eprintln!(
                        "  frame[{idx}] {addr:#x} -> {} + {:#x} (fileoff ~ {:#x})",
                        img.name.to_string_lossy(),
                        addr - img.header,
                        addr.wrapping_sub(img.slide as usize)
                    );
                    matched = true;
                    break;
                }
            }
            if !matched {
                eprintln!("  frame[{idx}] {addr:#x} (host)");
            }
        }
    }
    unsafe {
        backtrace_symbols_fd(frames.as_ptr(), n, 2);
    }
}

extern "C" fn pear_crash_handler(sig: i32) {
    dump_guest_backtrace(&format!("caught signal {sig}"));
    unsafe {
        _exit(128 + sig);
    }
}

extern "C" fn guest_abort() -> ! {
    dump_guest_backtrace("guest called abort()");
    std::process::abort()
}

pub fn install_crash_diagnostics() {
    unsafe {
        for sig in [4, 6, 10, 11] {
            // SIGILL=4, SIGABRT=6, SIGBUS=10(macOS)/7(linux), SIGSEGV=11
            signal(sig, pear_crash_handler as *const () as usize);
        }
        #[cfg(target_os = "linux")]
        signal(7, pear_crash_handler as *const () as usize);
    }
}

#[repr(C)]
pub struct DlInfo {
    pub dli_fname: *const c_char,
    pub dli_fbase: *mut c_void,
    pub dli_sname: *const c_char,
    pub dli_saddr: *mut c_void,
}

fn resolve_raw_gles_proc(cname: &str) -> usize {
    static LIBS: std::sync::OnceLock<(usize, usize, usize)> = std::sync::OnceLock::new();
    let (h_v1, h_v2, egl_get_proc) = *LIBS.get_or_init(|| unsafe {
        let mut h1 = std::ptr::null_mut();
        let mut h2 = std::ptr::null_mut();
        if let Ok(custom) = std::env::var("PEAR_GLES_LIB") {
            if let Ok(c) = std::ffi::CString::new(custom) {
                h2 = dlopen(c.as_ptr(), 1);
            }
        }
        #[cfg(target_os = "macos")]
        {
            if h2.is_null() {
                for cand in [c"libGLESv2.dylib".as_ptr(), c"build/host/PearANGLE/libGLESv2.dylib".as_ptr()] {
                    h2 = dlopen(cand, 1);
                    if !h2.is_null() { break; }
                }
            }
            for cand in [c"libGLESv1_CM.dylib".as_ptr()] {
                h1 = dlopen(cand, 1);
                if !h1.is_null() { break; }
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            for cand in [c"libGLESv1_CM.so.1".as_ptr(), c"libGLESv1_CM.so".as_ptr()] {
                h1 = dlopen(cand, 1);
                if !h1.is_null() { break; }
            }
            if h2.is_null() {
                for cand in [c"libGLESv2.so.2".as_ptr(), c"libGLESv2.so".as_ptr()] {
                    h2 = dlopen(cand, 1);
                    if !h2.is_null() { break; }
                }
            }
        }
        let mut egl_get = 0usize;
        #[cfg(not(target_os = "macos"))]
        {
            let mut hegl = dlopen(c"libEGL.so.1".as_ptr(), 1);
            if hegl.is_null() { hegl = dlopen(c"libEGL.so".as_ptr(), 1); }
            if !hegl.is_null() {
                egl_get = dlsym(hegl, c"eglGetProcAddress".as_ptr()) as usize;
            }
        }
        (h1 as usize, h2 as usize, egl_get)
    });
    if let Ok(cstr) = std::ffi::CString::new(cname) {
        for h in [h_v1, h_v2] {
            if h != 0 {
                let p = unsafe { dlsym(h as *mut c_void, cstr.as_ptr()) };
                if !p.is_null() {
                    return p as usize;
                }
            }
        }
        if egl_get_proc != 0 {
            let get_proc: unsafe extern "C" fn(*const c_char) -> *mut c_void =
                unsafe { std::mem::transmute(egl_get_proc) };
            let p = unsafe { get_proc(cstr.as_ptr()) };
            if !p.is_null() {
                return p as usize;
            }
        }
    }
    0
}

fn eagl_real_bind_fbo(target: u32, fbo: u32) {
    let p = resolve_raw_gles_proc("glBindFramebufferOES");
    let p = if p != 0 { p } else { resolve_raw_gles_proc("glBindFramebuffer") };
    if p != 0 {
        let f: extern "C" fn(u32, u32) = unsafe { std::mem::transmute(p) };
        f(target, fbo);
    }
}

extern "C" fn eagl_gl_bind_framebuffer_oes(target: u32, fbo: u32) {
    EAGL_LAST_BOUND_FBO.store(fbo, std::sync::atomic::Ordering::Release);
    let win_fbo = EAGL_WINDOW_FBO.load(std::sync::atomic::Ordering::Acquire);
    let actual = if fbo != 0 && fbo == win_fbo { 0 } else { fbo };
    eagl_real_bind_fbo(target, actual);
}

extern "C" fn eagl_gl_framebuffer_renderbuffer_oes(target: u32, attach: u32, rbt: u32, rbo: u32) {
    let cur = EAGL_LAST_BOUND_FBO.load(std::sync::atomic::Ordering::Acquire);
    let win_fbo = EAGL_WINDOW_FBO.load(std::sync::atomic::Ordering::Acquire);
    if cur != 0 && cur == win_fbo {
        return;
    }
    let p = resolve_raw_gles_proc("glFramebufferRenderbufferOES");
    let p = if p != 0 { p } else { resolve_raw_gles_proc("glFramebufferRenderbuffer") };
    if p != 0 {
        let f: extern "C" fn(u32, u32, u32, u32) = unsafe { std::mem::transmute(p) };
        f(target, attach, rbt, rbo);
    }
}

extern "C" fn eagl_gl_check_framebuffer_status_oes(target: u32) -> u32 {
    let cur = EAGL_LAST_BOUND_FBO.load(std::sync::atomic::Ordering::Acquire);
    let win_fbo = EAGL_WINDOW_FBO.load(std::sync::atomic::Ordering::Acquire);
    if cur == 0 || cur == win_fbo {
        return 0x8CD5; // GL_FRAMEBUFFER_COMPLETE_OES
    }
    let p = resolve_raw_gles_proc("glCheckFramebufferStatusOES");
    let p = if p != 0 { p } else { resolve_raw_gles_proc("glCheckFramebufferStatus") };
    if p != 0 {
        let f: extern "C" fn(u32) -> u32 = unsafe { std::mem::transmute(p) };
        f(target)
    } else {
        0x8CD5
    }
}

extern "C" fn eagl_gl_get_renderbuffer_param_oes(target: u32, pname: u32, params: *mut i32) {
    if !params.is_null() {
        let packed = EAGL_SURFACE_SIZE.load(std::sync::atomic::Ordering::Acquire);
        let w = (packed >> 32) as i32;
        let h = (packed & 0xFFFF_FFFF) as i32;
        match pname {
            0x8D42 => { unsafe { *params = w }; return; } // GL_RENDERBUFFER_WIDTH_OES
            0x8D43 => { unsafe { *params = h }; return; } // GL_RENDERBUFFER_HEIGHT_OES
            0x8D44 => { unsafe { *params = 0x8058 }; return; } // GL_RGBA8_OES
            _ => {}
        }
    }
    let p = resolve_raw_gles_proc("glGetRenderbufferParameterivOES");
    let p = if p != 0 { p } else { resolve_raw_gles_proc("glGetRenderbufferParameteriv") };
    if p != 0 {
        let f: extern "C" fn(u32, u32, *mut i32) = unsafe { std::mem::transmute(p) };
        f(target, pname, params);
    }
}

fn gles_dlsym(macho_sym: &str) -> usize {
    match macho_sym {
        "_glBindFramebufferOES" | "_glBindFramebuffer" => return eagl_gl_bind_framebuffer_oes as *const () as usize,
        "_glFramebufferRenderbufferOES" | "_glFramebufferRenderbuffer" => {
            return eagl_gl_framebuffer_renderbuffer_oes as *const () as usize;
        }
        "_glCheckFramebufferStatusOES" | "_glCheckFramebufferStatus" => {
            return eagl_gl_check_framebuffer_status_oes as *const () as usize;
        }
        "_glGetRenderbufferParameterivOES" | "_glGetRenderbufferParameteriv" => {
            return eagl_gl_get_renderbuffer_param_oes as *const () as usize;
        }
        _ => {}
    }
    if let Some(cname) = macho_sym.strip_prefix('_') {
        let p = resolve_raw_gles_proc(cname);
        if p != 0 {
            return p;
        }
    }
    status_ok_stub as *const () as usize
}

mod darwin_varargs {
    use super::{Id, STDERR_STUB, STDIN_STUB, STDOUT_STUB};
    use std::ffi::{c_char, c_int, c_void, CStr};

    extern "C" {
        fn snprintf(s: *mut c_char, n: usize, fmt: *const c_char, ...) -> c_int;
        fn sscanf(s: *const c_char, fmt: *const c_char, ...) -> c_int;
        fn fscanf(stream: *mut c_void, fmt: *const c_char, ...) -> c_int;
        fn fwrite(ptr: *const c_void, size: usize, nmemb: usize, stream: *mut c_void) -> usize;
    }

    pub unsafe fn format_darwin_printf(fmt: *const c_char, mut va: *const u64) -> Vec<u8> {
        if fmt.is_null() {
            return Vec::new();
        }
        let fbytes = CStr::from_ptr(fmt).to_bytes();
        let mut out = Vec::with_capacity(fbytes.len() + 32);
        let mut i = 0usize;
        let pop_u64 = |cursor: &mut *const u64| -> u64 {
            if cursor.is_null() {
                0
            } else {
                let v = **cursor;
                *cursor = (*cursor).add(1);
                v
            }
        };

        while i < fbytes.len() {
            if fbytes[i] != b'%' {
                out.push(fbytes[i]);
                i += 1;
                continue;
            }
            if i + 1 < fbytes.len() && fbytes[i + 1] == b'%' {
                out.push(b'%');
                i += 2;
                continue;
            }
            let start = i;
            i += 1;
            let mut sub = String::from("%");
            // flags
            while i < fbytes.len() && matches!(fbytes[i], b'-' | b'+' | b' ' | b'#' | b'0' | b'\'') {
                sub.push(fbytes[i] as char);
                i += 1;
            }
            // width
            if i < fbytes.len() && fbytes[i] == b'*' {
                let w = pop_u64(&mut va) as i32;
                sub.push_str(&w.to_string());
                i += 1;
            } else {
                while i < fbytes.len() && fbytes[i].is_ascii_digit() {
                    sub.push(fbytes[i] as char);
                    i += 1;
                }
            }
            // precision
            if i < fbytes.len() && fbytes[i] == b'.' {
                sub.push('.');
                i += 1;
                if i < fbytes.len() && fbytes[i] == b'*' {
                    let p = pop_u64(&mut va) as i32;
                    sub.push_str(&p.to_string());
                    i += 1;
                } else {
                    while i < fbytes.len() && fbytes[i].is_ascii_digit() {
                        sub.push(fbytes[i] as char);
                        i += 1;
                    }
                }
            }
            // length
            let mut is_long = false;
            if i < fbytes.len() {
                let len_start = i;
                while i < fbytes.len() && matches!(fbytes[i], b'h' | b'l' | b'z' | b't' | b'j' | b'L' | b'q') {
                    sub.push(fbytes[i] as char);
                    i += 1;
                }
                let lslice = &fbytes[len_start..i];
                if lslice == b"l" || lslice == b"ll" || lslice == b"z" || lslice == b"t" || lslice == b"j" || lslice == b"q" {
                    is_long = true;
                }
            }
            if i >= fbytes.len() {
                out.extend_from_slice(&fbytes[start..]);
                break;
            }
            let conv = fbytes[i];
            i += 1;
            sub.push(conv as char);

            if conv == b'@' {
                let obj = pop_u64(&mut va) as Id;
                if obj.is_null() {
                    out.extend_from_slice(b"(null)");
                } else if let Some(s) = pear_foundation::rust_str_from_nsstring(obj) {
                    out.extend_from_slice(s.as_bytes());
                } else {
                    out.extend_from_slice(b"<object>");
                }
                continue;
            }
            if conv == b'n' {
                let _ = pop_u64(&mut va);
                continue;
            }

            let Ok(csub) = std::ffi::CString::new(sub) else { continue };
            let slot = pop_u64(&mut va);
            let mut buf = [0 as c_char; 1024];
            let n = match conv {
                b'd' | b'i' => {
                    if is_long {
                        snprintf(buf.as_mut_ptr(), buf.len(), csub.as_ptr(), slot as i64)
                    } else {
                        snprintf(buf.as_mut_ptr(), buf.len(), csub.as_ptr(), slot as i32)
                    }
                }
                b'u' | b'o' | b'x' | b'X' => {
                    if is_long {
                        snprintf(buf.as_mut_ptr(), buf.len(), csub.as_ptr(), slot as u64)
                    } else {
                        snprintf(buf.as_mut_ptr(), buf.len(), csub.as_ptr(), slot as u32)
                    }
                }
                b'p' => snprintf(buf.as_mut_ptr(), buf.len(), csub.as_ptr(), slot as *const c_void),
                b's' => {
                    let sptr = slot as *const c_char;
                    if sptr.is_null() {
                        out.extend_from_slice(b"(null)");
                        continue;
                    }
                    let slen = CStr::from_ptr(sptr).to_bytes().len();
                    if slen + 64 < buf.len() {
                        snprintf(buf.as_mut_ptr(), buf.len(), csub.as_ptr(), sptr)
                    } else {
                        let mut dyn_buf = vec![0 as c_char; slen + 256];
                        let dn = snprintf(dyn_buf.as_mut_ptr(), dyn_buf.len(), csub.as_ptr(), sptr);
                        if dn > 0 {
                            let take = (dn as usize).min(dyn_buf.len() - 1);
                            out.extend_from_slice(std::slice::from_raw_parts(dyn_buf.as_ptr() as *const u8, take));
                        }
                        continue;
                    }
                }
                b'c' => snprintf(buf.as_mut_ptr(), buf.len(), csub.as_ptr(), slot as i32),
                b'f' | b'F' | b'e' | b'E' | b'g' | b'G' | b'a' | b'A' => {
                    snprintf(buf.as_mut_ptr(), buf.len(), csub.as_ptr(), f64::from_bits(slot))
                }
                _ => {
                    out.push(b'%');
                    out.push(conv);
                    continue;
                }
            };
            if n > 0 {
                let take = (n as usize).min(buf.len() - 1);
                out.extend_from_slice(std::slice::from_raw_parts(buf.as_ptr() as *const u8, take));
            }
        }
        out
    }

    pub unsafe extern "C" fn darwin_vsnprintf(
        dst: *mut c_char,
        size: usize,
        fmt: *const c_char,
        ap: *const u64,
    ) -> c_int {
        let out = format_darwin_printf(fmt, ap);
        if !dst.is_null() && size > 0 {
            let copy_len = out.len().min(size - 1);
            if copy_len > 0 {
                std::ptr::copy_nonoverlapping(out.as_ptr(), dst as *mut u8, copy_len);
            }
            *dst.add(copy_len) = 0;
        }
        out.len() as c_int
    }

    pub unsafe extern "C" fn darwin_vsprintf(
        dst: *mut c_char,
        fmt: *const c_char,
        ap: *const u64,
    ) -> c_int {
        darwin_vsnprintf(dst, usize::MAX / 2, fmt, ap)
    }

    pub unsafe extern "C" fn darwin_vsnprintf_chk(
        dst: *mut c_char,
        size: usize,
        _flag: c_int,
        _objlen: usize,
        fmt: *const c_char,
        ap: *const u64,
    ) -> c_int {
        darwin_vsnprintf(dst, size, fmt, ap)
    }

    pub unsafe extern "C" fn darwin_vsprintf_chk(
        dst: *mut c_char,
        _flag: c_int,
        objlen: usize,
        fmt: *const c_char,
        ap: *const u64,
    ) -> c_int {
        let cap = if objlen == 0 { usize::MAX / 2 } else { objlen };
        darwin_vsnprintf(dst, cap, fmt, ap)
    }

    pub unsafe extern "C" fn darwin_vprintf(fmt: *const c_char, ap: *const u64) -> c_int {
        let out = format_darwin_printf(fmt, ap);
        use std::io::Write;
        let _ = std::io::stdout().write_all(&out);
        out.len() as c_int
    }

    pub unsafe extern "C" fn darwin_vfprintf(
        stream: *mut c_void,
        fmt: *const c_char,
        ap: *const u64,
    ) -> c_int {
        let out = format_darwin_printf(fmt, ap);
        use std::io::Write;
        if stream.is_null() || stream == (&raw const STDERR_STUB) as *mut c_void {
            let _ = std::io::stderr().write_all(&out);
        } else if stream == (&raw const STDOUT_STUB) as *mut c_void {
            let _ = std::io::stdout().write_all(&out);
        } else {
            let _ = fwrite(out.as_ptr() as *const c_void, 1, out.len(), stream);
        }
        out.len() as c_int
    }

    unsafe fn count_scanf_outputs(fmt: *const c_char) -> usize {
        if fmt.is_null() { return 0; }
        let b = CStr::from_ptr(fmt).to_bytes();
        let mut i = 0;
        let mut count = 0;
        while i < b.len() {
            if b[i] == b'%' {
                i += 1;
                if i < b.len() && b[i] == b'%' {
                    i += 1;
                    continue;
                }
                let suppressed = i < b.len() && b[i] == b'*';
                if suppressed { i += 1; }
                while i < b.len() && (b[i].is_ascii_digit() || matches!(b[i], b'h' | b'l' | b'z' | b't' | b'j' | b'L' | b'q')) {
                    i += 1;
                }
                if i < b.len() && b[i] == b'[' {
                    i += 1;
                    if i < b.len() && b[i] == b'^' { i += 1; }
                    if i < b.len() && b[i] == b']' { i += 1; }
                    while i < b.len() && b[i] != b']' { i += 1; }
                }
                if i < b.len() {
                    if !suppressed && b[i] != b'n' {
                        count += 1;
                    }
                    i += 1;
                }
            } else {
                i += 1;
            }
        }
        count
    }

    pub unsafe extern "C" fn darwin_sscanf_va(
        src: *const c_char,
        fmt: *const c_char,
        va: *const *mut c_void,
    ) -> c_int {
        let n = count_scanf_outputs(fmt).min(8);
        let arg = |idx: usize| if idx < n && !va.is_null() { *va.add(idx) } else { std::ptr::null_mut() };
        match n {
            0 => sscanf(src, fmt),
            1 => sscanf(src, fmt, arg(0)),
            2 => sscanf(src, fmt, arg(0), arg(1)),
            3 => sscanf(src, fmt, arg(0), arg(1), arg(2)),
            4 => sscanf(src, fmt, arg(0), arg(1), arg(2), arg(3)),
            5 => sscanf(src, fmt, arg(0), arg(1), arg(2), arg(3), arg(4)),
            6 => sscanf(src, fmt, arg(0), arg(1), arg(2), arg(3), arg(4), arg(5)),
            _ => sscanf(src, fmt, arg(0), arg(1), arg(2), arg(3), arg(4), arg(5), arg(6), arg(7)),
        }
    }

    pub unsafe extern "C" fn darwin_fscanf_va(
        stream: *mut c_void,
        fmt: *const c_char,
        va: *const *mut c_void,
    ) -> c_int {
        if stream.is_null() || stream == (&raw const STDIN_STUB) as *mut c_void {
            return -1;
        }
        let n = count_scanf_outputs(fmt).min(8);
        let arg = |idx: usize| if idx < n && !va.is_null() { *va.add(idx) } else { std::ptr::null_mut() };
        match n {
            0 => fscanf(stream, fmt),
            1 => fscanf(stream, fmt, arg(0)),
            2 => fscanf(stream, fmt, arg(0), arg(1)),
            3 => fscanf(stream, fmt, arg(0), arg(1), arg(2)),
            4 => fscanf(stream, fmt, arg(0), arg(1), arg(2), arg(3)),
            5 => fscanf(stream, fmt, arg(0), arg(1), arg(2), arg(3), arg(4)),
            6 => fscanf(stream, fmt, arg(0), arg(1), arg(2), arg(3), arg(4), arg(5)),
            _ => fscanf(stream, fmt, arg(0), arg(1), arg(2), arg(3), arg(4), arg(5), arg(6), arg(7)),
        }
    }

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    #[unsafe(naked)]
    pub unsafe extern "C" fn darwin_snprintf() -> ! {
        core::arch::naked_asm!("mov x3, sp", "b {dst}", dst = sym darwin_vsnprintf);
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    #[unsafe(naked)]
    pub unsafe extern "C" fn darwin_sprintf() -> ! {
        core::arch::naked_asm!("mov x2, sp", "b {dst}", dst = sym darwin_vsprintf);
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    #[unsafe(naked)]
    pub unsafe extern "C" fn darwin_sprintf_chk() -> ! {
        core::arch::naked_asm!("mov x4, sp", "b {dst}", dst = sym darwin_vsprintf_chk);
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    #[unsafe(naked)]
    pub unsafe extern "C" fn darwin_snprintf_chk() -> ! {
        core::arch::naked_asm!("mov x5, sp", "b {dst}", dst = sym darwin_vsnprintf_chk);
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    #[unsafe(naked)]
    pub unsafe extern "C" fn darwin_printf() -> ! {
        core::arch::naked_asm!("mov x1, sp", "b {dst}", dst = sym darwin_vprintf);
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    #[unsafe(naked)]
    pub unsafe extern "C" fn darwin_fprintf() -> ! {
        core::arch::naked_asm!("mov x2, sp", "b {dst}", dst = sym darwin_vfprintf);
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    #[unsafe(naked)]
    pub unsafe extern "C" fn darwin_sscanf() -> ! {
        core::arch::naked_asm!("mov x2, sp", "b {dst}", dst = sym darwin_sscanf_va);
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    #[unsafe(naked)]
    pub unsafe extern "C" fn darwin_fscanf() -> ! {
        core::arch::naked_asm!("mov x2, sp", "b {dst}", dst = sym darwin_fscanf_va);
    }
}

fn libc_dlsym(macho_sym: &str) -> Option<usize> {
    let cname = macho_sym.strip_prefix('_')?;
    let cstr = std::ffi::CString::new(cname).ok()?;
    #[cfg(target_os = "macos")]
    let default_handle = -2isize as *mut c_void;
    #[cfg(not(target_os = "macos"))]
    let default_handle = std::ptr::null_mut();
    let ptr = unsafe { dlsym(default_handle, cstr.as_ptr()) };
    if ptr.is_null() { None } else { Some(ptr as usize) }
}

const GUEST_DL_FLAT_HANDLE: usize = 0x5045_4152_0000_0001;
static DLOPEN_HANDLES: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<usize, String>>> =
    std::sync::LazyLock::new(Default::default);
static NEXT_DL_HANDLE: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0x5045_4152_0000_0100);

extern "C" fn guest_dlopen(path: *const c_char, mode: i32) -> *mut c_void {
    if path.is_null() {
        return GUEST_DL_FLAT_HANDLE as *mut c_void;
    }
    let s = unsafe { std::ffi::CStr::from_ptr(path) }.to_string_lossy().into_owned();
    let is_apple_fw = s.starts_with("/System/Library/")
        || s.starts_with("/usr/lib/")
        || s.contains(".framework/");
    if is_apple_fw {
        let h = NEXT_DL_HANDLE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        DLOPEN_HANDLES.lock().unwrap().insert(h, s);
        return h as *mut c_void;
    }
    unsafe { dlopen(path, mode) }
}

extern "C" fn guest_dlsym(handle: *mut c_void, sym: *const c_char) -> *mut c_void {
    if sym.is_null() {
        return std::ptr::null_mut();
    }
    let Ok(name) = (unsafe { std::ffi::CStr::from_ptr(sym) }).to_str() else {
        return std::ptr::null_mut();
    };
    let macho_sym = format!("_{name}");
    let h_val = handle as usize;
    let lib_hint = DLOPEN_HANDLES.lock().unwrap().get(&h_val).cloned();
    if let Some(ref lib) = lib_hint {
        if let Some(addr) = resolve_symbol(Some(lib), &macho_sym) {
            return addr as *mut c_void;
        }
    }
    if h_val == GUEST_DL_FLAT_HANDLE || h_val == (-2isize as usize) || h_val == 0 || lib_hint.is_some() {
        if let Some(addr) = pear_metal_vk::symbol(&macho_sym)
            .or_else(|| resolve_symbol(Some("/System/Library/Frameworks/Foundation.framework/Foundation"), &macho_sym))
            .or_else(|| resolve_symbol(Some("/System/Library/Frameworks/UIKit.framework/UIKit"), &macho_sym))
            .or_else(|| resolve_symbol(Some("/System/Library/Frameworks/Security.framework/Security"), &macho_sym))
            .or_else(|| resolve_symbol(Some("/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics"), &macho_sym))
            .or_else(|| resolve_symbol(Some("/System/Library/Frameworks/QuartzCore.framework/QuartzCore"), &macho_sym))
            .or_else(|| resolve_symbol(None, &macho_sym))
        {
            return addr as *mut c_void;
        }
    }
    if lib_hint.is_some() || h_val == GUEST_DL_FLAT_HANDLE {
        return std::ptr::null_mut();
    }
    unsafe { dlsym(handle, sym) }
}

extern "C" fn guest_dlclose(handle: *mut c_void) -> i32 {
    let h_val = handle as usize;
    if h_val == GUEST_DL_FLAT_HANDLE || DLOPEN_HANDLES.lock().unwrap().remove(&h_val).is_some() {
        return 0;
    }
    0
}

extern "C" fn ca_current_media_time() -> f64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs_f64()
}

static STACK_CHK_GUARD: usize = 0x5065_6172_5369_6c69;

extern "C" fn stack_chk_fail_trap() -> ! {
    eprintln!("[pear-runtime] ___stack_chk_fail");
    std::process::abort()
}

extern "C" fn tls_errno_ptr() -> *mut i32 {
    extern "C" {
        #[cfg(target_os = "macos")]
        fn __error() -> *mut i32;
        #[cfg(target_os = "linux")]
        fn __errno_location() -> *mut i32;
    }
    #[cfg(target_os = "macos")]
    unsafe {
        return __error();
    }
    #[cfg(target_os = "linux")]
    unsafe {
        return __errno_location();
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        thread_local! {
            static ERRNO: std::cell::UnsafeCell<i32> = const { std::cell::UnsafeCell::new(0) };
        }
        ERRNO.with(|e| e.get())
    }
}

extern "C" fn memcpy_chk(dst: *mut c_void, src: *const c_void, n: usize, _dstlen: usize) -> *mut c_void {
    unsafe { memcpy(dst, src, n) }
}

#[repr(C)]
struct SincosfStret {
    sinval: f32,
    cosval: f32,
}

extern "C" fn sincosf_stret(x: f32) -> SincosfStret {
    let (s, c) = x.sin_cos();
    SincosfStret { sinval: s, cosval: c }
}

#[repr(C)]
struct SincosStret {
    sinval: f64,
    cosval: f64,
}

extern "C" fn sincos_stret(x: f64) -> SincosStret {
    let (s, c) = x.sin_cos();
    SincosStret { sinval: s, cosval: c }
}

extern "C" fn acos_impl(x: f64) -> f64 { x.acos() }
extern "C" fn tolower_impl(c: i32) -> i32 {
    if (0..128).contains(&c) { (c as u8).to_ascii_lowercase() as i32 } else { c }
}
extern "C" fn toupper_impl(c: i32) -> i32 {
    if (0..128).contains(&c) { (c as u8).to_ascii_uppercase() as i32 } else { c }
}
extern "C" fn os_atomic_add32(val: i32, ptr: *mut i32) -> i32 {
    if ptr.is_null() { return 0; }
    unsafe {
        let a = &*(ptr as *const std::sync::atomic::AtomicI32);
        a.fetch_add(val, std::sync::atomic::Ordering::SeqCst) + val
    }
}
extern "C" fn arc4random_impl() -> u32 {
    static STATE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0x5065_6172_3132_3334);
    let mut x = STATE.load(std::sync::atomic::Ordering::Relaxed);
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    STATE.store(x, std::sync::atomic::Ordering::Relaxed);
    x as u32
}
extern "C" fn availability_version_check(_count: u32, _versions: *const c_void) -> bool { true }

fn run_dispatch_once<F: FnOnce()>(pred: *mut usize, f: F) {
    if pred.is_null() {
        return;
    }
    let p = unsafe { &*(pred as *const std::sync::atomic::AtomicUsize) };
    loop {
        match p.load(std::sync::atomic::Ordering::Acquire) {
            usize::MAX => return,
            0 => {
                if p.compare_exchange(
                    0,
                    1,
                    std::sync::atomic::Ordering::AcqRel,
                    std::sync::atomic::Ordering::Acquire,
                )
                .is_ok()
                {
                    f();
                    p.store(usize::MAX, std::sync::atomic::Ordering::Release);
                    return;
                }
            }
            _ => std::thread::yield_now(),
        }
    }
}

extern "C" fn dispatch_once_f_impl(pred: *mut usize, ctx: *mut c_void, func: extern "C" fn(*mut c_void)) {
    run_dispatch_once(pred, || func(ctx));
}

extern "C" fn mach_absolute_time_impl() -> u64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_nanos() as u64
}

#[repr(C)]
struct MachTimebaseInfo {
    numer: u32,
    denom: u32,
}

extern "C" fn mach_timebase_info_impl(info: *mut MachTimebaseInfo) -> i32 {
    if !info.is_null() {
        unsafe {
            (*info).numer = 1;
            (*info).denom = 1;
        }
    }
    0
}

static MACH_TASK_SELF_STUB: u32 = 1;

unsafe extern "C" fn memset_pattern16_impl(b: *mut u8, pattern16: *const u8, len: usize) {
    if b.is_null() || pattern16.is_null() { return; }
    let pat = std::slice::from_raw_parts(pattern16, 16);
    for i in 0..len {
        *b.add(i) = pat[i & 15];
    }
}

extern "C" fn pthread_mach_thread_np_impl(t: usize) -> u32 {
    #[cfg(target_os = "macos")]
    {
        extern "C" {
            fn pthread_mach_thread_np(thread: usize) -> u32;
        }
        return unsafe { pthread_mach_thread_np(t) };
    }
    #[cfg(not(target_os = "macos"))]
    {
        static NEXT_TID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(100);
        static MAP: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<usize, u32>>> =
            std::sync::LazyLock::new(Default::default);
        *MAP.lock()
            .unwrap()
            .entry(t)
            .or_insert_with(|| NEXT_TID.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}
extern "C" fn avaudiosession_current_route(_self: Id, _cmd: Sel) -> Id {
    static ROUTE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *ROUTE.get_or_init(|| unsafe { alloc(AVAUDIOSESSIONROUTEDESCRIPTION_CLASS.ptr()) } as usize) as Id
}

unsafe fn write_sysctl_bytes(bytes: &[u8], oldp: *mut c_void, oldlenp: *mut usize) -> i32 {
    if oldlenp.is_null() {
        return -1;
    }
    if oldp.is_null() {
        *oldlenp = bytes.len();
        return 0;
    }
    let cap = *oldlenp;
    *oldlenp = bytes.len();
    if cap < bytes.len() {
        let ep = tls_errno_ptr();
        if !ep.is_null() {
            *ep = 12; // ENOMEM
        }
        return -1;
    }
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), oldp as *mut u8, bytes.len());
    0
}

extern "C" fn sysctlbyname_impl(
    name: *const c_char,
    oldp: *mut c_void,
    oldlenp: *mut usize,
    newp: *const c_void,
    newlen: usize,
) -> i32 {
    if name.is_null() || oldlenp.is_null() {
        return -1;
    }
    let key = unsafe { std::ffi::CStr::from_ptr(name) }.to_string_lossy();
    let ncpu = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(4);
    unsafe {
        match key.as_ref() {
            "hw.machine" => write_sysctl_bytes(b"iPhone15,2\0", oldp, oldlenp),
            "hw.model" => write_sysctl_bytes(b"D73AP\0", oldp, oldlenp),
            "kern.ostype" => write_sysctl_bytes(b"Darwin\0", oldp, oldlenp),
            "kern.osrelease" => write_sysctl_bytes(b"23.0.0\0", oldp, oldlenp),
            "kern.osversion" => write_sysctl_bytes(b"21A329\0", oldp, oldlenp),
            "kern.version" => write_sysctl_bytes(b"Darwin Kernel Version 23.0.0: ARM64\0", oldp, oldlenp),
            "kern.hostname" => write_sysctl_bytes(b"iPhone\0", oldp, oldlenp),
            "machdep.cpu.brand_string" => write_sysctl_bytes(b"Apple A16 Bionic\0", oldp, oldlenp),
            "hw.cputype" => write_sysctl_bytes(&0x0100_000ci32.to_ne_bytes(), oldp, oldlenp),
            "hw.cpusubtype" => write_sysctl_bytes(&0i32.to_ne_bytes(), oldp, oldlenp),
            "hw.ncpu" | "hw.activecpu" | "hw.physicalcpu" | "hw.physicalcpu_max"
            | "hw.logicalcpu" | "hw.logicalcpu_max" => {
                write_sysctl_bytes(&ncpu.to_ne_bytes(), oldp, oldlenp)
            }
            "hw.pagesize" | "vm.pagesize" => {
                let ps = VM_PAGE_SIZE_VAL.load(std::sync::atomic::Ordering::Relaxed) as u32;
                write_sysctl_bytes(&ps.to_ne_bytes(), oldp, oldlenp)
            }
            "hw.byteorder" => write_sysctl_bytes(&1234u32.to_ne_bytes(), oldp, oldlenp),
            "hw.memsize" => {
                write_sysctl_bytes(&(6u64 * 1024 * 1024 * 1024).to_ne_bytes(), oldp, oldlenp)
            }
            "hw.usermem" => {
                write_sysctl_bytes(&(4u64 * 1024 * 1024 * 1024).to_ne_bytes(), oldp, oldlenp)
            }
            "hw.tbfrequency" => {
                if !oldp.is_null() && *oldlenp == 4 {
                    write_sysctl_bytes(&24_000_000u32.to_ne_bytes(), oldp, oldlenp)
                } else {
                    write_sysctl_bytes(&24_000_000u64.to_ne_bytes(), oldp, oldlenp)
                }
            }
            "kern.boottime" => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(1_700_000_000);
                let tv = [now.saturating_sub(3600), 0i64];
                let bytes = std::slice::from_raw_parts(tv.as_ptr() as *const u8, 16);
                write_sysctl_bytes(bytes, oldp, oldlenp)
            }
            _ => {
                #[cfg(target_os = "macos")]
                if let Some(addr) = libc_dlsym("_sysctlbyname") {
                    let f: extern "C" fn(
                        *const c_char,
                        *mut c_void,
                        *mut usize,
                        *const c_void,
                        usize,
                    ) -> i32 = std::mem::transmute(addr);
                    return f(name, oldp, oldlenp, newp, newlen);
                }
                let _ = (newp, newlen);
                -1
            }
        }
    }
}

extern "C" fn sysctl_impl(
    name: *const i32,
    namelen: u32,
    oldp: *mut c_void,
    oldlenp: *mut usize,
    newp: *const c_void,
    newlen: usize,
) -> i32 {
    if name.is_null() || namelen < 2 || oldlenp.is_null() {
        return -1;
    }
    let mib = unsafe { std::slice::from_raw_parts(name, namelen as usize) };
    let mapped_name: Option<&std::ffi::CStr> = match (mib[0], mib[1]) {
        (6, 1) => Some(c"hw.machine"),
        (6, 2) => Some(c"hw.model"),
        (6, 3) => Some(c"hw.ncpu"),
        (6, 7) => Some(c"hw.pagesize"),
        (6, 24) => Some(c"hw.memsize"),
        (6, 25) => Some(c"hw.physicalcpu"),
        (1, 1) => Some(c"kern.ostype"),
        (1, 2) => Some(c"kern.osrelease"),
        (1, 4) => Some(c"kern.version"),
        (1, 10) => Some(c"kern.hostname"),
        (1, 21) => Some(c"kern.boottime"),
        (1, 65) => Some(c"kern.osversion"),
        (1, 14) => {
            // CTL_KERN / KERN_PROC: zero-fill kinfo_proc so P_TRACED == 0
            unsafe {
                if oldp.is_null() {
                    *oldlenp = 648;
                } else {
                    let n = (*oldlenp).min(4096);
                    std::ptr::write_bytes(oldp as *mut u8, 0, n);
                }
            }
            return 0;
        }
        _ => None,
    };
    if let Some(cname) = mapped_name {
        return sysctlbyname_impl(cname.as_ptr(), oldp, oldlenp, newp, newlen);
    }
    #[cfg(target_os = "macos")]
    if let Some(addr) = libc_dlsym("_sysctl") {
        let f: extern "C" fn(*const i32, u32, *mut c_void, *mut usize, *const c_void, usize) -> i32 =
            unsafe { std::mem::transmute(addr) };
        return f(name, namelen, oldp, oldlenp, newp, newlen);
    }
    -1
}

extern "C" fn task_info_impl(task: u32, flavor: u32, info: *mut c_void, count: *mut u32) -> i32 {
    #[cfg(target_os = "macos")]
    if let Some(addr) = libc_dlsym("_task_info") {
        let f: extern "C" fn(u32, u32, *mut c_void, *mut u32) -> i32 =
            unsafe { std::mem::transmute(addr) };
        return f(task, flavor, info, count);
    }
    let _ = (task, flavor);
    if !info.is_null() && !count.is_null() {
        unsafe {
            let n = (*count as usize).min(256) * 4;
            std::ptr::write_bytes(info as *mut u8, 0, n);
        }
        0
    } else {
        -1
    }
}
extern "C" fn thread_policy_set_impl(_t: u32, _flavor: u32, _policy: *const c_void, _count: u32) -> i32 { 0 }

#[repr(C)]
struct CMTime {
    value: i64,
    timescale: i32,
    flags: u32,
    epoch: i64,
}

static K_CM_TIME_ZERO: CMTime = CMTime { value: 0, timescale: 1, flags: 1, epoch: 0 };

extern "C" fn cm_time_make(value: i64, timescale: i32) -> CMTime {
    CMTime { value, timescale, flags: 1, epoch: 0 }
}

extern "C" fn cm_time_get_seconds(t: CMTime) -> f64 {
    if t.timescale == 0 { 0.0 } else { t.value as f64 / t.timescale as f64 }
}

extern "C" fn exp10_impl(x: f64) -> f64 { 10.0f64.powf(x) }
extern "C" fn acosf_impl(x: f32) -> f32 { x.acos() }
extern "C" fn asinf_impl(x: f32) -> f32 { x.asin() }
extern "C" fn atan2f_impl(y: f32, x: f32) -> f32 { y.atan2(x) }
extern "C" fn cxa_atexit_noop(_f: *const c_void, _arg: *const c_void, _dso: *const c_void) -> i32 { 0 }
static MAIN_IMAGE_HEADER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
static MAIN_IMAGE_SLIDE: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

struct RegisteredImage {
    header: usize,
    slide: isize,
    addr_start: usize,
    addr_end: usize,
    name: Box<std::ffi::CStr>,
}

static LOADED_IMAGES: std::sync::Mutex<Vec<RegisteredImage>> = std::sync::Mutex::new(Vec::new());

fn clear_loaded_images() {
    LOADED_IMAGES.lock().unwrap().clear();
    unwind::clear_images();
    MAIN_IMAGE_HEADER.store(0, std::sync::atomic::Ordering::Release);
    MAIN_IMAGE_SLIDE.store(0, std::sync::atomic::Ordering::Release);
}

fn register_loaded_image(img: &pear_dyld::LoadedImage, fallback_name: &str) {
    unwind::register_image(img);
    let label = img
        .path
        .as_deref()
        .or(img.install_name.as_deref())
        .unwrap_or(fallback_name);
    let cstr = std::ffi::CString::new(label)
        .unwrap_or_else(|_| std::ffi::CString::new("PearGuest").unwrap())
        .into_boxed_c_str();
    let header = img.header() as usize;
    let slide = img.slide() as isize;
    let addr_start = header;
    let addr_end = img
        .sections
        .iter()
        .map(|s| img.address(s.addr.saturating_add(s.size)) as usize)
        .max()
        .unwrap_or(addr_start);
    let mut guard = LOADED_IMAGES.lock().unwrap();
    if guard.is_empty() {
        MAIN_IMAGE_HEADER.store(header, std::sync::atomic::Ordering::Release);
        MAIN_IMAGE_SLIDE.store(slide, std::sync::atomic::Ordering::Release);
    }
    guard.push(RegisteredImage {
        header,
        slide,
        addr_start,
        addr_end,
        name: cstr,
    });
}

unsafe extern "C" fn guest_dladdr(addr: *const c_void, info: *mut DlInfo) -> i32 {
    if addr.is_null() || info.is_null() {
        return 0;
    }
    let a = addr as usize;
    {
        let guard = LOADED_IMAGES.lock().unwrap();
        for entry in guard.iter() {
            if a >= entry.addr_start && a < entry.addr_end {
                (*info).dli_fname = entry.name.as_ptr();
                (*info).dli_fbase = entry.header as *mut c_void;
                (*info).dli_sname = std::ptr::null();
                (*info).dli_saddr = std::ptr::null_mut();
                return 1;
            }
        }
    }
    dladdr(addr, info)
}

extern "C" fn dyld_image_count_stub() -> u32 {
    let n = LOADED_IMAGES.lock().unwrap().len();
    if n > 0 {
        n as u32
    } else if MAIN_IMAGE_HEADER.load(std::sync::atomic::Ordering::Acquire) != 0 {
        1
    } else {
        0
    }
}
extern "C" fn dyld_get_image_header_stub(idx: u32) -> *const c_void {
    let guard = LOADED_IMAGES.lock().unwrap();
    if let Some(entry) = guard.get(idx as usize) {
        return entry.header as *const c_void;
    }
    if idx == 0 {
        MAIN_IMAGE_HEADER.load(std::sync::atomic::Ordering::Acquire) as *const c_void
    } else {
        std::ptr::null()
    }
}
extern "C" fn dyld_get_image_vmaddr_slide_stub(idx: u32) -> isize {
    let guard = LOADED_IMAGES.lock().unwrap();
    if let Some(entry) = guard.get(idx as usize) {
        return entry.slide;
    }
    if idx == 0 {
        MAIN_IMAGE_SLIDE.load(std::sync::atomic::Ordering::Acquire)
    } else {
        0
    }
}
extern "C" fn dyld_get_image_name_stub(idx: u32) -> *const c_char {
    let guard = LOADED_IMAGES.lock().unwrap();
    if let Some(entry) = guard.get(idx as usize) {
        return entry.name.as_ptr();
    }
    if idx == 0 {
        c"PearGuest".as_ptr()
    } else {
        std::ptr::null()
    }
}

mod darwin_pthread {
    use std::collections::HashMap;
    use std::ffi::{c_int, c_void};
    use std::sync::{Arc, Condvar, Mutex, OnceLock};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    const DARWIN_MUTEX_RECURSIVE_SIG: u32 = 0x32AA_ABA2;
    const DARWIN_PTHREAD_MUTEX_RECURSIVE: c_int = 2;
    const DARWIN_PTHREAD_CREATE_DETACHED: c_int = 2;
    const LINUX_EBUSY: c_int = 16;
    const LINUX_EINVAL: c_int = 22;
    const LINUX_ETIMEDOUT: c_int = 110;

    #[repr(C)]
    pub struct DarwinTimespec {
        pub tv_sec: i64,
        pub tv_nsec: i64,
    }

    struct MtxState {
        owner: Option<std::thread::ThreadId>,
        depth: usize,
        recursive: bool,
    }

    type MtxEntry = Arc<(Mutex<MtxState>, Condvar)>;
    type CondEntry = Arc<(Mutex<u64>, Condvar)>;

    fn mutexes() -> &'static Mutex<HashMap<usize, MtxEntry>> {
        static M: OnceLock<Mutex<HashMap<usize, MtxEntry>>> = OnceLock::new();
        M.get_or_init(Default::default)
    }
    fn mutex_attrs() -> &'static Mutex<HashMap<usize, bool>> {
        static M: OnceLock<Mutex<HashMap<usize, bool>>> = OnceLock::new();
        M.get_or_init(Default::default)
    }
    fn conds() -> &'static Mutex<HashMap<usize, CondEntry>> {
        static M: OnceLock<Mutex<HashMap<usize, CondEntry>>> = OnceLock::new();
        M.get_or_init(Default::default)
    }
    fn thread_attrs() -> &'static Mutex<HashMap<usize, bool>> {
        static M: OnceLock<Mutex<HashMap<usize, bool>>> = OnceLock::new();
        M.get_or_init(Default::default)
    }

    unsafe fn get_or_init_mutex(ptr: *const c_void) -> MtxEntry {
        let key = ptr as usize;
        let mut g = mutexes().lock().unwrap();
        if let Some(e) = g.get(&key) {
            return e.clone();
        }
        let sig = if ptr.is_null() { 0 } else { *(ptr as *const u32) };
        let rec = sig == DARWIN_MUTEX_RECURSIVE_SIG;
        let entry = Arc::new((
            Mutex::new(MtxState {
                owner: None,
                depth: 0,
                recursive: rec,
            }),
            Condvar::new(),
        ));
        g.insert(key, entry.clone());
        entry
    }

    fn get_or_init_cond(ptr: *const c_void) -> CondEntry {
        let key = ptr as usize;
        let mut g = conds().lock().unwrap();
        g.entry(key)
            .or_insert_with(|| Arc::new((Mutex::new(0u64), Condvar::new())))
            .clone()
    }

    pub unsafe extern "C" fn mutexattr_init(attr: *mut c_void) -> c_int {
        if attr.is_null() { return LINUX_EINVAL; }
        mutex_attrs().lock().unwrap().insert(attr as usize, false);
        0
    }
    pub unsafe extern "C" fn mutexattr_destroy(attr: *mut c_void) -> c_int {
        if !attr.is_null() {
            mutex_attrs().lock().unwrap().remove(&(attr as usize));
        }
        0
    }
    pub unsafe extern "C" fn mutexattr_settype(attr: *mut c_void, kind: c_int) -> c_int {
        if attr.is_null() { return LINUX_EINVAL; }
        mutex_attrs()
            .lock()
            .unwrap()
            .insert(attr as usize, kind == DARWIN_PTHREAD_MUTEX_RECURSIVE);
        0
    }

    pub unsafe extern "C" fn mutex_init(mtx: *mut c_void, attr: *const c_void) -> c_int {
        if mtx.is_null() { return LINUX_EINVAL; }
        let rec = if attr.is_null() {
            false
        } else {
            mutex_attrs().lock().unwrap().get(&(attr as usize)).copied().unwrap_or(false)
        };
        let entry = Arc::new((
            Mutex::new(MtxState {
                owner: None,
                depth: 0,
                recursive: rec,
            }),
            Condvar::new(),
        ));
        mutexes().lock().unwrap().insert(mtx as usize, entry);
        0
    }
    pub unsafe extern "C" fn mutex_destroy(mtx: *mut c_void) -> c_int {
        if !mtx.is_null() {
            mutexes().lock().unwrap().remove(&(mtx as usize));
        }
        0
    }
    pub unsafe extern "C" fn mutex_lock(mtx: *mut c_void) -> c_int {
        if mtx.is_null() { return LINUX_EINVAL; }
        let entry = get_or_init_mutex(mtx);
        let me = std::thread::current().id();
        let mut st = entry.0.lock().unwrap();
        while let Some(owner) = st.owner {
            if owner == me && st.recursive {
                st.depth += 1;
                return 0;
            }
            st = entry.1.wait(st).unwrap();
        }
        st.owner = Some(me);
        st.depth = 1;
        0
    }
    pub unsafe extern "C" fn mutex_trylock(mtx: *mut c_void) -> c_int {
        if mtx.is_null() { return LINUX_EINVAL; }
        let entry = get_or_init_mutex(mtx);
        let me = std::thread::current().id();
        let mut st = entry.0.lock().unwrap();
        match st.owner {
            None => {
                st.owner = Some(me);
                st.depth = 1;
                0
            }
            Some(owner) if owner == me && st.recursive => {
                st.depth += 1;
                0
            }
            _ => LINUX_EBUSY,
        }
    }
    pub unsafe extern "C" fn mutex_unlock(mtx: *mut c_void) -> c_int {
        if mtx.is_null() { return LINUX_EINVAL; }
        let entry = get_or_init_mutex(mtx);
        let me = std::thread::current().id();
        let mut st = entry.0.lock().unwrap();
        if st.owner == Some(me) {
            if st.depth > 1 {
                st.depth -= 1;
            } else {
                st.owner = None;
                st.depth = 0;
                entry.1.notify_one();
            }
        }
        0
    }

    pub unsafe extern "C" fn cond_init(cond: *mut c_void, _attr: *const c_void) -> c_int {
        if cond.is_null() { return LINUX_EINVAL; }
        let _ = get_or_init_cond(cond);
        0
    }
    pub unsafe extern "C" fn cond_destroy(cond: *mut c_void) -> c_int {
        if !cond.is_null() {
            conds().lock().unwrap().remove(&(cond as usize));
        }
        0
    }
    pub unsafe extern "C" fn cond_signal(cond: *mut c_void) -> c_int {
        if cond.is_null() { return LINUX_EINVAL; }
        let c = get_or_init_cond(cond);
        let mut g = c.0.lock().unwrap();
        *g = g.wrapping_add(1);
        c.1.notify_one();
        0
    }
    pub unsafe extern "C" fn cond_broadcast(cond: *mut c_void) -> c_int {
        if cond.is_null() { return LINUX_EINVAL; }
        let c = get_or_init_cond(cond);
        let mut g = c.0.lock().unwrap();
        *g = g.wrapping_add(1);
        c.1.notify_all();
        0
    }
    unsafe fn cond_wait_common(cond: *mut c_void, mtx: *mut c_void, abs_ts: Option<&DarwinTimespec>) -> c_int {
        if cond.is_null() || mtx.is_null() { return LINUX_EINVAL; }
        let c = get_or_init_cond(cond);
        let m = get_or_init_mutex(mtx);
        let me = std::thread::current().id();

        let mut cg = c.0.lock().unwrap();
        let start_seq = *cg;
        let saved_depth = {
            let mut st = m.0.lock().unwrap();
            let d = if st.owner == Some(me) { st.depth.max(1) } else { 1 };
            st.owner = None;
            st.depth = 0;
            m.1.notify_one();
            d
        };

        let mut timed_out = false;
        if let Some(ts) = abs_ts {
            let target = UNIX_EPOCH + Duration::new(ts.tv_sec.max(0) as u64, ts.tv_nsec.clamp(0, 999_999_999) as u32);
            loop {
                if *cg != start_seq { break; }
                let now = SystemTime::now();
                let Ok(rem) = target.duration_since(now) else {
                    timed_out = true;
                    break;
                };
                let (next_g, res) = c.1.wait_timeout(cg, rem).unwrap();
                cg = next_g;
                if *cg != start_seq { break; }
                if res.timed_out() {
                    timed_out = true;
                    break;
                }
            }
        } else {
            while *cg == start_seq {
                cg = c.1.wait(cg).unwrap();
            }
        }
        drop(cg);

        let mut st = m.0.lock().unwrap();
        while st.owner.is_some() {
            st = m.1.wait(st).unwrap();
        }
        st.owner = Some(me);
        st.depth = saved_depth;
        if timed_out { LINUX_ETIMEDOUT } else { 0 }
    }
    pub unsafe extern "C" fn cond_wait(cond: *mut c_void, mtx: *mut c_void) -> c_int {
        cond_wait_common(cond, mtx, None)
    }
    pub unsafe extern "C" fn cond_timedwait(
        cond: *mut c_void,
        mtx: *mut c_void,
        abstime: *const DarwinTimespec,
    ) -> c_int {
        let ts = if abstime.is_null() { None } else { Some(&*abstime) };
        cond_wait_common(cond, mtx, ts)
    }

    pub unsafe extern "C" fn attr_init(attr: *mut c_void) -> c_int {
        if attr.is_null() { return LINUX_EINVAL; }
        thread_attrs().lock().unwrap().insert(attr as usize, false);
        0
    }
    pub unsafe extern "C" fn attr_destroy(attr: *mut c_void) -> c_int {
        if !attr.is_null() {
            thread_attrs().lock().unwrap().remove(&(attr as usize));
        }
        0
    }
    pub unsafe extern "C" fn attr_setdetachstate(attr: *mut c_void, state: c_int) -> c_int {
        if attr.is_null() { return LINUX_EINVAL; }
        thread_attrs()
            .lock()
            .unwrap()
            .insert(attr as usize, state == DARWIN_PTHREAD_CREATE_DETACHED);
        0
    }

    extern "C" {
        #[link_name = "pthread_create"]
        fn real_pthread_create(
            thread: *mut usize,
            attr: *const c_void,
            start: extern "C" fn(*mut c_void) -> *mut c_void,
            arg: *mut c_void,
        ) -> c_int;
        #[link_name = "pthread_attr_init"]
        fn real_attr_init(attr: *mut c_void) -> c_int;
        #[link_name = "pthread_attr_setdetachstate"]
        fn real_attr_setdetachstate(attr: *mut c_void, state: c_int) -> c_int;
        #[link_name = "pthread_attr_destroy"]
        fn real_attr_destroy(attr: *mut c_void) -> c_int;
    }

    pub unsafe extern "C" fn thread_create(
        thread: *mut usize,
        attr: *const c_void,
        start: extern "C" fn(*mut c_void) -> *mut c_void,
        arg: *mut c_void,
    ) -> c_int {
        let detached = if attr.is_null() {
            false
        } else {
            thread_attrs().lock().unwrap().get(&(attr as usize)).copied().unwrap_or(false)
        };
        if detached {
            let mut linux_attr = [0usize; 16];
            real_attr_init(linux_attr.as_mut_ptr() as *mut c_void);
            real_attr_setdetachstate(linux_attr.as_mut_ptr() as *mut c_void, 1);
            let rc = real_pthread_create(thread, linux_attr.as_ptr() as *const c_void, start, arg);
            real_attr_destroy(linux_attr.as_mut_ptr() as *mut c_void);
            rc
        } else {
            real_pthread_create(thread, std::ptr::null(), start, arg)
        }
    }

    pub unsafe extern "C" fn cond_timedwait_relative_np(
        cond: *mut c_void,
        mtx: *mut c_void,
        reltime: *const DarwinTimespec,
    ) -> c_int {
        if reltime.is_null() {
            return cond_wait(cond, mtx);
        }
        let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) else {
            return cond_wait(cond, mtx);
        };
        let rel = &*reltime;
        let total_nsec = now.subsec_nanos() as i64 + rel.tv_nsec.max(0);
        let abstime = DarwinTimespec {
            tv_sec: now.as_secs() as i64 + rel.tv_sec.max(0) + (total_nsec / 1_000_000_000),
            tv_nsec: total_nsec % 1_000_000_000,
        };
        cond_timedwait(cond, mtx, &abstime)
    }

    pub unsafe extern "C" fn threadid_np(_thread: usize, thread_id: *mut u64) -> c_int {
        if thread_id.is_null() {
            return LINUX_EINVAL;
        }
        static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        thread_local! {
            static TID: u64 = NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        *thread_id = TID.with(|&id| id);
        0
    }

    pub unsafe extern "C" fn getname_np(_thread: usize, buf: *mut std::ffi::c_char, len: usize) -> c_int {
        if buf.is_null() || len == 0 {
            return LINUX_EINVAL;
        }
        let name = b"pear-thread\0";
        let copy_n = name.len().min(len);
        std::ptr::copy_nonoverlapping(name.as_ptr(), buf as *mut u8, copy_n);
        *(buf.add(copy_n - 1)) = 0;
        0
    }
}

static DISPATCH_MAIN_Q: [usize; 16] = [0; 16];
const fn darwin_ascii_runetype(c: u8) -> u32 {
    match c {
        9 => 0x24200,
        10..=13 => 0x4200,
        0..=31 | 127 => 0x200,
        32 => 0x64000,
        b'0'..=b'9' => 0x50c00 | ((c - b'0') as u32),
        b'A'..=b'Z' => {
            0x48900 | if c <= b'F' { 0x10000 | ((c - b'A' + 10) as u32) } else { 0 }
        }
        b'a'..=b'z' => {
            0x41900 | if c <= b'f' { 0x10000 | ((c - b'a' + 10) as u32) } else { 0 }
        }
        33..=126 => 0x42800,
        _ => 0,
    }
}

const fn build_default_rune_locale() -> [u8; 4096] {
    let mut buf = [0u8; 4096];
    let mut i = 0usize;
    while i < 256 {
        let rt = if i < 128 { darwin_ascii_runetype(i as u8) } else { 0 };
        let rt_bytes = rt.to_le_bytes();
        let rt_off = 60 + i * 4;
        buf[rt_off] = rt_bytes[0];
        buf[rt_off + 1] = rt_bytes[1];
        buf[rt_off + 2] = rt_bytes[2];
        buf[rt_off + 3] = rt_bytes[3];

        let lo = if i >= b'A' as usize && i <= b'Z' as usize { (i + 32) as u32 } else { i as u32 };
        let lo_bytes = lo.to_le_bytes();
        let lo_off = 1084 + i * 4;
        buf[lo_off] = lo_bytes[0];
        buf[lo_off + 1] = lo_bytes[1];
        buf[lo_off + 2] = lo_bytes[2];
        buf[lo_off + 3] = lo_bytes[3];

        let up = if i >= b'a' as usize && i <= b'z' as usize { (i - 32) as u32 } else { i as u32 };
        let up_bytes = up.to_le_bytes();
        let up_off = 2108 + i * 4;
        buf[up_off] = up_bytes[0];
        buf[up_off + 1] = up_bytes[1];
        buf[up_off + 2] = up_bytes[2];
        buf[up_off + 3] = up_bytes[3];

        i += 1;
    }
    buf
}

static DEFAULT_RUNE_LOCALE: [u8; 4096] = build_default_rune_locale();
static mut STDIN_STUB: usize = 0;
static mut STDOUT_STUB: usize = 0;
static mut STDERR_STUB: usize = 0;

extern "C" fn dispatch_once_impl(pred: *mut usize, block: *mut c_void) {
    if block.is_null() {
        return;
    }
    run_dispatch_once(pred, || unsafe {
        let invoke = *((block as *const usize).add(2));
        let f: extern "C" fn(*mut c_void) = std::mem::transmute(invoke);
        f(block);
    });
}

extern "C" fn dispatch_sync_impl(_q: *mut c_void, block: *mut c_void) {
    if !block.is_null() {
        unsafe {
            let invoke = *((block as *const usize).add(2));
            let f: extern "C" fn(*mut c_void) = std::mem::transmute(invoke);
            f(block);
        }
    }
}

extern "C" fn dispatch_get_global_queue_impl(_id: isize, _flags: usize) -> *const c_void {
    &raw const DISPATCH_MAIN_Q as *const c_void
}

static DISPATCH_QUEUE_ATTR_STUB: [usize; 4] = [0; 4];
static OS_LOG_STUB: [usize; 4] = [0; 4];

fn dispatch_now_ns() -> u64 {
    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    let elapsed = START.get_or_init(std::time::Instant::now).elapsed().as_nanos() as u64;
    1_000_000_000u64.saturating_add(elapsed).min(u64::MAX - 1)
}

type SemaphoreEntry = std::sync::Arc<(std::sync::Mutex<(isize, usize)>, std::sync::Condvar)>;
type GroupEntry = std::sync::Arc<(std::sync::Mutex<isize>, std::sync::Condvar)>;

static DISPATCH_SEMAPHORES: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<usize, SemaphoreEntry>>,
> = std::sync::LazyLock::new(Default::default);
static DISPATCH_GROUPS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<usize, GroupEntry>>,
> = std::sync::LazyLock::new(Default::default);

extern "C" fn dispatch_semaphore_dealloc(this: Id, _cmd: Sel) {
    DISPATCH_SEMAPHORES.lock().unwrap().remove(&(this as usize));
    unsafe { pear_libobjc::object::dispose(this) };
}

extern "C" fn dispatch_group_dealloc(this: Id, _cmd: Sel) {
    DISPATCH_GROUPS.lock().unwrap().remove(&(this as usize));
    unsafe { pear_libobjc::object::dispose(this) };
}

extern "C" fn dispatch_release_impl(obj: *mut c_void) {
    if obj.is_null() {
        return;
    }
    let addr = obj as usize;
    if addr == (&raw const DISPATCH_MAIN_Q as usize)
        || addr == (&raw const DISPATCH_QUEUE_ATTR_STUB as usize)
        || addr == (&raw const OS_LOG_STUB as usize)
    {
        return;
    }
    if DISPATCH_SEMAPHORES.lock().unwrap().contains_key(&addr)
        || DISPATCH_GROUPS.lock().unwrap().contains_key(&addr)
    {
        pear_libobjc::object::release(obj as Id);
    }
}

extern "C" fn dispatch_queue_create_impl(_label: *const c_char, _attr: *const c_void) -> *const c_void {
    &raw const DISPATCH_MAIN_Q as *const c_void
}
extern "C" fn dispatch_queue_attr_qos_impl(attr: *const c_void, _qos: u32, _prio: i32) -> *const c_void {
    if attr.is_null() { &raw const DISPATCH_QUEUE_ATTR_STUB as *const c_void } else { attr }
}
extern "C" fn dispatch_queue_get_label_impl(_q: *const c_void) -> *const c_char {
    c"com.pear.main".as_ptr()
}
extern "C" fn dispatch_group_create_impl() -> *const c_void {
    let g = unsafe { alloc(OS_DISPATCH_GROUP_CLASS.ptr()) };
    let entry = std::sync::Arc::new((std::sync::Mutex::new(0isize), std::sync::Condvar::new()));
    DISPATCH_GROUPS.lock().unwrap().insert(g as usize, entry);
    g as *const c_void
}
extern "C" fn dispatch_group_enter_impl(g: *mut c_void) {
    if g.is_null() {
        return;
    }
    let entry = DISPATCH_GROUPS.lock().unwrap().get(&(g as usize)).cloned();
    if let Some(entry) = entry {
        let mut count = entry.0.lock().unwrap();
        *count += 1;
    }
}
extern "C" fn dispatch_group_leave_impl(g: *mut c_void) {
    if g.is_null() {
        return;
    }
    let entry = DISPATCH_GROUPS.lock().unwrap().get(&(g as usize)).cloned();
    if let Some(entry) = entry {
        let mut count = entry.0.lock().unwrap();
        *count = (*count - 1).max(0);
        if *count == 0 {
            entry.1.notify_all();
        }
    }
}
extern "C" fn dispatch_group_async_impl(_g: *mut c_void, q: *mut c_void, block: *mut c_void) {
    dispatch_sync_impl(q, block);
}
extern "C" fn dispatch_group_wait_impl(g: *mut c_void, timeout: u64) -> isize {
    if g.is_null() {
        return 0;
    }
    let Some(entry) = DISPATCH_GROUPS.lock().unwrap().get(&(g as usize)).cloned() else {
        return 0;
    };
    let mut guard = entry.0.lock().unwrap();
    loop {
        if *guard <= 0 {
            return 0;
        }
        if timeout == 0 {
            return 1;
        }
        if timeout == u64::MAX {
            guard = entry.1.wait(guard).unwrap();
        } else {
            let now = dispatch_now_ns();
            if now >= timeout {
                return 1;
            }
            let (g2, _) = entry
                .1
                .wait_timeout(guard, std::time::Duration::from_nanos(timeout - now))
                .unwrap();
            guard = g2;
        }
    }
}
extern "C" fn dispatch_after_impl(_when: u64, q: *mut c_void, block: *mut c_void) {
    let _ = (q, block);
}
extern "C" fn dispatch_time_impl(when: u64, delta: i64) -> u64 {
    if when == u64::MAX {
        return u64::MAX;
    }
    let base = if when == 0 { dispatch_now_ns() } else { when };
    if delta >= 0 {
        base.saturating_add(delta as u64).min(u64::MAX - 1)
    } else {
        base.saturating_sub(delta.unsigned_abs()).max(1)
    }
}
#[repr(C)]
struct DarwinTimespec {
    tv_sec: i64,
    tv_nsec: i64,
}
extern "C" fn dispatch_walltime_impl(ts: *const c_void, delta: i64) -> u64 {
    if ts.is_null() {
        return dispatch_time_impl(0, delta);
    }
    let spec = unsafe { &*(ts as *const DarwinTimespec) };
    let target_ns = (spec.tv_sec as i128)
        .saturating_mul(1_000_000_000)
        .saturating_add(spec.tv_nsec as i128)
        .saturating_add(delta as i128);
    let now_wall = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0i128, |d| d.as_nanos() as i128);
    let diff = target_ns - now_wall;
    let base = dispatch_now_ns();
    if diff >= 0 {
        base.saturating_add(diff.min((u64::MAX - 1) as i128) as u64).min(u64::MAX - 1)
    } else {
        base.saturating_sub((-diff).min(u64::MAX as i128) as u64).max(1)
    }
}
extern "C" fn dispatch_semaphore_create_impl(val: isize) -> *const c_void {
    if val < 0 {
        return std::ptr::null();
    }
    let sem = unsafe { alloc(OS_DISPATCH_SEMAPHORE_CLASS.ptr()) };
    let entry = std::sync::Arc::new((std::sync::Mutex::new((val, 0usize)), std::sync::Condvar::new()));
    DISPATCH_SEMAPHORES.lock().unwrap().insert(sem as usize, entry);
    sem as *const c_void
}
extern "C" fn dispatch_semaphore_signal_impl(sem: *mut c_void) -> isize {
    if sem.is_null() {
        return 0;
    }
    let Some(entry) = DISPATCH_SEMAPHORES.lock().unwrap().get(&(sem as usize)).cloned() else {
        return 0;
    };
    let mut guard = entry.0.lock().unwrap();
    guard.0 += 1;
    if guard.1 > 0 {
        entry.1.notify_one();
        1
    } else {
        0
    }
}
extern "C" fn dispatch_semaphore_wait_impl(sem: *mut c_void, timeout: u64) -> isize {
    if sem.is_null() {
        return 0;
    }
    let Some(entry) = DISPATCH_SEMAPHORES.lock().unwrap().get(&(sem as usize)).cloned() else {
        return 0;
    };
    let mut guard = entry.0.lock().unwrap();
    if guard.0 > 0 {
        guard.0 -= 1;
        return 0;
    }
    if timeout == 0 {
        return 1;
    }
    guard.1 += 1;
    loop {
        if timeout == u64::MAX {
            guard = entry.1.wait(guard).unwrap();
        } else {
            let now = dispatch_now_ns();
            if now >= timeout {
                guard.1 -= 1;
                return 1;
            }
            let (g2, _) = entry
                .1
                .wait_timeout(guard, std::time::Duration::from_nanos(timeout - now))
                .unwrap();
            guard = g2;
        }
        if guard.0 > 0 {
            guard.0 -= 1;
            guard.1 -= 1;
            return 0;
        }
    }
}
extern "C" fn dispatch_block_create_impl(_flags: usize, block: *mut c_void) -> *mut c_void { block }
extern "C" fn dispatch_source_create_impl(_t: *const c_void, _h: usize, _m: usize, _q: *const c_void) -> *const c_void {
    &raw const DISPATCH_MAIN_Q as *const c_void
}
extern "C" fn dispatch_noop_1(_a: *mut c_void) {}
extern "C" fn dispatch_noop_2(_a: *mut c_void, _b: *mut c_void) {}
extern "C" fn dispatch_source_testcancel_impl(_s: *const c_void) -> isize { 0 }
extern "C" fn os_log_create_impl(_sub: *const c_char, _cat: *const c_char) -> *const c_void {
    &raw const OS_LOG_STUB as *const c_void
}
extern "C" fn os_log_type_enabled_impl(_log: *const c_void, _t: u8) -> bool { false }
extern "C" fn os_unfair_lock_lock(lock: *mut std::sync::atomic::AtomicU32) {
    if lock.is_null() {
        return;
    }
    let atom = unsafe { &*lock };
    while atom
        .compare_exchange_weak(
            0,
            1,
            std::sync::atomic::Ordering::Acquire,
            std::sync::atomic::Ordering::Relaxed,
        )
        .is_err()
    {
        std::thread::yield_now();
    }
}
extern "C" fn os_unfair_lock_lock_with_options(lock: *mut std::sync::atomic::AtomicU32, _opts: u32) {
    os_unfair_lock_lock(lock);
}
extern "C" fn os_unfair_lock_trylock(lock: *mut std::sync::atomic::AtomicU32) -> bool {
    if lock.is_null() {
        return false;
    }
    unsafe { &*lock }
        .compare_exchange(
            0,
            1,
            std::sync::atomic::Ordering::Acquire,
            std::sync::atomic::Ordering::Relaxed,
        )
        .is_ok()
}
extern "C" fn os_unfair_lock_unlock(lock: *mut std::sync::atomic::AtomicU32) {
    if lock.is_null() {
        return;
    }
    unsafe { &*lock }.store(0, std::sync::atomic::Ordering::Release);
}

static VM_PAGE_SIZE_VAL: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(16384);

#[cfg(not(target_os = "macos"))]
extern "C" fn darwin_mmap_impl(
    addr: *mut c_void,
    len: usize,
    prot: i32,
    darwin_flags: i32,
    fd: i32,
    offset: i64,
) -> *mut c_void {
    extern "C" {
        fn mmap(addr: *mut c_void, len: usize, prot: i32, flags: i32, fd: i32, off: i64) -> *mut c_void;
    }
    let mut linux_flags = 0i32;
    if (darwin_flags & 0x0001) != 0 { linux_flags |= 0x01; } // MAP_SHARED
    if (darwin_flags & 0x0002) != 0 { linux_flags |= 0x02; } // MAP_PRIVATE
    if (darwin_flags & 0x0010) != 0 { linux_flags |= 0x10; } // MAP_FIXED
    if (darwin_flags & 0x1000) != 0 { linux_flags |= 0x20; } // MAP_ANON -> MAP_ANONYMOUS
    if linux_flags == 0 {
        linux_flags = 0x22; // MAP_PRIVATE | MAP_ANONYMOUS fallback
    }
    unsafe { mmap(addr, len, prot, linux_flags, fd, offset) }
}

#[cfg(not(target_os = "macos"))]
extern "C" fn darwin_sysconf_impl(name: i32) -> isize {
    extern "C" {
        fn sysconf(name: i32) -> isize;
    }
    match name {
        29 | 47 => VM_PAGE_SIZE_VAL.load(std::sync::atomic::Ordering::Relaxed) as isize, // Darwin _SC_PAGESIZE / _SC_PAGE_SIZE
        57 | 58 => std::thread::available_parallelism().map(|n| n.get() as isize).unwrap_or(4), // Darwin _SC_NPROCESSORS_*
        _ => unsafe { sysconf(name) },
    }
}

#[repr(C)]
struct DarwinMallocZone {
    reserved1: *const c_void,
    reserved2: *const c_void,
    size: unsafe extern "C" fn(*mut c_void, *const c_void) -> usize,
    malloc: unsafe extern "C" fn(*mut c_void, usize) -> *mut c_void,
    calloc: unsafe extern "C" fn(*mut c_void, usize, usize) -> *mut c_void,
    valloc: unsafe extern "C" fn(*mut c_void, usize) -> *mut c_void,
    free: unsafe extern "C" fn(*mut c_void, *mut c_void),
    realloc: unsafe extern "C" fn(*mut c_void, *mut c_void, usize) -> *mut c_void,
    destroy: extern "C" fn(*mut c_void),
    zone_name: *const c_char,
    batch_malloc: *const c_void,
    batch_free: *const c_void,
    introspect: *const c_void,
    version: u32,
    _pad: u32,
    memalign: unsafe extern "C" fn(*mut c_void, usize, usize) -> *mut c_void,
    free_definite_size: unsafe extern "C" fn(*mut c_void, *mut c_void, usize),
    pressure_relief: unsafe extern "C" fn(*mut c_void, usize) -> usize,
}
unsafe impl Sync for DarwinMallocZone {}

unsafe extern "C" fn malloc_zone_size_cb(_zone: *mut c_void, ptr: *const c_void) -> usize {
    malloc_size_impl(ptr)
}
extern "C" fn malloc_zone_destroy_cb(_zone: *mut c_void) {}
unsafe extern "C" fn malloc_zone_memalign_cb(_zone: *mut c_void, alignment: usize, sz: usize) -> *mut c_void {
    let mut p: *mut c_void = std::ptr::null_mut();
    let align = alignment.max(std::mem::size_of::<usize>());
    if posix_memalign(&mut p, align, sz.max(1)) == 0 { p } else { std::ptr::null_mut() }
}
unsafe extern "C" fn malloc_zone_free_definite_cb(_zone: *mut c_void, ptr: *mut c_void, _sz: usize) {
    free(ptr);
}
unsafe extern "C" fn malloc_zone_pressure_cb(_zone: *mut c_void, _goal: usize) -> usize {
    0
}

static MALLOC_DEFAULT_ZONE: DarwinMallocZone = DarwinMallocZone {
    reserved1: std::ptr::null(),
    reserved2: std::ptr::null(),
    size: malloc_zone_size_cb,
    malloc: malloc_zone_malloc_impl,
    calloc: malloc_zone_calloc_impl,
    valloc: malloc_zone_malloc_impl,
    free: malloc_zone_free_impl,
    realloc: malloc_zone_realloc_impl,
    destroy: malloc_zone_destroy_cb,
    zone_name: c"PearDefaultZone".as_ptr(),
    batch_malloc: std::ptr::null(),
    batch_free: std::ptr::null(),
    introspect: std::ptr::null(),
    version: 12,
    _pad: 0,
    memalign: malloc_zone_memalign_cb,
    free_definite_size: malloc_zone_free_definite_cb,
    pressure_relief: malloc_zone_pressure_cb,
};

unsafe extern "C" fn memset_chk(dst: *mut c_void, c: i32, n: usize, _dstlen: usize) -> *mut c_void {
    memset(dst, c, n)
}
unsafe extern "C" fn strlcpy_impl(dst: *mut c_char, src: *const c_char, size: usize) -> usize {
    let src_len = if src.is_null() { 0 } else { strlen(src) };
    if !dst.is_null() && size > 0 && !src.is_null() {
        let copy = src_len.min(size - 1);
        std::ptr::copy_nonoverlapping(src as *const u8, dst as *mut u8, copy);
        *(dst.add(copy)) = 0;
    }
    src_len
}
unsafe extern "C" fn strlcpy_chk(dst: *mut c_char, src: *const c_char, size: usize, _dstlen: usize) -> usize {
    strlcpy_impl(dst, src, size)
}
unsafe extern "C" fn strlcat_impl(dst: *mut c_char, src: *const c_char, size: usize) -> usize {
    let dst_len = if dst.is_null() {
        0
    } else {
        let mut i = 0usize;
        while i < size && *(dst.add(i)) != 0 {
            i += 1;
        }
        i
    };
    let src_len = if src.is_null() { 0 } else { strlen(src) };
    if dst_len == size {
        return size + src_len;
    }
    if !dst.is_null() && !src.is_null() && size > dst_len + 1 {
        let copy = src_len.min(size - dst_len - 1);
        std::ptr::copy_nonoverlapping(src as *const u8, dst.add(dst_len) as *mut u8, copy);
        *(dst.add(dst_len + copy)) = 0;
    }
    dst_len + src_len
}
unsafe extern "C" fn strlcat_chk(dst: *mut c_char, src: *const c_char, size: usize, _dstlen: usize) -> usize {
    strlcat_impl(dst, src, size)
}
unsafe extern "C" fn malloc_default_zone_impl() -> *mut c_void {
    &raw const MALLOC_DEFAULT_ZONE as *mut c_void
}
unsafe extern "C" fn malloc_create_zone_impl(_start: usize, _flags: u32) -> *mut c_void {
    malloc_default_zone_impl()
}
unsafe extern "C" fn malloc_zone_malloc_impl(_zone: *mut c_void, sz: usize) -> *mut c_void {
    malloc(sz)
}
unsafe extern "C" fn malloc_zone_calloc_impl(_zone: *mut c_void, n: usize, sz: usize) -> *mut c_void {
    calloc(n, sz)
}
unsafe extern "C" fn malloc_zone_realloc_impl(_zone: *mut c_void, p: *mut c_void, sz: usize) -> *mut c_void {
    realloc(p, sz)
}
unsafe extern "C" fn malloc_zone_free_impl(_zone: *mut c_void, p: *mut c_void) {
    free(p);
}
unsafe extern "C" fn malloc_size_impl(ptr: *const c_void) -> usize {
    if ptr.is_null() {
        return 0;
    }
    #[cfg(target_os = "macos")]
    return malloc_size(ptr);
    #[cfg(target_os = "linux")]
    return malloc_usable_size(ptr as *mut c_void);
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    0
}
extern "C" fn mach_host_self_impl() -> u32 { 1 }
unsafe extern "C" fn host_info_stub(
    _host: u32,
    _flavor: i32,
    out: *mut u32,
    count: *mut u32,
) -> i32 {
    if !out.is_null() && !count.is_null() {
        std::ptr::write_bytes(out, 0, (*count as usize).min(64));
    }
    0
}
extern "C" fn mach_error_string_stub(_err: i32) -> *const c_char {
    c"ok".as_ptr()
}
extern "C" fn maskrune_l_impl(c: i32, f: usize, _loc: *const c_void) -> i32 {
    maskrune_impl(c, f)
}
extern "C" fn mb_cur_max_l_impl(_loc: *const c_void) -> usize { 1 }
unsafe extern "C" fn srandomdev_impl() {
    let t = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos() ^ (d.as_secs() as u32))
        .unwrap_or(1);
    srand(t);
}
extern "C" fn resolv_zero_stub() -> i32 { 0 }

extern "C" fn sec_item_not_found() -> i32 { -25300 }
extern "C" fn asl_open_stub(_ident: *const c_char, _facility: *const c_char, _opts: u32) -> *const c_void {
    &raw const OS_LOG_STUB as *const c_void
}
extern "C" fn asl_ok_stub() -> i32 { 0 }

extern "C" fn cxx_new(sz: usize) -> *mut c_void {
    unsafe { malloc(sz.max(1)) }
}
extern "C" fn cxx_new_aligned(sz: usize, align: usize) -> *mut c_void {
    let a = align.max(std::mem::size_of::<*mut c_void>()).next_power_of_two();
    let mut ptr: *mut c_void = std::ptr::null_mut();
    unsafe {
        if posix_memalign(&mut ptr, a, sz.max(1)) == 0 && !ptr.is_null() {
            ptr
        } else {
            malloc(sz.max(1))
        }
    }
}
extern "C" fn cxx_delete(ptr: *mut c_void) {
    if !ptr.is_null() {
        unsafe { free(ptr) };
    }
}
extern "C" fn cxx_delete_aligned(ptr: *mut c_void, _align: usize) {
    cxx_delete(ptr);
}
extern "C" fn cxa_guard_acquire(guard: *mut u64) -> i32 {
    if guard.is_null() { return 0; }
    unsafe {
        if (*guard & 0xFF) == 0 { 1 } else { 0 }
    }
}
extern "C" fn cxa_guard_release(guard: *mut u64) {
    if !guard.is_null() {
        unsafe { *guard = 1 };
    }
}
extern "C" fn cxa_guard_abort(_guard: *mut u64) {}
extern "C" fn cxa_pure_virtual_trap() -> ! {
    eprintln!("[pear-runtime] __cxa_pure_virtual reached");
    std::process::abort()
}

/// Apple ARM64 libc++ (`_LIBCPP_ABI_ALTERNATE_STRING_LAYOUT`) runtime support.
/// Matches Darwin's 24-byte `std::__1::basic_string<char>` ABI (byte 23 bit 7 = long flag)
/// rather than Linux libc++'s default short-string layout (byte 0 bit 0 = long flag).
mod apple_libcxx {
    use std::collections::HashMap;
    use std::ffi::{c_char, c_int, c_void, CStr};
    use std::sync::{Condvar, Mutex, OnceLock};

    extern "C" {
        fn malloc(size: usize) -> *mut c_void;
        fn free(ptr: *mut c_void);
    }

    const LONG_FLAG: u8 = 0x80;
    const LONG_CAP_MASK: usize = 1usize << 63;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct AppleString {
        pub words: [usize; 3],
    }

    #[inline]
    unsafe fn is_long(s: *const u8) -> bool {
        (*s.add(23) & LONG_FLAG) != 0
    }

    #[inline]
    unsafe fn read_slice<'a>(s: *const AppleString) -> &'a [u8] {
        let p = s as *const u8;
        if is_long(p) {
            let ptr = *(s as *const *const u8);
            let len = *((s as *const usize).add(1));
            if ptr.is_null() || len == 0 {
                &[]
            } else {
                std::slice::from_raw_parts(ptr, len)
            }
        } else {
            let len = (*p.add(23) & 0x7F) as usize;
            std::slice::from_raw_parts(p, len.min(22))
        }
    }

    #[inline]
    unsafe fn cap_of(s: *const AppleString) -> usize {
        let p = s as *const u8;
        if is_long(p) {
            let raw_cap = *((s as *const usize).add(2));
            (raw_cap & !LONG_CAP_MASK).saturating_sub(1)
        } else {
            22
        }
    }

    unsafe fn set_bytes(s: *mut AppleString, bytes: &[u8]) {
        let old_p = s as *mut u8;
        let old_long = is_long(old_p);
        let old_ptr = if old_long { *(s as *const *mut u8) } else { std::ptr::null_mut() };
        let old_cap = if old_long {
            (*((s as *const usize).add(2)) & !LONG_CAP_MASK).saturating_sub(1)
        } else {
            22
        };

        if bytes.len() <= 22 && !old_long {
            std::ptr::write_bytes(old_p, 0, 24);
            if !bytes.is_empty() {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), old_p, bytes.len());
            }
            *old_p.add(23) = bytes.len() as u8;
            return;
        }

        if old_long && bytes.len() <= old_cap && !old_ptr.is_null() {
            if !bytes.is_empty() {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), old_ptr, bytes.len());
            }
            *old_ptr.add(bytes.len()) = 0;
            *((s as *mut usize).add(1)) = bytes.len();
            return;
        }

        let need = bytes.len() + 1;
        let alloc_cap = need.max(32).next_power_of_two();
        let buf = malloc(alloc_cap) as *mut u8;
        if !buf.is_null() {
            if !bytes.is_empty() {
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf, bytes.len());
            }
            *buf.add(bytes.len()) = 0;
        }
        if old_long && !old_ptr.is_null() {
            free(old_ptr as *mut c_void);
        }
        let words = s as *mut usize;
        *words.add(0) = buf as usize;
        *words.add(1) = bytes.len();
        *words.add(2) = alloc_cap | LONG_CAP_MASK;
    }

    pub unsafe extern "C" fn str_copy_ctor(this: *mut AppleString, other: *const AppleString) -> *mut AppleString {
        std::ptr::write_bytes(this as *mut u8, 0, 24);
        let data = if other.is_null() { Vec::new() } else { read_slice(other).to_vec() };
        set_bytes(this, &data);
        this
    }

    pub unsafe extern "C" fn str_substr_ctor(
        this: *mut AppleString,
        other: *const AppleString,
        pos: usize,
        n: usize,
        _alloc: *const c_void,
    ) -> *mut AppleString {
        std::ptr::write_bytes(this as *mut u8, 0, 24);
        let slice = if other.is_null() { &[] } else { read_slice(other) };
        let start = pos.min(slice.len());
        let end = start.saturating_add(n).min(slice.len());
        let copy = slice[start..end].to_vec();
        set_bytes(this, &copy);
        this
    }

    pub unsafe extern "C" fn str_dtor(this: *mut AppleString) -> *mut AppleString {
        if !this.is_null() && is_long(this as *const u8) {
            let ptr = *(this as *const *mut c_void);
            if !ptr.is_null() {
                free(ptr);
            }
            std::ptr::write_bytes(this as *mut u8, 0, 24);
        }
        this
    }

    pub unsafe extern "C" fn str_assign_str(this: *mut AppleString, other: *const AppleString) -> *mut AppleString {
        if this == other as *mut AppleString || other.is_null() {
            return this;
        }
        let data = read_slice(other).to_vec();
        set_bytes(this, &data);
        this
    }

    pub unsafe extern "C" fn str_assign_char(this: *mut AppleString, ch: c_char) -> *mut AppleString {
        let b = [ch as u8];
        set_bytes(this, &b);
        this
    }

    pub unsafe extern "C" fn str_assign_cstr(this: *mut AppleString, s: *const c_char) -> *mut AppleString {
        let data = if s.is_null() { &[][..] } else { CStr::from_ptr(s).to_bytes() }.to_vec();
        set_bytes(this, &data);
        this
    }

    pub unsafe extern "C" fn str_assign_cstr_n(this: *mut AppleString, s: *const c_char, n: usize) -> *mut AppleString {
        let data = if s.is_null() || n == 0 {
            Vec::new()
        } else {
            std::slice::from_raw_parts(s as *const u8, n).to_vec()
        };
        set_bytes(this, &data);
        this
    }

    pub unsafe extern "C" fn str_append_cstr(this: *mut AppleString, s: *const c_char) -> *mut AppleString {
        let tail = if s.is_null() { &[][..] } else { CStr::from_ptr(s).to_bytes() };
        let mut buf = read_slice(this).to_vec();
        buf.extend_from_slice(tail);
        set_bytes(this, &buf);
        this
    }

    pub unsafe extern "C" fn str_append_cstr_n(this: *mut AppleString, s: *const c_char, n: usize) -> *mut AppleString {
        let tail = if s.is_null() || n == 0 {
            &[][..]
        } else {
            std::slice::from_raw_parts(s as *const u8, n)
        };
        let mut buf = read_slice(this).to_vec();
        buf.extend_from_slice(tail);
        set_bytes(this, &buf);
        this
    }

    pub unsafe extern "C" fn str_push_back(this: *mut AppleString, ch: c_char) {
        let mut buf = read_slice(this).to_vec();
        buf.push(ch as u8);
        set_bytes(this, &buf);
    }

    pub unsafe extern "C" fn str_erase(this: *mut AppleString, pos: usize, n: usize) -> *mut AppleString {
        let cur = read_slice(this);
        let start = pos.min(cur.len());
        let end = start.saturating_add(n).min(cur.len());
        let mut out = Vec::with_capacity(cur.len() - (end - start));
        out.extend_from_slice(&cur[..start]);
        out.extend_from_slice(&cur[end..]);
        set_bytes(this, &out);
        this
    }

    pub unsafe extern "C" fn str_resize(this: *mut AppleString, n: usize, ch: c_char) {
        let mut cur = read_slice(this).to_vec();
        cur.resize(n, ch as u8);
        set_bytes(this, &cur);
    }

    pub unsafe extern "C" fn str_reserve(this: *mut AppleString, new_cap: usize) {
        if new_cap <= cap_of(this) {
            return;
        }
        let cur = read_slice(this).to_vec();
        let alloc_cap = (new_cap + 1).next_power_of_two();
        let buf = malloc(alloc_cap) as *mut u8;
        if !buf.is_null() {
            if !cur.is_empty() {
                std::ptr::copy_nonoverlapping(cur.as_ptr(), buf, cur.len());
            }
            *buf.add(cur.len()) = 0;
        }
        if is_long(this as *const u8) {
            let old = *(this as *const *mut c_void);
            if !old.is_null() {
                free(old);
            }
        }
        let words = this as *mut usize;
        *words.add(0) = buf as usize;
        *words.add(1) = cur.len();
        *words.add(2) = alloc_cap | LONG_CAP_MASK;
    }

    pub unsafe extern "C" fn str_find_char(this: *const AppleString, ch: c_char, pos: usize) -> usize {
        let s = read_slice(this);
        if pos >= s.len() {
            return usize::MAX;
        }
        s[pos..].iter().position(|&b| b == ch as u8).map_or(usize::MAX, |i| pos + i)
    }

    pub unsafe extern "C" fn str_rfind_char(this: *const AppleString, ch: c_char, pos: usize) -> usize {
        let s = read_slice(this);
        if s.is_empty() {
            return usize::MAX;
        }
        let limit = pos.min(s.len() - 1);
        s[..=limit].iter().rposition(|&b| b == ch as u8).unwrap_or(usize::MAX)
    }

    pub unsafe extern "C" fn str_compare_cstr(this: *const AppleString, rhs: *const c_char) -> c_int {
        let a = read_slice(this);
        let b = if rhs.is_null() { &[][..] } else { CStr::from_ptr(rhs).to_bytes() };
        match a.cmp(b) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }
    }

    pub unsafe extern "C" fn str_plus_cstr_str(lhs: *const c_char, rhs: *const AppleString) -> AppleString {
        let a = if lhs.is_null() { &[][..] } else { CStr::from_ptr(lhs).to_bytes() };
        let b = if rhs.is_null() { &[][..] } else { read_slice(rhs) };
        let mut out = AppleString { words: [0; 3] };
        let mut combined = Vec::with_capacity(a.len() + b.len());
        combined.extend_from_slice(a);
        combined.extend_from_slice(b);
        set_bytes(&mut out, &combined);
        out
    }

    pub unsafe extern "C" fn str_init_ptr_len(this: *mut AppleString, s: *const c_char, n: usize) {
        std::ptr::write_bytes(this as *mut u8, 0, 24);
        let data = if s.is_null() || n == 0 {
            &[][..]
        } else {
            std::slice::from_raw_parts(s as *const u8, n)
        };
        set_bytes(this, data);
    }

    pub unsafe extern "C" fn str_init_ptr_len_cap(
        this: *mut AppleString,
        s: *const c_char,
        len: usize,
        _cap: usize,
    ) {
        str_init_ptr_len(this, s, len);
    }

    pub unsafe extern "C" fn str_init_count_char(this: *mut AppleString, count: usize, ch: c_char) {
        std::ptr::write_bytes(this as *mut u8, 0, 24);
        let data = vec![ch as u8; count];
        set_bytes(this, &data);
    }

    pub unsafe extern "C" fn str_at(this: *mut AppleString, idx: usize) -> *mut c_char {
        let p = this as *mut u8;
        let (base, len) = if is_long(p) {
            (*(this as *const *mut u8), *((this as *const usize).add(1)))
        } else {
            (p, (*p.add(23) & 0x7F) as usize)
        };
        if idx >= len || base.is_null() {
            throw_abort();
        }
        base.add(idx) as *mut c_char
    }

    pub unsafe extern "C" fn str_compare_pos_n_cstr(
        this: *const AppleString,
        pos: usize,
        n: usize,
        rhs: *const c_char,
    ) -> c_int {
        let b = if rhs.is_null() { &[][..] } else { CStr::from_ptr(rhs).to_bytes() };
        str_compare_pos_n_cstr_len(this, pos, n, b.as_ptr() as *const c_char, b.len())
    }

    pub unsafe extern "C" fn str_compare_pos_n_cstr_len(
        this: *const AppleString,
        pos: usize,
        n: usize,
        rhs: *const c_char,
        rhs_len: usize,
    ) -> c_int {
        let slice = read_slice(this);
        if pos > slice.len() {
            throw_abort();
        }
        let sub = &slice[pos..pos.saturating_add(n).min(slice.len())];
        let b = if rhs.is_null() || rhs_len == 0 {
            &[][..]
        } else {
            std::slice::from_raw_parts(rhs as *const u8, rhs_len)
        };
        match sub.cmp(b) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }
    }

    pub unsafe extern "C" fn str_append_substr(
        this: *mut AppleString,
        other: *const AppleString,
        pos: usize,
        n: usize,
    ) -> *mut AppleString {
        let src = if other.is_null() { &[] } else { read_slice(other) };
        let start = pos.min(src.len());
        let end = start.saturating_add(n).min(src.len());
        let tail = src[start..end].to_vec();
        let mut buf = read_slice(this).to_vec();
        buf.extend_from_slice(&tail);
        set_bytes(this, &buf);
        this
    }

    pub unsafe extern "C" fn str_append_count_char(
        this: *mut AppleString,
        count: usize,
        ch: c_char,
    ) -> *mut AppleString {
        let mut buf = read_slice(this).to_vec();
        buf.resize(buf.len() + count, ch as u8);
        set_bytes(this, &buf);
        this
    }

    pub unsafe extern "C" fn str_assign_substr(
        this: *mut AppleString,
        other: *const AppleString,
        pos: usize,
        n: usize,
    ) -> *mut AppleString {
        let src = if other.is_null() { &[] } else { read_slice(other) };
        let start = pos.min(src.len());
        let end = start.saturating_add(n).min(src.len());
        let data = src[start..end].to_vec();
        set_bytes(this, &data);
        this
    }

    pub unsafe extern "C" fn str_assign_count_char(
        this: *mut AppleString,
        count: usize,
        ch: c_char,
    ) -> *mut AppleString {
        let data = vec![ch as u8; count];
        set_bytes(this, &data);
        this
    }

    pub unsafe extern "C" fn str_insert_cstr(
        this: *mut AppleString,
        pos: usize,
        s: *const c_char,
    ) -> *mut AppleString {
        let ins = if s.is_null() { &[][..] } else { CStr::from_ptr(s).to_bytes() };
        str_insert_cstr_n(this, pos, ins.as_ptr() as *const c_char, ins.len())
    }

    pub unsafe extern "C" fn str_insert_cstr_n(
        this: *mut AppleString,
        pos: usize,
        s: *const c_char,
        n: usize,
    ) -> *mut AppleString {
        let cur = read_slice(this);
        let at = pos.min(cur.len());
        let ins = if s.is_null() || n == 0 {
            &[][..]
        } else {
            std::slice::from_raw_parts(s as *const u8, n)
        };
        let mut out = Vec::with_capacity(cur.len() + ins.len());
        out.extend_from_slice(&cur[..at]);
        out.extend_from_slice(ins);
        out.extend_from_slice(&cur[at..]);
        set_bytes(this, &out);
        this
    }

    pub unsafe extern "C" fn str_grow_by_and_replace(
        this: *mut AppleString,
        old_cap: usize,
        delta_cap: usize,
        old_sz: usize,
        n_copy: usize,
        n_del: usize,
        n_add: usize,
        p_new_stuff: *const c_char,
    ) {
        let _ = (old_cap, delta_cap);
        let cur = read_slice(this);
        let prefix = &cur[..n_copy.min(old_sz).min(cur.len())];
        let tail_start = n_copy.saturating_add(n_del).min(old_sz).min(cur.len());
        let suffix = &cur[tail_start..old_sz.min(cur.len())];
        let add = if p_new_stuff.is_null() || n_add == 0 {
            &[][..]
        } else {
            std::slice::from_raw_parts(p_new_stuff as *const u8, n_add)
        };
        let mut out = Vec::with_capacity(prefix.len() + add.len() + suffix.len());
        out.extend_from_slice(prefix);
        out.extend_from_slice(add);
        out.extend_from_slice(suffix);
        set_bytes(this, &out);
    }

    #[derive(Default)]
    struct RecLockState {
        owner: Option<std::thread::ThreadId>,
        depth: usize,
    }

    fn rec_mutex_map() -> &'static Mutex<HashMap<usize, (&'static Mutex<RecLockState>, &'static Condvar)>> {
        static M: OnceLock<Mutex<HashMap<usize, (&'static Mutex<RecLockState>, &'static Condvar)>>> = OnceLock::new();
        M.get_or_init(Default::default)
    }

    fn get_rec_lock(ptr: usize) -> (&'static Mutex<RecLockState>, &'static Condvar) {
        let mut g = rec_mutex_map().lock().unwrap();
        *g.entry(ptr).or_insert_with(|| {
            (Box::leak(Box::new(Mutex::new(RecLockState::default()))), Box::leak(Box::new(Condvar::new())))
        })
    }

    pub extern "C" fn rmutex_ctor(this: *mut c_void) -> *mut c_void {
        if !this.is_null() {
            let _ = get_rec_lock(this as usize);
        }
        this
    }
    pub extern "C" fn rmutex_dtor(this: *mut c_void) -> *mut c_void {
        if !this.is_null() {
            rec_mutex_map().lock().unwrap().remove(&(this as usize));
        }
        this
    }
    pub extern "C" fn rmutex_lock(this: *mut c_void) {
        if this.is_null() { return; }
        let (mtx, cv) = get_rec_lock(this as usize);
        let me = std::thread::current().id();
        let mut st = mtx.lock().unwrap();
        while let Some(owner) = st.owner {
            if owner == me {
                st.depth += 1;
                return;
            }
            st = cv.wait(st).unwrap();
        }
        st.owner = Some(me);
        st.depth = 1;
    }
    pub extern "C" fn rmutex_unlock(this: *mut c_void) {
        if this.is_null() { return; }
        let (mtx, cv) = get_rec_lock(this as usize);
        let me = std::thread::current().id();
        let mut st = mtx.lock().unwrap();
        if st.owner == Some(me) {
            if st.depth > 1 {
                st.depth -= 1;
            } else {
                st.owner = None;
                st.depth = 0;
                cv.notify_one();
            }
        }
    }

    pub extern "C" fn next_prime(mut n: usize) -> usize {
        if n <= 2 { return 2; }
        if n % 2 == 0 { n += 1; }
        loop {
            let mut d = 3usize;
            let mut prime = true;
            while d.saturating_mul(d) <= n {
                if n % d == 0 { prime = false; break; }
                d += 2;
            }
            if prime { return n; }
            n += 2;
        }
    }

    pub unsafe extern "C" fn sort_ints(first: *mut c_int, last: *mut c_int, _comp: *const c_void) {
        if first.is_null() || last.is_null() || last <= first { return; }
        let len = (last as usize - first as usize) / std::mem::size_of::<c_int>();
        std::slice::from_raw_parts_mut(first, len).sort_unstable();
    }

    static mut RS_STUB: usize = 0;
    static RNG_STATE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0x9e37_79b9_7f4a_7c15);

    pub unsafe extern "C" fn rs_get() -> *mut c_void {
        &raw mut RS_STUB as *mut c_void
    }
    pub extern "C" fn rs_default_call(_this: *mut c_void) -> u32 {
        let mut x = RNG_STATE.load(std::sync::atomic::Ordering::Relaxed);
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        RNG_STATE.store(x, std::sync::atomic::Ordering::Relaxed);
        (x >> 32) as u32
    }
    pub extern "C" fn rs_default_dtor(this: *mut c_void) -> *mut c_void { this }

    pub extern "C" fn throw_abort() -> ! {
        eprintln!("[pear-runtime][libc++] container/length error");
        std::process::abort()
    }

    pub unsafe extern "C" fn call_once_impl(
        flag: *mut std::sync::atomic::AtomicUsize,
        arg: *mut c_void,
        func: unsafe extern "C" fn(*mut c_void),
    ) {
        use std::sync::atomic::Ordering;
        const DONE: usize = !0usize;
        if flag.is_null() {
            return;
        }
        let atom = &*flag;
        if atom.load(Ordering::Acquire) == DONE {
            return;
        }
        static ONCE_SYNC: OnceLock<(Mutex<()>, Condvar)> = OnceLock::new();
        let (mtx, cv) = ONCE_SYNC.get_or_init(|| (Mutex::new(()), Condvar::new()));
        let mut guard = mtx.lock().unwrap();
        loop {
            match atom.load(Ordering::Acquire) {
                DONE => return,
                0 => {
                    atom.store(1, Ordering::Release);
                    drop(guard);
                    func(arg);
                    let _done_guard = mtx.lock().unwrap();
                    atom.store(DONE, Ordering::Release);
                    cv.notify_all();
                    return;
                }
                _ => {
                    guard = cv.wait(guard).unwrap();
                }
            }
        }
    }

    fn plain_mutex_map() -> &'static Mutex<HashMap<usize, std::sync::Arc<(Mutex<bool>, Condvar)>>> {
        static M: OnceLock<Mutex<HashMap<usize, std::sync::Arc<(Mutex<bool>, Condvar)>>>> = OnceLock::new();
        M.get_or_init(Default::default)
    }

    fn get_plain_mutex(ptr: usize) -> std::sync::Arc<(Mutex<bool>, Condvar)> {
        let mut g = plain_mutex_map().lock().unwrap();
        g.entry(ptr)
            .or_insert_with(|| std::sync::Arc::new((Mutex::new(false), Condvar::new())))
            .clone()
    }

    pub extern "C" fn mutex_ctor(this: *mut c_void) -> *mut c_void {
        if !this.is_null() {
            let _ = get_plain_mutex(this as usize);
        }
        this
    }
    pub extern "C" fn mutex_dtor(this: *mut c_void) -> *mut c_void {
        if !this.is_null() {
            plain_mutex_map().lock().unwrap().remove(&(this as usize));
        }
        this
    }
    pub extern "C" fn mutex_lock(this: *mut c_void) {
        if this.is_null() { return; }
        let entry = get_plain_mutex(this as usize);
        let mut locked = entry.0.lock().unwrap();
        while *locked {
            locked = entry.1.wait(locked).unwrap();
        }
        *locked = true;
    }
    pub extern "C" fn mutex_trylock(this: *mut c_void) -> bool {
        if this.is_null() { return false; }
        let entry = get_plain_mutex(this as usize);
        let mut locked = entry.0.lock().unwrap();
        if *locked {
            false
        } else {
            *locked = true;
            true
        }
    }
    pub extern "C" fn mutex_unlock(this: *mut c_void) {
        if this.is_null() { return; }
        let entry = get_plain_mutex(this as usize);
        let mut locked = entry.0.lock().unwrap();
        *locked = false;
        entry.1.notify_one();
    }

    fn cv_map() -> &'static Mutex<HashMap<usize, std::sync::Arc<(Mutex<u64>, Condvar)>>> {
        static M: OnceLock<Mutex<HashMap<usize, std::sync::Arc<(Mutex<u64>, Condvar)>>>> = OnceLock::new();
        M.get_or_init(Default::default)
    }

    fn get_cv(ptr: usize) -> std::sync::Arc<(Mutex<u64>, Condvar)> {
        let mut g = cv_map().lock().unwrap();
        g.entry(ptr)
            .or_insert_with(|| std::sync::Arc::new((Mutex::new(0), Condvar::new())))
            .clone()
    }

    pub extern "C" fn cv_dtor(this: *mut c_void) -> *mut c_void {
        if !this.is_null() {
            cv_map().lock().unwrap().remove(&(this as usize));
        }
        this
    }
    pub unsafe extern "C" fn cv_wait(this: *mut c_void, unique_lock: *mut c_void) {
        if this.is_null() || unique_lock.is_null() { return; }
        let mtx = *(unique_lock as *const *mut c_void);
        let cv = get_cv(this as usize);
        let mut seq_guard = cv.0.lock().unwrap();
        let target_seq = *seq_guard;
        mutex_unlock(mtx);
        while *seq_guard == target_seq {
            seq_guard = cv.1.wait(seq_guard).unwrap();
        }
        drop(seq_guard);
        mutex_lock(mtx);
    }
    pub unsafe extern "C" fn cv_timed_wait(
        this: *mut c_void,
        unique_lock: *mut c_void,
        abs_ns_ptr: *const i64,
    ) {
        if abs_ns_ptr.is_null() {
            cv_wait(this, unique_lock);
            return;
        }
        if this.is_null() || unique_lock.is_null() { return; }
        let mtx = *(unique_lock as *const *mut c_void);
        let abs_ns = *abs_ns_ptr;
        let now_ns = chrono_system_now() * 1_000;
        let cv = get_cv(this as usize);
        let seq_guard = cv.0.lock().unwrap();
        let target_seq = *seq_guard;
        mutex_unlock(mtx);
        if abs_ns > now_ns {
            let dur = std::time::Duration::from_nanos((abs_ns - now_ns) as u64);
            let (g, _) = cv.1.wait_timeout_while(seq_guard, dur, |s| *s == target_seq).unwrap();
            drop(g);
        } else {
            drop(seq_guard);
        }
        mutex_lock(mtx);
    }
    pub extern "C" fn cv_notify_one(this: *mut c_void) {
        if this.is_null() { return; }
        let cv = get_cv(this as usize);
        let mut seq = cv.0.lock().unwrap();
        *seq = seq.wrapping_add(1);
        cv.1.notify_one();
    }
    pub extern "C" fn cv_notify_all(this: *mut c_void) {
        if this.is_null() { return; }
        let cv = get_cv(this as usize);
        let mut seq = cv.0.lock().unwrap();
        *seq = seq.wrapping_add(1);
        cv.1.notify_all();
    }

    pub unsafe extern "C" fn this_thread_sleep_for(ns_ptr: *const i64) {
        if !ns_ptr.is_null() && *ns_ptr > 0 {
            std::thread::sleep(std::time::Duration::from_nanos(*ns_ptr as u64));
        }
    }

    pub extern "C" fn chrono_steady_now() -> i64 {
        static ORIGIN: OnceLock<std::time::Instant> = OnceLock::new();
        let orig = ORIGIN.get_or_init(std::time::Instant::now);
        orig.elapsed().as_nanos().min(i64::MAX as u128) as i64
    }

    pub extern "C" fn chrono_system_now() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros().min(i64::MAX as u128) as i64)
            .unwrap_or(0)
    }

    pub unsafe extern "C" fn chrono_to_time_t(tp_us: *const i64) -> isize {
        if tp_us.is_null() { 0 } else { (*tp_us / 1_000_000) as isize }
    }

    pub extern "C" fn chrono_from_time_t(t: isize) -> i64 {
        (t as i64).saturating_mul(1_000_000)
    }

    fn sp_mut_pool() -> &'static [std::sync::atomic::AtomicU32; 16] {
        static POOL: OnceLock<[std::sync::atomic::AtomicU32; 16]> = OnceLock::new();
        POOL.get_or_init(|| std::array::from_fn(|_| std::sync::atomic::AtomicU32::new(0)))
    }
    pub extern "C" fn get_sp_mut(obj: *const c_void) -> *const c_void {
        let pool = sp_mut_pool();
        let idx = ((obj as usize) >> 4) & 15;
        &pool[idx] as *const std::sync::atomic::AtomicU32 as *const c_void
    }
    pub extern "C" fn sp_mut_lock(m: *mut c_void) {
        super::os_unfair_lock_lock(m as *mut std::sync::atomic::AtomicU32);
    }
    pub extern "C" fn sp_mut_unlock(m: *mut c_void) {
        super::os_unfair_lock_unlock(m as *mut std::sync::atomic::AtomicU32);
    }

    pub unsafe extern "C" fn shared_count_dtor(this: *mut c_void) -> *mut c_void { this }
    pub unsafe extern "C" fn shared_weak_release_weak(this: *mut c_void) {
        if this.is_null() { return; }
        let weak = &*((this as *const std::sync::atomic::AtomicIsize).add(2));
        if weak.fetch_sub(1, std::sync::atomic::Ordering::AcqRel) == 0 {
            let vtbl = *(this as *const *const usize);
            if !vtbl.is_null() {
                let on_zero_weak: unsafe extern "C" fn(*mut c_void) = std::mem::transmute(*vtbl.add(4));
                on_zero_weak(this);
            }
        }
    }
    pub unsafe extern "C" fn shared_weak_lock(this: *mut c_void) -> *mut c_void {
        if this.is_null() { return std::ptr::null_mut(); }
        let owners = &*((this as *const std::sync::atomic::AtomicIsize).add(1));
        let mut cur = owners.load(std::sync::atomic::Ordering::Acquire);
        loop {
            if cur < 0 {
                return std::ptr::null_mut();
            }
            match owners.compare_exchange_weak(
                cur,
                cur + 1,
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
            ) {
                Ok(_) => return this,
                Err(next) => cur = next,
            }
        }
    }
    pub unsafe extern "C" fn shared_weak_get_deleter(_this: *const c_void, _ti: *const c_void) -> *const c_void {
        std::ptr::null()
    }

    pub unsafe extern "C" fn sort_u32(first: *mut u32, last: *mut u32, _comp: *const c_void) {
        if first.is_null() || last.is_null() || last <= first { return; }
        let len = (last as usize - first as usize) / std::mem::size_of::<u32>();
        std::slice::from_raw_parts_mut(first, len).sort_unstable();
    }
    pub unsafe extern "C" fn sort_u64(first: *mut u64, last: *mut u64, _comp: *const c_void) {
        if first.is_null() || last.is_null() || last <= first { return; }
        let len = (last as usize - first as usize) / std::mem::size_of::<u64>();
        std::slice::from_raw_parts_mut(first, len).sort_unstable();
    }

    pub extern "C" fn random_device_ctor(this: *mut c_void, _token: *const c_void) -> *mut c_void { this }
    pub extern "C" fn random_device_dtor(this: *mut c_void) -> *mut c_void { this }
    pub extern "C" fn random_device_call(_this: *mut c_void) -> u32 {
        rs_default_call(std::ptr::null_mut())
    }
}

fn dlopen_dlsym(candidates: &[&std::ffi::CStr], macho_sym: &str) -> Option<usize> {
    let cname = macho_sym.strip_prefix('_')?;
    let cstr = std::ffi::CString::new(cname).ok()?;
    for cand in candidates {
        let h = unsafe { dlopen(cand.as_ptr(), 1) };
        if !h.is_null() {
            let p = unsafe { dlsym(h, cstr.as_ptr()) };
            if !p.is_null() {
                return Some(p as usize);
            }
        }
    }
    libc_dlsym(macho_sym)
}

extern "C" fn maskrune_impl(c: i32, f: usize) -> i32 {
    if (0..128).contains(&c) {
        let rt = darwin_ascii_runetype(c as u8) as usize;
        if (rt & f) != 0 { 1 } else { 0 }
    } else {
        0
    }
}

extern "C" fn ns_get_executable_path(buf: *mut c_char, bufsize: *mut u32) -> i32 {
    let path = std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "/PearGuest".to_string());
    let bytes = path.as_bytes();
    let needed = (bytes.len() + 1) as u32;
    if buf.is_null() || bufsize.is_null() || unsafe { *bufsize } < needed {
        if !bufsize.is_null() {
            unsafe { *bufsize = needed };
        }
        return -1;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, bytes.len());
        *buf.add(bytes.len()) = 0;
    }
    0
}

extern "C" fn status_ok_stub() -> i32 { 0 }
extern "C" fn ptr_null_stub() -> *mut c_void { std::ptr::null_mut() }
extern "C" fn false_stub() -> bool { false }

fn framework_const(key: &'static str) -> usize {
    static MAP: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<&'static str, usize>>> =
        std::sync::OnceLock::new();
    let m = MAP.get_or_init(Default::default);
    let mut g = m.lock().unwrap();
    *g.entry(key).or_insert_with(|| {
        let s = pear_foundation::nsstring_from_rust(key) as usize;
        Box::into_raw(Box::new(s)) as usize
    })
}

fn audiotoolbox_symbol(symbol: &str) -> Option<usize> {
    let ok = status_ok_stub as *const () as usize;
    let null = ptr_null_stub as *const () as usize;
    match symbol {
        "_AudioComponentFindNext" => Some(null),
        "_AudioComponentInstanceNew"
        | "_AudioComponentInstanceDispose"
        | "_AudioConverterNew"
        | "_AudioConverterDispose"
        | "_AudioConverterReset"
        | "_AudioConverterGetProperty"
        | "_AudioConverterSetProperty"
        | "_AudioConverterFillComplexBuffer"
        | "_AudioFileOpenWithCallbacks"
        | "_AudioFileClose"
        | "_AudioFileGetGlobalInfo"
        | "_AudioFileGetProperty"
        | "_AudioFileGetPropertyInfo"
        | "_AudioFileReadPackets"
        | "_AudioFormatGetProperty"
        | "_AudioOutputUnitStart"
        | "_AudioOutputUnitStop"
        | "_AudioSessionInitialize"
        | "_AudioSessionSetActive"
        | "_AudioSessionSetProperty"
        | "_AudioUnitAddPropertyListener"
        | "_AudioUnitGetProperty"
        | "_AudioUnitSetProperty"
        | "_AudioUnitInitialize"
        | "_AudioUnitUninitialize"
        | "_ExtAudioFileOpenURL"
        | "_ExtAudioFileDispose"
        | "_ExtAudioFileGetProperty"
        | "_ExtAudioFileSetProperty"
        | "_ExtAudioFileRead"
        | "_ExtAudioFileSeek"
        | "_NewAUGraph"
        | "_DisposeAUGraph"
        | "_AUGraphAddNode"
        | "_AUGraphConnectNodeInput"
        | "_AUGraphInitialize"
        | "_AUGraphNodeInfo"
        | "_AUGraphOpen"
        | "_AUGraphSetNodeInputCallback"
        | "_AUGraphStart"
        | "_AUGraphStop"
        | "_AudioServicesPlaySystemSound"
        | "_AudioServicesPlaySystemSoundWithCompletion"
        | "_AudioServicesAddSystemSoundCompletion"
        | "_AudioServicesCreateSystemSoundID"
        | "_AudioServicesDisposeSystemSoundID"
        | "_AudioServicesRemoveSystemSoundCompletion"
        | "_AudioServicesSetProperty"
        | "_AudioUnitSetParameter"
        | "_CAShow" => Some(ok),
        _ => None,
    }
}

fn cfnetwork_symbol(symbol: &str) -> Option<usize> {
    let null = ptr_null_stub as *const () as usize;
    let ok = status_ok_stub as *const () as usize;
    match symbol {
        "_CFHTTPMessageCreateRequest"
        | "_CFHTTPMessageCopySerializedMessage"
        | "_CFNetworkCopySystemProxySettings"
        | "_CFReadStreamCreateForHTTPRequest"
        | "_CFReadStreamCreateWithFTPURL"
        | "_CFHostCreateWithName"
        | "_CFHostGetAddressing" => Some(null),
        "_CFHTTPMessageAddAuthentication"
        | "_CFHTTPMessageAppendBytes"
        | "_CFHTTPMessageSetHeaderFieldValue"
        | "_CFHostStartInfoResolution" => Some(ok),
        "_kCFHTTPAuthenticationSchemeBasic" => Some(framework_const("Basic")),
        "_kCFHTTPVersion1_0" => Some(framework_const("HTTP/1.0")),
        "_kCFHTTPVersion1_1" => Some(framework_const("HTTP/1.1")),
        "_kCFStreamPropertyHTTPProxy" => Some(framework_const("HTTPProxy")),
        "_kCFStreamPropertyHTTPResponseHeader" => Some(framework_const("HTTPResponseHeader")),
        "_kCFStreamPropertySSLSettings" => Some(framework_const("SSLSettings")),
        "_kCFStreamSSLValidatesCertificateChain" => Some(framework_const("SSLValidatesCertificateChain")),
        "_kCFStreamPropertyFTPProxy" => Some(framework_const("FTPProxy")),
        "_kCFStreamPropertyFTPFetchResourceInfo" => Some(framework_const("FTPFetchResourceInfo")),
        "_kCFStreamPropertyFTPFileTransferOffset" => Some(framework_const("FTPFileTransferOffset")),
        "_kCFStreamPropertyFTPResourceSize" => Some(framework_const("FTPResourceSize")),
        "_kCFStreamPropertyFTPUsePassiveMode" => Some(framework_const("FTPUsePassiveMode")),
        _ => None,
    }
}

fn quartz_const(name: &str) -> usize {
    static MAP: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<&'static str, usize>>> =
        std::sync::OnceLock::new();
    let m = MAP.get_or_init(Default::default);
    let mut g = m.lock().unwrap();
    let key: &'static str = match name {
        "_kCAFilterLinear" => "linear",
        "_kCAGravityResize" => "resize",
        "_kCAMediaTimingFunctionEaseInEaseOut" => "easeInEaseOut",
        "_kCAMediaTimingFunctionLinear" => "linear",
        "_kCAMediaTimingFunctionEaseOut" => "easeOut",
        "_kCAFillModeForwards" => "forwards",
        _ => "",
    };
    *g.entry(key).or_insert_with(|| {
        let s = pear_foundation::nsstring_from_rust(key) as usize;
        Box::into_raw(Box::new(s)) as usize
    })
}

#[cfg(target_os = "linux")]
struct DefaultGuestEvents;

#[cfg(target_os = "linux")]
fn find_touch_target(win: Id) -> (Id, Id) {
    let sel_began = objc_rt::sel("touchesBegan:withEvent:");
    let noop_imp = touches_noop as *const () as usize;
    let overrides_touches = |obj: Id| -> bool {
        if obj.is_null() {
            return false;
        }
        let d = pear_libobjc::message::lookup_for_send(obj, sel_began);
        !d.imp.is_null() && (d.imp as usize) != noop_imp
    };
    let vc = if !win.is_null() {
        ui_window_root_vc(win, objc_rt::sel("rootViewController"))
    } else {
        std::ptr::null_mut()
    };
    let root_view = if !vc.is_null() {
        uivc_view_if_loaded(vc, objc_rt::sel("viewIfLoaded"))
    } else {
        std::ptr::null_mut()
    };

    let mut stack: Vec<Id> = Vec::new();
    if !root_view.is_null() {
        stack.push(root_view);
    }
    if !win.is_null() {
        let win_subs = VIEW_SUBVIEWS
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|m| m.get(&(win as usize)).cloned())
            .unwrap_or_default();
        for s in win_subs {
            if s != root_view as usize {
                stack.push(s as Id);
            }
        }
    }
    let mut visited = std::collections::HashSet::new();
    let mut best_view: Id = root_view;
    let mut dfs = Vec::new();
    for s in stack {
        dfs.push(s);
    }
    while let Some(v) = dfs.pop() {
        if v.is_null() || !visited.insert(v as usize) {
            continue;
        }
        if overrides_touches(v) {
            best_view = v;
        }
        let subs = VIEW_SUBVIEWS
            .lock()
            .unwrap()
            .as_ref()
            .and_then(|m| m.get(&(v as usize)).cloned())
            .unwrap_or_default();
        for ch in subs {
            dfs.push(ch as Id);
        }
    }
    if !best_view.is_null() && overrides_touches(best_view) {
        return (best_view, best_view);
    }
    if overrides_touches(vc) {
        return (vc, if !root_view.is_null() { root_view } else { win });
    }
    if !best_view.is_null() {
        return (best_view, best_view);
    }
    (win, win)
}

#[cfg(target_os = "linux")]
impl pear_host::GuestEvents for DefaultGuestEvents {
    fn pointer(
        &self,
        window: pear_host::GuestObject,
        phase: pear_host::TouchPhase,
        x: f64,
        y: f64,
        timestamp: f64,
    ) {
        use std::sync::atomic::Ordering;
        let mut win = window.0 as Id;
        if win.is_null() {
            win = app_key_window(std::ptr::null_mut(), objc_rt::sel("keyWindow"));
        }
        if win.is_null() {
            return;
        }
        let (target, touch_view) = find_touch_target(win);
        if target.is_null() {
            return;
        }
        let sel_name = match phase {
            pear_host::TouchPhase::Began => "touchesBegan:withEvent:",
            pear_host::TouchPhase::Ended => "touchesEnded:withEvent:",
            pear_host::TouchPhase::Cancelled => "touchesCancelled:withEvent:",
            _ => "touchesMoved:withEvent:",
        };
        let phase_val = phase as isize;
        let existing_touch = ACTIVE_TOUCH_OBJ.load(Ordering::Acquire);
        let touch: Id = if phase == pear_host::TouchPhase::Began || existing_touch == 0 {
            if existing_touch != 0 {
                if let Some(m) = TOUCH_DATA.lock().unwrap().as_mut() {
                    m.remove(&existing_touch);
                }
                pear_libobjc::object::release(existing_touch as Id);
            }
            let t = unsafe { alloc(UITOUCH_CLASS.ptr()) };
            TOUCH_DATA
                .lock()
                .unwrap()
                .get_or_insert_with(std::collections::HashMap::new)
                .insert(
                    t as usize,
                    TouchState {
                        x,
                        y,
                        prev_x: x,
                        prev_y: y,
                        timestamp,
                        phase: phase_val,
                        tap_count: 1,
                        view: touch_view as usize,
                        window: win as usize,
                    },
                );
            ACTIVE_TOUCH_OBJ.store(t as usize, Ordering::Release);
            t
        } else {
            let t = existing_touch as Id;
            let mut g = TOUCH_DATA.lock().unwrap();
            let map = g.get_or_insert_with(std::collections::HashMap::new);
            let prev = map.get(&(t as usize)).copied().unwrap_or(TouchState {
                x,
                y,
                prev_x: x,
                prev_y: y,
                timestamp,
                phase: phase_val,
                tap_count: 1,
                view: touch_view as usize,
                window: win as usize,
            });
            map.insert(
                t as usize,
                TouchState {
                    x,
                    y,
                    prev_x: prev.x,
                    prev_y: prev.y,
                    timestamp,
                    phase: phase_val,
                    tap_count: 1,
                    view: touch_view as usize,
                    window: win as usize,
                },
            );
            t
        };

        let touches_set = pear_foundation::ns_set_with_objects(&[touch]);
        let ev = unsafe { alloc(UIEVENT_CLASS.ptr()) };
        EVENT_TOUCHES
            .lock()
            .unwrap()
            .get_or_insert_with(std::collections::HashMap::new)
            .insert(ev as usize, (touches_set as usize, timestamp));

        unsafe {
            let sel = objc_rt::sel(sel_name);
            let d = pear_libobjc::message::lookup_for_send(target, sel);
            if !d.imp.is_null() {
                let f: extern "C" fn(Id, Sel, Id, Id) = std::mem::transmute(d.imp);
                f(d.receiver, sel, touches_set, ev);
            }
        }

        if let Some(m) = EVENT_TOUCHES.lock().unwrap().as_mut() {
            m.remove(&(ev as usize));
        }
        pear_libobjc::object::release(ev);
        pear_libobjc::object::release(touches_set);
        if matches!(
            phase,
            pear_host::TouchPhase::Ended | pear_host::TouchPhase::Cancelled
        ) {
            if ACTIVE_TOUCH_OBJ
                .compare_exchange(
                    touch as usize,
                    0,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                )
                .is_ok()
            {
                if let Some(m) = TOUCH_DATA.lock().unwrap().as_mut() {
                    m.remove(&(touch as usize));
                }
                pear_libobjc::object::release(touch);
            }
        }
    }
    fn key(&self, _: pear_host::KeyEvent) {}
    fn text(&self, _: &str) {}
    fn edit_command(&self, _: pear_host::EditCommand) {}
    fn text_input_active(&self) -> bool { false }
    fn window_resized(&self, _: pear_host::GuestObject, _: pear_host::Size, _: f64) {}
    fn window_became_key(&self, _: pear_host::GuestObject) {}
    fn lifecycle(&self, _: pear_host::Lifecycle) {}
}

/// Initializes the portable PearRuntime subsystems (ObjC runtime, Foundation,
/// and the platform's host backend when running on Linux).
pub fn init() {
    static STDIO_INIT: Once = Once::new();
    STDIO_INIT.call_once(|| {
        extern "C" {
            fn getpagesize() -> i32;
        }
        let ps = unsafe { getpagesize() };
        if ps >= 4096 {
            VM_PAGE_SIZE_VAL.store(ps as usize, std::sync::atomic::Ordering::Release);
        }
        for (dst, names) in [
            (&raw mut STDIN_STUB, &["___stdinp", "_stdin"][..]),
            (&raw mut STDOUT_STUB, &["___stdoutp", "_stdout"][..]),
            (&raw mut STDERR_STUB, &["___stderrp", "_stderr"][..]),
        ] {
            for &sym in names {
                if let Some(p) = libc_dlsym(sym) {
                    unsafe { *dst = *(p as *const usize) };
                    break;
                }
            }
        }
    });
    pear_libobjc::object::init_runtime();
    pear_libobjc::blocks::init_blocks();
    pear_foundation::init_foundation();
    pear_swift_rt::init();
    init_guest_frameworks();
    pear_metal_vk::init();
    #[cfg(target_os = "linux")]
    {
        let _ = pear_host::install_events(Box::new(DefaultGuestEvents));
        if std::env::var("PEAR_HEADLESS").as_deref() != Ok("1") {
            pear_host_linux::LinuxHost::install().initialize();
        }
    }
    if !pear_host::installed() {
        pear_host::install(Box::new(pear_host::NullHost::default()));
    }
}

fn libcxx_symbol(symbol: &str) -> Option<usize> {
    match symbol {
        "__Znwm" | "__Znam" => Some(cxx_new as *const () as usize),
        "__ZnwmSt11align_val_t" => Some(cxx_new_aligned as *const () as usize),
        "__ZdlPv" | "__ZdaPv" => Some(cxx_delete as *const () as usize),
        "__ZdlPvSt11align_val_t" => Some(cxx_delete_aligned as *const () as usize),
        "___cxa_allocate_exception" => Some(unwind::cxa_allocate_exception as *const () as usize),
        "___cxa_free_exception" => Some(unwind::cxa_free_exception as *const () as usize),
        "___cxa_throw" => Some(unwind::cxa_throw as *const () as usize),
        "___cxa_rethrow" => Some(unwind::cxa_rethrow as *const () as usize),
        "___cxa_begin_catch" => Some(unwind::cxa_begin_catch as *const () as usize),
        "___cxa_end_catch" => Some(unwind::cxa_end_catch as *const () as usize),
        "___cxa_current_exception_type" => Some(unwind::cxa_current_exception_type as *const () as usize),
        "___gxx_personality_v0" => Some(unwind::gxx_personality_v0 as *const () as usize),
        "___cxa_guard_acquire" => Some(cxa_guard_acquire as *const () as usize),
        "___cxa_guard_release" => Some(cxa_guard_release as *const () as usize),
        "___cxa_guard_abort" => Some(cxa_guard_abort as *const () as usize),
        "___cxa_pure_virtual" => Some(cxa_pure_virtual_trap as *const () as usize),
        "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEEC1ERKS5_"
        | "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEEC2ERKS5_" => {
            Some(apple_libcxx::str_copy_ctor as *const () as usize)
        }
        "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEEC1ERKS5_mmRKS4_"
        | "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEEC2ERKS5_mmRKS4_" => {
            Some(apple_libcxx::str_substr_ctor as *const () as usize)
        }
        "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE6__initEPKcm" => {
            Some(apple_libcxx::str_init_ptr_len as *const () as usize)
        }
        "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE6__initEPKcmm" => {
            Some(apple_libcxx::str_init_ptr_len_cap as *const () as usize)
        }
        "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE6__initEmc" => {
            Some(apple_libcxx::str_init_count_char as *const () as usize)
        }
        "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEED1Ev" => {
            Some(apple_libcxx::str_dtor as *const () as usize)
        }
        "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE2atEm"
        | "__ZNKSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE2atEm" => {
            Some(apple_libcxx::str_at as *const () as usize)
        }
        "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEEaSERKS5_" => {
            Some(apple_libcxx::str_assign_str as *const () as usize)
        }
        "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEEaSEc" => {
            Some(apple_libcxx::str_assign_char as *const () as usize)
        }
        "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE6assignEPKc" => {
            Some(apple_libcxx::str_assign_cstr as *const () as usize)
        }
        "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE6assignEPKcm" => {
            Some(apple_libcxx::str_assign_cstr_n as *const () as usize)
        }
        "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE6assignERKS5_mm" => {
            Some(apple_libcxx::str_assign_substr as *const () as usize)
        }
        "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE6assignEmc" => {
            Some(apple_libcxx::str_assign_count_char as *const () as usize)
        }
        "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE6appendEPKc" => {
            Some(apple_libcxx::str_append_cstr as *const () as usize)
        }
        "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE6appendEPKcm" => {
            Some(apple_libcxx::str_append_cstr_n as *const () as usize)
        }
        "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE6appendERKS5_mm" => {
            Some(apple_libcxx::str_append_substr as *const () as usize)
        }
        "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE6appendEmc" => {
            Some(apple_libcxx::str_append_count_char as *const () as usize)
        }
        "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE6insertEmPKc" => {
            Some(apple_libcxx::str_insert_cstr as *const () as usize)
        }
        "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE6insertEmPKcm" => {
            Some(apple_libcxx::str_insert_cstr_n as *const () as usize)
        }
        "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE21__grow_by_and_replaceEmmmmmmPKc" => {
            Some(apple_libcxx::str_grow_by_and_replace as *const () as usize)
        }
        "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE9push_backEc" => {
            Some(apple_libcxx::str_push_back as *const () as usize)
        }
        "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE5eraseEmm" => {
            Some(apple_libcxx::str_erase as *const () as usize)
        }
        "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE6resizeEmc" => {
            Some(apple_libcxx::str_resize as *const () as usize)
        }
        "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE7reserveEm" => {
            Some(apple_libcxx::str_reserve as *const () as usize)
        }
        "__ZNKSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE4findEcm" => {
            Some(apple_libcxx::str_find_char as *const () as usize)
        }
        "__ZNKSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE5rfindEcm" => {
            Some(apple_libcxx::str_rfind_char as *const () as usize)
        }
        "__ZNKSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE7compareEPKc" => {
            Some(apple_libcxx::str_compare_cstr as *const () as usize)
        }
        "__ZNKSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE7compareEmmPKc" => {
            Some(apple_libcxx::str_compare_pos_n_cstr as *const () as usize)
        }
        "__ZNKSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE7compareEmmPKcm" => {
            Some(apple_libcxx::str_compare_pos_n_cstr_len as *const () as usize)
        }
        "__ZNSt3__1plIcNS_11char_traitsIcEENS_9allocatorIcEEEENS_12basic_stringIT_T0_T1_EEPKS6_RKS9_" => {
            Some(apple_libcxx::str_plus_cstr_str as *const () as usize)
        }
        "__ZNSt3__111__call_onceERVmPvPFvS2_E" => Some(apple_libcxx::call_once_impl as *const () as usize),
        "__ZNSt3__15mutexC1Ev" => Some(apple_libcxx::mutex_ctor as *const () as usize),
        "__ZNSt3__15mutexD1Ev" => Some(apple_libcxx::mutex_dtor as *const () as usize),
        "__ZNSt3__15mutex4lockEv" => Some(apple_libcxx::mutex_lock as *const () as usize),
        "__ZNSt3__15mutex8try_lockEv" => Some(apple_libcxx::mutex_trylock as *const () as usize),
        "__ZNSt3__15mutex6unlockEv" => Some(apple_libcxx::mutex_unlock as *const () as usize),
        "__ZNSt3__115recursive_mutexC1Ev" => Some(apple_libcxx::rmutex_ctor as *const () as usize),
        "__ZNSt3__115recursive_mutexD1Ev" => Some(apple_libcxx::rmutex_dtor as *const () as usize),
        "__ZNSt3__115recursive_mutex4lockEv" => Some(apple_libcxx::rmutex_lock as *const () as usize),
        "__ZNSt3__115recursive_mutex6unlockEv" => Some(apple_libcxx::rmutex_unlock as *const () as usize),
        "__ZNSt3__118condition_variableD1Ev" => Some(apple_libcxx::cv_dtor as *const () as usize),
        "__ZNSt3__118condition_variable4waitERNS_11unique_lockINS_5mutexEEE" => {
            Some(apple_libcxx::cv_wait as *const () as usize)
        }
        "__ZNSt3__118condition_variable15__do_timed_waitERNS_11unique_lockINS_5mutexEEENS_6chrono10time_pointINS5_12system_clockENS5_8durationIxNS_5ratioILl1ELl1000000000EEEEEEE" => {
            Some(apple_libcxx::cv_timed_wait as *const () as usize)
        }
        "__ZNSt3__118condition_variable10notify_oneEv" => Some(apple_libcxx::cv_notify_one as *const () as usize),
        "__ZNSt3__118condition_variable10notify_allEv" => Some(apple_libcxx::cv_notify_all as *const () as usize),
        "__ZNSt3__111this_thread9sleep_forERKNS_6chrono8durationIxNS_5ratioILl1ELl1000000000EEEEE" => {
            Some(apple_libcxx::this_thread_sleep_for as *const () as usize)
        }
        "__ZNSt3__16chrono12steady_clock3nowEv" => Some(apple_libcxx::chrono_steady_now as *const () as usize),
        "__ZNSt3__16chrono12system_clock3nowEv" => Some(apple_libcxx::chrono_system_now as *const () as usize),
        "__ZNSt3__16chrono12system_clock9to_time_tERKNS0_10time_pointIS1_NS0_8durationIxNS_5ratioILl1ELl1000000EEEEEEE" => {
            Some(apple_libcxx::chrono_to_time_t as *const () as usize)
        }
        "__ZNSt3__16chrono12system_clock11from_time_tEl" => {
            Some(apple_libcxx::chrono_from_time_t as *const () as usize)
        }
        "__ZNSt3__112__get_sp_mutEPKv" => Some(apple_libcxx::get_sp_mut as *const () as usize),
        "__ZNSt3__18__sp_mut4lockEv" => Some(apple_libcxx::sp_mut_lock as *const () as usize),
        "__ZNSt3__18__sp_mut6unlockEv" => Some(apple_libcxx::sp_mut_unlock as *const () as usize),
        "__ZNSt3__114__shared_countD2Ev"
        | "__ZNSt3__119__shared_weak_countD2Ev" => Some(apple_libcxx::shared_count_dtor as *const () as usize),
        "__ZNSt3__119__shared_weak_count14__release_weakEv" => {
            Some(apple_libcxx::shared_weak_release_weak as *const () as usize)
        }
        "__ZNSt3__119__shared_weak_count4lockEv" => Some(apple_libcxx::shared_weak_lock as *const () as usize),
        "__ZNKSt3__119__shared_weak_count13__get_deleterERKSt9type_info" => {
            Some(apple_libcxx::shared_weak_get_deleter as *const () as usize)
        }
        "__ZNSt3__112__next_primeEm" => Some(apple_libcxx::next_prime as *const () as usize),
        "__ZNSt3__16__sortIRNS_6__lessIiiEEPiEEvT0_S5_T_" => Some(apple_libcxx::sort_ints as *const () as usize),
        "__ZNSt3__16__sortIRNS_6__lessIjjEEPjEEvT0_S5_T_" => Some(apple_libcxx::sort_u32 as *const () as usize),
        "__ZNSt3__16__sortIRNS_6__lessImmEEPmEEvT0_S5_T_" => Some(apple_libcxx::sort_u64 as *const () as usize),
        "__ZNSt3__113random_deviceC1ERKNS_12basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEEE" => {
            Some(apple_libcxx::random_device_ctor as *const () as usize)
        }
        "__ZNSt3__113random_deviceD1Ev" => Some(apple_libcxx::random_device_dtor as *const () as usize),
        "__ZNSt3__113random_deviceclEv" => Some(apple_libcxx::random_device_call as *const () as usize),
        "__ZNSt3__18__rs_getEv" => Some(apple_libcxx::rs_get as *const () as usize),
        "__ZNSt3__112__rs_defaultclEv" => Some(apple_libcxx::rs_default_call as *const () as usize),
        "__ZNSt3__112__rs_defaultD1Ev" => Some(apple_libcxx::rs_default_dtor as *const () as usize),
        "__ZNKSt3__120__vector_base_commonILb1EE20__throw_length_errorEv"
        | "__ZNKSt3__120__vector_base_commonILb1EE20__throw_out_of_rangeEv"
        | "__ZNKSt3__121__basic_string_commonILb1EE20__throw_length_errorEv"
        | "__ZNSt20bad_array_new_lengthC1Ev" => Some(apple_libcxx::throw_abort as *const () as usize),
        _ => dlopen_dlsym(&[c"libc++.so.1", c"libc++abi.so.1", c"libstdc++.so.6"], symbol),
    }
}

/// Unified symbol resolver for `pear-dyld`: resolves `libobjc`, `libSystem`
/// (libclosure + libc basics), `CoreFoundation`, and `Foundation` imports.
pub fn resolve_symbol(library: Option<&str>, symbol: &str) -> Option<usize> {
    init();
    let resolved = match library {
        Some("/usr/lib/libobjc.A.dylib") => pear_libobjc::symbol(symbol),
        Some(path) if path.contains("libswift") || path.contains("SwiftUI") => {
            pear_swift_rt::symbol(Some(path), symbol).or_else(|| libc_dlsym(symbol))
        }
        Some(path) if path.contains("libc++") => libcxx_symbol(symbol),
        Some(path) if path.contains("libSystem") || path.contains("libdispatch") => match symbol {
            "__Unwind_RaiseException" => Some(unwind::unwind_raise_exception as *const () as usize),
            "__Unwind_Resume" => Some(unwind::unwind_resume as *const () as usize),
            "__Unwind_DeleteException" => Some(unwind::unwind_delete_exception as *const () as usize),
            "__Unwind_GetIP" => Some(unwind::unwind_get_ip as *const () as usize),
            "__Unwind_GetRegionStart" => Some(unwind::unwind_get_region_start as *const () as usize),
            "__Unwind_GetLanguageSpecificData" => Some(unwind::unwind_get_lsda as *const () as usize),
            "__Unwind_SetGR" => Some(unwind::unwind_set_gr as *const () as usize),
            "__Unwind_SetIP" => Some(unwind::unwind_set_ip as *const () as usize),
            "___stack_chk_guard" => Some(&raw const STACK_CHK_GUARD as usize),
            "___stack_chk_fail" => Some(stack_chk_fail_trap as *const () as usize),
            "___error" => Some(tls_errno_ptr as *const () as usize),
            "___memcpy_chk" => Some(memcpy_chk as *const () as usize),
            "___memset_chk" => Some(memset_chk as *const () as usize),
            "_strlcpy" => Some(strlcpy_impl as *const () as usize),
            "___strlcpy_chk" => Some(strlcpy_chk as *const () as usize),
            "_strlcat" => Some(strlcat_impl as *const () as usize),
            "___strlcat_chk" => Some(strlcat_chk as *const () as usize),
            "_realpath$DARWIN_EXTSN" => libc_dlsym("_realpath"),
            "_dlopen" => Some(guest_dlopen as *const () as usize),
            "_dlsym" => Some(guest_dlsym as *const () as usize),
            "_dlclose" => Some(guest_dlclose as *const () as usize),
            "_dladdr" => Some(guest_dladdr as *const () as usize),
            "__tlv_atexit" => Some(dispatch_noop_2 as *const () as usize),
            "___sincosf_stret" => Some(sincosf_stret as *const () as usize),
            "___sincos_stret" => Some(sincos_stret as *const () as usize),
            "___exp10" => Some(exp10_impl as *const () as usize),
            "_acos" => Some(acos_impl as *const () as usize),
            "_acosf" => Some(acosf_impl as *const () as usize),
            "_asinf" => Some(asinf_impl as *const () as usize),
            "_atan2f" => Some(atan2f_impl as *const () as usize),
            "___tolower" => Some(tolower_impl as *const () as usize),
            "___toupper" => Some(toupper_impl as *const () as usize),
            "_OSAtomicAdd32" => Some(os_atomic_add32 as *const () as usize),
            "_arc4random" => Some(arc4random_impl as *const () as usize),
            "__availability_version_check" => Some(availability_version_check as *const () as usize),
            "___cxa_atexit" => Some(cxa_atexit_noop as *const () as usize),
            "__dyld_image_count" => Some(dyld_image_count_stub as *const () as usize),
            "__dyld_get_image_header" => Some(dyld_get_image_header_stub as *const () as usize),
            "__dyld_get_image_vmaddr_slide" => Some(dyld_get_image_vmaddr_slide_stub as *const () as usize),
            "__dyld_get_image_name" => Some(dyld_get_image_name_stub as *const () as usize),
            "__dispatch_main_q" => Some(&raw const DISPATCH_MAIN_Q as usize),
            "__dispatch_queue_attr_concurrent"
            | "__dispatch_source_type_timer" => Some(&raw const DISPATCH_QUEUE_ATTR_STUB as usize),
            "_OBJC_CLASS_$_OS_dispatch_queue"
            | "_OBJC_CLASS_$_OS_dispatch_source"
            | "_OBJC_CLASS_$_OS_os_log" => Some(UIRESPONDER_CLASS.ptr() as usize),
            "_dispatch_once" => Some(dispatch_once_impl as *const () as usize),
            "_dispatch_once_f" => Some(dispatch_once_f_impl as *const () as usize),
            "_dispatch_sync"
            | "_dispatch_async"
            | "_dispatch_barrier_sync"
            | "_dispatch_barrier_async" => Some(dispatch_sync_impl as *const () as usize),
            "_dispatch_get_global_queue" => Some(dispatch_get_global_queue_impl as *const () as usize),
            "_dispatch_queue_create" => Some(dispatch_queue_create_impl as *const () as usize),
            "_dispatch_queue_attr_make_with_qos_class" => Some(dispatch_queue_attr_qos_impl as *const () as usize),
            "_dispatch_queue_get_label" => Some(dispatch_queue_get_label_impl as *const () as usize),
            "_dispatch_group_create" => Some(dispatch_group_create_impl as *const () as usize),
            "_dispatch_group_enter" => Some(dispatch_group_enter_impl as *const () as usize),
            "_dispatch_group_leave" => Some(dispatch_group_leave_impl as *const () as usize),
            "_dispatch_resume"
            | "_dispatch_block_cancel"
            | "_dispatch_source_cancel"
            | "_dispatch_assert_queue$V2" => Some(dispatch_noop_1 as *const () as usize),
            "_dispatch_release"
            | "_os_release" => Some(dispatch_release_impl as *const () as usize),
            "_dispatch_set_target_queue"
            | "_dispatch_source_set_event_handler" => Some(dispatch_noop_2 as *const () as usize),
            "_dispatch_source_set_timer" => Some(status_ok_stub as *const () as usize),
            "_dispatch_source_create" => Some(dispatch_source_create_impl as *const () as usize),
            "_dispatch_source_testcancel" => Some(dispatch_source_testcancel_impl as *const () as usize),
            "_dispatch_group_async"
            | "_dispatch_group_notify" => Some(dispatch_group_async_impl as *const () as usize),
            "_dispatch_group_wait"
            | "_dispatch_block_wait" => Some(dispatch_group_wait_impl as *const () as usize),
            "_dispatch_after" => Some(dispatch_after_impl as *const () as usize),
            "_dispatch_time" => Some(dispatch_time_impl as *const () as usize),
            "_dispatch_walltime" => Some(dispatch_walltime_impl as *const () as usize),
            "_dispatch_semaphore_create" => Some(dispatch_semaphore_create_impl as *const () as usize),
            "_dispatch_semaphore_signal" => Some(dispatch_semaphore_signal_impl as *const () as usize),
            "_dispatch_semaphore_wait" => Some(dispatch_semaphore_wait_impl as *const () as usize),
            "_dispatch_block_create" => Some(dispatch_block_create_impl as *const () as usize),
            "__os_log_default" => Some(&raw const OS_LOG_STUB as usize),
            "_os_log_create" => Some(os_log_create_impl as *const () as usize),
            "_os_log_type_enabled" => Some(os_log_type_enabled_impl as *const () as usize),
            "__os_log_impl" => Some(status_ok_stub as *const () as usize),
            "_os_unfair_lock_lock" => Some(os_unfair_lock_lock as *const () as usize),
            "_os_unfair_lock_lock_with_options" => Some(os_unfair_lock_lock_with_options as *const () as usize),
            "_os_unfair_lock_trylock" => Some(os_unfair_lock_trylock as *const () as usize),
            "_os_unfair_lock_unlock" => Some(os_unfair_lock_unlock as *const () as usize),
            "_asl_open" => Some(asl_open_stub as *const () as usize),
            "_asl_log"
            | "_asl_add_log_file"
            | "_asl_set_filter" => Some(asl_ok_stub as *const () as usize),
            "_mach_absolute_time" => Some(mach_absolute_time_impl as *const () as usize),
            "_mach_timebase_info" => Some(mach_timebase_info_impl as *const () as usize),
            "_mach_task_self_" => {
                #[cfg(target_os = "macos")]
                { libc_dlsym(symbol).or(Some(&raw const MACH_TASK_SELF_STUB as usize)) }
                #[cfg(not(target_os = "macos"))]
                { Some(&raw const MACH_TASK_SELF_STUB as usize) }
            }
            "_mach_host_self" => {
                #[cfg(target_os = "macos")]
                { libc_dlsym(symbol).or(Some(mach_host_self_impl as *const () as usize)) }
                #[cfg(not(target_os = "macos"))]
                { Some(mach_host_self_impl as *const () as usize) }
            }
            "_host_info" | "_host_statistics64" => {
                #[cfg(target_os = "macos")]
                { libc_dlsym(symbol).or(Some(host_info_stub as *const () as usize)) }
                #[cfg(not(target_os = "macos"))]
                { Some(host_info_stub as *const () as usize) }
            }
            "_mach_error_string" => Some(mach_error_string_stub as *const () as usize),
            "_mach_port_allocate"
            | "_mach_port_construct"
            | "_mach_port_deallocate"
            | "_mach_port_mod_refs"
            | "_mach_port_peek"
            | "_mach_msg"
            | "_mach_msg_send"
            | "_mach_msg_destroy" => {
                #[cfg(target_os = "macos")]
                { libc_dlsym(symbol).or(Some(status_ok_stub as *const () as usize)) }
                #[cfg(not(target_os = "macos"))]
                {
                    match symbol {
                        "_mach_port_allocate" => Some(frameworks::linux_mach_kqueue::mach_port_allocate as *const () as usize),
                        "_mach_port_construct" => Some(frameworks::linux_mach_kqueue::mach_port_construct as *const () as usize),
                        "_mach_port_deallocate" => Some(frameworks::linux_mach_kqueue::mach_port_deallocate as *const () as usize),
                        "_mach_port_mod_refs" => Some(frameworks::linux_mach_kqueue::mach_port_mod_refs as *const () as usize),
                        "_mach_port_peek" => Some(frameworks::linux_mach_kqueue::mach_port_peek as *const () as usize),
                        "_mach_msg" | "_mach_msg_send" => Some(frameworks::linux_mach_kqueue::mach_msg as *const () as usize),
                        _ => Some(status_ok_stub as *const () as usize),
                    }
                }
            }
            "_kqueue" | "_kevent64" => {
                #[cfg(target_os = "macos")]
                { libc_dlsym(symbol) }
                #[cfg(not(target_os = "macos"))]
                {
                    match symbol {
                        "_kqueue" => Some(frameworks::linux_mach_kqueue::kqueue_impl as *const () as usize),
                        "_kevent64" => Some(frameworks::linux_mach_kqueue::kevent64_impl as *const () as usize),
                        _ => None,
                    }
                }
            }
            "_thread_switch" => Some(status_ok_stub as *const () as usize),
            "_vm_page_size" => Some(&raw const VM_PAGE_SIZE_VAL as usize),
            #[cfg(not(target_os = "macos"))]
            "_mmap" => Some(darwin_mmap_impl as *const () as usize),
            #[cfg(not(target_os = "macos"))]
            "_sysconf" => Some(darwin_sysconf_impl as *const () as usize),
            "_malloc_default_zone" => Some(malloc_default_zone_impl as *const () as usize),
            "_malloc_create_zone" => Some(malloc_create_zone_impl as *const () as usize),
            "_malloc_set_zone_name" => Some(dispatch_noop_2 as *const () as usize),
            "_malloc_zone_malloc" => Some(malloc_zone_malloc_impl as *const () as usize),
            "_malloc_zone_calloc" => Some(malloc_zone_calloc_impl as *const () as usize),
            "_malloc_zone_realloc" => Some(malloc_zone_realloc_impl as *const () as usize),
            "_malloc_zone_free" => Some(malloc_zone_free_impl as *const () as usize),
            "_malloc_size" => Some(malloc_size_impl as *const () as usize),
            "_memset_pattern16" => Some(memset_pattern16_impl as *const () as usize),
            "_pthread_mach_thread_np" => Some(pthread_mach_thread_np_impl as *const () as usize),
            "_pthread_mutexattr_init" => Some(darwin_pthread::mutexattr_init as *const () as usize),
            "_pthread_mutexattr_destroy" => Some(darwin_pthread::mutexattr_destroy as *const () as usize),
            "_pthread_mutexattr_settype" => Some(darwin_pthread::mutexattr_settype as *const () as usize),
            "_pthread_mutexattr_setprotocol"
            | "_pthread_attr_setstacksize"
            | "_pthread_set_qos_class_self_np"
            | "_pthread_setname_np" => Some(status_ok_stub as *const () as usize),
            "_pthread_threadid_np" => Some(darwin_pthread::threadid_np as *const () as usize),
            "_pthread_getname_np" => Some(darwin_pthread::getname_np as *const () as usize),
            "_pthread_mutex_init" => Some(darwin_pthread::mutex_init as *const () as usize),
            "_pthread_mutex_destroy" => Some(darwin_pthread::mutex_destroy as *const () as usize),
            "_pthread_mutex_lock" => Some(darwin_pthread::mutex_lock as *const () as usize),
            "_pthread_mutex_trylock" => Some(darwin_pthread::mutex_trylock as *const () as usize),
            "_pthread_mutex_unlock" => Some(darwin_pthread::mutex_unlock as *const () as usize),
            "_pthread_cond_init" => Some(darwin_pthread::cond_init as *const () as usize),
            "_pthread_cond_destroy" => Some(darwin_pthread::cond_destroy as *const () as usize),
            "_pthread_cond_signal" => Some(darwin_pthread::cond_signal as *const () as usize),
            "_pthread_cond_broadcast" => Some(darwin_pthread::cond_broadcast as *const () as usize),
            "_pthread_cond_wait" => Some(darwin_pthread::cond_wait as *const () as usize),
            "_pthread_cond_timedwait" => Some(darwin_pthread::cond_timedwait as *const () as usize),
            "_pthread_cond_timedwait_relative_np" => Some(darwin_pthread::cond_timedwait_relative_np as *const () as usize),
            "_pthread_attr_init" => Some(darwin_pthread::attr_init as *const () as usize),
            "_pthread_attr_destroy" => Some(darwin_pthread::attr_destroy as *const () as usize),
            "_pthread_attr_setdetachstate" => Some(darwin_pthread::attr_setdetachstate as *const () as usize),
            "_pthread_create" => Some(darwin_pthread::thread_create as *const () as usize),
            "_sysctl" => Some(sysctl_impl as *const () as usize),
            "_sysctlbyname" => Some(sysctlbyname_impl as *const () as usize),
            "_task_info" => Some(task_info_impl as *const () as usize),
            "_thread_policy_set" => Some(thread_policy_set_impl as *const () as usize),
            "__DefaultRuneLocale" => Some(&raw const DEFAULT_RUNE_LOCALE as usize),
            "___maskrune" => Some(maskrune_impl as *const () as usize),
            "___maskrune_l" => Some(maskrune_l_impl as *const () as usize),
            "____mb_cur_max_l" => Some(mb_cur_max_l_impl as *const () as usize),
            "_srandomdev" => Some(srandomdev_impl as *const () as usize),
            "___stdinp" => Some(&raw const STDIN_STUB as usize),
            "___stdoutp" => Some(&raw const STDOUT_STUB as usize),
            "___stderrp" => Some(&raw const STDERR_STUB as usize),
            "__NSGetExecutablePath" => Some(ns_get_executable_path as *const () as usize),
            "_malloc" => Some(malloc as *const () as usize),
            "_calloc" => Some(calloc as *const () as usize),
            "_realloc" => Some(realloc as *const () as usize),
            "_free" => Some(free as *const () as usize),
            "_memcpy" => Some(memcpy as *const () as usize),
            "_memset" => Some(memset as *const () as usize),
            "_strlen" => Some(strlen as *const () as usize),
            "_strcmp" => Some(strcmp as *const () as usize),
            "_getenv" => Some(getenv as *const () as usize),
            "_abort" => Some(guest_abort as *const () as usize),
            "_vsnprintf" => Some(darwin_varargs::darwin_vsnprintf as *const () as usize),
            "_vsprintf" => Some(darwin_varargs::darwin_vsprintf as *const () as usize),
            "_vprintf" => Some(darwin_varargs::darwin_vprintf as *const () as usize),
            "_vfprintf" => Some(darwin_varargs::darwin_vfprintf as *const () as usize),
            "___vsnprintf_chk" => Some(darwin_varargs::darwin_vsnprintf_chk as *const () as usize),
            #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
            "_snprintf" => Some(darwin_varargs::darwin_snprintf as *const () as usize),
            #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
            "_sprintf" => Some(darwin_varargs::darwin_sprintf as *const () as usize),
            #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
            "___sprintf_chk" => Some(darwin_varargs::darwin_sprintf_chk as *const () as usize),
            #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
            "___snprintf_chk" => Some(darwin_varargs::darwin_snprintf_chk as *const () as usize),
            #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
            "_printf" => Some(darwin_varargs::darwin_printf as *const () as usize),
            #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
            "_fprintf" => Some(darwin_varargs::darwin_fprintf as *const () as usize),
            #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
            "_sscanf" => Some(darwin_varargs::darwin_sscanf as *const () as usize),
            #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
            "_fscanf" => Some(darwin_varargs::darwin_fscanf as *const () as usize),
            _ => pear_libobjc::symbol(symbol).or_else(|| libc_dlsym(symbol)),
        },
        Some(path) if path.contains("AVFAudio") || path.contains("AVFoundation") => match symbol {
            "_OBJC_CLASS_$_AVAudioSession" => Some(AVAUDIOSESSION_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_AVAudioPlayer" => Some(AVAUDIOPLAYER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_AVPlayer" => Some(AVPLAYER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_AVPlayerItem" => Some(AVPLAYERITEM_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_AVURLAsset" => Some(AVURLASSET_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_AVAssetReader" => Some(AVASSETREADER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_AVAssetReaderTrackOutput" => Some(AVASSETREADERTRACKOUTPUT_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_AVPlayerItemLegibleOutput" => Some(AVPLAYERITEMLEGIBLEOUTPUT_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_AVPlayerItemVideoOutput" => Some(AVPLAYERITEMVIDEOOUTPUT_CLASS.ptr() as usize),
            s if s.starts_with("_AV") => Some(framework_const(Box::leak(s.to_string().into_boxed_str()))),
            _ => None,
        },
        Some(path) if path.contains("CloudKit") => match symbol {
            "_OBJC_CLASS_$_CKContainer" => Some(CKCONTAINER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_CKDatabase" => Some(CKDATABASE_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_CKModifyRecordsOperation" => Some(CKMODIFYRECORDSOPERATION_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_CKQuery" => Some(CKQUERY_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_CKQueryOperation" => Some(CKQUERYOPERATION_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_CKRecord" => Some(CKRECORD_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_CKRecordID" => Some(CKRECORDID_CLASS.ptr() as usize),
            "_CKQueryOperationMaximumResults" => Some(framework_const("CKQueryOperationMaximumResults")),
            _ => None,
        },
        Some(path) if path.contains("CoreFoundation") || path.contains("Foundation") => {
            pear_foundation::symbol(symbol)
        }
        Some(path) if path.contains("CoreGraphics") => pear_graphics::symbol(symbol),
        Some(path) if path.contains("QuartzCore") => match symbol {
            "_CACurrentMediaTime" => Some(ca_current_media_time as *const () as usize),
            "_OBJC_CLASS_$_CALayer" => Some(CALAYER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_CAMetalLayer" => Some(CAMETALLAYER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_CAEAGLLayer" => Some(CAEAGLLAYER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_CATransaction" => Some(CATRANSACTION_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_CADisplayLink" => Some(CADISPLAYLINK_CLASS.ptr() as usize),
            "_kCAFilterLinear"
            | "_kCAGravityResize"
            | "_kCAMediaTimingFunctionEaseInEaseOut"
            | "_kCAMediaTimingFunctionLinear"
            | "_kCAMediaTimingFunctionEaseOut"
            | "_kCAFillModeForwards" => Some(quartz_const(symbol)),
            _ => pear_graphics::quartz_symbol(symbol),
        },
        Some(path) if path.contains("UIKit") => match symbol {
            "_OBJC_CLASS_$_UIApplication" => Some(UIAPPLICATION_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIWindow" => Some(UIWINDOW_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIView" => Some(UIVIEW_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIScreen" => Some(UISCREEN_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIColor" => Some(UICOLOR_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIImage" => Some(UIIMAGE_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UILabel" => Some(UILABEL_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIFont" => Some(UIFONT_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIDevice" => Some(UIDEVICE_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIResponder" => Some(UIRESPONDER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIViewController" => Some(UIVIEWCONTROLLER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIGestureRecognizer" => Some(UIGESTURERECOGNIZER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UILongPressGestureRecognizer" => Some(UILONGPRESSGESTURERECOGNIZER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UITapGestureRecognizer" => Some(UITAPGESTURERECOGNIZER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIPanGestureRecognizer" => Some(UIPANGESTURERECOGNIZER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIStoryboard" => Some(UISTORYBOARD_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIControl" => Some(UICONTROL_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIButton" => Some(UIBUTTON_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIDocument" => Some(UIDOCUMENT_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIAccessibilityElement" => Some(UIACCESSIBILITYELEMENT_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UITextPosition" => Some(UITEXTPOSITION_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UITextRange" => Some(UITEXTRANGE_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UITextField" => Some(UITEXTFIELD_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UITextView" => Some(UITEXTVIEW_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIScrollView" => Some(UISCROLLVIEW_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UINavigationController" => Some(UINAVIGATIONCONTROLLER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIAlertController" => Some(UIALERTCONTROLLER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIAccelerometer" => Some(UIACCELEROMETER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIActivityViewController" => Some(UIACTIVITYVIEWCONTROLLER_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIAlertView" => Some(UIALERTVIEW_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIPasteboard" => Some(UIPASTEBOARD_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIWindowScene" => Some(UIWINDOWSCENE_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UISceneSession" => Some(UISCENESESSION_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UISceneConnectionOptions" => Some(UISCENECONNECTIONOPTIONS_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UITouch" => Some(UITOUCH_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIEvent" => Some(UIEVENT_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIViewLayoutRegion" => Some(UIVIEWLAYOUTREGION_CLASS.ptr() as usize),
            "_NSStringFromCGRect" => Some(ns_string_from_cgrect as *const () as usize),
            "_NSStringFromCGSize" => Some(ns_string_from_cgsize as *const () as usize),
            "_NSStringFromCGPoint" => Some(ns_string_from_cgpoint as *const () as usize),
            "_OBJC_METACLASS_$_UIDocument" => Some(UIDOCUMENT_META.ptr() as usize),
            "_OBJC_METACLASS_$_UIView" => Some(UIVIEW_META.ptr() as usize),
            "_OBJC_METACLASS_$_UIViewController" => Some(UIVIEWCONTROLLER_META.ptr() as usize),
            "_UIApplicationMain" => Some(ui_application_main as *const () as usize),
            s if frameworks::uikit_ext_symbol(s).is_some() => frameworks::uikit_ext_symbol(s),
            s if s.starts_with("_UI") => Some(framework_const(Box::leak(s.to_string().into_boxed_str()))),
            _ => None,
        },
        Some(path) if path.contains("StoreKit") => match symbol {
            "_OBJC_CLASS_$_SKRequest" => Some(SKREQUEST_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_SKProductsRequest" => Some(SKPRODUCTSREQUEST_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_SKPayment" => Some(SKPAYMENT_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_SKPaymentQueue" => Some(SKPAYMENTQUEUE_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_SKStoreReviewController" => Some(SKSTOREREVIEWCONTROLLER_CLASS.ptr() as usize),
            "_SKErrorDomain" => Some(framework_const("SKErrorDomain")),
            _ => frameworks::misc_framework_symbol(path, symbol),
        },
        Some(path) if path.contains("AudioToolbox") => {
            audiotoolbox_symbol(symbol).or_else(|| frameworks::audiotoolbox_ext_symbol(symbol))
        }
        Some(path) if path.contains("CFNetwork") => cfnetwork_symbol(symbol)
            .or_else(|| frameworks::cfnetwork_ext_symbol(symbol))
            .or_else(|| pear_foundation::symbol(symbol)),
        Some(path) if path.contains("Network") => frameworks::network_symbol(symbol),
        Some(path) if path.contains("Security") => security::symbol(symbol),
        Some(path) if path.contains("Accelerate") => accelerate::symbol(symbol),
        Some(path) if path.contains("ImageIO") => pear_graphics::imageio_symbol(symbol),
        Some(path) if path.contains("CoreText") => pear_graphics::coretext_symbol(symbol),
        Some(path) if path.contains("CoreServices") => frameworks::coreservices_symbol(symbol),
        Some(path) if path.contains("VideoToolbox") => frameworks::videotoolbox_symbol(symbol),
        Some(path) if path.contains("JavaScriptCore") => frameworks::jsc_symbol(symbol),
        Some(path) if path.contains("SystemConfiguration") => match symbol {
            "_SCNetworkReachabilityCreateWithName"
            | "_SCNetworkReachabilityCreateWithAddress" => Some(ptr_null_stub as *const () as usize),
            "_SCNetworkReachabilityGetFlags"
            | "_SCNetworkReachabilitySetCallback"
            | "_SCNetworkReachabilityScheduleWithRunLoop"
            | "_SCNetworkReachabilityUnscheduleFromRunLoop"
            | "_SCNetworkReachabilitySetDispatchQueue" => Some(false_stub as *const () as usize),
            _ => frameworks::misc_framework_symbol(path, symbol),
        },
        Some(path) if path.contains("GLKit") => match symbol {
            "_OBJC_CLASS_$_GLKView" => Some(GLKVIEW_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_GLKViewController" => Some(GLKVIEWCONTROLLER_CLASS.ptr() as usize),
            _ => None,
        },
        Some(path) if path.contains("MetalKit") => match symbol {
            "_OBJC_CLASS_$_MTKView" => Some(MTKVIEW_CLASS.ptr() as usize),
            _ => pear_metal_vk::symbol(symbol),
        },
        Some(path) if path.contains("libz") => dlopen_dlsym(&[c"libz.so.1", c"libz.1.dylib"], symbol),
        Some(path) if path.contains("libsqlite3") => dlopen_dlsym(&[c"libsqlite3.so.0", c"libsqlite3.dylib"], symbol),
        Some(path) if path.contains("libxml2") => dlopen_dlsym(&[c"libxml2.so.2", c"libxml2.2.dylib"], symbol),
        Some(path) if path.contains("libiconv") => dlopen_dlsym(&[c"libiconv.so.2", c"libiconv.2.dylib", c"libc.so.6"], symbol),
        Some(path) if path.contains("libbz2") => dlopen_dlsym(&[c"libbz2.so.1.0", c"libbz2.so.1", c"libbz2.1.0.dylib"], symbol),
        Some(path) if path.contains("liblzma") => dlopen_dlsym(&[c"liblzma.so.5", c"liblzma.5.dylib"], symbol),
        Some(path) if path.contains("libresolv") => match symbol {
            "_res_9_init" | "_res_9_ninit" | "_res_9_nclose" | "_res_9_ndestroy" | "_res_9_getservers" => {
                Some(resolv_zero_stub as *const () as usize)
            }
            _ => None,
        },
        Some(path) if path.contains("Metal") && !path.contains("MetalKit") => pear_metal_vk::symbol(symbol),
        Some(path) if path.contains("GameController") => match symbol {
            "_OBJC_CLASS_$_GCController" => Some(GCCONTROLLER_CLASS.ptr() as usize),
            "_GCControllerDidConnectNotification" => Some(framework_const("GCControllerDidConnectNotification")),
            "_GCControllerDidDisconnectNotification" => Some(framework_const("GCControllerDidDisconnectNotification")),
            _ => None,
        },
        Some(path) if path.contains("CoreMedia") => frameworks::coremedia_symbol(symbol),
        Some(path) if path.contains("CoreVideo") => frameworks::corevideo_symbol(symbol),
        Some(path)
            if path.contains("CoreTelephony")
                || path.contains("MediaPlayer")
                || path.contains("CoreHaptics")
                || path.contains("AuthenticationServices")
                || path.contains("CoreImage")
                || path.contains("Photos")
                || path.contains("GameKit")
                || path.contains("WebKit") =>
        {
            frameworks::misc_framework_symbol(path, symbol)
        }
        Some(path) if path.contains("OpenGLES") => match symbol {
            "_OBJC_CLASS_$_EAGLContext" => Some(EAGLCONTEXT_CLASS.ptr() as usize),
            "_kEAGLColorFormatRGBA8" => Some(framework_const("kEAGLColorFormatRGBA8")),
            "_kEAGLDrawablePropertyColorFormat" => Some(framework_const("kEAGLDrawablePropertyColorFormat")),
            "_kEAGLDrawablePropertyRetainedBacking" => Some(framework_const("kEAGLDrawablePropertyRetainedBacking")),
            s if s.starts_with("_gl") => Some(gles_dlsym(s)),
            _ => None,
        },
        None => pear_libobjc::symbol(symbol)
            .or_else(|| pear_foundation::symbol(symbol))
            .or_else(|| pear_graphics::symbol(symbol))
            .or_else(|| pear_swift_rt::symbol(None, symbol))
            .or_else(|| libcxx_symbol(symbol))
            .or_else(|| resolve_symbol(Some("/usr/lib/libSystem.B.dylib"), symbol))
            .or_else(|| libc_dlsym(symbol)),
        _ => None,
    };
    if resolved.is_some() {
        return resolved;
    }
    static SWIFT_FORCE_LOAD_ANCHOR: usize = 0;
    if symbol.starts_with("__swift_FORCE_LOAD_$_") {
        return Some(&raw const SWIFT_FORCE_LOAD_ANCHOR as usize);
    }
    if symbol.starts_with("_$s") || symbol.starts_with("_$S") {
        return pear_swift_rt::overlays::resolve_mangled_symbol(library, symbol);
    }
    if let Some(cls_name) = symbol.strip_prefix("_OBJC_CLASS_$_") {
        if let Some(cls) = objc_rt::class_named(cls_name) {
            return Some(cls as usize);
        }
    } else if let Some(meta_name) = symbol.strip_prefix("_OBJC_METACLASS_$_") {
        if let Some(meta) = objc_rt::metaclass_named(meta_name) {
            return Some(meta as usize);
        }
    }
    None
}

/// Portable EGL + OpenGL ES 2.0 surface bridge (`libEGL.so.1` + `libGLESv2.so.2`)
/// for Linux/X11 windows (including Mesa `llvmpipe` inside headless/VNC containers).
pub mod egl_bridge {
    use std::ffi::{c_char, c_float, c_int, c_uint, c_void};

    const EGL_OPENGL_ES_API: c_uint = 0x30A0;
    const EGL_SURFACE_TYPE: c_int = 0x3033;
    const EGL_WINDOW_BIT: c_int = 0x0004;
    const EGL_RENDERABLE_TYPE: c_int = 0x3040;
    const EGL_OPENGL_ES_BIT: c_int = 0x0001;
    const EGL_OPENGL_ES2_BIT: c_int = 0x0004;
    const EGL_RED_SIZE: c_int = 0x3024;
    const EGL_GREEN_SIZE: c_int = 0x3023;
    const EGL_BLUE_SIZE: c_int = 0x3022;
    const EGL_ALPHA_SIZE: c_int = 0x3021;
    const EGL_CONTEXT_CLIENT_VERSION: c_int = 0x3098;
    const EGL_NONE: c_int = 0x3038;

    const GL_COLOR_BUFFER_BIT: c_uint = 0x0000_4000;
    const GL_TRIANGLES: c_uint = 0x0004;
    const GL_FLOAT: c_uint = 0x1406;
    const GL_FRAGMENT_SHADER: c_uint = 0x8B30;
    const GL_VERTEX_SHADER: c_uint = 0x8B31;

    pub struct EglSurfaceCtx {
        dpy: *mut c_void,
        surf: *mut c_void,
        ctx: *mut c_void,
        program: c_uint,
        u_angle: c_int,
        egl_swap_buffers: unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_uint,
        gl_viewport: unsafe extern "C" fn(c_int, c_int, c_int, c_int),
        gl_clear_color: unsafe extern "C" fn(c_float, c_float, c_float, c_float),
        gl_clear: unsafe extern "C" fn(c_uint),
        gl_use_program: unsafe extern "C" fn(c_uint),
        gl_uniform1f: unsafe extern "C" fn(c_int, c_float),
        gl_vertex_attrib_pointer: unsafe extern "C" fn(c_uint, c_int, c_uint, u8, c_int, *const c_void),
        gl_enable_vertex_attrib_array: unsafe extern "C" fn(c_uint),
        gl_draw_arrays: unsafe extern "C" fn(c_uint, c_int, c_int),
    }

    unsafe impl Send for EglSurfaceCtx {}
    unsafe impl Sync for EglSurfaceCtx {}

    impl EglSurfaceCtx {
        pub fn create_from_x11(x11_display: *mut c_void, x11_window: u64) -> Option<Self> {
            Self::create_for_api(x11_display, x11_window, 2)
        }

        pub fn create_for_api(x11_display: *mut c_void, x11_window: u64, api: usize) -> Option<Self> {
            unsafe {
                let mut egl_lib = super::dlopen(c"libEGL.so.1".as_ptr(), 1);
                if egl_lib.is_null() {
                    egl_lib = super::dlopen(c"libEGL.so".as_ptr(), 1);
                }
                let mut gles_lib = if api <= 1 {
                    let p = super::dlopen(c"libGLESv1_CM.so.1".as_ptr(), 1);
                    if p.is_null() { super::dlopen(c"libGLESv1_CM.so".as_ptr(), 1) } else { p }
                } else {
                    let p = super::dlopen(c"libGLESv2.so.2".as_ptr(), 1);
                    if p.is_null() { super::dlopen(c"libGLESv2.so".as_ptr(), 1) } else { p }
                };
                if gles_lib.is_null() {
                    gles_lib = super::dlopen(c"libGLESv2.so.2".as_ptr(), 1);
                }
                if egl_lib.is_null() || gles_lib.is_null() {
                    eprintln!("[pear-egl] failed to dlopen libEGL / libGLES");
                    return None;
                }

                let get_proc: unsafe extern "C" fn(*const c_char) -> *mut c_void =
                    std::mem::transmute(super::dlsym(egl_lib, c"eglGetProcAddress".as_ptr()));
                let load_egl = |name: &std::ffi::CStr| -> *mut c_void {
                    let p = super::dlsym(egl_lib, name.as_ptr());
                    if !p.is_null() { p } else { get_proc(name.as_ptr()) }
                };
                let load_gl = |name: &std::ffi::CStr| -> *mut c_void {
                    let p = super::dlsym(gles_lib, name.as_ptr());
                    if !p.is_null() { p } else { get_proc(name.as_ptr()) }
                };

                let egl_get_display: unsafe extern "C" fn(*mut c_void) -> *mut c_void =
                    std::mem::transmute(load_egl(c"eglGetDisplay"));
                let egl_initialize: unsafe extern "C" fn(*mut c_void, *mut c_int, *mut c_int) -> c_uint =
                    std::mem::transmute(load_egl(c"eglInitialize"));
                let egl_bind_api: unsafe extern "C" fn(c_uint) -> c_uint =
                    std::mem::transmute(load_egl(c"eglBindAPI"));
                let egl_choose_config: unsafe extern "C" fn(*mut c_void, *const c_int, *mut *mut c_void, c_int, *mut c_int) -> c_uint =
                    std::mem::transmute(load_egl(c"eglChooseConfig"));
                let egl_create_window_surface: unsafe extern "C" fn(*mut c_void, *mut c_void, usize, *const c_int) -> *mut c_void =
                    std::mem::transmute(load_egl(c"eglCreateWindowSurface"));
                let egl_create_context: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *const c_int) -> *mut c_void =
                    std::mem::transmute(load_egl(c"eglCreateContext"));
                let egl_make_current: unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void) -> c_uint =
                    std::mem::transmute(load_egl(c"eglMakeCurrent"));
                let egl_swap_buffers: unsafe extern "C" fn(*mut c_void, *mut c_void) -> c_uint =
                    std::mem::transmute(load_egl(c"eglSwapBuffers"));
                let egl_get_error: unsafe extern "C" fn() -> c_int =
                    std::mem::transmute(load_egl(c"eglGetError"));

                let dpy = egl_get_display(x11_display);
                if dpy.is_null() {
                    eprintln!("[pear-egl] eglGetDisplay returned null");
                    return None;
                }
                let mut maj = 0;
                let mut min = 0;
                if egl_initialize(dpy, &mut maj, &mut min) == 0 {
                    eprintln!("[pear-egl] eglInitialize failed err=0x{:x}", egl_get_error());
                    return None;
                }
                let _ = egl_bind_api(EGL_OPENGL_ES_API);

                let renderable = if api <= 1 { EGL_OPENGL_ES_BIT } else { EGL_OPENGL_ES2_BIT };
                let cfg_attrs = [
                    EGL_SURFACE_TYPE, EGL_WINDOW_BIT,
                    EGL_RENDERABLE_TYPE, renderable,
                    EGL_RED_SIZE, 8,
                    EGL_GREEN_SIZE, 8,
                    EGL_BLUE_SIZE, 8,
                    EGL_ALPHA_SIZE, 8,
                    EGL_NONE,
                ];
                let mut cfg: *mut c_void = std::ptr::null_mut();
                let mut num_cfg = 0;
                if egl_choose_config(dpy, cfg_attrs.as_ptr(), &mut cfg, 1, &mut num_cfg) == 0 || num_cfg == 0 {
                    eprintln!("[pear-egl] eglChooseConfig failed err=0x{:x}", egl_get_error());
                    return None;
                }

                let surf = egl_create_window_surface(dpy, cfg, x11_window as usize, std::ptr::null());
                if surf.is_null() {
                    eprintln!("[pear-egl] eglCreateWindowSurface failed err=0x{:x}", egl_get_error());
                    return None;
                }

                let ver = if api <= 1 { 1 } else { 2 };
                let ctx_attrs = [EGL_CONTEXT_CLIENT_VERSION, ver, EGL_NONE];
                let ctx = egl_create_context(dpy, cfg, std::ptr::null_mut(), ctx_attrs.as_ptr());
                if ctx.is_null() {
                    eprintln!("[pear-egl] eglCreateContext failed err=0x{:x}", egl_get_error());
                    return None;
                }

                if egl_make_current(dpy, surf, surf, ctx) == 0 {
                    eprintln!("[pear-egl] eglMakeCurrent failed err=0x{:x}", egl_get_error());
                    return None;
                }

                let (program, u_angle) = if api >= 2 {
                    let gl_create_shader: unsafe extern "C" fn(c_uint) -> c_uint = std::mem::transmute(load_gl(c"glCreateShader"));
                    let gl_shader_source: unsafe extern "C" fn(c_uint, c_int, *const *const c_char, *const c_int) = std::mem::transmute(load_gl(c"glShaderSource"));
                    let gl_compile_shader: unsafe extern "C" fn(c_uint) = std::mem::transmute(load_gl(c"glCompileShader"));
                    let gl_create_program: unsafe extern "C" fn() -> c_uint = std::mem::transmute(load_gl(c"glCreateProgram"));
                    let gl_attach_shader: unsafe extern "C" fn(c_uint, c_uint) = std::mem::transmute(load_gl(c"glAttachShader"));
                    let gl_bind_attrib_location: unsafe extern "C" fn(c_uint, c_uint, *const c_char) = std::mem::transmute(load_gl(c"glBindAttribLocation"));
                    let gl_link_program: unsafe extern "C" fn(c_uint) = std::mem::transmute(load_gl(c"glLinkProgram"));
                    let gl_get_uniform_location: unsafe extern "C" fn(c_uint, *const c_char) -> c_int = std::mem::transmute(load_gl(c"glGetUniformLocation"));

                    let vs_src = c"attribute vec2 aPos; attribute vec3 aCol; varying vec3 vCol; uniform float uAngle; void main() { float c = cos(uAngle); float s = sin(uAngle); gl_Position = vec4(aPos.x * c - aPos.y * s, aPos.x * s + aPos.y * c, 0.0, 1.0); vCol = aCol; }";
                    let fs_src = c"precision mediump float; varying vec3 vCol; void main() { gl_FragColor = vec4(vCol, 1.0); }";

                    let vs = gl_create_shader(GL_VERTEX_SHADER);
                    let vs_ptr = vs_src.as_ptr();
                    gl_shader_source(vs, 1, &vs_ptr, std::ptr::null());
                    gl_compile_shader(vs);

                    let fs = gl_create_shader(GL_FRAGMENT_SHADER);
                    let fs_ptr = fs_src.as_ptr();
                    gl_shader_source(fs, 1, &fs_ptr, std::ptr::null());
                    gl_compile_shader(fs);

                    let program = gl_create_program();
                    gl_attach_shader(program, vs);
                    gl_attach_shader(program, fs);
                    gl_bind_attrib_location(program, 0, c"aPos".as_ptr());
                    gl_bind_attrib_location(program, 1, c"aCol".as_ptr());
                    gl_link_program(program);
                    let u_angle = gl_get_uniform_location(program, c"uAngle".as_ptr());
                    (program, u_angle)
                } else {
                    (0, -1)
                };

                eprintln!("[pear-egl] EGL {maj}.{min} + GLESv{ver} initialized (surf={surf:?} ctx={ctx:?})");

                Some(Self {
                    dpy,
                    surf,
                    ctx,
                    program,
                    u_angle,
                    egl_swap_buffers,
                    gl_viewport: std::mem::transmute(load_gl(c"glViewport")),
                    gl_clear_color: std::mem::transmute(load_gl(c"glClearColor")),
                    gl_clear: std::mem::transmute(load_gl(c"glClear")),
                    gl_use_program: std::mem::transmute(load_gl(c"glUseProgram")),
                    gl_uniform1f: std::mem::transmute(load_gl(c"glUniform1f")),
                    gl_vertex_attrib_pointer: std::mem::transmute(load_gl(c"glVertexAttribPointer")),
                    gl_enable_vertex_attrib_array: std::mem::transmute(load_gl(c"glEnableVertexAttribArray")),
                    gl_draw_arrays: std::mem::transmute(load_gl(c"glDrawArrays")),
                })
            }
        }

        pub fn swap_buffers(&self) {
            unsafe {
                let _ = (self.egl_swap_buffers)(self.dpy, self.surf);
            }
        }

        pub fn render_demo_frame(&self, width: i32, height: i32, angle: f32) {
            let _ = self.ctx;
            #[rustfmt::skip]
            let verts: [f32; 15] = [
                 0.0,   0.65,  0.30, 0.85, 0.95,
                -0.60, -0.45,  0.95, 0.35, 0.55,
                 0.60, -0.45,  0.35, 0.95, 0.65,
            ];
            unsafe {
                (self.gl_viewport)(0, 0, width, height);
                let bg = 0.08 + 0.04 * angle.sin();
                (self.gl_clear_color)(bg, 0.11, 0.17, 1.0);
                (self.gl_clear)(GL_COLOR_BUFFER_BIT);
                (self.gl_use_program)(self.program);
                if self.u_angle >= 0 {
                    (self.gl_uniform1f)(self.u_angle, angle);
                }
                let stride = (5 * std::mem::size_of::<f32>()) as c_int;
                (self.gl_vertex_attrib_pointer)(0, 2, GL_FLOAT, 0, stride, verts.as_ptr() as *const c_void);
                (self.gl_enable_vertex_attrib_array)(0);
                (self.gl_vertex_attrib_pointer)(1, 3, GL_FLOAT, 0, stride, verts.as_ptr().add(2) as *const c_void);
                (self.gl_enable_vertex_attrib_array)(1);
                (self.gl_draw_arrays)(GL_TRIANGLES, 0, 3);
                let _ = (self.egl_swap_buffers)(self.dpy, self.surf);
            }
        }
    }
}

/// 32-bit iOS API & `PearLegacyEngine` + `PearLegacyJIT` integration layer.
pub mod api32 {
    use pear_jit::{ExecutionMode, ExecutionPlan};

    /// Initializes the OS-independent 32-bit host bridge (`pear-legacy-host`)
    /// and prepares `PearLegacyJIT` (A32) for 32-bit iOS execution.
    pub fn init(plan: &ExecutionPlan) {
        debug_assert_eq!(plan.mode, ExecutionMode::Legacy32BitJit);
        pear_legacy_host::init_legacy_host();
    }
}

/// 64-bit iOS API layer (`pear-dyld` + `pear-libobjc` + `pear-foundation` + `pear-graphics` + `pear-host`),
/// executing natively on ARM64 hosts or via `PearLegacyJIT` (A64 -> x86_64) on x86_64 hosts.
pub mod api64 {
    use pear_jit::{ExecutionMode, ExecutionPlan};

    pub fn init(plan: &ExecutionPlan) {
        match plan.mode {
            ExecutionMode::NativeDirect | ExecutionMode::Arm64ToX86_64Jit => super::init(),
            ExecutionMode::Legacy32BitJit => super::api32::init(plan),
        }
    }
}

/// Inspects a Mach-O or FAT binary and selects the optimal OS/arch/API/JIT plan.
pub fn plan_execution(data: &[u8]) -> Option<pear_jit::ExecutionPlan> {
    let arch = pear_macho::detect_guest_arch(data)?;
    Some(pear_jit::ExecutionPlan::current_host(arch))
}

/// Maps, fixes up, registers ObjC/Foundation metadata, calls `+load`, and runs
/// C/C++ initializers for an arm64 Mach-O image.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn stub_third_party_sdks() {
    if std::env::var("PEAR_DISABLE_AD_SDKS").as_deref() == Ok("0") {
        return;
    }
    let null_imp = ptr_null_stub as Imp;
    let sig = c"@@:".as_ptr();
    let targets: &[(&str, &[&str])] = &[
        ("FIRApp", &["configure", "configureWithOptions:", "configureWithName:options:", "defaultApp", "appNamed:"]),
        ("FIRConfiguration", &["sharedInstance"]),
        ("FIRRemoteConfig", &["remoteConfig", "remoteConfigWithApp:"]),
        ("FIRAnalytics", &["setAnalyticsCollectionEnabled:", "logEventWithName:parameters:", "setUserPropertyString:forName:", "appInstanceID"]),
        ("ALSdk", &["shared", "sharedWithKey:", "sharedWithKey:settings:"]),
        ("GADMobileAds", &["sharedInstance"]),
        ("UnityAds", &["initialize:", "initialize:testMode:", "initialize:testMode:initializationDelegate:", "isInitialized"]),
    ];
    unsafe {
        for &(cls_name, sels) in targets {
            let Some(cls) = objc_rt::class_named(cls_name) else { continue };
            let meta = objc_rt::object_class(cls as Id);
            if meta.is_null() { continue; }
            for &sel_name in sels {
                let _ = pear_libobjc::reflection::class_replace_method(
                    meta,
                    objc_rt::sel(sel_name),
                    null_imp,
                    sig,
                );
            }
        }
    }
}

pub fn load_and_boot(
    data: &[u8],
    options: pear_dyld::Options,
) -> Result<(pear_dyld::LoadedImage, pear_libobjc::ImageReport), pear_dyld::Error> {
    init();
    let image = pear_dyld::load(data, &resolve_symbol, options)?;
    register_loaded_image(&image, "PearGuest");
    let report = unsafe { pear_libobjc::register_image(&image) };
    #[cfg(target_os = "linux")]
    stub_third_party_sdks();
    unsafe {
        pear_libobjc::call_load_methods(&image);
        image.run_initializers();
    }
    Ok((image, report))
}

/// Maps a main executable together with its embedded `@rpath` / `@executable_path` /
/// `@loader_path` dependencies, registers ObjC metadata in dependency-first order,
/// calls `+load` methods, and runs initializers across the full [`pear_dyld::ImageSet`].
pub fn load_bundle_and_boot(
    executable_path: &std::path::Path,
    options: pear_dyld::Options,
) -> Result<(pear_dyld::ImageSet, pear_libobjc::ImageReport), pear_dyld::Error> {
    init();
    let set = pear_dyld::load_bundle(executable_path, &resolve_symbol, options)?;
    clear_loaded_images();
    let fallback_name = executable_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("PearGuest");
    for img in &set.images {
        register_loaded_image(img, fallback_name);
    }

    let mut total = pear_libobjc::ImageReport {
        classes: 0,
        unrealized: Vec::new(),
        categories: 0,
        selectors: 0,
    };
    for &idx in &set.init_order {
        let name = set.images[idx].path.as_deref().unwrap_or("<image>");
        eprintln!("[PearRuntime][boot] register_image {name}");
        let report = unsafe { pear_libobjc::register_image(&set.images[idx]) };
        total.classes += report.classes;
        total.categories += report.categories;
        total.selectors += report.selectors;
        total.unrealized.extend(report.unrealized);
    }

    #[cfg(target_os = "linux")]
    stub_third_party_sdks();

    unsafe {
        for &idx in &set.init_order {
            let name = set.images[idx].path.as_deref().unwrap_or("<image>");
            eprintln!("[PearRuntime][boot] call_load_methods {name}");
            pear_libobjc::call_load_methods(&set.images[idx]);
            eprintln!("[PearRuntime][boot] run_initializers {name}");
            set.images[idx].run_initializers();
        }
    }
    Ok((set, total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unified_runtime_boots_both_objc_fixtures_and_foundation() {
        init();
        let f1 = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../pear-libobjc/fixtures/fixture-objc.dylib")).unwrap();
        let (img1, _) = load_and_boot(&f1, pear_dyld::Options::default()).unwrap();
        let run1: extern "C" fn() -> i32 = unsafe { std::mem::transmute(img1.symbol("_fixture_objc").unwrap()) };
        assert_eq!(run1(), 255);

        let f2 = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../pear-libobjc/fixtures/fixture2-objc.dylib")).unwrap();
        let (img2, _) = load_and_boot(&f2, pear_dyld::Options::default()).unwrap();
        let run2: extern "C" fn() -> i32 = unsafe { std::mem::transmute(img2.symbol("_fixture2_objc").unwrap()) };
        assert_eq!(run2(), 511);

        // Verify Foundation, CoreGraphics, QuartzCore & CFConstantString resolution
        assert!(resolve_symbol(Some("/System/Library/Frameworks/Foundation.framework/Foundation"), "_OBJC_CLASS_$_NSString").is_some());
        assert!(resolve_symbol(Some("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation"), "_CFStringCreateWithCString").is_some());
        assert!(resolve_symbol(Some("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation"), "___CFConstantStringClassReference").is_some());
        assert!(resolve_symbol(Some("/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics"), "_CGRectInset").is_some());
        assert!(resolve_symbol(Some("/System/Library/Frameworks/QuartzCore.framework/QuartzCore"), "_CACurrentMediaTime").is_some());
        assert!(resolve_symbol(Some("/System/Library/Frameworks/QuartzCore.framework/QuartzCore"), "_OBJC_CLASS_$_CAMetalLayer").is_some());
        assert!(resolve_symbol(Some("/System/Library/Frameworks/UIKit.framework/UIKit"), "_OBJC_CLASS_$_UIApplication").is_some());
        assert!(resolve_symbol(Some("/System/Library/Frameworks/UIKit.framework/UIKit"), "_OBJC_CLASS_$_UIScreen").is_some());
        assert!(resolve_symbol(Some("/usr/lib/swift/libswiftCore.dylib"), "_swift_once").is_some());
        assert!(resolve_symbol(Some("/usr/lib/swift/libswiftCore.dylib"), "_swift_bridgeObjectRetain").is_some());
        assert!(resolve_symbol(Some("/usr/lib/swift/libswiftCore.dylib"), "_swift_allocObject").is_some());
        assert!(resolve_symbol(Some("/usr/lib/swift/libswiftCore.dylib"), "_swift_dynamicCast").is_some());
        assert!(resolve_symbol(Some("/usr/lib/swift/libswiftCore.dylib"), "_OBJC_CLASS_$__TtCs12_SwiftObject").is_some());
        assert!(resolve_symbol(Some("/usr/lib/swift/libswiftCore.dylib"), "_OBJC_METACLASS_$__TtCs12_SwiftObject").is_some());
        assert!(resolve_symbol(Some("/usr/lib/swift/libswiftFoundation.dylib"), "_$s10Foundation10URLRequestVMa").is_some());
        assert!(resolve_symbol(Some("/usr/lib/swift/libswiftDispatch.dylib"), "_$s8Dispatch0A4TimeV3nowACyFZ").is_some());
        assert!(resolve_symbol(Some("/System/Library/Frameworks/WebKit.framework/WebKit"), "_OBJC_CLASS_$_WKWebView").is_some());
        assert!(resolve_symbol(Some("/System/Library/Frameworks/UIKit.framework/UIKit"), "_OBJC_METACLASS_$_UIView").is_some());

        unsafe {
            let app_cls = resolve_symbol(Some("/System/Library/Frameworks/UIKit.framework/UIKit"), "_OBJC_CLASS_$_UIApplication").unwrap() as Id;
            let sel_shared = objc_rt::sel("sharedApplication");
            let d1 = pear_libobjc::message::lookup_for_send(app_cls, sel_shared);
            let f1: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d1.imp);
            assert!(!f1(d1.receiver, sel_shared).is_null());

            let dev_cls = resolve_symbol(Some("/System/Library/Frameworks/UIKit.framework/UIKit"), "_OBJC_CLASS_$_UIDevice").unwrap() as Id;
            let sel_cur = objc_rt::sel("currentDevice");
            let d2 = pear_libobjc::message::lookup_for_send(dev_cls, sel_cur);
            let f2: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d2.imp);
            let dev = f2(d2.receiver, sel_cur);
            let sel_sys = objc_rt::sel("systemName");
            let d3 = pear_libobjc::message::lookup_for_send(dev, sel_sys);
            let f3: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d3.imp);
            let name = f3(d3.receiver, sel_sys);
            assert_eq!(pear_foundation::rust_str_from_nsstring(name).as_deref(), Some("iOS"));
        }

        let app_bin = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/fixture-ios-app")).unwrap();
        let (app_img, _) = load_and_boot(&app_bin, pear_dyld::Options::default()).unwrap();
        let app_main: extern "C" fn(i32, *const *const c_char) -> i32 =
            unsafe { std::mem::transmute(app_img.symbol("_main").unwrap()) };
        assert_eq!(app_main(0, std::ptr::null()), 63);

        // guest_dladdr resolves addresses inside loaded guest images
        unsafe {
            let mut info = std::mem::zeroed::<DlInfo>();
            assert_ne!(guest_dladdr(app_main as *const c_void, &mut info), 0);
            assert!(!info.dli_fbase.is_null());
            assert!(!info.dli_fname.is_null());
        }
    }

    #[test]
    fn os_unfair_lock_and_apple_libcxx_runtime_work() {
        let lock = std::sync::atomic::AtomicU32::new(0);
        let ptr = &lock as *const _ as *mut std::sync::atomic::AtomicU32;
        assert!(os_unfair_lock_trylock(ptr));
        assert!(!os_unfair_lock_trylock(ptr));
        os_unfair_lock_unlock(ptr);
        os_unfair_lock_lock(ptr);
        assert!(!os_unfair_lock_trylock(ptr));
        os_unfair_lock_unlock(ptr);

        unsafe {
            let mut s = apple_libcxx::AppleString { words: [0xDEAD; 3] };
            apple_libcxx::str_init_ptr_len(&mut s, c"hello".as_ptr(), 5);
            apple_libcxx::str_append_cstr(&mut s, c" pear".as_ptr());
            apple_libcxx::str_insert_cstr(&mut s, 6, c"world ".as_ptr());
            assert_eq!(apple_libcxx::str_compare_cstr(&s, c"hello world pear".as_ptr()), 0);
            assert_eq!(*apple_libcxx::str_at(&mut s, 6) as u8, b'w');
            apple_libcxx::str_grow_by_and_replace(&mut s, 22, 32, 16, 5, 11, 7, c" runtime".as_ptr());
            assert_eq!(apple_libcxx::str_compare_cstr(&s, c"hello runtim".as_ptr()), 0);
            apple_libcxx::str_dtor(&mut s);

            static CALL_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
            unsafe extern "C" fn bump(_arg: *mut c_void) {
                CALL_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
            let once_flag = std::sync::atomic::AtomicUsize::new(0);
            apple_libcxx::call_once_impl(&once_flag as *const _ as *mut _, std::ptr::null_mut(), bump);
            apple_libcxx::call_once_impl(&once_flag as *const _ as *mut _, std::ptr::null_mut(), bump);
            assert_eq!(CALL_COUNT.load(std::sync::atomic::Ordering::SeqCst), 1);

            let t1 = apple_libcxx::chrono_steady_now();
            apple_libcxx::this_thread_sleep_for(&1_000_000i64);
            let t2 = apple_libcxx::chrono_steady_now();
            assert!(t2 > t1);
            let sys = apple_libcxx::chrono_system_now();
            let tt = apple_libcxx::chrono_to_time_t(&sys);
            assert_eq!(apple_libcxx::chrono_from_time_t(tt), (tt as i64) * 1_000_000);
        }
    }

    #[test]
    fn phase4_security_accelerate_cfnetwork_coremedia_and_corevideo_work() {
        init();
        unsafe {
            // Security: Keychain Add -> CopyMatching -> Update -> Delete & SecRandomCopyBytes
            let attrs = pear_foundation::nsdict_from_pairs(vec![
                ("class".into(), pear_foundation::nsstring_from_rust("genp")),
                ("svce".into(), pear_foundation::nsstring_from_rust("pear.test.svc")),
                ("acct".into(), pear_foundation::nsstring_from_rust("player1")),
                ("v_Data".into(), pear_foundation::nsdata_from_bytes(b"secret-token-1")),
            ]);
            let _ = security::sec_item_delete(attrs);
            assert_eq!(security::sec_item_add(attrs, std::ptr::null_mut()), 0);

            let query = pear_foundation::nsdict_from_pairs(vec![
                ("class".into(), pear_foundation::nsstring_from_rust("genp")),
                ("svce".into(), pear_foundation::nsstring_from_rust("pear.test.svc")),
                ("acct".into(), pear_foundation::nsstring_from_rust("player1")),
                ("r_Data".into(), 1usize as Id),
            ]);
            let mut out: Id = std::ptr::null_mut();
            assert_eq!(security::sec_item_copy_matching(query, &mut out), 0);
            assert_eq!(pear_foundation::rust_bytes_from_nsdata(out).as_deref(), Some(&b"secret-token-1"[..]));

            let upd = pear_foundation::nsdict_from_pairs(vec![
                ("v_Data".into(), pear_foundation::nsdata_from_bytes(b"secret-token-2")),
            ]);
            assert_eq!(security::sec_item_update(query, upd), 0);
            assert_eq!(security::sec_item_copy_matching(query, &mut out), 0);
            assert_eq!(pear_foundation::rust_bytes_from_nsdata(out).as_deref(), Some(&b"secret-token-2"[..]));
            assert_eq!(security::sec_item_delete(query), 0);
            assert_ne!(security::sec_item_copy_matching(query, &mut out), 0);

            let mut rnd = [0u8; 32];
            assert_eq!(security::sec_random_copy_bytes(std::ptr::null(), rnd.len(), rnd.as_mut_ptr()), 0);
            assert!(rnd.iter().any(|&b| b != 0));

            // Accelerate: vDSP + vvexpf
            let a = [1.0f32, 2.0, 3.0];
            let b = [4.0f32, 5.0, 6.0];
            let mut dot = 0.0f32;
            accelerate::vdsp_dotpr(a.as_ptr(), 1, b.as_ptr(), 1, &mut dot, 3);
            assert!((dot - 32.0).abs() < 1e-5);
            let mut maxv = 0.0f32;
            accelerate::vdsp_maxv(b.as_ptr(), 1, &mut maxv, 3);
            assert_eq!(maxv, 6.0);
            let ma = [1.0f32, 2.0, 3.0, 4.0];
            let mb = [5.0f32, 6.0, 7.0, 8.0];
            let mut mc = [0.0f32; 4];
            accelerate::vdsp_mmul(ma.as_ptr(), 1, mb.as_ptr(), 1, mc.as_mut_ptr(), 1, 2, 2, 2);
            assert_eq!(mc, [19.0, 22.0, 43.0, 50.0]);
            let mut ez = [0.0f32; 2];
            let ex = [0.0f32, 1.0];
            let en = 2i32;
            accelerate::vvexpf(ez.as_mut_ptr(), ex.as_ptr(), &en);
            assert!((ez[0] - 1.0).abs() < 1e-5 && (ez[1] - std::f32::consts::E).abs() < 1e-5);

            // CFNetwork CFHTTPMessage + CoreMedia CMTime + CoreVideo CVPixelBuffer
            let msg = frameworks::cf_http_msg_create_request(
                std::ptr::null(),
                pear_foundation::nsstring_from_rust("POST"),
                pear_foundation::nsstring_from_rust("https://example.com/api"),
                pear_foundation::nsstring_from_rust("HTTP/1.1"),
            );
            frameworks::cf_http_msg_set_header(
                msg,
                pear_foundation::nsstring_from_rust("Content-Type"),
                pear_foundation::nsstring_from_rust("application/json"),
            );
            frameworks::cf_http_msg_set_body(msg, pear_foundation::nsdata_from_bytes(b"{}"));
            let serialized = frameworks::cf_http_msg_copy_serialized(msg);
            let raw = String::from_utf8(pear_foundation::rust_bytes_from_nsdata(serialized).unwrap()).unwrap();
            assert!(raw.starts_with("POST https://example.com/api HTTP/1.1\r\n"));
            assert!(raw.contains("Content-Type: application/json\r\n"));
            assert!(raw.ends_with("\r\n\r\n{}"));

            let t1 = frameworks::cm_time_make(30, 60);
            let t2 = frameworks::cm_time_make(90, 60);
            let sum = frameworks::cm_time_add(t1, t2);
            assert!((frameworks::cm_time_get_seconds(sum) - 2.0).abs() < 1e-6);

            let mut pb: *mut c_void = std::ptr::null_mut();
            assert_eq!(frameworks::cv_pixel_buffer_create(std::ptr::null(), 16, 12, 0x42475241, std::ptr::null_mut(), &mut pb), 0);
            assert_eq!(frameworks::cv_pb_get_width(pb), 16);
            assert_eq!(frameworks::cv_pb_get_height(pb), 12);
            assert!(!frameworks::cv_pb_get_base(pb).is_null());
            frameworks::cv_pb_release(pb);

            // Foundation collection & concurrency ObjC classes
            let mt_cls = objc_rt::class_named("NSMapTable").unwrap() as Id;
            let sel_mk = objc_rt::sel("mapTableWithKeyOptions:valueOptions:");
            let d_mk = pear_libobjc::message::lookup_for_send(mt_cls, sel_mk);
            let f_mk: extern "C" fn(Id, Sel, usize, usize) -> Id = std::mem::transmute(d_mk.imp);
            let mt = f_mk(d_mk.receiver, sel_mk, 0, 5);
            assert!(!mt.is_null());
            let key = pear_foundation::nsstring_from_rust("k1");
            let val = pear_foundation::nsstring_from_rust("v1");
            let sel_set = objc_rt::sel("setObject:forKey:");
            let d_set = pear_libobjc::message::lookup_for_send(mt, sel_set);
            let f_set: extern "C" fn(Id, Sel, Id, Id) = std::mem::transmute(d_set.imp);
            f_set(d_set.receiver, sel_set, val, key);
            let sel_get = objc_rt::sel("objectForKey:");
            let d_get = pear_libobjc::message::lookup_for_send(mt, sel_get);
            let f_get: extern "C" fn(Id, Sel, Id) -> Id = std::mem::transmute(d_get.imp);
            assert_eq!(
                pear_foundation::rust_str_from_nsstring(f_get(d_get.receiver, sel_get, key)).as_deref(),
                Some("v1")
            );

            let lock_cls = objc_rt::class_named("NSRecursiveLock").unwrap() as Id;
            let lock = alloc(lock_cls as *const ClassT);
            let s_try = objc_rt::sel("tryLock");
            let d_try = pear_libobjc::message::lookup_for_send(lock, s_try);
            let f_try: extern "C" fn(Id, Sel) -> bool = std::mem::transmute(d_try.imp);
            assert!(f_try(d_try.receiver, s_try));
            assert!(f_try(d_try.receiver, s_try));
            let s_un = objc_rt::sel("unlock");
            let d_un = pear_libobjc::message::lookup_for_send(lock, s_un);
            let f_un: extern "C" fn(Id, Sel) = std::mem::transmute(d_un.imp);
            f_un(d_un.receiver, s_un);
            f_un(d_un.receiver, s_un);

            // DarwinMallocZone vtable check
            let zone = malloc_default_zone_impl() as *const DarwinMallocZone;
            let buf = ((*zone).malloc)(zone as *mut c_void, 64);
            assert!(!buf.is_null());
            assert!(((*zone).size)(zone as *mut c_void, buf) >= 64);
            ((*zone).free)(zone as *mut c_void, buf);

            // Phase 5.2: sysctlbyname, sysctl, AVAudioSession route description
            let mut mlen = 0usize;
            assert_eq!(sysctlbyname_impl(c"hw.machine".as_ptr(), std::ptr::null_mut(), &mut mlen, std::ptr::null(), 0), 0);
            assert!(mlen >= 6);
            let mut small = [0u8; 2];
            let mut slen = 2usize;
            assert_eq!(sysctlbyname_impl(c"hw.machine".as_ptr(), small.as_mut_ptr() as *mut c_void, &mut slen, std::ptr::null(), 0), -1);
            assert_eq!(*tls_errno_ptr(), 12);
            let mut mbuf = [0u8; 64];
            let mut flen = mbuf.len();
            assert_eq!(sysctlbyname_impl(c"hw.machine".as_ptr(), mbuf.as_mut_ptr() as *mut c_void, &mut flen, std::ptr::null(), 0), 0);
            assert_eq!(std::ffi::CStr::from_ptr(mbuf.as_ptr() as *const c_char).to_str().unwrap(), "iPhone15,2");

            let mut cpu_type = 0i32;
            let mut cpu_len = std::mem::size_of::<i32>();
            assert_eq!(sysctlbyname_impl(c"hw.cputype".as_ptr(), &mut cpu_type as *mut i32 as *mut c_void, &mut cpu_len, std::ptr::null(), 0), 0);
            assert_eq!(cpu_type, 0x0100_000c);

            let mib = [6i32, 1i32];
            let mut kbuf = [0u8; 64];
            let mut klen = kbuf.len();
            assert_eq!(sysctl_impl(mib.as_ptr(), 2, kbuf.as_mut_ptr() as *mut c_void, &mut klen, std::ptr::null(), 0), 0);
            assert_eq!(std::ffi::CStr::from_ptr(kbuf.as_ptr() as *const c_char).to_str().unwrap(), "iPhone15,2");

            let audio_cls = objc_rt::class_named("AVAudioSession").unwrap() as Id;
            let s_shared = objc_rt::sel("sharedInstance");
            let d_sh = pear_libobjc::message::lookup_for_send(audio_cls, s_shared);
            let f_sh: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d_sh.imp);
            let sess = f_sh(d_sh.receiver, s_shared);
            let s_route = objc_rt::sel("currentRoute");
            let d_rt = pear_libobjc::message::lookup_for_send(sess, s_route);
            let f_rt: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d_rt.imp);
            let route = f_rt(d_rt.receiver, s_route);
            let s_out = objc_rt::sel("outputs");
            let d_out = pear_libobjc::message::lookup_for_send(route, s_out);
            let f_out: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d_out.imp);
            let outs = f_out(d_out.receiver, s_out);
            assert!(!outs.is_null());
        }
    }

    #[test]
    fn phase5_3_layer_class_screens_and_dlsym() {
        init();
        guest_class!(TEST_METAL_VIEW_CLASS, TEST_METAL_VIEW_META);
        extern "C" fn test_metal_layer_class(_cls: Id, _cmd: Sel) -> Id {
            CAMETALLAYER_CLASS.ptr() as Id
        }
        objc_rt::register_builtin(
            TEST_METAL_VIEW_CLASS.ptr(),
            TEST_METAL_VIEW_META.ptr(),
            "PearTestMetalView",
            64,
            &[],
            &[("layerClass", test_metal_layer_class as *const () as Imp)],
        );

        unsafe {
            // UIView.layer invokes +[cls layerClass]
            let metal_view = alloc(TEST_METAL_VIEW_CLASS.ptr());
            let metal_layer = uiview_layer(metal_view, objc_rt::sel("layer"));
            assert!(!metal_layer.is_null());
            assert_eq!(objc_rt::object_class(metal_layer), CAMETALLAYER_CLASS.ptr());

            let plain_view = alloc(UIVIEW_CLASS.ptr());
            let eagl_layer = uiview_layer(plain_view, objc_rt::sel("layer"));
            assert!(!eagl_layer.is_null());
            assert_eq!(objc_rt::object_class(eagl_layer), CAEAGLLAYER_CLASS.ptr());

            // +[UIScreen screens] and -[UIScreen currentMode]
            let scr_cls = objc_rt::class_named("UIScreen").unwrap() as Id;
            let s_main = objc_rt::sel("mainScreen");
            let d_main = pear_libobjc::message::lookup_for_send(scr_cls, s_main);
            let f_main: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d_main.imp);
            let main_scr = f_main(d_main.receiver, s_main);
            assert!(!main_scr.is_null());

            let s_screens = objc_rt::sel("screens");
            let d_screens = pear_libobjc::message::lookup_for_send(scr_cls, s_screens);
            let f_screens: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d_screens.imp);
            let screens = f_screens(d_screens.receiver, s_screens);
            assert!(!screens.is_null());
            let s_count = objc_rt::sel("count");
            let d_count = pear_libobjc::message::lookup_for_send(screens, s_count);
            let f_count: extern "C" fn(Id, Sel) -> usize = std::mem::transmute(d_count.imp);
            assert_eq!(f_count(d_count.receiver, s_count), 1);
            let s_obj_at = objc_rt::sel("objectAtIndex:");
            let d_obj_at = pear_libobjc::message::lookup_for_send(screens, s_obj_at);
            let f_obj_at: extern "C" fn(Id, Sel, usize) -> Id = std::mem::transmute(d_obj_at.imp);
            assert_eq!(f_obj_at(d_obj_at.receiver, s_obj_at, 0), main_scr);

            let s_mode = objc_rt::sel("currentMode");
            let d_mode = pear_libobjc::message::lookup_for_send(main_scr, s_mode);
            let f_mode: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d_mode.imp);
            let mode = f_mode(d_mode.receiver, s_mode);
            assert!(!mode.is_null());
            let s_size = objc_rt::sel("size");
            let d_size = pear_libobjc::message::lookup_for_send(mode, s_size);
            let f_size: extern "C" fn(Id, Sel) -> pear_graphics::CGSize = std::mem::transmute(d_size.imp);
            let sz = f_size(d_size.receiver, s_size);
            assert!(sz.width > 0.0 && sz.height > 0.0);

            // guest_dlopen + guest_dlsym resolve Metal framework symbol cleanly
            let h = guest_dlopen(c"/System/Library/Frameworks/Metal.framework/Metal".as_ptr(), 0);
            assert!(!h.is_null());
            let sym = guest_dlsym(h, c"MTLCreateSystemDefaultDevice".as_ptr());
            assert_eq!(sym, pear_metal_vk::create_system_default_device as *mut c_void);
            assert_eq!(guest_dlclose(h), 0);
        }
    }

    #[test]
    fn phase5_4_map_table_ownership_and_metal_surface_methods() {
        init();
        guest_class!(TEST_MT_ITEM_CLASS, TEST_MT_ITEM_META);
        static DEALLOC_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        extern "C" fn tracked_dealloc(this: Id, _cmd: Sel) {
            DEALLOC_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            unsafe { pear_libobjc::object::dispose(this) };
        }
        objc_rt::register_builtin(
            TEST_MT_ITEM_CLASS.ptr(),
            TEST_MT_ITEM_META.ptr(),
            "PearTrackedMapItem",
            16,
            &[("dealloc", tracked_dealloc as *const () as Imp)],
            &[],
        );

        unsafe {
            // 1. NSMapTable retains strong keys/values on 0x200 (StrongMemory | ObjectPointerPersonality)
            DEALLOC_COUNT.store(0, std::sync::atomic::Ordering::SeqCst);
            let mt_cls = objc_rt::class_named("NSMapTable").unwrap() as Id;
            let s_mk = objc_rt::sel("mapTableWithKeyOptions:valueOptions:");
            let d_mk = pear_libobjc::message::lookup_for_send(mt_cls, s_mk);
            let f_mk: extern "C" fn(Id, Sel, usize, usize) -> Id = std::mem::transmute(d_mk.imp);
            let mt = f_mk(d_mk.receiver, s_mk, 0x200, 0x200);
            assert!(!mt.is_null());

            let key_obj = alloc(TEST_MT_ITEM_CLASS.ptr());
            let val_obj = alloc(TEST_MT_ITEM_CLASS.ptr());
            let s_set = objc_rt::sel("setObject:forKey:");
            let d_set = pear_libobjc::message::lookup_for_send(mt, s_set);
            let f_set: extern "C" fn(Id, Sel, Id, Id) = std::mem::transmute(d_set.imp);
            f_set(d_set.receiver, s_set, val_obj, key_obj);

            // Caller drops its own references; map table must keep both alive
            pear_libobjc::object::release(key_obj);
            pear_libobjc::object::release(val_obj);
            assert_eq!(DEALLOC_COUNT.load(std::sync::atomic::Ordering::SeqCst), 0);

            // Removing key from map table must release both value and key
            let s_rem = objc_rt::sel("removeObjectForKey:");
            let d_rem = pear_libobjc::message::lookup_for_send(mt, s_rem);
            let f_rem: extern "C" fn(Id, Sel, Id) = std::mem::transmute(d_rem.imp);
            f_rem(d_rem.receiver, s_rem, key_obj);
            assert_eq!(DEALLOC_COUNT.load(std::sync::atomic::Ordering::SeqCst), 2);
            pear_libobjc::object::release(mt);

            // 2. CAMetalLayer responds to setPresentsWithTransaction:, setDrawsAsynchronously:, setBackgroundColor:
            let layer = alloc(CAMETALLAYER_CLASS.ptr());
            for (sel_name, expected) in [
                ("setPresentsWithTransaction:", true),
                ("setDrawsAsynchronously:", true),
                ("setBackgroundColor:", true),
            ] {
                let s_resp = objc_rt::sel("respondsToSelector:");
                let d_resp = pear_libobjc::message::lookup_for_send(layer, s_resp);
                let f_resp: extern "C" fn(Id, Sel, Sel) -> bool = std::mem::transmute(d_resp.imp);
                assert_eq!(f_resp(d_resp.receiver, s_resp, objc_rt::sel(sel_name)), expected);
            }
            let s_pwt = objc_rt::sel("setPresentsWithTransaction:");
            let d_pwt = pear_libobjc::message::lookup_for_send(layer, s_pwt);
            let f_pwt: extern "C" fn(Id, Sel, bool) = std::mem::transmute(d_pwt.imp);
            f_pwt(d_pwt.receiver, s_pwt, true);

            // 3. MTLTexture created from MTLDevice supports setPurgeableState:
            let dev = pear_metal_vk::create_system_default_device();
            assert!(!dev.is_null());
            let td_cls = objc_rt::class_named("MTLTextureDescriptor").unwrap() as Id;
            let s_2d = objc_rt::sel("texture2DDescriptorWithPixelFormat:width:height:mipmapped:");
            let d_2d = pear_libobjc::message::lookup_for_send(td_cls, s_2d);
            let f_2d: extern "C" fn(Id, Sel, usize, usize, usize, bool) -> Id = std::mem::transmute(d_2d.imp);
            let desc = f_2d(d_2d.receiver, s_2d, 80, 16, 16, false);
            assert!(!desc.is_null());
            let s_nt = objc_rt::sel("newTextureWithDescriptor:");
            let d_nt = pear_libobjc::message::lookup_for_send(dev, s_nt);
            let f_nt: extern "C" fn(Id, Sel, Id) -> Id = std::mem::transmute(d_nt.imp);
            let tex = f_nt(d_nt.receiver, s_nt, desc);
            assert!(!tex.is_null());
            let s_ps = objc_rt::sel("setPurgeableState:");
            let d_ps = pear_libobjc::message::lookup_for_send(tex, s_ps);
            let f_ps: extern "C" fn(Id, Sel, usize) -> usize = std::mem::transmute(d_ps.imp);
            assert_eq!(f_ps(d_ps.receiver, s_ps, 1), 2);
            assert_eq!(f_ps(d_ps.receiver, s_ps, 2), 2);
        }
    }

    #[test]
    fn phase5_5_dispatch_and_window_scene_work() {
        init();
        // 1. Semaphore wait/timeout/signal
        let sem = dispatch_semaphore_create_impl(0) as *mut c_void;
        assert!(!sem.is_null());
        assert_eq!(dispatch_semaphore_wait_impl(sem, 0), 1);
        let t_short = dispatch_time_impl(0, 15_000_000);
        assert_eq!(dispatch_semaphore_wait_impl(sem, t_short), 1);

        let sem_addr = sem as usize;
        let t1 = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(10));
            dispatch_semaphore_signal_impl(sem_addr as *mut c_void);
        });
        assert_eq!(dispatch_semaphore_wait_impl(sem, u64::MAX), 0);
        t1.join().unwrap();

        // 2. Dispatch group enter/leave/wait
        let grp = dispatch_group_create_impl() as *mut c_void;
        assert!(!grp.is_null());
        dispatch_group_enter_impl(grp);
        assert_eq!(dispatch_group_wait_impl(grp, 0), 1);
        let grp_addr = grp as usize;
        let t2 = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(10));
            dispatch_group_leave_impl(grp_addr as *mut c_void);
        });
        assert_eq!(dispatch_group_wait_impl(grp, u64::MAX), 0);
        t2.join().unwrap();

        // 3. dispatch_once_f runs once across 4 threads and publishes usize::MAX
        static ONCE_PRED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        static ONCE_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        extern "C" fn once_cb(_ctx: *mut c_void) {
            std::thread::sleep(std::time::Duration::from_millis(10));
            ONCE_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
        ONCE_PRED.store(0, std::sync::atomic::Ordering::SeqCst);
        ONCE_COUNT.store(0, std::sync::atomic::Ordering::SeqCst);
        let pred_ptr = &raw const ONCE_PRED as usize;
        let handles: Vec<_> = (0..4)
            .map(|_| {
                std::thread::spawn(move || {
                    dispatch_once_f_impl(pred_ptr as *mut usize, std::ptr::null_mut(), once_cb);
                    assert_eq!(ONCE_PRED.load(std::sync::atomic::Ordering::Acquire), usize::MAX);
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(ONCE_COUNT.load(std::sync::atomic::Ordering::SeqCst), 1);

        // 4. dispatch_release safely frees sem/grp and ignores DISPATCH_MAIN_Q
        dispatch_release_impl(&raw const DISPATCH_MAIN_Q as *mut c_void);
        dispatch_release_impl(sem);
        dispatch_release_impl(grp);
        assert!(!DISPATCH_SEMAPHORES.lock().unwrap().contains_key(&(sem as usize)));
        assert!(!DISPATCH_GROUPS.lock().unwrap().contains_key(&(grp as usize)));

        // 5. UIWindowScene, UISceneSession and UIWindow initWithWindowScene:
        unsafe {
            let scene = alloc(UIWINDOWSCENE_CLASS.ptr());
            let s_scr = objc_rt::sel("screen");
            let d_scr = pear_libobjc::message::lookup_for_send(scene, s_scr);
            let f_scr: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d_scr.imp);
            assert!(!f_scr(d_scr.receiver, s_scr).is_null());

            let s_act = objc_rt::sel("activationState");
            let d_act = pear_libobjc::message::lookup_for_send(scene, s_act);
            let f_act: extern "C" fn(Id, Sel) -> isize = std::mem::transmute(d_act.imp);
            assert_eq!(f_act(d_act.receiver, s_act), 0);

            let sess = alloc(UISCENESESSION_CLASS.ptr());
            let s_role = objc_rt::sel("role");
            let d_role = pear_libobjc::message::lookup_for_send(sess, s_role);
            let f_role: extern "C" fn(Id, Sel) -> Id = std::mem::transmute(d_role.imp);
            let role = f_role(d_role.receiver, s_role);
            assert_eq!(
                pear_foundation::rust_str_from_nsstring(role).as_deref(),
                Some("UIWindowSceneSessionRoleApplication")
            );

            let win = alloc(UIWINDOW_CLASS.ptr());
            let s_iws = objc_rt::sel("initWithWindowScene:");
            let d_iws = pear_libobjc::message::lookup_for_send(win, s_iws);
            let f_iws: extern "C" fn(Id, Sel, Id) -> Id = std::mem::transmute(d_iws.imp);
            assert_eq!(f_iws(d_iws.receiver, s_iws, scene), win);
        }
    }
}
