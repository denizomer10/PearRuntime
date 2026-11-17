//! UIApplication, UIApplicationMain, UIKey, UIPress, UIPressesEvent, UITextView,
//! UINib, UIRuntime*Connection, NSCoder geometry extensions, NSBundle.loadNibNamed,
//! and the AppKit backend (PearBackend*, PearHostView, PearHostWindowDelegate,
//! PearHostAppDelegate, PearHIDUsageForMacKeyCode).

use crate::basics::PearHostScreenInfo;
use pear_nib::{NibArchive, NibValue, NibValuePayload};
use pear_objc::{
    cls, is_kind_of, msg0, msg1, msg2, msg3, msg4, nsarray_count, nsarray_from_ids,
    nsarray_object_at, nsdata_from_bytes, nsdata_to_vec, nsnumber_bool, nsnumber_f64, nsnumber_i64,
    nsstring, nsstring_to_string, objc_autorelease, objc_getAssociatedObject, objc_msgSendSuper,
    objc_release, objc_retain, objc_setAssociatedObject, replace_method as add_method, responds_to,
    sel, CGPoint, CGRect, CGSize, Class, Id, Imp, NSRange, Sel, UIEdgeInsets, NIL,
    OBJC_ASSOCIATION_RETAIN,
};
use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicUsize, Ordering};
use std::sync::Mutex;

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

#[repr(C)]
#[derive(Copy, Clone)]
struct CAFrameRateRange {
    minimum: f32,
    maximum: f32,
    preferred: f32,
}

