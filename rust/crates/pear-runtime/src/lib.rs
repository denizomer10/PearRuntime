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
guest_class!(AVAUDIOSESSION_CLASS, AVAUDIOSESSION_META);
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

extern "C" fn shared_app_stub(_self: Id, _cmd: Sel) -> Id {
    static APP: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *APP.get_or_init(|| unsafe { alloc(UIAPPLICATION_CLASS.ptr()) } as usize) as Id
}

extern "C" fn main_screen_stub(_self: Id, _cmd: Sel) -> Id {
    static SCR: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *SCR.get_or_init(|| unsafe { alloc(UISCREEN_CLASS.ptr()) } as usize) as Id
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
    let mut g = VIEW_LAYERS.lock().unwrap();
    let map = g.get_or_insert_with(std::collections::HashMap::new);
    if let Some(&layer) = map.get(&(self_obj as usize)) {
        return layer as Id;
    }
    let layer = unsafe { alloc(CAEAGLLAYER_CLASS.ptr()) };
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
    #[cfg(target_os = "linux")]
    pear_host_linux::set_frame_tick(pump_display_links);
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
                    let sel_did = objc_rt::sel("application:didFinishLaunchingWithOptions:");
                    let d_did = pear_libobjc::message::lookup_for_send(del, sel_did);
                    let f_did: extern "C" fn(Id, Sel, Id, Id) -> bool = std::mem::transmute(d_did.imp);
                    let _ = f_did(d_did.receiver, sel_did, app, std::ptr::null_mut());
                    let sel_active = objc_rt::sel("applicationDidBecomeActive:");
                    if pear_libobjc::runtime::lookup(cls, sel_active).is_some() {
                        let d_act = pear_libobjc::message::lookup_for_send(del, sel_active);
                        let f_act: extern "C" fn(Id, Sel, Id) = std::mem::transmute(d_act.imp);
                        f_act(d_act.receiver, sel_active, app);
                    }
                }
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