extern "C" {
    static _dispatch_main_q: c_void;
    fn dispatch_async_f(queue: *const c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
    fn dispatch_sync_f(queue: *const c_void, context: *mut c_void, work: extern "C" fn(*mut c_void));
    fn _Block_copy(aBlock: *const c_void) -> *mut c_void;
    fn _Block_release(aBlock: *const c_void);
    fn objc_allocateClassPair(superclass: Class, name: *const c_char, extraBytes: usize) -> Class;
    fn objc_registerClassPair(cls: Class);
    fn objc_getProtocol(name: *const c_char) -> *mut c_void;
    fn class_conformsToProtocol(cls: Class, proto: *mut c_void) -> bool;
    fn class_replaceMethod(cls: Class, name: Sel, imp: Imp, types: *const c_char) -> Imp;
    fn object_getClass(obj: Id) -> Class;
    fn class_getSuperclass(cls: Class) -> Class;
    fn class_getName(cls: Class) -> *const c_char;
    fn class_getInstanceMethod(cls: Class, name: Sel) -> *mut c_void;
    fn method_getImplementation(m: *mut c_void) -> Imp;
    fn sel_getName(sel: Sel) -> *const c_char;
    fn PearMissingReport(kind: c_int, class_name: *const c_char, selector: *const c_char, caller: *const c_void);
    fn PearHostLogMessage(tag: *const c_char, message: *const c_char);
    fn PearUIKitRunCatchingExceptions(ctx: *mut c_void, fn_ptr: extern "C" fn(*mut c_void));
    fn PearUIKitTrySetValueForKey(target: Id, value: Id, key: Id) -> bool;
    fn PearUIKitTryProbeDisplayLinkSelector(link: Id, selector: Sel, is_setter: bool) -> bool;
    fn PearBackendInvokeScriptCall(target: Id, selector: Sel, args: *const *const c_char, count: usize);
    fn PearUIKitFirstResponder() -> Id;
    fn PearUIKitDeliverPointer(window: Id, phase: isize, point: CGPoint, timestamp: f64);
    fn PearUIKitWindowDidResize(window: Id, point_size: CGSize, scale: f64);
    fn IOPMAssertionCreateWithName(
        assertion_type: *const c_void,
        level: u32,
        name: *const c_void,
        assertion_id: *mut u32,
    ) -> i32;
    fn IOPMAssertionRelease(assertion_id: u32) -> i32;
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

unsafe fn log_msg(tag: &str, msg: &str) {
    let t = CString::new(tag).unwrap_or_default();
    let m = CString::new(msg).unwrap_or_default();
    PearHostLogMessage(t.as_ptr(), m.as_ptr());
}

unsafe fn invoke_bool_block(block: Id, arg: bool) {
    if block.is_null() {
        return;
    }
    let hdr = &*(block as *const BlockHeader);
    let f: unsafe extern "C" fn(Id, bool) = std::mem::transmute(hdr.invoke);
    f(block, arg);
}

struct MainBoolCtx {
    block: *mut c_void,
    arg: bool,
}
extern "C" fn main_bool_trampoline(ctx: *mut c_void) {
    unsafe {
        let c = Box::from_raw(ctx as *mut MainBoolCtx);
        invoke_bool_block(c.block as Id, c.arg);
        _Block_release(c.block);
    }
}

// ============================================================================
// UIApplication & Window Registry
// ============================================================================

static G_APP: AtomicUsize = AtomicUsize::new(0);
static G_KEY_WINDOW: AtomicUsize = AtomicUsize::new(0);
static G_WINDOWS: Mutex<Vec<usize>> = Mutex::new(Vec::new());
static G_APP_STATE: AtomicIsize = AtomicIsize::new(1); // 0=active, 1=inactive, 2=background
static G_LAUNCHED: AtomicBool = AtomicBool::new(false);
static G_MAIN_NIB_OBJECTS: AtomicUsize = AtomicUsize::new(0);

#[no_mangle]
pub extern "C" fn PearUIKitRegisterWindow(window: Id) {
    if window.is_null() {
        return;
    }
    unsafe { objc_retain(window); }
    G_WINDOWS.lock().unwrap().push(window as usize);
}

#[no_mangle]
pub extern "C" fn PearUIKitSetKeyWindow(window: Id) {
    G_KEY_WINDOW.store(window as usize, Ordering::Release);
}

#[no_mangle]
pub extern "C" fn PearUIKitWindows() -> Id {
    let guard = G_WINDOWS.lock().unwrap();
    let ids: Vec<Id> = guard.iter().map(|&p| p as Id).collect();
    unsafe { nsarray_from_ids(&ids) }
}

#[no_mangle]
pub extern "C" fn PearUIKitKeyWindow() -> Id {
    G_KEY_WINDOW.load(Ordering::Acquire) as Id
}

static APP_DEL_KEY: u8 = 0;
static APP_STRONG_DEL_KEY: u8 = 0;
static APP_IDLE_KEY: u8 = 0;
static APP_BADGE_KEY: u8 = 0;
static APP_NEXT_TASK: AtomicUsize = AtomicUsize::new(0);

unsafe extern "C" fn app_shared(_cls: Id, _c: Sel) -> Id {
    G_APP.load(Ordering::Acquire) as Id
}
unsafe extern "C" fn app_init(self_obj: Id, _c: Sel) -> Id {
    let o: Id = super0(self_obj, "UIApplication", "init");
    if !o.is_null() {
        let _ = G_APP.compare_exchange(0, o as usize, Ordering::AcqRel, Ordering::Acquire);
    }
    o
}
unsafe extern "C" fn app_del(s: Id, _c: Sel) -> Id { assoc_get(s, &APP_DEL_KEY) }
unsafe extern "C" fn app_set_del(s: Id, _c: Sel, d: Id) { assoc_set(s, &APP_DEL_KEY, d); }
unsafe extern "C" fn app_retain_del(s: Id, _c: Sel, d: Id) {
    assoc_set(s, &APP_STRONG_DEL_KEY, d);
    assoc_set(s, &APP_DEL_KEY, d);
}
unsafe extern "C" fn app_key_win(_s: Id, _c: Sel) -> Id { PearUIKitKeyWindow() }
unsafe extern "C" fn app_windows(_s: Id, _c: Sel) -> Id { PearUIKitWindows() }
unsafe extern "C" fn app_empty_set(_s: Id, _c: Sel) -> Id { msg0(cls("NSSet"), "set") }
unsafe extern "C" fn app_false(_s: Id, _c: Sel) -> bool { false }
unsafe extern "C" fn app_true(_s: Id, _c: Sel) -> bool { true }
unsafe extern "C" fn app_state(_s: Id, _c: Sel) -> isize { G_APP_STATE.load(Ordering::Acquire) }
unsafe extern "C" fn app_idle(s: Id, _c: Sel) -> bool {
    let n = assoc_get(s, &APP_IDLE_KEY);
    !n.is_null() && msg0::<bool>(n, "boolValue")
}
unsafe extern "C" fn app_set_idle(s: Id, _c: Sel, d: bool) {
    assoc_set(s, &APP_IDLE_KEY, nsnumber_bool(d));
    PearBackendSetIdleTimerDisabled(d);
}
unsafe extern "C" fn app_orient(_s: Id, _c: Sel) -> isize { 4 }
unsafe extern "C" fn app_noop(_s: Id, _c: Sel) {}
unsafe extern "C" fn app_noop_b(_s: Id, _c: Sel, _b: bool) {}
unsafe extern "C" fn app_noop_bi(_s: Id, _c: Sel, _b: bool, _i: isize) {}
unsafe extern "C" fn app_noop_id(_s: Id, _c: Sel, _x: Id) {}
unsafe extern "C" fn app_noop_u(_s: Id, _c: Sel, _u: usize) {}
unsafe extern "C" fn app_noop_f(_s: Id, _c: Sel, _f: f64) {}
unsafe extern "C" fn app_zero_rect(_s: Id, _c: Sel) -> CGRect { CGRect::default() }
unsafe extern "C" fn app_zero_int(_s: Id, _c: Sel) -> isize { 0 }
unsafe extern "C" fn app_badge(s: Id, _c: Sel) -> isize {
    let n = assoc_get(s, &APP_BADGE_KEY);
    if n.is_null() { 0 } else { msg0(n, "integerValue") }
}
unsafe extern "C" fn app_set_badge(s: Id, _c: Sel, v: isize) { assoc_set(s, &APP_BADGE_KEY, nsnumber_i64(v as i64)); }
unsafe extern "C" fn app_bg_rem(_s: Id, _c: Sel) -> f64 { 180.0 }
unsafe extern "C" fn app_pref_cat(_s: Id, _c: Sel) -> Id { nsstring("UICTContentSizeCategoryL") }
unsafe extern "C" fn app_can_open(_s: Id, _c: Sel, url: Id) -> bool {
    if url.is_null() { return false; }
    let scheme = nsstring_to_string(msg0(url, "scheme")).unwrap_or_default().to_lowercase();
    scheme == "http" || scheme == "https" || scheme == "mailto"
}
unsafe extern "C" fn app_open_url(s: Id, _c: Sel, url: Id) -> bool {
    app_can_open(s, sel("canOpenURL:"), url) && PearBackendOpenURL(url)
}
unsafe extern "C" fn app_open_url_opts(s: Id, _c: Sel, url: Id, _opts: Id, completion: Id) {
    let ok = app_open_url(s, sel("openURL:"), url);
    if !completion.is_null() {
        let ctx = Box::into_raw(Box::new(MainBoolCtx {
            block: _Block_copy(completion as *const c_void),
            arg: ok,
        }));
        dispatch_async_f(&_dispatch_main_q as *const c_void, ctx as *mut c_void, main_bool_trampoline);
    }
}
unsafe extern "C" fn app_reg_remote(s: Id, _c: Sel) {
    let del = assoc_get(s, &APP_DEL_KEY);
    if del.is_null() { return; }
    let fail = "application:didFailToRegisterForRemoteNotificationsWithError:";
    if responds_to(del, fail) {
        let info: Id = msg2(
            cls("NSDictionary"),
            "dictionaryWithObject:forKey:",
            nsstring("remote notifications are not supported"),
            nsstring("NSLocalizedDescription"),
        );
        let err: Id = msg3(cls("NSError"), "errorWithDomain:code:userInfo:", nsstring("NSCocoaErrorDomain"), 3010isize, info);
        msg2::<Id, Id, ()>(del, fail, s, err);
    }
}
unsafe extern "C" fn app_begin_bg(_s: Id, _c: Sel, _h: Id) -> usize {
    APP_NEXT_TASK.fetch_add(1, Ordering::Relaxed) + 1
}
unsafe extern "C" fn app_begin_bg_name(_s: Id, _c: Sel, _n: Id, _h: Id) -> usize {
    APP_NEXT_TASK.fetch_add(1, Ordering::Relaxed) + 1
}
unsafe extern "C" fn app_send_action(_s: Id, _c: Sel, action: Sel, target: Id, sender: Id, _ev: Id) -> bool {
    if target.is_null() || action.is_null() { return false; }
    let ok: c_int = msg1(target, "respondsToSelector:", action);
    if ok == 0 { return false; }
    let f: unsafe extern "C" fn(Id, Sel, Id) = std::mem::transmute(pear_objc::objc_msgSend as *const c_void);
    f(target, action, sender);
    true
}

unsafe fn call_app_delegate(sel_name: &str) {
    let app = G_APP.load(Ordering::Acquire) as Id;
    if app.is_null() { return; }
    let del: Id = msg0(app, "delegate");
    if !del.is_null() && responds_to(del, sel_name) {
        msg1::<Id, ()>(del, sel_name, app);
    }
}

unsafe fn post_app_notif(name: &str) {
    let app = G_APP.load(Ordering::Acquire) as Id;
    let nc: Id = msg0(cls("NSNotificationCenter"), "defaultCenter");
    msg2::<Id, Id, ()>(nc, "postNotificationName:object:", nsstring(name), app);
}

#[no_mangle]
pub extern "C" fn PearUIKitApplicationDidBecomeActive() {
    if !G_LAUNCHED.load(Ordering::Acquire) || G_APP_STATE.load(Ordering::Acquire) == 0 {
        return;
    }
    G_APP_STATE.store(0, Ordering::Release);
    unsafe {
        call_app_delegate("applicationDidBecomeActive:");
        post_app_notif("UIApplicationDidBecomeActiveNotification");
    }
}

#[no_mangle]
pub extern "C" fn PearUIKitApplicationWillResignActive() {
    if !G_LAUNCHED.load(Ordering::Acquire) || G_APP_STATE.load(Ordering::Acquire) != 0 {
        return;
    }
    G_APP_STATE.store(1, Ordering::Release);
    unsafe {
        call_app_delegate("applicationWillResignActive:");
        post_app_notif("UIApplicationWillResignActiveNotification");
    }
}

#[no_mangle]
pub extern "C" fn PearUIKitApplicationDidEnterBackground() {
    if !G_LAUNCHED.load(Ordering::Acquire) {
        return;
    }
    PearUIKitApplicationWillResignActive();
    G_APP_STATE.store(2, Ordering::Release);
    unsafe {
        call_app_delegate("applicationDidEnterBackground:");
        post_app_notif("UIApplicationDidEnterBackgroundNotification");
    }
}

#[no_mangle]
pub extern "C" fn PearUIKitApplicationWillEnterForeground() {
    if !G_LAUNCHED.load(Ordering::Acquire) || G_APP_STATE.load(Ordering::Acquire) != 2 {
        return;
    }
    G_APP_STATE.store(1, Ordering::Release);
    unsafe {
        call_app_delegate("applicationWillEnterForeground:");
        post_app_notif("UIApplicationWillEnterForegroundNotification");
    }
}

#[no_mangle]
pub extern "C" fn PearUIKitApplicationWillTerminate() {
    unsafe {
        call_app_delegate("applicationWillTerminate:");
        post_app_notif("UIApplicationWillTerminateNotification");
    }
}

extern "C" fn launch_finish_inner(ctx: *mut c_void) {
    unsafe {
        let app = ctx as Id;
        let bundle: Id = msg0(cls("NSBundle"), "mainBundle");
        let info: Id = msg0(bundle, "infoDictionary");
        let mut main_nib = nsstring_to_string(msg1(info, "objectForKey:", nsstring("NSMainNibFile~ipad")));
        if main_nib.is_none() {
            main_nib = nsstring_to_string(msg1(info, "objectForKey:", nsstring("NSMainNibFile")));
        }

        let mut nib_windows: Vec<Id> = Vec::new();
        if let Some(ref mn) = main_nib {
            if !mn.is_empty() {
                let nib: Id = msg2(cls("UINib"), "nibWithNibName:bundle:", nsstring(mn), NIL);
                if !nib.is_null() {
                    let mut out_wins: Id = NIL;
                    let objs: Id = msg3(nib, "pear_instantiateWithOwner:options:visibleWindows:", app, NIL, &mut out_wins as *mut Id);
                    if !objs.is_null() {
                        objc_retain(objs);
                        G_MAIN_NIB_OBJECTS.store(objs as usize, Ordering::Release);
                    }
                    for i in 0..nsarray_count(out_wins) {
                        nib_windows.push(nsarray_object_at(out_wins, i));
                    }
                }
            }
        }

        let del: Id = msg0(app, "delegate");
        if del.is_null() {
            log_msg("app", "no application delegate after launch setup");
        } else {
            let options: Id = msg0(cls("NSDictionary"), "dictionary");
            if responds_to(del, "application:willFinishLaunchingWithOptions:") {
                let _: bool = msg2(del, "application:willFinishLaunchingWithOptions:", app, options);
            }
            if responds_to(del, "application:configurationForConnectingSceneSession:options:") {
                log_msg("app", "delegate supports scenes; launching without a scene session");
            }
            if responds_to(del, "application:didFinishLaunchingWithOptions:") {
                let _: bool = msg2(del, "application:didFinishLaunchingWithOptions:", app, options);
            } else if responds_to(del, "applicationDidFinishLaunching:") {
                msg1::<Id, ()>(del, "applicationDidFinishLaunching:", app);
            }
        }

        for w in nib_windows {
            if !msg0::<bool>(w, "isKeyWindow") {
                msg0::<()>(w, "makeKeyAndVisible");
            }
        }
        G_LAUNCHED.store(true, Ordering::Release);
        post_app_notif("UIApplicationDidFinishLaunchingNotification");
        let wcount = G_WINDOWS.lock().unwrap().len();
        log_msg("app", &format!("launch complete; {wcount} window(s)"));
        PearUIKitApplicationDidBecomeActive();
        PearBackendStartInputScript();
    }
}

extern "C" fn launch_on_did_finish(ctx: *mut c_void) {
    unsafe {
        PearUIKitRunCatchingExceptions(ctx, launch_finish_inner);
    }
}

#[no_mangle]
pub extern "C" fn UIApplicationMain(
    _argc: c_int,
    _argv: *mut *mut c_char,
    principal_cls_name: Id,
    delegate_cls_name: Id,
) -> c_int {
    unsafe {
        pear_objc::autorelease_pool(|| {
            let p_name = nsstring_to_string(principal_cls_name);
            let d_name = nsstring_to_string(delegate_cls_name);
            log_msg(
                "app",
                &format!(
                    "UIApplicationMain principal={} delegate={}",
                    p_name.as_deref().unwrap_or("(default)"),
                    d_name.as_deref().unwrap_or("(none)")
                ),
            );
            PearBackendInitialize();

            let mut app_cls = cls("UIApplication");
            if let Some(ref pn) = p_name {
                if !pn.is_empty() {
                    let c = cls(pn);
                    if !c.is_null() {
                        app_cls = c;
                    } else {
                        log_msg("app", &format!("class {pn} not found"));
                    }
                }
            }
            let app: Id = msg0(app_cls as Id, "new");
            G_APP.store(app as usize, Ordering::Release);

            let mut del: Id = NIL;
            if let Some(ref dn) = d_name {
                if !dn.is_empty() {
                    let dc = cls(dn);
                    if !dc.is_null() {
                        del = msg0(dc as Id, "new");
                    } else {
                        log_msg("app", &format!("class {dn} not found"));
                    }
                }
            }
            if del.is_null() {
                let pname = CString::new("UIApplicationDelegate").unwrap();
                let proto = objc_getProtocol(pname.as_ptr());
                if !proto.is_null() && class_conformsToProtocol(object_getClass(app), proto) {
                    del = app;
                }
            }
            msg1::<Id, ()>(app, "pear_retainDelegate:", del);
            if !del.is_null() && del != app {
                objc_release(del);
            }

            let info: Id = msg0(msg0(cls("NSBundle"), "mainBundle"), "infoDictionary");
            let has_sb = !msg1::<Id, Id>(info, "objectForKey:", nsstring("UIMainStoryboardFile")).is_null();
            if d_name.as_deref().unwrap_or("").is_empty() && has_sb {
                let c1 = CString::new("UIApplicationMain").unwrap();
                let c2 = CString::new("main storyboard launch").unwrap();
                PearMissingReport(1, c1.as_ptr(), c2.as_ptr(), std::ptr::null());
            }

            PearBackendRun(app as *mut c_void, launch_on_did_finish);
        });
    }
    0
}

// ============================================================================
// UIKey / UIPress / UIPressesEvent / UITextView
// ============================================================================

static KEY_CHARS_KEY: u8 = 0;
static KEY_NOMOD_KEY: u8 = 0;
static KEY_MODS_KEY: u8 = 0;
static KEY_CODE_KEY: u8 = 0;

unsafe extern "C" fn uikey_init(s: Id, _c: Sel, chars: Id, nomod: Id, mods: isize, code: isize) -> Id {
    let o: Id = super0(s, "UIKey", "init");
    if !o.is_null() {
        assoc_set(o, &KEY_CHARS_KEY, if chars.is_null() { nsstring("") } else { chars });
        assoc_set(o, &KEY_NOMOD_KEY, if nomod.is_null() { nsstring("") } else { nomod });
        assoc_set(o, &KEY_MODS_KEY, nsnumber_i64(mods as i64));
        assoc_set(o, &KEY_CODE_KEY, nsnumber_i64(code as i64));
    }
    o
}
unsafe extern "C" fn uikey_chars(s: Id, _c: Sel) -> Id { assoc_get(s, &KEY_CHARS_KEY) }
unsafe extern "C" fn uikey_nomod(s: Id, _c: Sel) -> Id { assoc_get(s, &KEY_NOMOD_KEY) }
unsafe extern "C" fn uikey_mods(s: Id, _c: Sel) -> isize { msg0(assoc_get(s, &KEY_MODS_KEY), "integerValue") }
unsafe extern "C" fn uikey_code(s: Id, _c: Sel) -> isize { msg0(assoc_get(s, &KEY_CODE_KEY), "integerValue") }
unsafe extern "C" fn copy_self(s: Id, _c: Sel, _z: *mut c_void) -> Id { objc_retain(s) }

static PRESS_KEY_KEY: u8 = 0;
static PRESS_TYPE_KEY: u8 = 0;
static PRESS_PHASE_KEY: u8 = 0;
static PRESS_TS_KEY: u8 = 0;

unsafe extern "C" fn press_init(s: Id, _c: Sel, key: Id, ty: isize, phase: isize, ts: f64) -> Id {
    let o: Id = super0(s, "UIPress", "init");
    if !o.is_null() {
        assoc_set(o, &PRESS_KEY_KEY, key);
        assoc_set(o, &PRESS_TYPE_KEY, nsnumber_i64(ty as i64));
        assoc_set(o, &PRESS_PHASE_KEY, nsnumber_i64(phase as i64));
        assoc_set(o, &PRESS_TS_KEY, nsnumber_f64(ts));
    }
    o
}
unsafe extern "C" fn press_key(s: Id, _c: Sel) -> Id { assoc_get(s, &PRESS_KEY_KEY) }
unsafe extern "C" fn press_type(s: Id, _c: Sel) -> isize { msg0(assoc_get(s, &PRESS_TYPE_KEY), "integerValue") }
unsafe extern "C" fn press_phase(s: Id, _c: Sel) -> isize { msg0(assoc_get(s, &PRESS_PHASE_KEY), "integerValue") }
unsafe extern "C" fn press_ts(s: Id, _c: Sel) -> f64 { msg0(assoc_get(s, &PRESS_TS_KEY), "doubleValue") }
unsafe extern "C" fn press_force(s: Id, _c: Sel) -> f64 {
    if press_phase(s, sel("phase")) == 3 { 0.0 } else { 1.0 }
}
unsafe extern "C" fn press_nil(_s: Id, _c: Sel) -> Id { NIL }
unsafe extern "C" fn press_win(_s: Id, _c: Sel) -> Id { PearUIKitKeyWindow() }
unsafe extern "C" fn press_empty_arr(_s: Id, _c: Sel) -> Id { msg0(cls("NSArray"), "array") }

static PEV_PRESSES_KEY: u8 = 0;
static PEV_TS_KEY: u8 = 0;

unsafe fn super2<A1, A2, R>(obj: Id, base_cls_name: &str, selector: &str, a1: A1, a2: A2) -> R {
    let mut sup = ObjcSuper {
        receiver: obj,
        super_class: class_getSuperclass(cls(base_cls_name)),
    };
    let f: unsafe extern "C" fn(&mut ObjcSuper, Sel, A1, A2) -> R =
        std::mem::transmute(objc_msgSendSuper as *const c_void);
    f(&mut sup, sel(selector), a1, a2)
}

unsafe extern "C" fn pev_init(s: Id, _c: Sel, presses: Id, ts: f64) -> Id {
    let empty: Id = msg0(cls("NSSet"), "set");
    let o: Id = super2(s, "UIPressesEvent", "initWithTouches:timestamp:", empty, ts);
    let _ = ts;
    if !o.is_null() {
        assoc_set(o, &PEV_PRESSES_KEY, presses);
        assoc_set(o, &PEV_TS_KEY, nsnumber_f64(ts));
    }
    o
}
unsafe extern "C" fn pev_type(_s: Id, _c: Sel) -> isize { 3 } // presses
unsafe extern "C" fn pev_ts(s: Id, _c: Sel) -> f64 { msg0(assoc_get(s, &PEV_TS_KEY), "doubleValue") }
unsafe extern "C" fn pev_all_presses(s: Id, _c: Sel) -> Id { assoc_get(s, &PEV_PRESSES_KEY) }
unsafe extern "C" fn pev_for_gr(_s: Id, _c: Sel, _g: Id) -> Id { msg0(cls("NSSet"), "set") }
unsafe extern "C" fn pev_all_touches(_s: Id, _c: Sel) -> Id { NIL }

static SOFT_KB_SHOWN: AtomicBool = AtomicBool::new(false);

unsafe fn post_keyboard(show: bool) {
    if SOFT_KB_SHOWN.swap(show, Ordering::AcqRel) == show {
        return;
    }
    let scr_b: CGRect = msg0(msg0(cls("UIScreen"), "mainScreen"), "bounds");
    let frame = CGRect {
        origin: CGPoint { x: 0.0, y: scr_b.size.height },
        size: CGSize { width: scr_b.size.width, height: 0.0 },
    };
    let fv: Id = msg1(cls("NSValue"), "valueWithRect:", frame);
    let info: Id = msg0(cls("NSMutableDictionary"), "dictionary");
    msg2::<Id, Id, ()>(info, "setObject:forKey:", fv, nsstring("UIKeyboardFrameBeginUserInfoKey"));
    msg2::<Id, Id, ()>(info, "setObject:forKey:", fv, nsstring("UIKeyboardFrameEndUserInfoKey"));
    msg2::<Id, Id, ()>(info, "setObject:forKey:", nsnumber_f64(0.0), nsstring("UIKeyboardAnimationDurationUserInfoKey"));
    msg2::<Id, Id, ()>(info, "setObject:forKey:", nsnumber_i64(7), nsstring("UIKeyboardAnimationCurveUserInfoKey"));
    msg2::<Id, Id, ()>(info, "setObject:forKey:", nsnumber_bool(true), nsstring("UIKeyboardIsLocalUserInfoKey"));
    let nc: Id = msg0(cls("NSNotificationCenter"), "defaultCenter");
    let n1 = if show { "UIKeyboardWillShowNotification" } else { "UIKeyboardWillHideNotification" };
    let n2 = if show { "UIKeyboardDidShowNotification" } else { "UIKeyboardDidHideNotification" };
    msg3::<Id, Id, Id, ()>(nc, "postNotificationName:object:userInfo:", nsstring(n1), NIL, info);
    msg3::<Id, Id, Id, ()>(nc, "postNotificationName:object:userInfo:", nsstring("UIKeyboardWillChangeFrameNotification"), NIL, info);
    msg3::<Id, Id, Id, ()>(nc, "postNotificationName:object:userInfo:", nsstring(n2), NIL, info);
    msg3::<Id, Id, Id, ()>(nc, "postNotificationName:object:userInfo:", nsstring("UIKeyboardDidChangeFrameNotification"), NIL, info);
    log_msg("keyboard", if show { "text input started" } else { "text input ended" });
}

static TV_TEXT_KEY: u8 = 0;
static TV_SEL_KEY: u8 = 0;
static TV_DEL_KEY: u8 = 0;
static TV_FLAGS_KEY: u8 = 0;
static TV_COLOR_KEY: u8 = 0;
static TV_FONT_KEY: u8 = 0;

#[repr(C)]
#[derive(Copy, Clone)]
struct TvFlags {
    editable: bool,
    editing: bool,
    kb_type: isize,
    ret_type: isize,
    autocap: isize,
    autocorrect: isize,
    spell: isize,
    smart_q: isize,
    smart_d: isize,
    secure: bool,
}

impl Default for TvFlags {
    fn default() -> Self {
        Self {
            editable: true,
            editing: false,
            kb_type: 0,
            ret_type: 0,
            autocap: 0,
            autocorrect: 0,
            spell: 0,
            smart_q: 0,
            smart_d: 0,
            secure: false,
        }
    }
}

unsafe fn tv_text_buf(tv: Id) -> Id {
    let b = assoc_get(tv, &TV_TEXT_KEY);
    if !b.is_null() { return b; }
    let b: Id = msg0(cls("NSMutableString"), "string");
    assoc_set(tv, &TV_TEXT_KEY, b);
    b
}
unsafe fn tv_flags(tv: Id) -> TvFlags {
    let d = assoc_get(tv, &TV_FLAGS_KEY);
    if let Some(b) = nsdata_to_vec(d) {
        if b.len() == std::mem::size_of::<TvFlags>() {
            return std::ptr::read_unaligned(b.as_ptr() as *const TvFlags);
        }
    }
    TvFlags::default()
}
unsafe fn set_tv_flags(tv: Id, f: TvFlags) {
    let bytes = std::slice::from_raw_parts(&f as *const TvFlags as *const u8, std::mem::size_of::<TvFlags>());
    assoc_set(tv, &TV_FLAGS_KEY, nsdata_from_bytes(bytes));
}
unsafe fn tv_sel(tv: Id) -> NSRange {
    let d = assoc_get(tv, &TV_SEL_KEY);
    if let Some(b) = nsdata_to_vec(d) {
        if b.len() == std::mem::size_of::<NSRange>() {
            return std::ptr::read_unaligned(b.as_ptr() as *const NSRange);
        }
    }
    NSRange::default()
}
unsafe fn set_tv_sel(tv: Id, mut r: NSRange) {
    let len: usize = msg0(tv_text_buf(tv), "length");
    if r.location > len { r.location = len; }
    if r.location + r.length > len { r.length = len - r.location; }
    let bytes = std::slice::from_raw_parts(&r as *const NSRange as *const u8, std::mem::size_of::<NSRange>());
    assoc_set(tv, &TV_SEL_KEY, nsdata_from_bytes(bytes));
}

unsafe extern "C" fn tv_init_frame(s: Id, _c: Sel, frame: CGRect) -> Id {
    let o: Id = super1(s, "UITextView", "initWithFrame:", frame);
    if !o.is_null() { set_tv_flags(o, TvFlags::default()); }
    o
}
unsafe extern "C" fn tv_init_tc(s: Id, _c: Sel, frame: CGRect, _tc: Id) -> Id {
    msg1(s, "initWithFrame:", frame)
}
unsafe extern "C" fn tv_init_coder(s: Id, _c: Sel, coder: Id) -> Id {
    let o: Id = super1(s, "UITextView", "initWithCoder:", coder);
    if !o.is_null() {
        set_tv_flags(o, TvFlags::default());
        if !coder.is_null() {
            let dec: Id = msg1(coder, "decodeObjectForKey:", nsstring("UIText"));
            if is_kind_of(dec, "NSString") {
                msg1::<Id, ()>(tv_text_buf(o), "setString:", dec);
            }
        }
    }
    o
}
unsafe extern "C" fn tv_del(s: Id, _c: Sel) -> Id { assoc_get(s, &TV_DEL_KEY) }
unsafe extern "C" fn tv_set_del(s: Id, _c: Sel, d: Id) { assoc_set(s, &TV_DEL_KEY, d); }
unsafe extern "C" fn tv_text(s: Id, _c: Sel) -> Id {
    let c: Id = msg0(tv_text_buf(s), "copy");
    objc_autorelease(c)
}
unsafe extern "C" fn tv_set_text(s: Id, _c: Sel, t: Id) {
    let buf = tv_text_buf(s);
    msg1::<Id, ()>(buf, "setString:", if t.is_null() { nsstring("") } else { t });
    let len: usize = msg0(buf, "length");
    set_tv_sel(s, NSRange { location: len, length: 0 });
}
unsafe extern "C" fn tv_attr_text(s: Id, _c: Sel) -> Id {
    let a: Id = msg1(msg0(cls("NSAttributedString"), "alloc"), "initWithString:", tv_text(s, sel("text")));
    objc_autorelease(a)
}
unsafe extern "C" fn tv_set_attr_text(s: Id, _c: Sel, at: Id) {
    let str_val: Id = if at.is_null() { nsstring("") } else { msg0(at, "string") };
    tv_set_text(s, sel("setText:"), str_val);
}
unsafe extern "C" fn tv_get_sel(s: Id, _c: Sel) -> NSRange { tv_sel(s) }
unsafe extern "C" fn tv_set_sel_range(s: Id, _c: Sel, r: NSRange) { set_tv_sel(s, r); }
unsafe extern "C" fn tv_editable(s: Id, _c: Sel) -> bool { tv_flags(s).editable }
unsafe extern "C" fn tv_set_editable(s: Id, _c: Sel, b: bool) {
    let mut f = tv_flags(s); f.editable = b; set_tv_flags(s, f);
}
unsafe extern "C" fn tv_has_text(s: Id, _c: Sel) -> bool { msg0::<usize>(tv_text_buf(s), "length") > 0 }
unsafe extern "C" fn tv_font(s: Id, _c: Sel) -> Id { assoc_get(s, &TV_FONT_KEY) }
unsafe extern "C" fn tv_set_font(s: Id, _c: Sel, v: Id) { assoc_set(s, &TV_FONT_KEY, v); }
unsafe extern "C" fn tv_color(s: Id, _c: Sel) -> Id { assoc_get(s, &TV_COLOR_KEY) }
unsafe extern "C" fn tv_set_color(s: Id, _c: Sel, v: Id) { assoc_set(s, &TV_COLOR_KEY, v); }
unsafe extern "C" fn tv_false(_s: Id, _c: Sel) -> bool { false }
unsafe extern "C" fn tv_true(_s: Id, _c: Sel) -> bool { true }
unsafe extern "C" fn tv_zero_int(_s: Id, _c: Sel) -> isize { 0 }
unsafe extern "C" fn tv_nil(_s: Id, _c: Sel) -> Id { NIL }
unsafe extern "C" fn tv_noop(_s: Id, _c: Sel) {}
unsafe extern "C" fn tv_noop_id(_s: Id, _c: Sel, _a: Id) {}
unsafe extern "C" fn tv_noop_int(_s: Id, _c: Sel, _a: isize) {}
unsafe extern "C" fn tv_noop_bool(_s: Id, _c: Sel, _a: bool) {}
unsafe extern "C" fn tv_noop_range(_s: Id, _c: Sel, _r: NSRange) {}
unsafe extern "C" fn tv_noop_insets(_s: Id, _c: Sel, _i: UIEdgeInsets) {}
unsafe extern "C" fn tv_zero_insets(_s: Id, _c: Sel) -> UIEdgeInsets { UIEdgeInsets::default() }

unsafe extern "C" fn tv_kb_type(s: Id, _c: Sel) -> isize { tv_flags(s).kb_type }
unsafe extern "C" fn tv_set_kb_type(s: Id, _c: Sel, v: isize) { let mut f = tv_flags(s); f.kb_type = v; set_tv_flags(s, f); }
unsafe extern "C" fn tv_ret_type(s: Id, _c: Sel) -> isize { tv_flags(s).ret_type }
unsafe extern "C" fn tv_set_ret_type(s: Id, _c: Sel, v: isize) { let mut f = tv_flags(s); f.ret_type = v; set_tv_flags(s, f); }
unsafe extern "C" fn tv_autocap(s: Id, _c: Sel) -> isize { tv_flags(s).autocap }
unsafe extern "C" fn tv_set_autocap(s: Id, _c: Sel, v: isize) { let mut f = tv_flags(s); f.autocap = v; set_tv_flags(s, f); }
unsafe extern "C" fn tv_autocorrect(s: Id, _c: Sel) -> isize { tv_flags(s).autocorrect }
unsafe extern "C" fn tv_set_autocorrect(s: Id, _c: Sel, v: isize) { let mut f = tv_flags(s); f.autocorrect = v; set_tv_flags(s, f); }
unsafe extern "C" fn tv_spell(s: Id, _c: Sel) -> isize { tv_flags(s).spell }
unsafe extern "C" fn tv_set_spell(s: Id, _c: Sel, v: isize) { let mut f = tv_flags(s); f.spell = v; set_tv_flags(s, f); }
unsafe extern "C" fn tv_smart_q(s: Id, _c: Sel) -> isize { tv_flags(s).smart_q }
unsafe extern "C" fn tv_set_smart_q(s: Id, _c: Sel, v: isize) { let mut f = tv_flags(s); f.smart_q = v; set_tv_flags(s, f); }
unsafe extern "C" fn tv_smart_d(s: Id, _c: Sel) -> isize { tv_flags(s).smart_d }
unsafe extern "C" fn tv_set_smart_d(s: Id, _c: Sel, v: isize) { let mut f = tv_flags(s); f.smart_d = v; set_tv_flags(s, f); }
unsafe extern "C" fn tv_secure(s: Id, _c: Sel) -> bool { tv_flags(s).secure }
unsafe extern "C" fn tv_set_secure(s: Id, _c: Sel, b: bool) { let mut f = tv_flags(s); f.secure = b; set_tv_flags(s, f); }

unsafe extern "C" fn tv_can_become_first(s: Id, _c: Sel) -> bool { tv_flags(s).editable }
unsafe extern "C" fn tv_become_first(s: Id, _c: Sel) -> bool {
    if tv_flags(s).editing { return true; }
    let d = assoc_get(s, &TV_DEL_KEY);
    if !d.is_null() && responds_to(d, "textViewShouldBeginEditing:") && !msg1::<Id, bool>(d, "textViewShouldBeginEditing:", s) {
        return false;
    }
    if !super0::<bool>(s, "UITextView", "becomeFirstResponder") {
        return false;
    }
    let mut f = tv_flags(s);
    f.editing = true;
    set_tv_flags(s, f);
    post_keyboard(true);
    if !d.is_null() && responds_to(d, "textViewDidBeginEditing:") {
        msg1::<Id, ()>(d, "textViewDidBeginEditing:", s);
    }
    let nc: Id = msg0(cls("NSNotificationCenter"), "defaultCenter");
    msg2::<Id, Id, ()>(nc, "postNotificationName:object:", nsstring("UITextViewTextDidBeginEditingNotification"), s);
    true
}
unsafe extern "C" fn tv_resign_first(s: Id, _c: Sel) -> bool {
    if !tv_flags(s).editing {
        return super0(s, "UITextView", "resignFirstResponder");
    }
    let d = assoc_get(s, &TV_DEL_KEY);
    if !d.is_null() && responds_to(d, "textViewShouldEndEditing:") && !msg1::<Id, bool>(d, "textViewShouldEndEditing:", s) {
        return false;
    }
    let _: bool = super0(s, "UITextView", "resignFirstResponder");
    let mut f = tv_flags(s);
    f.editing = false;
    set_tv_flags(s, f);
    if !d.is_null() && responds_to(d, "textViewDidEndEditing:") {
        msg1::<Id, ()>(d, "textViewDidEndEditing:", s);
    }
    let nc: Id = msg0(cls("NSNotificationCenter"), "defaultCenter");
    msg2::<Id, Id, ()>(nc, "postNotificationName:object:", nsstring("UITextViewTextDidEndEditingNotification"), s);
    post_keyboard(false);
    true
}
unsafe extern "C" fn tv_remove_from_sv(s: Id, _c: Sel) {
    if tv_flags(s).editing {
        let _: bool = msg0(s, "resignFirstResponder");
    }
    super0::<()>(s, "UITextView", "removeFromSuperview");
}
unsafe fn tv_replace_range(tv: Id, range: NSRange, rep: Id) {
    if !tv_flags(tv).editable { return; }
    let buf = tv_text_buf(tv);
    let len: usize = msg0(buf, "length");
    if range.location + range.length > len { return; }
    let d = assoc_get(tv, &TV_DEL_KEY);
    if !d.is_null() && responds_to(d, "textView:shouldChangeTextInRange:replacementText:") {
        if !msg3::<Id, NSRange, Id, bool>(d, "textView:shouldChangeTextInRange:replacementText:", tv, range, rep) {
            return;
        }
    }
    msg2::<NSRange, Id, ()>(buf, "replaceCharactersInRange:withString:", range, rep);
    let rep_len: usize = msg0(rep, "length");
    set_tv_sel(tv, NSRange { location: range.location + rep_len, length: 0 });
    if !d.is_null() {
        if responds_to(d, "textViewDidChange:") { msg1::<Id, ()>(d, "textViewDidChange:", tv); }
        if responds_to(d, "textViewDidChangeSelection:") { msg1::<Id, ()>(d, "textViewDidChangeSelection:", tv); }
    }
    let nc: Id = msg0(cls("NSNotificationCenter"), "defaultCenter");
    msg2::<Id, Id, ()>(nc, "postNotificationName:object:", nsstring("UITextViewTextDidChangeNotification"), tv);
}
unsafe extern "C" fn tv_insert_text(s: Id, _c: Sel, text: Id) {
    tv_replace_range(s, tv_sel(s), if text.is_null() { nsstring("") } else { text });
}
unsafe extern "C" fn tv_delete_backward(s: Id, _c: Sel) {
    let mut r = tv_sel(s);
    if r.length == 0 {
        if r.location == 0 { return; }
        r = msg1(tv_text_buf(s), "rangeOfComposedCharacterSequenceAtIndex:", r.location - 1);
    }
    tv_replace_range(s, r, nsstring(""));
}
unsafe extern "C" fn tv_move_cursor(s: Id, _c: Sel, delta: isize) {
    let cur = tv_sel(s).location as isize;
    let loc = (cur + delta).max(0) as usize;
    set_tv_sel(s, NSRange { location: loc, length: 0 });
    let d = assoc_get(s, &TV_DEL_KEY);
    if !d.is_null() && responds_to(d, "textViewDidChangeSelection:") {
        msg1::<Id, ()>(d, "textViewDidChangeSelection:", s);
    }
}

unsafe fn is_text_responder(r: Id) -> bool {
    !r.is_null() && responds_to(r, "insertText:") && responds_to(r, "deleteBackward") && responds_to(r, "hasText")
}

#[no_mangle]
pub extern "C" fn PearUIKitTextInputActive() -> bool {
    unsafe { is_text_responder(PearUIKitFirstResponder()) }
}

#[no_mangle]
pub extern "C" fn PearUIKitDeliverText(text: Id) {
    unsafe {
        let r = PearUIKitFirstResponder();
        if is_text_responder(r) {
            msg1::<Id, ()>(r, "insertText:", text);
        }
    }
}

#[no_mangle]
pub extern "C" fn PearUIKitDeliverEditCommand(cmd: c_int) {
    unsafe {
        let r = PearUIKitFirstResponder();
        if !is_text_responder(r) { return; }
        match cmd {
            0 => msg0::<()>(r, "deleteBackward"),
            1 => PearUIKitDeliverText(nsstring("\n")),
            2 => if is_kind_of(r, "UITextView") { msg1::<isize, ()>(r, "pear_moveCursor:", -1); },
            3 => if is_kind_of(r, "UITextView") { msg1::<isize, ()>(r, "pear_moveCursor:", 1); },
            4 => { let _: bool = msg0(r, "resignFirstResponder"); },
            _ => {}
        }
    }
}

fn press_type_for_hid(usage: isize) -> isize {
    match usage {
        0x52 => 0,
        0x51 => 1,
        0x50 => 2,
        0x4F => 3,
        0x28 => 4,
        0x29 => 5,
        _ => 1000,
    }
}

#[no_mangle]
pub extern "C" fn PearUIKitDeliverKey(
    hid_usage: isize,
    chars: Id,
    chars_no_mods: Id,
    modifiers: isize,
    down: bool,
    timestamp: f64,
) {
    unsafe {
        let key: Id = msg4(
            msg0(cls("UIKey"), "alloc"),
            "initWithCharacters:ignoringModifiers:modifiers:keyCode:",
            chars,
            chars_no_mods,
            modifiers,
            hid_usage,
        );
        let press: Id = msg4(
            msg0(cls("UIPress"), "alloc"),
            "initWithKey:type:phase:timestamp:",
            key,
            press_type_for_hid(hid_usage),
            if down { 0isize } else { 3isize },
            timestamp,
        );
        let set: Id = msg1(cls("NSSet"), "setWithObject:", press);
        let ev: Id = msg2(msg0(cls("UIPressesEvent"), "alloc"), "initWithPresses:timestamp:", set, timestamp);
        let mut target = PearUIKitFirstResponder();
        if target.is_null() {
            let w = PearUIKitKeyWindow();
            let vc: Id = if w.is_null() { NIL } else { msg0(w, "rootViewController") };
            let v: Id = if vc.is_null() { NIL } else { msg0(vc, "viewIfLoaded") };
            target = if !v.is_null() { v } else if !vc.is_null() { vc } else { w };
        }
        if !target.is_null() {
            if down {
                msg2::<Id, Id, ()>(target, "pressesBegan:withEvent:", set, ev);
            } else {
                msg2::<Id, Id, ()>(target, "pressesEnded:withEvent:", set, ev);
            }
        }
        objc_release(ev);
        objc_release(press);
        objc_release(key);
    }
}

// ============================================================================
// UINib & PearNibDecoder (powered by pear_nib::NibArchive)
// ============================================================================

struct NibDecoderState {
    archive: NibArchive,
    owner: Id,
    external: Id,
    current: u32,
    decoded: HashMap<u32, Id>,
    all_objects: Vec<Id>,
}

impl NibDecoderState {
    fn value_for_key(&self, key: &str) -> Option<&NibValue> {
        let o = self.archive.objects.get(self.current as usize)?;
        let start = o.first as usize;
        let end = start + o.count as usize;
        for pv in self.archive.values.get(start..end)? {
            if self.archive.keys.get(pv.key as usize).map(|s| s.as_str()) == Some(key) {
                return Some(pv);
            }
        }
        None
    }

    fn number_of(v: Option<&NibValue>) -> f64 {
        match v.map(|x| &x.payload) {
            Some(NibValuePayload::Double(d)) => *d,
            Some(NibValuePayload::Int(i)) => *i as f64,
            _ => 0.0,
        }
    }

    fn integer_of(v: Option<&NibValue>) -> i64 {
        match v.map(|x| &x.payload) {
            Some(NibValuePayload::Double(d)) => *d as i64,
            Some(NibValuePayload::Int(i)) => *i,
            _ => 0,
        }
    }

    fn doubles_for_key(&self, key: &str, count: usize) -> Option<Vec<f64>> {
        let v = self.value_for_key(key)?;
        if let NibValuePayload::Bytes(b) = &v.payload {
            if b.len() >= 1 + 8 * count && b[0] == 7 {
                let mut out = Vec::with_capacity(count);
                for k in 0..count {
                    let s: [u8; 8] = b[1 + k * 8..1 + (k + 1) * 8].try_into().ok()?;
                    out.push(f64::from_le_bytes(s));
                }
                return Some(out);
            }
        }
        None
    }

    unsafe fn decode_object(&mut self, decoder_obj: Id, key: &str) -> Id {
        let payload = match self.value_for_key(key) {
            Some(v) => (v.value_type, v.payload.clone()),
            None => return NIL,
        };
        match payload.1 {
            NibValuePayload::Ref(r) => self.object_at(decoder_obj, r),
            NibValuePayload::Bytes(b) => nsdata_from_bytes(&b),
            NibValuePayload::Int(i) => {
                if payload.0 == 4 || payload.0 == 5 {
                    nsnumber_bool(i != 0)
                } else {
                    nsnumber_i64(i)
                }
            }
            NibValuePayload::Double(d) => nsnumber_f64(d),
            NibValuePayload::Null => NIL,
        }
    }

    unsafe fn object_at(&mut self, decoder_obj: Id, index: u32) -> Id {
        if (index as usize) >= self.archive.objects.len() {
            return NIL;
        }
        if let Some(&cached) = self.decoded.get(&index) {
            return cached;
        }
        let saved = self.current;
        self.current = index;
        let cls_idx = self.archive.objects[index as usize].cls as usize;
        let cls_name = self.archive.classes.get(cls_idx).cloned().unwrap_or_default();
        let mut obj: Id = NIL;

        if cls_name == "NSString" || cls_name == "NSMutableString" {
            if let Some(v) = self.value_for_key("NS.bytes") {
                if let NibValuePayload::Bytes(b) = &v.payload {
                    let s = String::from_utf8_lossy(b);
                    obj = nsstring(&s);
                }
            }
            if obj.is_null() {
                obj = nsstring("");
            }
        } else if cls_name == "NSArray" || cls_name == "NSMutableArray" {
            let o = self.archive.objects[index as usize];
            let start = o.first as usize;
            let end = start + o.count as usize;
            let mut refs = Vec::new();
            for pv in &self.archive.values[start..end] {
                if let NibValuePayload::Ref(r) = pv.payload {
                    if self.archive.keys.get(pv.key as usize).map(|s| s.as_str()) == Some("UINibEncoderEmptyKey") {
                        refs.push(r);
                    }
                }
            }
            let mut elems = Vec::new();
            for r in refs {
                let e = self.object_at(decoder_obj, r);
                if !e.is_null() { elems.push(e); }
            }
            obj = nsarray_from_ids(&elems);
        } else if cls_name == "NSNumber" {
            let iv = self.decode_object(decoder_obj, "NS.intval");
            obj = if !iv.is_null() {
                iv
            } else {
                let dv = self.decode_object(decoder_obj, "NS.dblval");
                if !dv.is_null() { dv } else { nsnumber_i64(0) }
            };
        } else if cls_name == "UIColor" {
            let alpha = if self.value_for_key("UIAlpha").is_some() {
                Self::number_of(self.value_for_key("UIAlpha"))
            } else {
                1.0
            };
            let r = Self::number_of(self.value_for_key("UIRed"));
            let g = Self::number_of(self.value_for_key("UIGreen"));
            let b = Self::number_of(self.value_for_key("UIBlue"));
            obj = msg4(cls("UIColor"), "colorWithRed:green:blue:alpha:", r, g, b, alpha);
        } else if cls_name == "UIProxyObject" {
            let ref_idx = match self.value_for_key("UIProxiedObjectIdentifier").map(|v| &v.payload) {
                Some(NibValuePayload::Ref(r)) => Some(*r),
                _ => None,
            };
            let ident = ref_idx.and_then(|r| nsstring_to_string(self.object_at(decoder_obj, r)));
            self.current = index;
            if ident.as_deref() == Some("IBFilesOwner") {
                obj = self.owner;
            } else if ident.as_deref() == Some("IBFirstResponder") {
                obj = NIL;
            } else if let Some(ref id_str) = ident {
                if !self.external.is_null() {
                    obj = msg1(self.external, "objectForKey:", nsstring(id_str));
                }
            }
        } else if cls_name == "UIClassSwapper" || cls_name == "UICustomObject" {
            let cn_ref = match self.value_for_key("UIClassName").map(|v| &v.payload) {
                Some(NibValuePayload::Ref(r)) => Some(*r),
                _ => None,
            };
            let on_ref = match self.value_for_key("UIOriginalClassName").map(|v| &v.payload) {
                Some(NibValuePayload::Ref(r)) => Some(*r),
                _ => None,
            };
            let class_name = cn_ref.and_then(|r| nsstring_to_string(self.object_at(decoder_obj, r)));
            let orig_name = on_ref.and_then(|r| nsstring_to_string(self.object_at(decoder_obj, r)));
            self.current = index;
            let c = class_name.as_deref().map(cls).filter(|p| !p.is_null())
                .or_else(|| orig_name.as_deref().map(cls).filter(|p| !p.is_null()));
            if let Some(c) = c {
                if cls_name == "UICustomObject"
                    || orig_name.as_deref() == Some("UICustomObject")
                    || !class_getInstanceMethod(c, sel("initWithCoder:")).is_null() == false
                {
                    obj = objc_autorelease(msg0(c as Id, "new"));
                } else {
                    obj = objc_autorelease(msg1(msg0(c as Id, "alloc"), "initWithCoder:", decoder_obj));
                }
            } else {
                let c1 = CString::new("UINib").unwrap();
                let c2 = CString::new(format!("class {} not found", class_name.as_deref().unwrap_or("(null)"))).unwrap();
                PearMissingReport(1, c1.as_ptr(), c2.as_ptr(), std::ptr::null());
            }
        } else {
            let c = cls(&cls_name);
            if !c.is_null() {
                if !class_getInstanceMethod(c, sel("initWithCoder:")).is_null() {
                    obj = objc_autorelease(msg1(msg0(c as Id, "alloc"), "initWithCoder:", decoder_obj));
                } else {
                    obj = objc_autorelease(msg0(c as Id, "new"));
                }
            } else {
                let c1 = CString::new("UINib").unwrap();
                let c2 = CString::new(format!("archived class {cls_name} not found")).unwrap();
                PearMissingReport(1, c1.as_ptr(), c2.as_ptr(), std::ptr::null());
            }
        }

        self.current = saved;
        if !obj.is_null() {
            objc_retain(obj);
        }
        self.decoded.insert(index, obj);
        if !obj.is_null() && !cls_name.starts_with("NS") && cls_name != "UIProxyObject" {
            self.all_objects.push(obj);
        }
        obj
    }
}

static NIB_DEC_PTR_KEY: u8 = 0;

unsafe fn dec_state<'a>(obj: Id) -> Option<&'a mut NibDecoderState> {
    let val = assoc_get(obj, &NIB_DEC_PTR_KEY);
    if val.is_null() { None } else {
        let ptr = msg0::<*mut NibDecoderState>(val, "pointerValue");
        ptr.as_mut()
    }
}

unsafe extern "C" fn dec_dealloc(s: Id, _c: Sel) {
    let val = assoc_get(s, &NIB_DEC_PTR_KEY);
    if !val.is_null() {
        let ptr = msg0::<*mut NibDecoderState>(val, "pointerValue");
        if !ptr.is_null() {
            let st = Box::from_raw(ptr);
            for (_, obj) in st.decoded {
                if !obj.is_null() { objc_release(obj); }
            }
        }
    }
    let mut sup = ObjcSuper { receiver: s, super_class: cls("NSCoder") };
    let f: unsafe extern "C" fn(&mut ObjcSuper, Sel) = std::mem::transmute(objc_msgSendSuper as *const c_void);
    f(&mut sup, sel("dealloc"));
}
unsafe extern "C" fn dec_allows_keyed(_s: Id, _c: Sel) -> bool { true }
unsafe extern "C" fn dec_req_secure(_s: Id, _c: Sel) -> bool { false }
unsafe extern "C" fn dec_version(_s: Id, _c: Sel, _n: Id) -> isize { 0 }
unsafe extern "C" fn dec_contains(s: Id, _c: Sel, key: Id) -> bool {
    let k = nsstring_to_string(key).unwrap_or_default();
    dec_state(s).map_or(false, |st| st.value_for_key(&k).is_some())
}
unsafe extern "C" fn dec_bool(s: Id, _c: Sel, key: Id) -> bool {
    let k = nsstring_to_string(key).unwrap_or_default();
    dec_state(s).map_or(false, |st| NibDecoderState::integer_of(st.value_for_key(&k)) != 0)
}
unsafe extern "C" fn dec_i32(s: Id, _c: Sel, key: Id) -> i32 {
    let k = nsstring_to_string(key).unwrap_or_default();
    dec_state(s).map_or(0, |st| NibDecoderState::integer_of(st.value_for_key(&k)) as i32)
}
unsafe extern "C" fn dec_i64(s: Id, _c: Sel, key: Id) -> i64 {
    let k = nsstring_to_string(key).unwrap_or_default();
    dec_state(s).map_or(0, |st| NibDecoderState::integer_of(st.value_for_key(&k)))
}
unsafe extern "C" fn dec_int(s: Id, _c: Sel, key: Id) -> isize {
    let k = nsstring_to_string(key).unwrap_or_default();
    dec_state(s).map_or(0, |st| NibDecoderState::integer_of(st.value_for_key(&k)) as isize)
}
unsafe extern "C" fn dec_f32(s: Id, _c: Sel, key: Id) -> f32 {
    let k = nsstring_to_string(key).unwrap_or_default();
    dec_state(s).map_or(0.0, |st| NibDecoderState::number_of(st.value_for_key(&k)) as f32)
}
unsafe extern "C" fn dec_f64(s: Id, _c: Sel, key: Id) -> f64 {
    let k = nsstring_to_string(key).unwrap_or_default();
    dec_state(s).map_or(0.0, |st| NibDecoderState::number_of(st.value_for_key(&k)))
}
unsafe extern "C" fn dec_rect(s: Id, _c: Sel, key: Id) -> CGRect {
    let k = nsstring_to_string(key).unwrap_or_default();
    dec_state(s).and_then(|st| st.doubles_for_key(&k, 4)).map_or(CGRect::default(), |d| CGRect {
        origin: CGPoint { x: d[0], y: d[1] },
        size: CGSize { width: d[2], height: d[3] },
    })
}
unsafe extern "C" fn dec_point(s: Id, _c: Sel, key: Id) -> CGPoint {
    let k = nsstring_to_string(key).unwrap_or_default();
    dec_state(s).and_then(|st| st.doubles_for_key(&k, 2)).map_or(CGPoint::default(), |d| CGPoint { x: d[0], y: d[1] })
}
unsafe extern "C" fn dec_size(s: Id, _c: Sel, key: Id) -> CGSize {
    let k = nsstring_to_string(key).unwrap_or_default();
    dec_state(s).and_then(|st| st.doubles_for_key(&k, 2)).map_or(CGSize::default(), |d| CGSize { width: d[0], height: d[1] })
}
unsafe extern "C" fn dec_obj(s: Id, _c: Sel, key: Id) -> Id {
    let k = nsstring_to_string(key).unwrap_or_default();
    if let Some(st) = dec_state(s) {
        st.decode_object(s, &k)
    } else {
        NIL
    }
}

static NIB_DATA_KEY: u8 = 0;
static NIB_NAME_KEY: u8 = 0;

unsafe extern "C" fn nib_with_name(_cls: Id, _c: Sel, name: Id, bundle: Id) -> Id {
    let name_str = nsstring_to_string(name).unwrap_or_default();
    let b = if bundle.is_null() { msg0(cls("NSBundle"), "mainBundle") } else { bundle };
    let fm: Id = msg0(cls("NSFileManager"), "defaultManager");
    let mut found_path: Id = NIL;
    for cand in [format!("{name_str}~ipad"), name_str.clone()] {
        let p: Id = msg2(b, "pathForResource:ofType:", nsstring(&cand), nsstring("nib"));
        if p.is_null() { continue; }
        let mut is_dir = false;
        let exists: bool = msg2(fm, "fileExistsAtPath:isDirectory:", p, &mut is_dir as *mut bool);
        if exists && is_dir {
            let mut inner: Id = msg1(p, "stringByAppendingPathComponent:", nsstring("runtime.nib"));
            if !msg1::<Id, bool>(fm, "fileExistsAtPath:", inner) {
                inner = msg1(p, "stringByAppendingPathComponent:", nsstring("objects-13.0+.nib"));
            }
            found_path = inner;
        } else {
            found_path = p;
        }
        break;
    }
    let data: Id = if found_path.is_null() { NIL } else { msg1(cls("NSData"), "dataWithContentsOfFile:", found_path) };
    if data.is_null() {
        log_msg("nib", &format!("nib {name_str} not found"));
        return NIL;
    }
    let nib: Id = msg2(cls("UINib"), "nibWithData:bundle:", data, bundle);
    assoc_set(nib, &NIB_NAME_KEY, name);
    nib
}

unsafe extern "C" fn nib_with_data(_cls: Id, _c: Sel, data: Id, _bundle: Id) -> Id {
    let nib: Id = msg0(cls("UINib"), "new");
    assoc_set(nib, &NIB_DATA_KEY, data);
    objc_autorelease(nib)
}

unsafe extern "C" fn nib_inst(s: Id, _c: Sel, owner: Id, opts: Id) -> Id {
    let mut unused: Id = NIL;
    msg3(s, "pear_instantiateWithOwner:options:visibleWindows:", owner, opts, &mut unused as *mut Id)
}

unsafe extern "C" fn nib_pear_inst(s: Id, _c: Sel, owner: Id, opts: Id, vis_wins: *mut Id) -> Id {
    let data = assoc_get(s, &NIB_DATA_KEY);
    let name_str = nsstring_to_string(assoc_get(s, &NIB_NAME_KEY)).unwrap_or_else(|| "(data)".to_string());
    let archive = nsdata_to_vec(data).and_then(|b| NibArchive::parse(&b)).filter(|a| !a.objects.is_empty());
    let Some(archive) = archive else {
        let c1 = CString::new("UINib").unwrap();
        let c2 = CString::new(format!("{name_str}: not a NIBArchive (other nib formats are not supported yet)")).unwrap();
        PearMissingReport(1, c1.as_ptr(), c2.as_ptr(), std::ptr::null());
        return msg0(cls("NSArray"), "array");
    };
    let ext: Id = if opts.is_null() { NIL } else { msg1(opts, "objectForKey:", nsstring("UINibExternalObjects")) };
    let dec: Id = msg0(cls("PearNibDecoder"), "new");
    let state = Box::new(NibDecoderState {
        archive,
        owner,
        external: ext,
        current: 0,
        decoded: HashMap::new(),
        all_objects: Vec::new(),
    });
    let state_ptr = Box::into_raw(state);
    assoc_set(dec, &NIB_DEC_PTR_KEY, msg1(cls("NSValue"), "valueWithPointer:", state_ptr as *const c_void));

    let top: Id = msg1(dec, "decodeObjectForKey:", nsstring("UINibTopLevelObjectsKey"));
    (*state_ptr).current = 0;
    let _: Id = msg1(dec, "decodeObjectForKey:", nsstring("UINibObjectsKey"));
    (*state_ptr).current = 0;
    let conns: Id = msg1(dec, "decodeObjectForKey:", nsstring("UINibConnectionsKey"));
    (*state_ptr).current = 0;
    let wins: Id = msg1(dec, "decodeObjectForKey:", nsstring("UINibVisibleWindowsKey"));

    for i in 0..nsarray_count(conns) {
        let c = nsarray_object_at(conns, i);
        if is_kind_of(c, "UIRuntimeConnection") {
            msg0::<()>(c, "pear_connect");
        }
    }
    for &o in &(*state_ptr).all_objects {
        if o != owner && responds_to(o, "awakeFromNib") {
            msg0::<()>(o, "awakeFromNib");
        }
    }
    log_msg("nib", &format!("{name_str}: {} top-level objects, {} connections, {} visible window(s)",
        nsarray_count(top), nsarray_count(conns), nsarray_count(wins)));
    if !vis_wins.is_null() {
        *vis_wins = if wins.is_null() { msg0(cls("NSArray"), "array") } else { wins };
    }
    let mut filtered = Vec::new();
    for i in 0..nsarray_count(top) {
        let o = nsarray_object_at(top, i);
        if o != owner { filtered.push(o); }
    }
    let ret = nsarray_from_ids(&filtered);
    objc_release(dec);
    ret
}

static RC_SRC_KEY: u8 = 0;
static RC_DST_KEY: u8 = 0;
static RC_LBL_KEY: u8 = 0;

unsafe extern "C" fn rc_init_coder(s: Id, _c: Sel, coder: Id) -> Id {
    let o: Id = super0(s, "UIRuntimeConnection", "init");
    if !o.is_null() && !coder.is_null() {
        assoc_set(o, &RC_SRC_KEY, msg1(coder, "decodeObjectForKey:", nsstring("UISource")));
        assoc_set(o, &RC_DST_KEY, msg1(coder, "decodeObjectForKey:", nsstring("UIDestination")));
        assoc_set(o, &RC_LBL_KEY, msg1(coder, "decodeObjectForKey:", nsstring("UILabel")));
    }
    o
}
unsafe extern "C" fn rc_encode(_s: Id, _c: Sel, _coder: Id) {}
unsafe extern "C" fn rc_noop(_s: Id, _c: Sel) {}
unsafe extern "C" fn roc_connect(s: Id, _c: Sel) {
    let src = assoc_get(s, &RC_SRC_KEY);
    let dst = assoc_get(s, &RC_DST_KEY);
    let lbl = assoc_get(s, &RC_LBL_KEY);
    if src.is_null() || lbl.is_null() { return; }
    if !PearUIKitTrySetValueForKey(src, dst, lbl) {
        let cname = class_getName(object_getClass(src));
        let ls = nsstring_to_string(lbl).unwrap_or_default();
        let m = CString::new(format!("nib outlet {ls} could not be set")).unwrap();
        PearMissingReport(1, cname, m.as_ptr(), std::ptr::null());
    }
}
unsafe extern "C" fn rocc_connect(_s: Id, _c: Sel) {
    let c1 = CString::new("UINib").unwrap();
    let c2 = CString::new("outlet collections").unwrap();
    PearMissingReport(1, c1.as_ptr(), c2.as_ptr(), std::ptr::null());
}
unsafe extern "C" fn rec_connect(_s: Id, _c: Sel) {
    let c1 = CString::new("UINib").unwrap();
    let c2 = CString::new("target-action (event) connections").unwrap();
    PearMissingReport(1, c1.as_ptr(), c2.as_ptr(), std::ptr::null());
}

fn parse_nums(s: &str, count: usize) -> Vec<f64> {
    let mut out = Vec::with_capacity(count);
    for tok in s.split(|c: char| c == '{' || c == '}' || c == ',' || c.is_whitespace()) {
        if let Ok(v) = tok.parse::<f64>() {
            out.push(v);
            if out.len() == count { break; }
        }
    }
    while out.len() < count { out.push(0.0); }
    out
}
unsafe extern "C" fn coder_enc_pt(s: Id, _c: Sel, pt: CGPoint, key: Id) {
    msg2::<Id, Id, ()>(s, "encodeObject:forKey:", nsstring(&format!("{{{}, {}}}", pt.x, pt.y)), key);
}
unsafe extern "C" fn coder_enc_sz(s: Id, _c: Sel, sz: CGSize, key: Id) {
    msg2::<Id, Id, ()>(s, "encodeObject:forKey:", nsstring(&format!("{{{}, {}}}", sz.width, sz.height)), key);
}
unsafe extern "C" fn coder_enc_rect(s: Id, _c: Sel, r: CGRect, key: Id) {
    msg2::<Id, Id, ()>(s, "encodeObject:forKey:", nsstring(&format!("{{{{{}, {}}}, {{{}, {}}}}}", r.origin.x, r.origin.y, r.size.width, r.size.height)), key);
}
unsafe extern "C" fn coder_dec_pt(s: Id, _c: Sel, key: Id) -> CGPoint {
    let str_v = nsstring_to_string(msg1(s, "decodeObjectForKey:", key)).unwrap_or_default();
    let d = parse_nums(&str_v, 2);
    CGPoint { x: d[0], y: d[1] }
}
unsafe extern "C" fn coder_dec_sz(s: Id, _c: Sel, key: Id) -> CGSize {
    let str_v = nsstring_to_string(msg1(s, "decodeObjectForKey:", key)).unwrap_or_default();
    let d = parse_nums(&str_v, 2);
    CGSize { width: d[0], height: d[1] }
}
unsafe extern "C" fn coder_dec_rect(s: Id, _c: Sel, key: Id) -> CGRect {
    let str_v = nsstring_to_string(msg1(s, "decodeObjectForKey:", key)).unwrap_or_default();
    let d = parse_nums(&str_v, 4);
    CGRect { origin: CGPoint { x: d[0], y: d[1] }, size: CGSize { width: d[2], height: d[3] } }
}
unsafe extern "C" fn bundle_load_nib(s: Id, _c: Sel, name: Id, owner: Id, opts: Id) -> Id {
    let nib: Id = msg2(cls("UINib"), "nibWithNibName:bundle:", name, s);
    if nib.is_null() { NIL } else { msg2(nib, "instantiateWithOwner:options:", owner, opts) }
}

// ============================================================================
// AppKit Backend (PearBackend* + PearHostView + PearHostWindowDelegate + PearHostAppDelegate)
// ============================================================================

static G_NATIVE_WINDOWS: Mutex<Vec<usize>> = Mutex::new(Vec::new());
static G_PRIMARY_VIEW: AtomicUsize = AtomicUsize::new(0);
static G_SCREEN: Mutex<PearHostScreenInfo> = Mutex::new(PearHostScreenInfo {
    point_size: CGSize { width: 0.0, height: 0.0 },
    scale: 2.0,
    max_fps: 60,
});
static G_IDLE_ASSERTION: AtomicUsize = AtomicUsize::new(0);
static G_APP_DELEGATE: AtomicUsize = AtomicUsize::new(0);
static G_DID_FINISH_CB: Mutex<Option<(usize, extern "C" fn(*mut c_void))>> = Mutex::new(None);

unsafe fn default_point_size() -> CGSize {
    if let Ok(val) = std::env::var("PEAR_HOST_SCREEN") {
        let parts: Vec<&str> = val.split('x').collect();
        if parts.len() == 2 {
            if let (Ok(w), Ok(h)) = (parts[0].parse::<i32>(), parts[1].parse::<i32>()) {
                if w > 0 && h > 0 {
                    return CGSize { width: w as f64, height: h as f64 };
                }
            }
        }
    }
    let main_scr: Id = msg0(cls("NSScreen"), "mainScreen");
    let vis: CGRect = if main_scr.is_null() {
        CGRect { origin: CGPoint::default(), size: CGSize { width: 1280.0, height: 800.0 } }
    } else {
        msg0(main_scr, "visibleFrame")
    };
    let mut width = (vis.size.width * 0.8).floor();
    let mut height = (width * 9.0 / 16.0).floor();
    if height > vis.size.height * 0.85 {
        height = (vis.size.height * 0.85).floor();
        width = (height * 16.0 / 9.0).floor();
    }
    CGSize { width, height }
}

#[no_mangle]
pub extern "C" fn PearBackendInitialize() {
    unsafe {
        let ns_app: Id = msg0(cls("NSApplication"), "sharedApplication");
        msg1::<isize, bool>(ns_app, "setActivationPolicy:", 0);
        G_NATIVE_WINDOWS.lock().unwrap().clear();
        let main_scr: Id = msg0(cls("NSScreen"), "mainScreen");
        let scale: f64 = if main_scr.is_null() { 2.0 } else { msg0(main_scr, "backingScaleFactor") };
        let max_fps: isize = if main_scr.is_null() { 60 } else { msg0(main_scr, "maximumFramesPerSecond") };
        let info = PearHostScreenInfo {
            point_size: default_point_size(),
            scale: if scale > 0.0 { scale } else { 2.0 },
            max_fps: if max_fps > 0 { max_fps } else { 60 },
        };
        *G_SCREEN.lock().unwrap() = info;
        PearBackendInstallQuartzCoreCompat();

        let obs: Id = msg0(cls("PearHostAppObserver"), "new");
        let nc: Id = msg0(cls("NSNotificationCenter"), "defaultCenter");
        for (notif, sel_name) in [
            ("NSApplicationDidBecomeActiveNotification", "onBecomeActive:"),
            ("NSApplicationWillResignActiveNotification", "onResignActive:"),
            ("NSApplicationDidHideNotification", "onDidHide:"),
            ("NSApplicationWillUnhideNotification", "onWillUnhide:"),
            ("NSApplicationWillTerminateNotification", "onWillTerminate:"),
        ] {
            msg4::<Id, Sel, Id, Id, ()>(nc, "addObserver:selector:name:object:", obs, sel(sel_name), nsstring(notif), NIL);
        }

        let w = info.point_size.width.round() as isize;
        let h = info.point_size.height.round() as isize;
        log_msg("host", &format!("AppKit backend: screen {w}x{h} pt @{:.1}x, {} Hz", info.scale, info.max_fps));
    }
}

#[no_mangle]
pub extern "C" fn PearBackendScreenInfo() -> PearHostScreenInfo {
    *G_SCREEN.lock().unwrap()
}

static HV_UIWIN_KEY: u8 = 0;
static WD_UIWIN_KEY: u8 = 0;
static W_DEL_KEY: u8 = 0;

#[no_mangle]
pub extern "C" fn PearBackendCreateWindow(ui_window: Id, frame: CGRect) -> *mut c_void {
    unsafe {
        let content = CGRect { origin: CGPoint::default(), size: frame.size };
        let w: Id = msg4(
            msg0(cls("NSWindow"), "alloc"),
            "initWithContentRect:styleMask:backing:defer:",
            content,
            (1usize << 0) | (1usize << 1) | (1usize << 2) | (1usize << 3), // titled|closable|miniaturizable|resizable
            2usize, // buffered
            false,
        );
        let info: Id = msg0(msg0(cls("NSBundle"), "mainBundle"), "infoDictionary");
        let disp: Id = msg1(info, "objectForKey:", nsstring("CFBundleDisplayName"));
        let title = if !disp.is_null() {
            disp
        } else {
            msg0(msg0(cls("NSProcessInfo"), "processInfo"), "processName")
        };
        msg1::<Id, ()>(w, "setTitle:", title);
        msg1::<bool, ()>(w, "setReleasedWhenClosed:", false);
        msg1::<bool, ()>(w, "setAcceptsMouseMovedEvents:", true);
        let cb: usize = msg0(w, "collectionBehavior");
        msg1::<usize, ()>(w, "setCollectionBehavior:", cb | (1usize << 7));
        msg1::<CGSize, ()>(w, "setContentAspectRatio:", content.size);

        let v: Id = msg1(msg0(cls("PearHostView"), "alloc"), "initWithFrame:", content);
        assoc_set(v, &HV_UIWIN_KEY, ui_window);
        msg1::<Id, ()>(w, "setContentView:", v);
        msg1::<Id, bool>(w, "makeFirstResponder:", v);

        let d: Id = msg0(cls("PearHostWindowDelegate"), "new");
        assoc_set(d, &WD_UIWIN_KEY, ui_window);
        msg1::<Id, ()>(w, "setDelegate:", d);
        assoc_set(w, &W_DEL_KEY, d);
        objc_release(d);

        msg0::<()>(w, "center");
        G_NATIVE_WINDOWS.lock().unwrap().push(w as usize);
        let _ = G_PRIMARY_VIEW.compare_exchange(0, v as usize, Ordering::AcqRel, Ordering::Acquire);
        objc_release(v);
        w as *mut c_void
    }
}

#[no_mangle]
pub extern "C" fn PearBackendSetRootLayer(handle: *mut c_void, layer: Id) {
    unsafe {
        if handle.is_null() { return; }
        let w = handle as Id;
        let v: Id = msg0(w, "contentView");
        if v.is_null() { return; }
        msg1::<Id, ()>(v, "setLayer:", layer);
        msg1::<bool, ()>(v, "setWantsLayer:", true);
        let scale: f64 = msg0(w, "backingScaleFactor");
        msg1::<f64, ()>(layer, "setContentsScale:", scale);
    }
}

#[no_mangle]
pub extern "C" fn PearBackendShowWindow(handle: *mut c_void, key: bool) {
    unsafe {
        if handle.is_null() { return; }
        let w = handle as Id;
        if key {
            msg1::<Id, ()>(w, "makeKeyAndOrderFront:", NIL);
        } else {
            msg1::<Id, ()>(w, "orderFront:", NIL);
        }
        let ns_app: Id = msg0(cls("NSApplication"), "sharedApplication");
        msg1::<bool, ()>(ns_app, "activateIgnoringOtherApps:", true);
    }
}

#[no_mangle]
pub extern "C" fn PearBackendDisplayLink(target: Id, selector: Sel) -> Id {
    unsafe {
        let pv = G_PRIMARY_VIEW.load(Ordering::Acquire) as Id;
        if !pv.is_null() {
            return msg2(pv, "displayLinkWithTarget:selector:", target, selector);
        }
        let scr: Id = msg0(cls("NSScreen"), "mainScreen");
        if scr.is_null() { NIL } else { msg2(scr, "displayLinkWithTarget:selector:", target, selector) }
    }
}

unsafe extern "C" fn dl_factory(_cls: Id, _c: Sel, target: Id, selector: Sel) -> Id {
    PearBackendDisplayLink(target, selector)
}
static DL_FI_KEY: u8 = 0;
static DL_PFPS_KEY: u8 = 0;
unsafe fn dl_apply_rate(link: Id, raw_fps: f64) {
    let max_fps = PearBackendScreenInfo().max_fps as f64;
    let fps = if raw_fps <= 0.0 || raw_fps > max_fps { max_fps } else { raw_fps } as f32;
    let r = CAFrameRateRange { minimum: fps, maximum: fps, preferred: fps };
    msg1::<CAFrameRateRange, ()>(link, "setPreferredFrameRateRange:", r);
}
unsafe extern "C" fn dl_fi(s: Id, _c: Sel) -> isize {
    let n = assoc_get(s, &DL_FI_KEY);
    if n.is_null() { 1 } else { msg0(n, "integerValue") }
}
unsafe extern "C" fn dl_set_fi(s: Id, _c: Sel, raw: isize) {
    let iv = raw.max(1);
    assoc_set(s, &DL_FI_KEY, nsnumber_i64(iv as i64));
    dl_apply_rate(s, (PearBackendScreenInfo().max_fps as f64) / (iv as f64));
}
unsafe extern "C" fn dl_pfps(s: Id, _c: Sel) -> isize {
    let n = assoc_get(s, &DL_PFPS_KEY);
    if n.is_null() { 0 } else { msg0(n, "integerValue") }
}
unsafe extern "C" fn dl_set_pfps(s: Id, _c: Sel, fps: isize) {
    assoc_set(s, &DL_PFPS_KEY, nsnumber_i64(fps as i64));
    dl_apply_rate(s, fps as f64);
}

#[no_mangle]
pub extern "C" fn PearBackendInstallQuartzCoreCompat() {
    unsafe {
        let dl_cls = cls("CADisplayLink");
        if dl_cls.is_null() {
            let c1 = CString::new("QuartzCore").unwrap();
            let c2 = CString::new("CADisplayLink class absent (macOS < 14)").unwrap();
            PearMissingReport(1, c1.as_ptr(), c2.as_ptr(), std::ptr::null());
            return;
        }
        if !responds_to(dl_cls as Id, "displayLinkWithTarget:selector:") {
            add_class_method(dl_cls, "displayLinkWithTarget:selector:", dl_factory as Imp, "@@:@:");
            log_msg("host", "+[CADisplayLink displayLinkWithTarget:selector:] provided by the host");
        }
        let scr: Id = msg0(cls("NSScreen"), "mainScreen");
        if scr.is_null() { return; }
        let probe: Id = msg2(scr, "displayLinkWithTarget:selector:", msg0::<Id>(cls("NSNull"), "null"), sel("classForCoder"));
        if probe.is_null() { return; }
        let concrete = object_getClass(probe);
        let table: [(&str, Imp, &str, bool); 4] = [
            ("frameInterval", dl_fi as Imp, "q@:", false),
            ("setFrameInterval:", dl_set_fi as Imp, "v@:q", true),
            ("preferredFramesPerSecond", dl_pfps as Imp, "q@:", false),
            ("setPreferredFramesPerSecond:", dl_set_pfps as Imp, "v@:q", true),
        ];
        for (sname, imp, types, is_setter) in table {
            let s = sel(sname);
            let works = responds_to(probe, sname) && PearUIKitTryProbeDisplayLinkSelector(probe, s, is_setter);
            if !works {
                let ct = CString::new(types).unwrap();
                class_replaceMethod(concrete, s, imp, ct.as_ptr());
                let cn = CStr::from_ptr(class_getName(concrete)).to_string_lossy();
                log_msg("host", &format!("-[{cn} {sname}] provided by the host"));
            }
        }
        msg0::<()>(probe, "invalidate");
    }
}

#[no_mangle]
pub extern "C" fn PearBackendOpenURL(url: Id) -> bool {
    unsafe {
        if url.is_null() { return false; }
        let ws: Id = msg0(cls("NSWorkspace"), "sharedWorkspace");
        msg1(ws, "openURL:", url)
    }
}

#[no_mangle]
pub extern "C" fn PearBackendSetIdleTimerDisabled(disabled: bool) {
    unsafe {
        let cur = G_IDLE_ASSERTION.load(Ordering::Acquire) as u32;
        if disabled && cur == 0 {
            let mut aid: u32 = 0;
            let ty = nsstring("PreventUserIdleDisplaySleep");
            let reason = nsstring("iOS app disabled the idle timer");
            IOPMAssertionCreateWithName(ty as *const c_void, 255, reason as *const c_void, &mut aid);
            G_IDLE_ASSERTION.store(aid as usize, Ordering::Release);
        } else if !disabled && cur != 0 {
            IOPMAssertionRelease(cur);
            G_IDLE_ASSERTION.store(0, Ordering::Release);
        }
    }
}

#[no_mangle]
pub extern "C" fn PearBackendRun(ctx: *mut c_void, did_finish_launching: extern "C" fn(*mut c_void)) {
    unsafe {
        *G_DID_FINISH_CB.lock().unwrap() = Some((ctx as usize, did_finish_launching));
        let del: Id = msg0(cls("PearHostAppDelegate"), "new");
        G_APP_DELEGATE.store(del as usize, Ordering::Release);
        let ns_app: Id = msg0(cls("NSApplication"), "sharedApplication");
        msg1::<Id, ()>(ns_app, "setDelegate:", del);

        let bar: Id = msg0(cls("NSMenu"), "new");
        let app_item: Id = msg0(cls("NSMenuItem"), "new");
        msg1::<Id, ()>(bar, "addItem:", app_item);
        let app_menu: Id = msg0(cls("NSMenu"), "new");
        msg3::<Id, Sel, Id, Id>(app_menu, "addItemWithTitle:action:keyEquivalent:", nsstring("Quit"), sel("terminate:"), nsstring("q"));
        msg1::<Id, ()>(app_item, "setSubmenu:", app_menu);
        msg1::<Id, ()>(ns_app, "setMainMenu:", bar);
        objc_release(app_menu);
        objc_release(app_item);
        objc_release(bar);

        msg0::<()>(ns_app, "run");
    }
}

struct AlertCtx {
    title: String,
    message: String,
    buttons: Vec<String>,
    ctx: usize,
    on_button: extern "C" fn(*mut c_void, isize),
}
extern "C" fn present_alert_main(raw: *mut c_void) {
    unsafe {
        let a = Box::from_raw(raw as *mut AlertCtx);
        let alert: Id = msg0(cls("NSAlert"), "new");
        msg1::<Id, ()>(alert, "setMessageText:", nsstring(if a.title.is_empty() { " " } else { &a.title }));
        msg1::<Id, ()>(alert, "setInformativeText:", nsstring(&a.message));
        if a.buttons.is_empty() {
            msg1::<Id, Id>(alert, "addButtonWithTitle:", nsstring("OK"));
        } else {
            for b in &a.buttons {
                msg1::<Id, Id>(alert, "addButtonWithTitle:", nsstring(b));
            }
        }
        let resp: isize = msg0(alert, "runModal");
        let idx = if a.buttons.is_empty() { -1 } else { resp - 1000 };
        (a.on_button)(a.ctx as *mut c_void, idx);
        objc_release(alert);
    }
}

#[no_mangle]
pub extern "C" fn PearBackendPresentAlert(
    title: *const c_char,
    message: *const c_char,
    buttons: *const *const c_char,
    button_count: usize,
    ctx: *mut c_void,
    on_button: extern "C" fn(*mut c_void, isize),
) {
    unsafe {
        let t = if title.is_null() { String::new() } else { CStr::from_ptr(title).to_string_lossy().into_owned() };
        let m = if message.is_null() { String::new() } else { CStr::from_ptr(message).to_string_lossy().into_owned() };
        let mut btns = Vec::with_capacity(button_count);
        if !buttons.is_null() {
            for i in 0..button_count {
                let p = *buttons.add(i);
                if !p.is_null() {
                    btns.push(CStr::from_ptr(p).to_string_lossy().into_owned());
                }
            }
        }
        let boxed = Box::into_raw(Box::new(AlertCtx { title: t, message: m, buttons: btns, ctx: ctx as usize, on_button }));
        dispatch_async_f(&_dispatch_main_q as *const c_void, boxed as *mut c_void, present_alert_main);
    }
}

#[no_mangle]
pub extern "C" fn PearHIDUsageForMacKeyCode(code: u16) -> isize {
    match code {
        0x00 => 0x04, 0x0B => 0x05, 0x08 => 0x06, 0x02 => 0x07, 0x0E => 0x08, 0x03 => 0x09, 0x05 => 0x0A,
        0x04 => 0x0B, 0x22 => 0x0C, 0x26 => 0x0D, 0x28 => 0x0E, 0x25 => 0x0F, 0x2E => 0x10, 0x2D => 0x11,
        0x1F => 0x12, 0x23 => 0x13, 0x0C => 0x14, 0x0F => 0x15, 0x01 => 0x16, 0x11 => 0x17, 0x20 => 0x18,
        0x09 => 0x19, 0x0D => 0x1A, 0x07 => 0x1B, 0x10 => 0x1C, 0x06 => 0x1D,
        0x12 => 0x1E, 0x13 => 0x1F, 0x14 => 0x20, 0x15 => 0x21, 0x17 => 0x22, 0x16 => 0x23, 0x1A => 0x24,
        0x1C => 0x25, 0x19 => 0x26, 0x1D => 0x27,
        0x24 => 0x28, 0x35 => 0x29, 0x33 => 0x2A, 0x30 => 0x2B, 0x31 => 0x2C, 0x1B => 0x2D, 0x18 => 0x2E,
        0x21 => 0x2F, 0x1E => 0x30, 0x2A => 0x31, 0x29 => 0x33, 0x27 => 0x34, 0x32 => 0x35, 0x2B => 0x36,
        0x2F => 0x37, 0x2C => 0x38, 0x39 => 0x39, 0x0A => 0x64,
        0x7A => 0x3A, 0x78 => 0x3B, 0x63 => 0x3C, 0x76 => 0x3D, 0x60 => 0x3E, 0x61 => 0x3F, 0x62 => 0x40,
        0x64 => 0x41, 0x65 => 0x42, 0x6D => 0x43, 0x67 => 0x44, 0x6F => 0x45,
        0x73 => 0x4A, 0x74 => 0x4B, 0x75 => 0x4C, 0x77 => 0x4D, 0x79 => 0x4E,
        0x7C => 0x4F, 0x7B => 0x50, 0x7D => 0x51, 0x7E => 0x52,
        0x47 => 0x53, 0x4B => 0x54, 0x43 => 0x55, 0x4E => 0x56, 0x45 => 0x57, 0x4C => 0x58, 0x53 => 0x59,
        0x54 => 0x5A, 0x55 => 0x5B, 0x56 => 0x5C, 0x57 => 0x5D, 0x58 => 0x5E, 0x59 => 0x5F, 0x5B => 0x60,
        0x5C => 0x61, 0x52 => 0x62, 0x41 => 0x63, 0x51 => 0x67,
        0x3B => 0xE0, 0x38 => 0xE1, 0x3A => 0xE2, 0x37 => 0xE3, 0x3E => 0xE4, 0x3C => 0xE5, 0x3D => 0xE6,
        0x36 => 0xE7,
        _ => 0,
    }
}

// PearHostAppDelegate / PearHostAppObserver / PearHostWindowDelegate / PearHostView methods
unsafe extern "C" fn app_del_did_finish(_s: Id, _c: Sel, _n: Id) {
    if let Some((ctx, cb)) = G_DID_FINISH_CB.lock().unwrap().take() {
        cb(ctx as *mut c_void);
    }
}
unsafe extern "C" fn app_del_should_term(_s: Id, _c: Sel, _app: Id) -> bool { true }
unsafe extern "C" fn obs_active(_s: Id, _c: Sel, _n: Id) { PearUIKitApplicationDidBecomeActive(); }
unsafe extern "C" fn obs_resign(_s: Id, _c: Sel, _n: Id) { PearUIKitApplicationWillResignActive(); }
unsafe extern "C" fn obs_hide(_s: Id, _c: Sel, _n: Id) { PearUIKitApplicationDidEnterBackground(); }
unsafe extern "C" fn obs_unhide(_s: Id, _c: Sel, _n: Id) { PearUIKitApplicationWillEnterForeground(); }
unsafe extern "C" fn obs_term(_s: Id, _c: Sel, _n: Id) { PearUIKitApplicationWillTerminate(); }

unsafe extern "C" fn wd_did_resize(s: Id, _c: Sel, notif: Id) {
    let w: Id = msg0(notif, "object");
    let ui = assoc_get(s, &WD_UIWIN_KEY);
    if w.is_null() || ui.is_null() { return; }
    let cv: Id = msg0(w, "contentView");
    if cv.is_null() { return; }
    let b: CGRect = msg0(cv, "bounds");
    let scale: f64 = msg0(w, "backingScaleFactor");
    PearUIKitWindowDidResize(ui, b.size, scale);
}
unsafe extern "C" fn wd_became_key(s: Id, _c: Sel, _notif: Id) {
    PearUIKitSetKeyWindow(assoc_get(s, &WD_UIWIN_KEY));
}

unsafe extern "C" fn hv_true(_s: Id, _c: Sel) -> bool { true }
unsafe extern "C" fn hv_accepts_mouse(_s: Id, _c: Sel, _e: Id) -> bool { true }
unsafe fn hv_pointer(view: Id, event: Id, phase: isize) {
    let loc: CGPoint = msg0(event, "locationInWindow");
    let p: CGPoint = msg2(view, "convertPoint:fromView:", loc, NIL);
    let w = assoc_get(view, &HV_UIWIN_KEY);
    if w.is_null() { return; }
    let wb: CGRect = msg0(w, "bounds");
    let vb: CGRect = msg0(view, "bounds");
    let ui = CGPoint {
        x: p.x * wb.size.width / vb.size.width.max(1.0),
        y: p.y * wb.size.height / vb.size.height.max(1.0),
    };
    let ts: f64 = msg0(event, "timestamp");
    PearUIKitDeliverPointer(w, phase, ui, ts);
}
unsafe extern "C" fn hv_mouse_down(s: Id, _c: Sel, e: Id) { hv_pointer(s, e, 0); }
unsafe extern "C" fn hv_mouse_drag(s: Id, _c: Sel, e: Id) { hv_pointer(s, e, 1); }
unsafe extern "C" fn hv_mouse_up(s: Id, _c: Sel, e: Id) { hv_pointer(s, e, 3); }
unsafe fn hv_send_key(event: Id, down: bool) {
    let code: u16 = msg0(event, "keyCode");
    let usage = PearHIDUsageForMacKeyCode(code);
    if usage == 0 { return; }
    let flags: usize = msg0(event, "modifierFlags");
    let mods = (flags & 0xFFFF_0000) as isize;
    let chars: Id = msg0(event, "characters");
    let nomod: Id = msg0(event, "charactersIgnoringModifiers");
    let ts: f64 = msg0(event, "timestamp");
    PearUIKitDeliverKey(usage, chars, nomod, mods, down, ts);
}
unsafe extern "C" fn hv_key_down(s: Id, _c: Sel, e: Id) {
    if PearUIKitTextInputActive() {
        msg1::<Id, ()>(s, "interpretKeyEvents:", nsarray_from_ids(&[e]));
        return;
    }
    if msg0::<bool>(e, "isARepeat") { return; }
    hv_send_key(e, true);
}
unsafe extern "C" fn hv_key_up(_s: Id, _c: Sel, e: Id) {
    if PearUIKitTextInputActive() { return; }
    hv_send_key(e, false);
}
unsafe extern "C" fn hv_flags_changed(_s: Id, _c: Sel, e: Id) {
    let code: u16 = msg0(e, "keyCode");
    let usage = PearHIDUsageForMacKeyCode(code);
    let bit: usize = match usage {
        0xE1 | 0xE5 => 1 << 17,
        0xE0 | 0xE4 => 1 << 18,
        0xE2 | 0xE6 => 1 << 19,
        0xE3 | 0xE7 => 1 << 20,
        0x39 => 1 << 16,
        _ => return,
    };
    let flags: usize = msg0(e, "modifierFlags");
    let down = (flags & bit) != 0;
    let mods = (flags & 0xFFFF_0000) as isize;
    let ts: f64 = msg0(e, "timestamp");
    PearUIKitDeliverKey(usage, nsstring(""), nsstring(""), mods, down, ts);
}
unsafe extern "C" fn hv_insert_text(_s: Id, _c: Sel, obj: Id) {
    let str_id = if is_kind_of(obj, "NSAttributedString") { msg0(obj, "string") } else { obj };
    if is_kind_of(str_id, "NSString") && msg0::<usize>(str_id, "length") > 0 {
        PearUIKitDeliverText(str_id);
    }
}
unsafe extern "C" fn hv_do_cmd(_s: Id, _c: Sel, selector: Sel) {
    let name = CStr::from_ptr(sel_getName(selector)).to_string_lossy();
    match name.as_ref() {
        "deleteBackward:" => PearUIKitDeliverEditCommand(0),
        "insertNewline:" | "insertLineBreak:" => PearUIKitDeliverEditCommand(1),
        "moveLeft:" => PearUIKitDeliverEditCommand(2),
        "moveRight:" => PearUIKitDeliverEditCommand(3),
        "cancelOperation:" => PearUIKitDeliverEditCommand(4),
        "insertTab:" => PearUIKitDeliverText(nsstring("\t")),
        _ => {}
    }
}

#[no_mangle]
pub extern "C" fn PearBackendStartInputScript() {
    let Ok(path) = std::env::var("PEAR_HOST_INPUT_SCRIPT") else { return; };
    if path.is_empty() { return; }
    let Ok(text) = std::fs::read_to_string(&path) else {
        unsafe { log_msg("script", &format!("cannot read {path}")); }
        return;
    };
    let lines: Vec<String> = text.lines().map(|s| s.trim().to_string()).collect();
    std::thread::spawn(move || {
        for line in lines {
            if line.is_empty() || line.starts_with('#') { continue; }
            unsafe { log_msg("script", &line); }
            let mut parts = line.splitn(2, ' ');
            let cmd = parts.next().unwrap_or("");
            let arg = parts.next().unwrap_or("");
            let args: Vec<&str> = arg.split(' ').collect();
            if cmd == "wait" {
                let secs = arg.parse::<f64>().unwrap_or(0.0);
                std::thread::sleep(std::time::Duration::from_secs_f64(secs.max(0.0)));
            } else if cmd == "tap" && args.len() >= 2 {
                let x = args[0].parse::<f64>().unwrap_or(0.0);
                let y = args[1].parse::<f64>().unwrap_or(0.0);
                struct TapCtx { x: f64, y: f64, phase: isize }
                extern "C" fn do_tap(p: *mut c_void) {
                    unsafe {
                        let t = Box::from_raw(p as *mut TapCtx);
                        let pv = G_PRIMARY_VIEW.load(Ordering::Acquire) as Id;
                        if pv.is_null() { return; }
                        let w = assoc_get(pv, &HV_UIWIN_KEY);
                        if w.is_null() { return; }
                        let b: CGRect = msg0(w, "bounds");
                        let pt = CGPoint { x: t.x * b.size.width, y: t.y * b.size.height };
                        let ts: f64 = msg0(msg0(cls("NSProcessInfo"), "processInfo"), "systemUptime");
                        PearUIKitDeliverPointer(w, t.phase, pt, ts);
                    }
                }
                unsafe {
                    dispatch_sync_f(&_dispatch_main_q as *const c_void, Box::into_raw(Box::new(TapCtx { x, y, phase: 0 })) as *mut c_void, do_tap);
                    std::thread::sleep(std::time::Duration::from_millis(80));
                    dispatch_sync_f(&_dispatch_main_q as *const c_void, Box::into_raw(Box::new(TapCtx { x, y, phase: 3 })) as *mut c_void, do_tap);
                }
            } else if cmd == "call" && args.len() >= 2 {
                let owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
                extern "C" fn do_call(p: *mut c_void) {
                    unsafe {
                        let args = Box::from_raw(p as *mut Vec<String>);
                        let w = PearUIKitKeyWindow();
                        let app = G_APP.load(Ordering::Acquire) as Id;
                        let target: Id = match args[0].as_str() {
                            "delegate" => if app.is_null() { NIL } else { msg0(app, "delegate") },
                            "rootvc" => if w.is_null() { NIL } else { msg0(w, "rootViewController") },
                            "rootview" => {
                                let vc: Id = if w.is_null() { NIL } else { msg0(w, "rootViewController") };
                                if vc.is_null() { NIL } else { msg0(vc, "view") }
                            }
                            "responder" => PearUIKitFirstResponder(),
                            other => cls(other) as Id,
                        };
                        if !target.is_null() && responds_to(target, &args[1]) {
                            let cstrs: Vec<CString> = args.iter().map(|s| CString::new(s.as_str()).unwrap_or_default()).collect();
                            let ptrs: Vec<*const c_char> = cstrs.iter().map(|c| c.as_ptr()).collect();
                            PearBackendInvokeScriptCall(target, sel(&args[1]), ptrs.as_ptr(), ptrs.len());
                        }
                    }
                }
                unsafe {
                    dispatch_sync_f(&_dispatch_main_q as *const c_void, Box::into_raw(Box::new(owned)) as *mut c_void, do_call);
                }
            }
        }
        unsafe { log_msg("script", "done"); }
    });
}

pub unsafe fn register_classes() {
    let app = cls("UIApplication");
    if !app.is_null() {
        add_class_method(app, "sharedApplication", app_shared as Imp, "@@:");
        add_method(app, "init", app_init as Imp, "@@:");
        add_method(app, "delegate", app_del as Imp, "@@:");
        add_method(app, "setDelegate:", app_set_del as Imp, "v@:@");
        add_method(app, "pear_retainDelegate:", app_retain_del as Imp, "v@:@");
        add_method(app, "keyWindow", app_key_win as Imp, "@@:");
        add_method(app, "windows", app_windows as Imp, "@@:");
        add_method(app, "connectedScenes", app_empty_set as Imp, "@@:");
        add_method(app, "supportsMultipleScenes", app_false as Imp, "B@:");
        add_method(app, "applicationState", app_state as Imp, "q@:");
        add_method(app, "isIdleTimerDisabled", app_idle as Imp, "B@:");
        add_method(app, "setIdleTimerDisabled:", app_set_idle as Imp, "v@:B");
        add_method(app, "statusBarOrientation", app_orient as Imp, "q@:");
        add_method(app, "isStatusBarHidden", app_true as Imp, "B@:");
        add_method(app, "setStatusBarHidden:", app_noop_b as Imp, "v@:B");
        add_method(app, "setStatusBarHidden:withAnimation:", app_noop_bi as Imp, "v@:Bq");
        add_method(app, "statusBarFrame", app_zero_rect as Imp, "{CGRect={CGPoint=dd}{CGSize=dd}}@:");
        add_method(app, "applicationIconBadgeNumber", app_badge as Imp, "q@:");
        add_method(app, "setApplicationIconBadgeNumber:", app_set_badge as Imp, "v@:q");
        add_method(app, "backgroundTimeRemaining", app_bg_rem as Imp, "d@:");
        add_method(app, "isProtectedDataAvailable", app_true as Imp, "B@:");
        add_method(app, "isRegisteredForRemoteNotifications", app_false as Imp, "B@:");
        add_method(app, "userInterfaceLayoutDirection", app_zero_int as Imp, "q@:");
        add_method(app, "preferredContentSizeCategory", app_pref_cat as Imp, "@@:");
        add_method(app, "canOpenURL:", app_can_open as Imp, "B@:@");
        add_method(app, "openURL:", app_open_url as Imp, "B@:@");
        add_method(app, "openURL:options:completionHandler:", app_open_url_opts as Imp, "v@:@@@?");
        add_method(app, "registerForRemoteNotifications", app_reg_remote as Imp, "v@:");
        add_method(app, "unregisterForRemoteNotifications", app_noop as Imp, "v@:");
        add_method(app, "registerUserNotificationSettings:", app_noop_id as Imp, "v@:@");
        add_method(app, "beginBackgroundTaskWithExpirationHandler:", app_begin_bg as Imp, "Q@:@?");
        add_method(app, "beginBackgroundTaskWithName:expirationHandler:", app_begin_bg_name as Imp, "Q@:@@?");
        add_method(app, "endBackgroundTask:", app_noop_u as Imp, "v@:Q");
        add_method(app, "setMinimumBackgroundFetchInterval:", app_noop_f as Imp, "v@:d");
        add_method(app, "sendAction:to:from:forEvent:", app_send_action as Imp, "B@::@@@");
        add_method(app, "beginReceivingRemoteControlEvents", app_noop as Imp, "v@:");
        add_method(app, "endReceivingRemoteControlEvents", app_noop as Imp, "v@:");
    }

    let k = cls("UIKey");
    if !k.is_null() {
        add_method(k, "initWithCharacters:ignoringModifiers:modifiers:keyCode:", uikey_init as Imp, "@@:@@qq");
        add_method(k, "characters", uikey_chars as Imp, "@@:");
        add_method(k, "charactersIgnoringModifiers", uikey_nomod as Imp, "@@:");
        add_method(k, "modifierFlags", uikey_mods as Imp, "q@:");
        add_method(k, "keyCode", uikey_code as Imp, "q@:");
        add_method(k, "copyWithZone:", copy_self as Imp, "@@:^{_NSZone=}");
    }
    let p = cls("UIPress");
    if !p.is_null() {
        add_method(p, "initWithKey:type:phase:timestamp:", press_init as Imp, "@@:@qqd");
        add_method(p, "key", press_key as Imp, "@@:");
        add_method(p, "type", press_type as Imp, "q@:");
        add_method(p, "phase", press_phase as Imp, "q@:");
        add_method(p, "timestamp", press_ts as Imp, "d@:");
        add_method(p, "force", press_force as Imp, "d@:");
        add_method(p, "responder", press_nil as Imp, "@@:");
        add_method(p, "window", press_win as Imp, "@@:");
        add_method(p, "gestureRecognizers", press_empty_arr as Imp, "@@:");
    }
    let pev = cls("UIPressesEvent");
    if !pev.is_null() {
        add_method(pev, "initWithPresses:timestamp:", pev_init as Imp, "@@:@d");
        add_method(pev, "type", pev_type as Imp, "q@:");
        add_method(pev, "timestamp", pev_ts as Imp, "d@:");
        add_method(pev, "allPresses", pev_all_presses as Imp, "@@:");
        add_method(pev, "pressesForGestureRecognizer:", pev_for_gr as Imp, "@@:@");
        add_method(pev, "allTouches", pev_all_touches as Imp, "@@:");
    }
    let tv = cls("UITextView");
    if !tv.is_null() {
        add_method(tv, "initWithFrame:", tv_init_frame as Imp, "@@:{CGRect={CGPoint=dd}{CGSize=dd}}");
        add_method(tv, "initWithFrame:textContainer:", tv_init_tc as Imp, "@@:{CGRect={CGPoint=dd}{CGSize=dd}}@");
        add_method(tv, "initWithCoder:", tv_init_coder as Imp, "@@:@");
        add_method(tv, "delegate", tv_del as Imp, "@@:");
        add_method(tv, "setDelegate:", tv_set_del as Imp, "v@:@");
        add_method(tv, "text", tv_text as Imp, "@@:");
        add_method(tv, "setText:", tv_set_text as Imp, "v@:@");
        add_method(tv, "attributedText", tv_attr_text as Imp, "@@:");
        add_method(tv, "setAttributedText:", tv_set_attr_text as Imp, "v@:@");
        add_method(tv, "selectedRange", tv_get_sel as Imp, "{_NSRange=QQ}@:");
        add_method(tv, "setSelectedRange:", tv_set_sel_range as Imp, "v@:{_NSRange=QQ}");
        add_method(tv, "isEditable", tv_editable as Imp, "B@:");
        add_method(tv, "setEditable:", tv_set_editable as Imp, "v@:B");
        add_method(tv, "isSelectable", tv_true as Imp, "B@:");
        add_method(tv, "setSelectable:", tv_noop_bool as Imp, "v@:B");
        add_method(tv, "hasText", tv_has_text as Imp, "B@:");
        add_method(tv, "font", tv_font as Imp, "@@:");
        add_method(tv, "setFont:", tv_set_font as Imp, "v@:@");
        add_method(tv, "textColor", tv_color as Imp, "@@:");
        add_method(tv, "setTextColor:", tv_set_color as Imp, "v@:@");
        add_method(tv, "textAlignment", tv_zero_int as Imp, "q@:");
        add_method(tv, "setTextAlignment:", tv_noop_int as Imp, "v@:q");
        add_method(tv, "isScrollEnabled", tv_false as Imp, "B@:");
        add_method(tv, "setScrollEnabled:", tv_noop_bool as Imp, "v@:B");
        add_method(tv, "textContainer", tv_nil as Imp, "@@:");
        add_method(tv, "textContainerInset", tv_zero_insets as Imp, "{UIEdgeInsets=dddd}@:");
        add_method(tv, "setTextContainerInset:", tv_noop_insets as Imp, "v@:{UIEdgeInsets=dddd}");
        add_method(tv, "scrollRangeToVisible:", tv_noop_range as Imp, "v@:{_NSRange=QQ}");
        add_method(tv, "inputDelegate", tv_nil as Imp, "@@:");
        add_method(tv, "setInputDelegate:", tv_noop_id as Imp, "v@:@");
        add_method(tv, "keyboardType", tv_kb_type as Imp, "q@:");
        add_method(tv, "setKeyboardType:", tv_set_kb_type as Imp, "v@:q");
        add_method(tv, "returnKeyType", tv_ret_type as Imp, "q@:");
        add_method(tv, "setReturnKeyType:", tv_set_ret_type as Imp, "v@:q");
        add_method(tv, "autocapitalizationType", tv_autocap as Imp, "q@:");
        add_method(tv, "setAutocapitalizationType:", tv_set_autocap as Imp, "v@:q");
        add_method(tv, "autocorrectionType", tv_autocorrect as Imp, "q@:");
        add_method(tv, "setAutocorrectionType:", tv_set_autocorrect as Imp, "v@:q");
        add_method(tv, "spellCheckingType", tv_spell as Imp, "q@:");
        add_method(tv, "setSpellCheckingType:", tv_set_spell as Imp, "v@:q");
        add_method(tv, "smartQuotesType", tv_smart_q as Imp, "q@:");
        add_method(tv, "setSmartQuotesType:", tv_set_smart_q as Imp, "v@:q");
        add_method(tv, "smartDashesType", tv_smart_d as Imp, "q@:");
        add_method(tv, "setSmartDashesType:", tv_set_smart_d as Imp, "v@:q");
        add_method(tv, "smartInsertDeleteType", tv_zero_int as Imp, "q@:");
        add_method(tv, "setSmartInsertDeleteType:", tv_noop_int as Imp, "v@:q");
        add_method(tv, "keyboardAppearance", tv_zero_int as Imp, "q@:");
        add_method(tv, "setKeyboardAppearance:", tv_noop_int as Imp, "v@:q");
        add_method(tv, "enablesReturnKeyAutomatically", tv_false as Imp, "B@:");
        add_method(tv, "setEnablesReturnKeyAutomatically:", tv_noop_bool as Imp, "v@:B");
        add_method(tv, "isSecureTextEntry", tv_secure as Imp, "B@:");
        add_method(tv, "setSecureTextEntry:", tv_set_secure as Imp, "v@:B");
        add_method(tv, "textContentType", tv_nil as Imp, "@@:");
        add_method(tv, "setTextContentType:", tv_noop_id as Imp, "v@:@");
        add_method(tv, "canBecomeFirstResponder", tv_can_become_first as Imp, "B@:");
        add_method(tv, "becomeFirstResponder", tv_become_first as Imp, "B@:");
        add_method(tv, "resignFirstResponder", tv_resign_first as Imp, "B@:");
        add_method(tv, "removeFromSuperview", tv_remove_from_sv as Imp, "v@:");
        add_method(tv, "insertText:", tv_insert_text as Imp, "v@:@");
        add_method(tv, "deleteBackward", tv_delete_backward as Imp, "v@:");
        add_method(tv, "pear_moveCursor:", tv_move_cursor as Imp, "v@:q");
    }

    if cls("PearNibDecoder").is_null() {
        let name = CString::new("PearNibDecoder").unwrap();
        let c = objc_allocateClassPair(cls("NSCoder"), name.as_ptr(), 0);
        if !c.is_null() {
            add_method(c, "dealloc", dec_dealloc as Imp, "v@:");
            add_method(c, "allowsKeyedCoding", dec_allows_keyed as Imp, "B@:");
            add_method(c, "requiresSecureCoding", dec_req_secure as Imp, "B@:");
            add_method(c, "versionForClassName:", dec_version as Imp, "q@:@");
            add_method(c, "containsValueForKey:", dec_contains as Imp, "B@:@");
            add_method(c, "decodeBoolForKey:", dec_bool as Imp, "B@:@");
            add_method(c, "decodeCIntForKey:", dec_i32 as Imp, "i@:@");
            add_method(c, "decodeInt32ForKey:", dec_i32 as Imp, "i@:@");
            add_method(c, "decodeInt64ForKey:", dec_i64 as Imp, "q@:@");
            add_method(c, "decodeIntegerForKey:", dec_int as Imp, "q@:@");
            add_method(c, "decodeFloatForKey:", dec_f32 as Imp, "f@:@");
            add_method(c, "decodeDoubleForKey:", dec_f64 as Imp, "d@:@");
            add_method(c, "decodeCGRectForKey:", dec_rect as Imp, "{CGRect={CGPoint=dd}{CGSize=dd}}@:@");
            add_method(c, "decodeCGPointForKey:", dec_point as Imp, "{CGPoint=dd}@:@");
            add_method(c, "decodeCGSizeForKey:", dec_size as Imp, "{CGSize=dd}@:@");
            add_method(c, "decodeObjectForKey:", dec_obj as Imp, "@@:@");
            objc_registerClassPair(c);
        }
    }

    let nib = cls("UINib");
    if !nib.is_null() {
        add_class_method(nib, "nibWithNibName:bundle:", nib_with_name as Imp, "@@:@@");
        add_class_method(nib, "nibWithData:bundle:", nib_with_data as Imp, "@@:@@");
        add_method(nib, "instantiateWithOwner:options:", nib_inst as Imp, "@@:@@");
        add_method(nib, "pear_instantiateWithOwner:options:visibleWindows:", nib_pear_inst as Imp, "@@:@@^@");
    }
    let rc = cls("UIRuntimeConnection");
    if !rc.is_null() {
        add_method(rc, "initWithCoder:", rc_init_coder as Imp, "@@:@");
        add_method(rc, "encodeWithCoder:", rc_encode as Imp, "v@:@");
        add_method(rc, "pear_connect", rc_noop as Imp, "v@:");
    }
    let roc = cls("UIRuntimeOutletConnection");
    if !roc.is_null() { add_method(roc, "pear_connect", roc_connect as Imp, "v@:"); }
    let rocc = cls("UIRuntimeOutletCollectionConnection");
    if !rocc.is_null() { add_method(rocc, "pear_connect", rocc_connect as Imp, "v@:"); }
    let rec = cls("UIRuntimeEventConnection");
    if !rec.is_null() { add_method(rec, "pear_connect", rec_connect as Imp, "v@:"); }

    let nscoder = cls("NSCoder");
    if !nscoder.is_null() {
        add_method(nscoder, "encodeCGPoint:forKey:", coder_enc_pt as Imp, "v@:{CGPoint=dd}@");
        add_method(nscoder, "encodeCGSize:forKey:", coder_enc_sz as Imp, "v@:{CGSize=dd}@");
        add_method(nscoder, "encodeCGRect:forKey:", coder_enc_rect as Imp, "v@:{CGRect={CGPoint=dd}{CGSize=dd}}@");
        add_method(nscoder, "decodeCGPointForKey:", coder_dec_pt as Imp, "{CGPoint=dd}@:@");
        add_method(nscoder, "decodeCGSizeForKey:", coder_dec_sz as Imp, "{CGSize=dd}@:@");
        add_method(nscoder, "decodeCGRectForKey:", coder_dec_rect as Imp, "{CGRect={CGPoint=dd}{CGSize=dd}}@:@");
    }
    let bdl = cls("NSBundle");
    if !bdl.is_null() {
        add_method(bdl, "loadNibNamed:owner:options:", bundle_load_nib as Imp, "@@:@@@");
    }

    // AppKit helper classes
    if cls("PearHostAppDelegate").is_null() {
        let name = CString::new("PearHostAppDelegate").unwrap();
        let c = objc_allocateClassPair(cls("NSObject"), name.as_ptr(), 0);
        if !c.is_null() {
            add_method(c, "applicationDidFinishLaunching:", app_del_did_finish as Imp, "v@:@");
            add_method(c, "applicationShouldTerminateAfterLastWindowClosed:", app_del_should_term as Imp, "B@:@");
            objc_registerClassPair(c);
        }
    }
    if cls("PearHostAppObserver").is_null() {
        let name = CString::new("PearHostAppObserver").unwrap();
        let c = objc_allocateClassPair(cls("NSObject"), name.as_ptr(), 0);
        if !c.is_null() {
            add_method(c, "onBecomeActive:", obs_active as Imp, "v@:@");
            add_method(c, "onResignActive:", obs_resign as Imp, "v@:@");
            add_method(c, "onDidHide:", obs_hide as Imp, "v@:@");
            add_method(c, "onWillUnhide:", obs_unhide as Imp, "v@:@");
            add_method(c, "onWillTerminate:", obs_term as Imp, "v@:@");
            objc_registerClassPair(c);
        }
    }
    if cls("PearHostWindowDelegate").is_null() {
        let name = CString::new("PearHostWindowDelegate").unwrap();
        let c = objc_allocateClassPair(cls("NSObject"), name.as_ptr(), 0);
        if !c.is_null() {
            add_method(c, "windowDidResize:", wd_did_resize as Imp, "v@:@");
            add_method(c, "windowDidChangeBackingProperties:", wd_did_resize as Imp, "v@:@");
            add_method(c, "windowDidBecomeKey:", wd_became_key as Imp, "v@:@");
            objc_registerClassPair(c);
        }
    }
    if cls("PearHostView").is_null() {
        let name = CString::new("PearHostView").unwrap();
        let c = objc_allocateClassPair(cls("NSView"), name.as_ptr(), 0);
        if !c.is_null() {
            add_method(c, "isFlipped", hv_true as Imp, "B@:");
            add_method(c, "acceptsFirstResponder", hv_true as Imp, "B@:");
            add_method(c, "acceptsFirstMouse:", hv_accepts_mouse as Imp, "B@:@");
            add_method(c, "wantsUpdateLayer", hv_true as Imp, "B@:");
            add_method(c, "mouseDown:", hv_mouse_down as Imp, "v@:@");
            add_method(c, "mouseDragged:", hv_mouse_drag as Imp, "v@:@");
            add_method(c, "mouseUp:", hv_mouse_up as Imp, "v@:@");
            add_method(c, "keyDown:", hv_key_down as Imp, "v@:@");
            add_method(c, "keyUp:", hv_key_up as Imp, "v@:@");
            add_method(c, "flagsChanged:", hv_flags_changed as Imp, "v@:@");
            add_method(c, "insertText:", hv_insert_text as Imp, "v@:@");
            add_method(c, "doCommandBySelector:", hv_do_cmd as Imp, "v@::");
            objc_registerClassPair(c);
        }
    }
}