fn init_guest_frameworks() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let i = |p: *const ()| p as Imp;
        for (cls, meta, name) in [
            (&CAMETALLAYER_CLASS, &CAMETALLAYER_META, "CAMetalLayer"),
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
            (&UIWINDOWSCENE_CLASS, &UIWINDOWSCENE_META, "UIWindowScene"),
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
                ("applicationState", i(app_state as *const ())),
                ("statusBarOrientation", i(app_orientation as *const ())),
                ("isStatusBarHidden", i(app_true as *const ())),
                ("setIdleTimerDisabled:", i(app_noop1 as *const ())),
                ("canOpenURL:", i(app_false1 as *const ())),
                ("openURL:", i(app_false1 as *const ())),
                ("isRegisteredForRemoteNotifications", i(app_false as *const ())),
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
            ],
            &[("mainScreen", i(main_screen_stub as *const ()))],
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
                ("setDrawableProperties:", i(app_noop1 as *const ())),
            ],
            &[],
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
extern "C" fn ret_self1(self_obj: Id, _cmd: Sel, _arg: Id) -> Id {
    self_obj
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
                ("currentRoute", i(ret_empty_array as *const ())),
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
    thread_local! {
        static ERRNO: std::cell::UnsafeCell<i32> = const { std::cell::UnsafeCell::new(0) };
    }
    ERRNO.with(|e| e.get())
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

extern "C" fn dispatch_once_f_impl(pred: *mut usize, ctx: *mut c_void, func: extern "C" fn(*mut c_void)) {
    if pred.is_null() { return; }
    unsafe {
        let p = &*(pred as *const std::sync::atomic::AtomicUsize);
        if p.swap(1, std::sync::atomic::Ordering::AcqRel) == 0 {
            func(ctx);
        }
    }
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

extern "C" fn pthread_mach_thread_np_impl(_t: usize) -> u32 { 1 }
extern "C" fn sysctlbyname_impl(_n: *const c_char, _oldp: *mut c_void, _oldlenp: *mut usize, _newp: *const c_void, _newlen: usize) -> i32 { -1 }
extern "C" fn task_info_impl(_task: u32, _flavor: u32, _info: *mut c_void, _count: *mut u32) -> i32 { -1 }
extern "C" fn thread_policy_set_impl(_t: u32, _flavor: u32, _policy: *const c_void, _count: u32) -> i32 { 0 }

#[repr(C)]
struct SwiftOsVersion {
    major: isize,
    minor: isize,
    patch: isize,
}

extern "C" fn swift_stdlib_os_version() -> SwiftOsVersion {
    SwiftOsVersion { major: 17, minor: 0, patch: 0 }
}

extern "C" fn swift_once_impl(pred: *mut usize, f: extern "C" fn(*mut c_void), ctx: *mut c_void) {
    dispatch_once_f_impl(pred, ctx, f);
}

extern "C" fn swift_retain_impl(obj: Id) -> Id {
    let addr = obj as usize;
    if addr != 0 && (addr & 0x8000_0000_0000_0001) == 0 {
        pear_libobjc::object::retain(obj)
    } else {
        obj
    }
}

extern "C" fn swift_release_impl(obj: Id) {
    let addr = obj as usize;
    if addr != 0 && (addr & 0x8000_0000_0000_0001) == 0 {
        pear_libobjc::object::release(obj);
    }
}

extern "C" fn swift_identity_ptr(p: *const c_void) -> *const c_void { p }

extern "C" fn swift_alloc_object_impl(meta: *const ClassT, size: usize, _align_mask: usize) -> Id {
    if meta.is_null() {
        return std::ptr::null_mut();
    }
    let sz = size.max(16);
    unsafe {
        let p = malloc(sz) as Id;
        if !p.is_null() {
            std::ptr::write_bytes(p as *mut u8, 0, sz);
            *(p as *mut *const ClassT) = meta;
        }
        p
    }
}

extern "C" fn swift_dealloc_object_impl(obj: Id, _sz: usize, _align_mask: usize) {
    if !obj.is_null() {
        unsafe { free(obj as *mut c_void) };
    }
}

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

fn swift_core_symbol(symbol: &str) -> Option<usize> {
    let f = |p: *const ()| Some(p as usize);
    match symbol {
        "_swift_retain"
        | "_swift_bridgeObjectRetain"
        | "_swift_unknownObjectRetain"
        | "_swift_errorRetain" => f(swift_retain_impl as *const ()),
        "_swift_release"
        | "_swift_bridgeObjectRelease"
        | "_swift_unknownObjectRelease"
        | "_swift_errorRelease" => f(swift_release_impl as *const ()),
        "_swift_once" => f(swift_once_impl as *const ()),
        "_swift_allocObject" => f(swift_alloc_object_impl as *const ()),
        "_swift_deallocObject" | "_swift_deallocClassInstance" => f(swift_dealloc_object_impl as *const ()),
        "_swift_getObjCClassMetadata"
        | "_swift_getObjCClassFromMetadata"
        | "_swift_getInitializedObjCClass" => f(swift_identity_ptr as *const ()),
        "__swift_stdlib_operatingSystemVersion" => f(swift_stdlib_os_version as *const ()),
        _ => None,
    }
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
    name: Box<std::ffi::CStr>,
}

static LOADED_IMAGES: std::sync::Mutex<Vec<RegisteredImage>> = std::sync::Mutex::new(Vec::new());

fn clear_loaded_images() {
    LOADED_IMAGES.lock().unwrap().clear();
    MAIN_IMAGE_HEADER.store(0, std::sync::atomic::Ordering::Release);
    MAIN_IMAGE_SLIDE.store(0, std::sync::atomic::Ordering::Release);
}

fn register_loaded_image(img: &pear_dyld::LoadedImage, fallback_name: &str) {
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
    let mut guard = LOADED_IMAGES.lock().unwrap();
    if guard.is_empty() {
        MAIN_IMAGE_HEADER.store(header, std::sync::atomic::Ordering::Release);
        MAIN_IMAGE_SLIDE.store(slide, std::sync::atomic::Ordering::Release);
    }
    guard.push(RegisteredImage {
        header,
        slide,
        name: cstr,
    });
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
    if pred.is_null() || block.is_null() {
        return;
    }
    unsafe {
        let p = &*(pred as *const std::sync::atomic::AtomicUsize);
        if p.swap(1, std::sync::atomic::Ordering::AcqRel) == 0 {
            let invoke = *((block as *const usize).add(2));
            let f: extern "C" fn(*mut c_void) = std::mem::transmute(invoke);
            f(block);
        }
    }
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
    &raw const DISPATCH_MAIN_Q as *const c_void
}
extern "C" fn dispatch_group_async_impl(_g: *mut c_void, q: *mut c_void, block: *mut c_void) {
    dispatch_sync_impl(q, block);
}
extern "C" fn dispatch_group_wait_impl(_g: *mut c_void, _timeout: u64) -> isize { 0 }
extern "C" fn dispatch_after_impl(_when: u64, q: *mut c_void, block: *mut c_void) {
    let _ = (q, block);
}
extern "C" fn dispatch_time_impl(when: u64, delta: i64) -> u64 {
    if delta >= 0 { when.saturating_add(delta as u64) } else { when.saturating_sub((-delta) as u64) }
}
extern "C" fn dispatch_semaphore_create_impl(_val: isize) -> *const c_void {
    &raw const DISPATCH_MAIN_Q as *const c_void
}
extern "C" fn dispatch_semaphore_signal_impl(_sem: *mut c_void) -> isize { 1 }
extern "C" fn dispatch_semaphore_wait_impl(_sem: *mut c_void, _timeout: u64) -> isize { 0 }
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
extern "C" fn os_unfair_lock_noop(_lock: *mut c_void) {}
extern "C" fn sec_item_not_found() -> i32 { -25300 }
extern "C" fn asl_open_stub(_ident: *const c_char, _facility: *const c_char, _opts: u32) -> *const c_void {
    &raw const OS_LOG_STUB as *const c_void
}
extern "C" fn asl_ok_stub() -> i32 { 0 }

extern "C" fn cxx_new(sz: usize) -> *mut c_void {
    unsafe { malloc(sz.max(1)) }
}
extern "C" fn cxx_delete(ptr: *mut c_void) {
    if !ptr.is_null() {
        unsafe { free(ptr) };
    }
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

extern "C" fn unwind_resume_trap() -> ! {
    eprintln!("[pear-runtime] _Unwind_Resume reached");
    std::process::abort()
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
    pear_libobjc::object::init_runtime();
    pear_libobjc::blocks::init_blocks();
    pear_foundation::init_foundation();
    init_guest_frameworks();
    #[cfg(target_os = "linux")]
    {
        let _ = pear_host::install_events(Box::new(DefaultGuestEvents));
        pear_host_linux::LinuxHost::install().initialize();
    }
}

fn libcxx_symbol(symbol: &str) -> Option<usize> {
    match symbol {
        "__Znwm" | "__Znam" => Some(cxx_new as *const () as usize),
        "__ZdlPv" | "__ZdaPv" => Some(cxx_delete as *const () as usize),
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
        "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEED1Ev" => {
            Some(apple_libcxx::str_dtor as *const () as usize)
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
        "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE6appendEPKc" => {
            Some(apple_libcxx::str_append_cstr as *const () as usize)
        }
        "__ZNSt3__112basic_stringIcNS_11char_traitsIcEENS_9allocatorIcEEE6appendEPKcm" => {
            Some(apple_libcxx::str_append_cstr_n as *const () as usize)
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
        "__ZNSt3__1plIcNS_11char_traitsIcEENS_9allocatorIcEEEENS_12basic_stringIT_T0_T1_EEPKS6_RKS9_" => {
            Some(apple_libcxx::str_plus_cstr_str as *const () as usize)
        }
        "__ZNSt3__115recursive_mutexC1Ev" => Some(apple_libcxx::rmutex_ctor as *const () as usize),
        "__ZNSt3__115recursive_mutexD1Ev" => Some(apple_libcxx::rmutex_dtor as *const () as usize),
        "__ZNSt3__115recursive_mutex4lockEv" => Some(apple_libcxx::rmutex_lock as *const () as usize),
        "__ZNSt3__115recursive_mutex6unlockEv" => Some(apple_libcxx::rmutex_unlock as *const () as usize),
        "__ZNSt3__112__next_primeEm" => Some(apple_libcxx::next_prime as *const () as usize),
        "__ZNSt3__16__sortIRNS_6__lessIiiEEPiEEvT0_S5_T_" => Some(apple_libcxx::sort_ints as *const () as usize),
        "__ZNSt3__18__rs_getEv" => Some(apple_libcxx::rs_get as *const () as usize),
        "__ZNSt3__112__rs_defaultclEv" => Some(apple_libcxx::rs_default_call as *const () as usize),
        "__ZNSt3__112__rs_defaultD1Ev" => Some(apple_libcxx::rs_default_dtor as *const () as usize),
        "__ZNKSt3__120__vector_base_commonILb1EE20__throw_length_errorEv"
        | "__ZNKSt3__120__vector_base_commonILb1EE20__throw_out_of_rangeEv"
        | "__ZNSt20bad_array_new_lengthC1Ev" => Some(apple_libcxx::throw_abort as *const () as usize),
        _ => dlopen_dlsym(&[c"libc++.so.1", c"libc++abi.so.1", c"libstdc++.so.6"], symbol),
    }
}

/// Unified symbol resolver for `pear-dyld`: resolves `libobjc`, `libSystem`
/// (libclosure + libc basics), `CoreFoundation`, and `Foundation` imports.
pub fn resolve_symbol(library: Option<&str>, symbol: &str) -> Option<usize> {
    init();
    match library {
        Some("/usr/lib/libobjc.A.dylib") => pear_libobjc::symbol(symbol),
        Some(path) if path.contains("libc++") => libcxx_symbol(symbol),
        Some(path) if path.contains("libSystem") || path.contains("libdispatch") => match symbol {
            "__Unwind_Resume" => Some(unwind_resume_trap as *const () as usize),
            "___stack_chk_guard" => Some(&raw const STACK_CHK_GUARD as usize),
            "___stack_chk_fail" => Some(stack_chk_fail_trap as *const () as usize),
            "___error" => Some(tls_errno_ptr as *const () as usize),
            "___memcpy_chk" => Some(memcpy_chk as *const () as usize),
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
            "_dispatch_group_enter"
            | "_dispatch_group_leave"
            | "_dispatch_resume"
            | "_dispatch_block_cancel"
            | "_dispatch_source_cancel"
            | "_dispatch_assert_queue$V2"
            | "_os_release" => Some(dispatch_noop_1 as *const () as usize),
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
            "_dispatch_time"
            | "_dispatch_walltime" => Some(dispatch_time_impl as *const () as usize),
            "_dispatch_semaphore_create" => Some(dispatch_semaphore_create_impl as *const () as usize),
            "_dispatch_semaphore_signal" => Some(dispatch_semaphore_signal_impl as *const () as usize),
            "_dispatch_semaphore_wait" => Some(dispatch_semaphore_wait_impl as *const () as usize),
            "_dispatch_block_create" => Some(dispatch_block_create_impl as *const () as usize),
            "_os_log_create" => Some(os_log_create_impl as *const () as usize),
            "_os_log_type_enabled" => Some(os_log_type_enabled_impl as *const () as usize),
            "__os_log_impl" => Some(status_ok_stub as *const () as usize),
            "_os_unfair_lock_lock"
            | "_os_unfair_lock_unlock" => Some(os_unfair_lock_noop as *const () as usize),
            "_asl_open" => Some(asl_open_stub as *const () as usize),
            "_asl_log"
            | "_asl_add_log_file"
            | "_asl_set_filter" => Some(asl_ok_stub as *const () as usize),
            "_mach_absolute_time" => Some(mach_absolute_time_impl as *const () as usize),
            "_mach_timebase_info" => Some(mach_timebase_info_impl as *const () as usize),
            "_mach_task_self_" => Some(&raw const MACH_TASK_SELF_STUB as usize),
            "_memset_pattern16" => Some(memset_pattern16_impl as *const () as usize),
            "_pthread_mach_thread_np" => Some(pthread_mach_thread_np_impl as *const () as usize),
            "_pthread_mutexattr_init" => Some(darwin_pthread::mutexattr_init as *const () as usize),
            "_pthread_mutexattr_destroy" => Some(darwin_pthread::mutexattr_destroy as *const () as usize),
            "_pthread_mutexattr_settype" => Some(darwin_pthread::mutexattr_settype as *const () as usize),
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
            "_pthread_attr_init" => Some(darwin_pthread::attr_init as *const () as usize),
            "_pthread_attr_destroy" => Some(darwin_pthread::attr_destroy as *const () as usize),
            "_pthread_attr_setdetachstate" => Some(darwin_pthread::attr_setdetachstate as *const () as usize),
            "_pthread_create" => Some(darwin_pthread::thread_create as *const () as usize),
            "_sysctlbyname" => Some(sysctlbyname_impl as *const () as usize),
            "_task_info" => Some(task_info_impl as *const () as usize),
            "_thread_policy_set" => Some(thread_policy_set_impl as *const () as usize),
            "__DefaultRuneLocale" => Some(&raw const DEFAULT_RUNE_LOCALE as usize),
            "___maskrune" => Some(maskrune_impl as *const () as usize),
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
            "_abort" => Some(abort as *const () as usize),
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
            "_OBJC_CLASS_$_UITouch" => Some(UITOUCH_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_UIEvent" => Some(UIEVENT_CLASS.ptr() as usize),
            "_NSStringFromCGRect" => Some(ns_string_from_cgrect as *const () as usize),
            "_NSStringFromCGSize" => Some(ns_string_from_cgsize as *const () as usize),
            "_NSStringFromCGPoint" => Some(ns_string_from_cgpoint as *const () as usize),
            "_OBJC_METACLASS_$_UIDocument" => Some(UIDOCUMENT_META.ptr() as usize),
            "_OBJC_METACLASS_$_UIView" => Some(UIVIEW_META.ptr() as usize),
            "_OBJC_METACLASS_$_UIViewController" => Some(UIVIEWCONTROLLER_META.ptr() as usize),
            "_UIApplicationMain" => Some(ui_application_main as *const () as usize),
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
            _ => None,
        },
        Some(path) if path.contains("AudioToolbox") => audiotoolbox_symbol(symbol),
        Some(path) if path.contains("CFNetwork") => cfnetwork_symbol(symbol).or_else(|| pear_foundation::symbol(symbol)),
        Some(path) if path.contains("Security") => match symbol {
            "_SecItemCopyMatching" | "_SecItemAdd" | "_SecItemUpdate" | "_SecItemDelete" => {
                Some(sec_item_not_found as *const () as usize)
            }
            "_SecKeyCreateWithData" => Some(ptr_null_stub as *const () as usize),
            "_SecKeyVerifySignature" | "_SecTrustEvaluate" => Some(status_ok_stub as *const () as usize),
            "_SecRandomCopyBytes" => Some(status_ok_stub as *const () as usize),
            _ => None,
        },
        Some(path) if path.contains("SystemConfiguration") => match symbol {
            "_SCNetworkReachabilityCreateWithName"
            | "_SCNetworkReachabilityCreateWithAddress" => Some(ptr_null_stub as *const () as usize),
            "_SCNetworkReachabilityGetFlags"
            | "_SCNetworkReachabilitySetCallback"
            | "_SCNetworkReachabilityScheduleWithRunLoop"
            | "_SCNetworkReachabilityUnscheduleFromRunLoop"
            | "_SCNetworkReachabilitySetDispatchQueue" => Some(false_stub as *const () as usize),
            _ => None,
        },
        Some(path) if path.contains("GLKit") => match symbol {
            "_OBJC_CLASS_$_GLKView" => Some(GLKVIEW_CLASS.ptr() as usize),
            "_OBJC_CLASS_$_GLKViewController" => Some(GLKVIEWCONTROLLER_CLASS.ptr() as usize),
            _ => None,
        },
        Some(path) if path.contains("MetalKit") => match symbol {
            "_OBJC_CLASS_$_MTKView" => Some(MTKVIEW_CLASS.ptr() as usize),
            _ => None,
        },
        Some(path) if path.contains("libswiftCore") => swift_core_symbol(symbol).or_else(|| libc_dlsym(symbol)),
        Some(path) if path.contains("libz") => dlopen_dlsym(&[c"libz.so.1", c"libz.1.dylib"], symbol),
        Some(path) if path.contains("libsqlite3") => dlopen_dlsym(&[c"libsqlite3.so.0", c"libsqlite3.dylib"], symbol),
        Some(path) if path.contains("libxml2") => dlopen_dlsym(&[c"libxml2.so.2", c"libxml2.2.dylib"], symbol),
        Some(path) if path.contains("Metal") && !path.contains("MetalKit") => match symbol {
            "_MTLCreateSystemDefaultDevice" | "_MTLCopyAllDevices" => Some(ptr_null_stub as *const () as usize),
            _ => None,
        },
        Some(path) if path.contains("GameController") => match symbol {
            "_OBJC_CLASS_$_GCController" => Some(GCCONTROLLER_CLASS.ptr() as usize),
            "_GCControllerDidConnectNotification" => Some(framework_const("GCControllerDidConnectNotification")),
            "_GCControllerDidDisconnectNotification" => Some(framework_const("GCControllerDidDisconnectNotification")),
            _ => None,
        },
        Some(path) if path.contains("CoreMedia") => match symbol {
            "_CMTimeMake" => Some(cm_time_make as *const () as usize),
            "_CMTimeGetSeconds" => Some(cm_time_get_seconds as *const () as usize),
            "_kCMTimeZero" => Some(&raw const K_CM_TIME_ZERO as usize),
            _ => None,
        },
        Some(path) if path.contains("CoreVideo") => match symbol {
            "_CVPixelBufferLockBaseAddress"
            | "_CVPixelBufferUnlockBaseAddress"
            | "_CVPixelBufferGetWidth"
            | "_CVPixelBufferGetHeight" => Some(status_ok_stub as *const () as usize),
            "_CVPixelBufferGetBaseAddress" => Some(ptr_null_stub as *const () as usize),
            _ => None,
        },
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
            .or_else(|| libcxx_symbol(symbol))
            .or_else(|| resolve_symbol(Some("/usr/lib/libSystem.B.dylib"), symbol))
            .or_else(|| libc_dlsym(symbol)),
        _ => None,
    }
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
            pear_libobjc::call_load_methods(&set.images[idx]);
        }
        set.run_initializers();
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
    }
}
