//! UIApplication, UIApplicationMain, UIKey, UIPress, UIPressesEvent, UITextView,
//! UINib, UIRuntime*Connection, NSCoder geometry extensions, NSBundle.loadNibNamed,
//! and the event sink hosts deliver to (PearUIKitDeliver*, lifecycle). The
//! host itself is reached through pear-host (macOS: pear-host-macos).

use pear_nib::{NibArchive, NibValue, NibValuePayload};
use pear_objc::{
    cls, is_kind_of, msg0, msg1, msg2, msg3, msg4, nsarray_count, nsarray_from_ids,
    nsarray_object_at, nsdata_from_bytes, nsdata_to_vec, nsnumber_bool, nsnumber_f64, nsnumber_i64,
    nsstring, nsstring_to_string, objc_autorelease, objc_getAssociatedObject, objc_msgSendSuper,
    objc_release, objc_retain, objc_setAssociatedObject, replace_method as add_method, responds_to,
    sel, CGPoint, CGRect, CGSize, Class, Id, Imp, NSRange, Sel, UIEdgeInsets, NIL,
    OBJC_ASSOCIATION_RETAIN,
};
use std::cell::Cell;
use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void, CString};
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
    fn object_getClass(obj: Id) -> Class;
    fn class_getSuperclass(cls: Class) -> Class;
    fn class_getName(cls: Class) -> *const c_char;
    fn class_getInstanceMethod(cls: Class, name: Sel) -> *mut c_void;
    fn PearMissingReport(kind: c_int, class_name: *const c_char, selector: *const c_char, caller: *const c_void);
    fn PearHostLogMessage(tag: *const c_char, message: *const c_char);
    fn PearUIKitFirstResponder() -> Id;
    fn PearUIKitDeliverPointer(window: Id, phase: isize, point: CGPoint, timestamp: f64);
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
static APP_NOTIF_SETTINGS_KEY: u8 = 0;
static APP_NEXT_TASK: AtomicUsize = AtomicUsize::new(0);
static G_CONNECTED_SCENES: Mutex<Vec<usize>> = Mutex::new(Vec::new());

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
unsafe extern "C" fn app_del(s: Id, _c: Sel) -> Id { pear_objc::assoc_get_weak(s, &APP_DEL_KEY) }
unsafe extern "C" fn app_set_del(s: Id, _c: Sel, d: Id) { pear_objc::assoc_set_weak(s, &APP_DEL_KEY, d); }
unsafe extern "C" fn app_retain_del(s: Id, _c: Sel, d: Id) {
    assoc_set(s, &APP_STRONG_DEL_KEY, d);
    pear_objc::assoc_set_weak(s, &APP_DEL_KEY, d);
}
unsafe extern "C" fn app_key_win(_s: Id, _c: Sel) -> Id { PearUIKitKeyWindow() }
unsafe extern "C" fn app_windows(_s: Id, _c: Sel) -> Id { PearUIKitWindows() }
unsafe extern "C" fn app_connected_scenes(_s: Id, _c: Sel) -> Id {
    let guard = G_CONNECTED_SCENES.lock().unwrap();
    let ids: Vec<Id> = guard.iter().map(|&p| p as Id).collect();
    let arr = nsarray_from_ids(&ids);
    msg1(cls("NSSet"), "setWithArray:", arr)
}
unsafe extern "C" fn app_open_sessions(_s: Id, _c: Sel) -> Id {
    let guard = G_CONNECTED_SCENES.lock().unwrap();
    let ids: Vec<Id> = guard
        .iter()
        .filter_map(|&p| {
            let sess: Id = msg0(p as Id, "session");
            (!sess.is_null()).then_some(sess)
        })
        .collect();
    let arr = nsarray_from_ids(&ids);
    msg1(cls("NSSet"), "setWithArray:", arr)
}
unsafe extern "C" fn app_reg_notif_settings(s: Id, _c: Sel, settings: Id) {
    assoc_set(s, &APP_NOTIF_SETTINGS_KEY, settings);
}
unsafe extern "C" fn app_cur_notif_settings(s: Id, _c: Sel) -> Id {
    let stored = assoc_get(s, &APP_NOTIF_SETTINGS_KEY);
    if !stored.is_null() {
        return stored;
    }
    let settings: Id = msg2(cls("UIUserNotificationSettings"), "settingsForTypes:categories:", 0usize, NIL);
    assoc_set(s, &APP_NOTIF_SETTINGS_KEY, settings);
    settings
}
unsafe extern "C" fn app_false(_s: Id, _c: Sel) -> bool { false }
unsafe extern "C" fn app_true(_s: Id, _c: Sel) -> bool { true }
unsafe extern "C" fn app_state(_s: Id, _c: Sel) -> isize { G_APP_STATE.load(Ordering::Acquire) }
unsafe extern "C" fn app_idle(s: Id, _c: Sel) -> bool {
    let n = assoc_get(s, &APP_IDLE_KEY);
    !n.is_null() && msg0::<bool>(n, "boolValue")
}
unsafe extern "C" fn app_set_idle(s: Id, _c: Sel, d: bool) {
    assoc_set(s, &APP_IDLE_KEY, nsnumber_bool(d));
    pear_host::host().set_idle_timer_disabled(d);
}
unsafe extern "C" fn app_orient(_s: Id, _c: Sel) -> isize {
    crate::view::window_orientation(PearUIKitKeyWindow())
}

thread_local! {
    static QUERYING_ORIENTATION_DELEGATE: Cell<bool> = const { Cell::new(false) };
}

unsafe extern "C" fn app_supported_orientations(app: Id, _cmd: Sel, window: Id) -> usize {
    let delegate: Id = msg0(app, "delegate");
    let callback = "application:supportedInterfaceOrientationsForWindow:";
    if let Some(mask) = QUERYING_ORIENTATION_DELEGATE.with(|querying| {
        if querying.get() || !responds_to(delegate, callback) {
            return None;
        }
        querying.set(true);
        let mask = msg2(delegate, callback, app, window);
        querying.set(false);
        Some(mask)
    }) {
        return mask;
    }
    // A delegate may query UIApplication for the bundle's permitted
    // orientations while computing its own mask.
    let info: Id = msg0(msg0(cls("NSBundle"), "mainBundle"), "infoDictionary");
    let orientations: Id = msg1(info, "objectForKey:", nsstring("UISupportedInterfaceOrientations"));
    let mut mask = 0;
    for index in 0..nsarray_count(orientations) {
        mask |= match nsstring_to_string(nsarray_object_at(orientations, index)).as_deref() {
            Some("UIInterfaceOrientationPortrait") => 1 << 1,
            Some("UIInterfaceOrientationPortraitUpsideDown") => 1 << 2,
            Some("UIInterfaceOrientationLandscapeLeft") => 1 << 3,
            Some("UIInterfaceOrientationLandscapeRight") => 1 << 4,
            _ => 0,
        };
    }
    if mask == 0 { (1 << 3) | (1 << 4) } else { mask }
}
unsafe extern "C" fn app_bg_refresh_status(_self_obj: Id, _cmd: Sel) -> isize { 2 }
unsafe extern "C" fn app_noop(_s: Id, _c: Sel) {}
unsafe extern "C" fn app_noop_b(_s: Id, _c: Sel, _b: bool) {}
unsafe extern "C" fn app_noop_bi(_s: Id, _c: Sel, _b: bool, _i: isize) {}
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
    app_can_open(s, sel("canOpenURL:"), url)
        && pear_host::host().open_url(&nsstring_to_string(msg0(url, "absoluteString")).unwrap_or_default())
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
    let del = pear_objc::assoc_get_weak(s, &APP_DEL_KEY);
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

static SCENE_DEL_KEY: u8 = 0;
static SCENE_SESSION_KEY: u8 = 0;
static SCENE_TITLE_KEY: u8 = 0;
static SESSION_ROLE_KEY: u8 = 0;
static SESSION_CONFIG_KEY: u8 = 0;
static SESSION_SCENE_KEY: u8 = 0;
static SCFG_NAME_KEY: u8 = 0;
static SCFG_ROLE_KEY: u8 = 0;
static SCFG_DELEGATE_CLS_KEY: u8 = 0;
static SCFG_SCENE_CLS_KEY: u8 = 0;

fn cls_opt(name: Option<&str>) -> Class {
    match name {
        Some(s) if !s.is_empty() && !s.contains('\0') => cls(s),
        _ => std::ptr::null_mut(),
    }
}

unsafe fn post_notif(name: &str, obj: Id) {
    let nc: Id = msg0(cls("NSNotificationCenter"), "defaultCenter");
    msg2::<Id, Id, ()>(nc, "postNotificationName:object:", nsstring(name), obj);
}

unsafe fn post_app_notif(name: &str) {
    post_notif(name, G_APP.load(Ordering::Acquire) as Id);
}

unsafe fn notify_scenes(selector: &str, notif: &str) {
    let scenes: Vec<Id> = G_CONNECTED_SCENES.lock().unwrap().iter().map(|&p| p as Id).collect();
    for scene in scenes {
        let sdel: Id = msg0(scene, "delegate");
        if !sdel.is_null() && responds_to(sdel, selector) {
            msg1::<Id, ()>(sdel, selector, scene);
        }
        post_notif(notif, scene);
    }
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
        notify_scenes("sceneDidBecomeActive:", "UISceneDidActivateNotification");
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
        notify_scenes("sceneWillResignActive:", "UISceneWillDeactivateNotification");
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
        notify_scenes("sceneDidEnterBackground:", "UISceneDidEnterBackgroundNotification");
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
        notify_scenes("sceneWillEnterForeground:", "UISceneWillEnterForegroundNotification");
    }
}

#[no_mangle]
pub extern "C" fn PearUIKitApplicationWillTerminate() {
    unsafe {
        call_app_delegate("applicationWillTerminate:");
        post_app_notif("UIApplicationWillTerminateNotification");
    }
}

unsafe fn extract_scene_manifest_entry(manifest: Id) -> (Option<String>, Option<String>) {
    if manifest.is_null() || !is_kind_of(manifest, "NSDictionary") {
        return (None, None);
    }
    let cfgs: Id = msg1(manifest, "objectForKey:", nsstring("UISceneConfigurations"));
    if cfgs.is_null() || !is_kind_of(cfgs, "NSDictionary") {
        return (None, None);
    }
    let arr: Id = msg1(cfgs, "objectForKey:", nsstring("UIWindowSceneSessionRoleApplication"));
    if arr.is_null() || !is_kind_of(arr, "NSArray") || nsarray_count(arr) == 0 {
        return (None, None);
    }
    let entry = nsarray_object_at(arr, 0);
    if entry.is_null() || !is_kind_of(entry, "NSDictionary") {
        return (None, None);
    }
    let name = nsstring_to_string(msg1(entry, "objectForKey:", nsstring("UISceneConfigurationName")));
    let del_name = nsstring_to_string(msg1(entry, "objectForKey:", nsstring("UISceneDelegateClassName")));
    (name, del_name)
}

unsafe fn connect_main_scene(app: Id, app_del: Id, manifest: Id) {
    let (cfg_name, del_cls_name) = extract_scene_manifest_entry(manifest);
    let role = nsstring("UIWindowSceneSessionRoleApplication");
    let name_id = cfg_name.as_deref().map(|s| nsstring(s)).unwrap_or(NIL);
    let mut cfg: Id = msg2(cls("UISceneConfiguration"), "configurationWithName:sessionRole:", name_id, role);
    let manifest_del_cls = cls_opt(del_cls_name.as_deref());
    if !manifest_del_cls.is_null() {
        msg1::<Class, ()>(cfg, "setDelegateClass:", manifest_del_cls);
    }

    let session: Id = msg0(cls("UISceneSession"), "new");
    assoc_set(session, &SESSION_ROLE_KEY, role);
    assoc_set(session, &SESSION_CONFIG_KEY, cfg);
    let opts: Id = msg0(cls("UISceneConnectionOptions"), "new");

    let cfg_cb = "application:configurationForConnectingSceneSession:options:";
    if !app_del.is_null() && responds_to(app_del, cfg_cb) {
        let custom: Id = msg3(app_del, cfg_cb, app, session, opts);
        if !custom.is_null() {
            cfg = custom;
            assoc_set(session, &SESSION_CONFIG_KEY, cfg);
        }
    }

    let mut scene_cls: Class = msg0(cfg, "sceneClass");
    if scene_cls.is_null() {
        scene_cls = cls("UIWindowScene");
    }
    let scene: Id = msg0(scene_cls as Id, "new");
    if !scene.is_null() {
        assoc_set(scene, &SCENE_SESSION_KEY, session);
        pear_objc::assoc_set_weak(session, &SESSION_SCENE_KEY, scene);

        let mut del_cls: Class = msg0(cfg, "delegateClass");
        if del_cls.is_null() {
            del_cls = manifest_del_cls;
        }
        if !del_cls.is_null() {
            let scene_del: Id = msg0(del_cls as Id, "new");
            if !scene_del.is_null() {
                msg1::<Id, ()>(scene, "setDelegate:", scene_del);
                objc_release(scene_del);
            }
        }
        G_CONNECTED_SCENES.lock().unwrap().push(scene as usize);

        let scene_del: Id = msg0(scene, "delegate");
        if !scene_del.is_null() && responds_to(scene_del, "scene:willConnectToSession:options:") {
            msg3::<Id, Id, Id, ()>(scene_del, "scene:willConnectToSession:options:", scene, session, opts);
        }
        post_notif("UISceneWillConnectNotification", scene);
        if !scene_del.is_null() && responds_to(scene_del, "sceneWillEnterForeground:") {
            msg1::<Id, ()>(scene_del, "sceneWillEnterForeground:", scene);
        }
        post_notif("UISceneWillEnterForegroundNotification", scene);
    }

    objc_release(session);
    objc_release(opts);
}

extern "C-unwind" fn launch_finish_inner(ctx: *mut c_void) {
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
        let manifest: Id = msg1(info, "objectForKey:", nsstring("UIApplicationSceneManifest"));
        let has_scene_cb = !del.is_null() && responds_to(del, "application:configurationForConnectingSceneSession:options:");
        if main_nib.is_none() && manifest.is_null() && !has_scene_cb {
            let mut sb_name = nsstring_to_string(msg1(info, "objectForKey:", nsstring("UIMainStoryboardFile~ipad")));
            if sb_name.as_deref().unwrap_or("").is_empty() {
                sb_name = nsstring_to_string(msg1(info, "objectForKey:", nsstring("UIMainStoryboardFile")));
            }
            if let Some(ref name) = sb_name {
                if !name.is_empty() {
                    let board: Id = msg2(cls("UIStoryboard"), "storyboardWithName:bundle:", nsstring(name), bundle);
                    let controller: Id = msg0(board, "instantiateInitialViewController");
                    if !controller.is_null() {
                        let screen: Id = msg0(cls("UIScreen"), "mainScreen");
                        let bounds: CGRect = msg0(screen, "bounds");
                        let window: Id = msg1(msg0(cls("UIWindow"), "alloc"), "initWithFrame:", bounds);
                        if !del.is_null() && responds_to(del, "setWindow:") {
                            msg1::<Id, ()>(del, "setWindow:", window);
                        }
                        msg1::<Id, ()>(window, "setRootViewController:", controller);
                        nib_windows.push(window);
                        objc_autorelease(window);
                    } else {
                        log_msg("app", "could not load main storyboard entry point");
                    }
                }
            }
        }
        if del.is_null() {
            log_msg("app", "no application delegate after launch setup");
        } else {
            let options: Id = msg0(cls("NSDictionary"), "dictionary");
            if responds_to(del, "application:willFinishLaunchingWithOptions:") {
                let _: bool = msg2(del, "application:willFinishLaunchingWithOptions:", app, options);
            }
            if responds_to(del, "application:didFinishLaunchingWithOptions:") {
                let _: bool = msg2(del, "application:didFinishLaunchingWithOptions:", app, options);
            } else if responds_to(del, "applicationDidFinishLaunching:") {
                msg1::<Id, ()>(del, "applicationDidFinishLaunching:", app);
            }
        }

        if !manifest.is_null() || has_scene_cb {
            connect_main_scene(app, del, manifest);
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
        crate::globals::PearUIKitRunCatchingExceptions(ctx, launch_finish_inner);
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
            pear_host::host().initialize();

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

            let app_ptr = app as usize;
            pear_host::host().run(Box::new(move || launch_on_did_finish(app_ptr as *mut c_void)));
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
static TV_INPUT_ACCESSORY_KEY: u8 = 0;

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
unsafe extern "C" fn tv_del(s: Id, _c: Sel) -> Id { pear_objc::assoc_get_weak(s, &TV_DEL_KEY) }
unsafe extern "C" fn tv_set_del(s: Id, _c: Sel, d: Id) { pear_objc::assoc_set_weak(s, &TV_DEL_KEY, d); }
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

unsafe extern "C" fn tv_input_accessory(view: Id, _cmd: Sel) -> Id {
    assoc_get(view, &TV_INPUT_ACCESSORY_KEY)
}

unsafe fn tv_update_input_accessory(view: Id, show: bool) {
    let accessory = tv_input_accessory(view, sel("inputAccessoryView"));
    let window: Id = msg0(view, "window");
    if accessory.is_null() || window.is_null() {
        return;
    }
    let parent: Id = msg0(accessory, "superview");
    if !show {
        if parent == window {
            msg0::<()>(accessory, "removeFromSuperview");
        }
        return;
    }
    let bounds: CGRect = msg0(window, "bounds");
    let frame: CGRect = msg0(accessory, "frame");
    let height = frame.size.height.min(bounds.size.height);
    msg1::<CGRect, ()>(accessory, "setFrame:", CGRect {
        origin: CGPoint { x: 0.0, y: bounds.size.height - height },
        size: CGSize { width: bounds.size.width, height },
    });
    msg1::<usize, ()>(accessory, "setAutoresizingMask:", (1 << 1) | (1 << 3));
    msg1::<Id, ()>(window, "addSubview:", accessory);
}

unsafe extern "C" fn tv_set_input_accessory(view: Id, _cmd: Sel, accessory: Id) {
    if tv_input_accessory(view, sel("inputAccessoryView")) == accessory || accessory == view {
        return;
    }
    let editing = tv_flags(view).editing;
    if editing { tv_update_input_accessory(view, false); }
    assoc_set(view, &TV_INPUT_ACCESSORY_KEY, accessory);
    if editing { tv_update_input_accessory(view, true); }
}

unsafe extern "C" fn tv_become_first(s: Id, _c: Sel) -> bool {
    if tv_flags(s).editing { return true; }
    let d = pear_objc::assoc_get_weak(s, &TV_DEL_KEY);
    if !d.is_null() && responds_to(d, "textViewShouldBeginEditing:") && !msg1::<Id, bool>(d, "textViewShouldBeginEditing:", s) {
        return false;
    }
    if !super0::<bool>(s, "UITextView", "becomeFirstResponder") {
        return false;
    }
    let mut f = tv_flags(s);
    f.editing = true;
    set_tv_flags(s, f);
    tv_update_input_accessory(s, true);
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
    let d = pear_objc::assoc_get_weak(s, &TV_DEL_KEY);
    if !d.is_null() && responds_to(d, "textViewShouldEndEditing:") && !msg1::<Id, bool>(d, "textViewShouldEndEditing:", s) {
        return false;
    }
    let _: bool = super0(s, "UITextView", "resignFirstResponder");
    let mut f = tv_flags(s);
    f.editing = false;
    set_tv_flags(s, f);
    tv_update_input_accessory(s, false);
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
    let d = pear_objc::assoc_get_weak(tv, &TV_DEL_KEY);
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
    let d = pear_objc::assoc_get_weak(s, &TV_DEL_KEY);
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
static STORYBOARD_DIRECTORY_KEY: u8 = 0;
static STORYBOARD_INFO_KEY: u8 = 0;
static VC_STORYBOARD_KEY: u8 = 0;

unsafe fn load_nib_data_at_path(p: Id) -> Id {
    if p.is_null() {
        return NIL;
    }
    let fm: Id = msg0(cls("NSFileManager"), "defaultManager");
    let mut is_dir = false;
    let exists: bool = msg2(fm, "fileExistsAtPath:isDirectory:", p, &mut is_dir as *mut bool);
    if !exists {
        return NIL;
    }
    let target = if is_dir {
        let mut inner: Id = msg1(p, "stringByAppendingPathComponent:", nsstring("runtime.nib"));
        if !msg1::<Id, bool>(fm, "fileExistsAtPath:", inner) {
            inner = msg1(p, "stringByAppendingPathComponent:", nsstring("objects-13.0+.nib"));
        }
        inner
    } else {
        p
    };
    msg1(cls("NSData"), "dataWithContentsOfFile:", target)
}

unsafe extern "C" fn nib_with_name(_cls: Id, _c: Sel, name: Id, bundle: Id) -> Id {
    let name_str = nsstring_to_string(name).unwrap_or_default();
    let b = if bundle.is_null() { msg0(cls("NSBundle"), "mainBundle") } else { bundle };
    let mut data: Id = NIL;
    for cand in [format!("{name_str}~ipad"), name_str.clone()] {
        let p: Id = msg2(b, "pathForResource:ofType:", nsstring(&cand), nsstring("nib"));
        data = load_nib_data_at_path(p);
        if !data.is_null() {
            break;
        }
    }
    if data.is_null() {
        log_msg("nib", &format!("nib {name_str} not found"));
        return NIL;
    }
    let nib: Id = msg2(cls("UINib"), "nibWithData:bundle:", data, bundle);
    assoc_set(nib, &NIB_NAME_KEY, name);
    nib
}

unsafe extern "C" fn storyboard_with_name(_cls: Id, _cmd: Sel, name: Id, bundle: Id) -> Id {
    let Some(name_str) = nsstring_to_string(name).filter(|s| !s.is_empty()) else {
        return NIL;
    };
    let bundle = if bundle.is_null() { msg0(cls("NSBundle"), "mainBundle") } else { bundle };
    for cand in [format!("{name_str}~ipad"), name_str] {
        let directory: Id = msg2(bundle, "pathForResource:ofType:", nsstring(&cand), nsstring("storyboardc"));
        if directory.is_null() {
            continue;
        }
        let plist: Id = msg1(directory, "stringByAppendingPathComponent:", nsstring("Info.plist"));
        let info: Id = msg1(cls("NSDictionary"), "dictionaryWithContentsOfFile:", plist);
        if info.is_null() {
            continue;
        }
        let board: Id = objc_autorelease(msg0(cls("UIStoryboard"), "new"));
        assoc_set(board, &STORYBOARD_DIRECTORY_KEY, directory);
        assoc_set(board, &STORYBOARD_INFO_KEY, info);
        return board;
    }
    NIL
}

unsafe extern "C" fn storyboard_instantiate(board: Id, _cmd: Sel, identifier: Id) -> Id {
    if identifier.is_null() {
        return NIL;
    }
    let info = assoc_get(board, &STORYBOARD_INFO_KEY);
    let names: Id = msg1(info, "objectForKey:", nsstring("UIViewControllerIdentifiersToNibNames"));
    let name: Id = msg1(names, "objectForKey:", identifier);
    if name.is_null() {
        return NIL;
    }
    let directory = assoc_get(board, &STORYBOARD_DIRECTORY_KEY);
    let nib_name: Id = msg1(name, "stringByAppendingPathExtension:", nsstring("nib"));
    let path: Id = msg1(directory, "stringByAppendingPathComponent:", nib_name);
    let data: Id = load_nib_data_at_path(path);
    if data.is_null() {
        return NIL;
    }
    let nib: Id = msg2(cls("UINib"), "nibWithData:bundle:", data, NIL);
    assoc_set(nib, &NIB_NAME_KEY, name);
    let dictionary = cls("NSDictionary");
    let external: Id = msg2(dictionary, "dictionaryWithObject:forKey:", board, nsstring("UIStoryboardPlaceholder"));
    let options: Id = msg2(dictionary, "dictionaryWithObject:forKey:", external, nsstring("UINibExternalObjects"));
    let objects: Id = msg2(nib, "instantiateWithOwner:options:", NIL, options);
    for i in 0..nsarray_count(objects) {
        let obj = nsarray_object_at(objects, i);
        if is_kind_of(obj, "UIViewController") {
            assoc_set(obj, &VC_STORYBOARD_KEY, board);
            return obj;
        }
    }
    NIL
}

unsafe extern "C" fn storyboard_initial(board: Id, cmd: Sel) -> Id {
    let info = assoc_get(board, &STORYBOARD_INFO_KEY);
    let entry: Id = msg1(info, "objectForKey:", nsstring("UIStoryboardDesignatedEntryPointIdentifier"));
    storyboard_instantiate(board, cmd, entry)
}

unsafe extern "C" fn storyboard_initial_creator(board: Id, cmd: Sel, creator: Id) -> Id {
    if !creator.is_null() {
        let hdr = &*(creator as *const BlockHeader);
        let f: unsafe extern "C" fn(Id, Id) -> Id = std::mem::transmute(hdr.invoke);
        let res = f(creator, NIL);
        if !res.is_null() {
            assoc_set(res, &VC_STORYBOARD_KEY, board);
            return res;
        }
    }
    storyboard_initial(board, cmd)
}

unsafe extern "C" fn vc_storyboard(controller: Id, _cmd: Sel) -> Id {
    assoc_get(controller, &VC_STORYBOARD_KEY)
}

unsafe extern "C" fn vc_set_storyboard(controller: Id, _cmd: Sel, board: Id) {
    assoc_set(controller, &VC_STORYBOARD_KEY, board);
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
    if !crate::globals::PearUIKitTrySetValueForKey(src, dst, lbl) {
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
                        // The key window, else the first window: as the
                        // host's primary window, without asking the host.
                        let mut w = PearUIKitKeyWindow();
                        if w.is_null() {
                            w = G_WINDOWS.lock().unwrap().first().copied().unwrap_or(0) as Id;
                        }
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
                            invoke_script_call(target, sel(&args[1]), &args[2..]);
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

/// Splits script call arguments: `"quoted text"` is one string, anything
/// else is a space-separated token.
fn script_tokens(joined: &str) -> Vec<(bool, String)> {
    let mut out = Vec::new();
    let mut rest = joined.trim_start();
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix('"') {
            let end = r.find('"').unwrap_or(r.len());
            out.push((true, r[..end].to_string()));
            rest = r.get(end + 1..).unwrap_or("");
        } else {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            out.push((false, rest[..end].to_string()));
            rest = &rest[end..];
        }
        rest = rest.trim_start();
    }
    out
}

/// Calls `target`'s `selector` from an input-script line through NSInvocation,
/// converting each token to the method's argument type.
unsafe fn invoke_script_call(target: Id, selector: Sel, args: &[String]) {
    let sig: Id = msg1(target, "methodSignatureForSelector:", selector);
    if sig.is_null() {
        return;
    }
    let inv: Id = msg1(cls("NSInvocation"), "invocationWithMethodSignature:", sig);
    msg1::<Id, ()>(inv, "setTarget:", target);
    msg1::<Sel, ()>(inv, "setSelector:", selector);
    let nargs: usize = msg0(sig, "numberOfArguments");
    let tokens = script_tokens(&args.join(" "));
    let truthy = |v: &str| v == "YES" || v.parse::<i64>().map(|x| x != 0).unwrap_or(false);
    for (i, (quoted, v)) in tokens.iter().enumerate() {
        let index = i + 2;
        if index >= nargs {
            break;
        }
        let ty: *const c_char = msg1(sig, "getArgumentTypeAtIndex:", index);
        // Skip method type qualifiers (const, in, out, inout, bycopy, byref, oneway).
        let mut tp = ty;
        while !tp.is_null() && matches!(*tp as u8, b'r' | b'n' | b'N' | b'o' | b'O' | b'R' | b'V') {
            tp = tp.add(1);
        }
        let t = if tp.is_null() { b'q' } else { *tp as u8 };
        let set = |p: *const c_void| msg2::<*const c_void, isize, ()>(inv, "setArgument:atIndex:", p, index as isize);
        match t {
            b'@' => {
                let o: Id = if !*quoted && v == "nil" { NIL } else { nsstring(v) };
                set(&o as *const Id as *const c_void);
            }
            b'B' => {
                let b: bool = truthy(v);
                set(&b as *const bool as *const c_void);
            }
            b'c' | b'C' => {
                let c: i8 = if v == "YES" { 1 } else { v.parse::<i64>().unwrap_or(0) as i8 };
                set(&c as *const i8 as *const c_void);
            }
            b'i' | b'I' => {
                let x: i32 = v.parse::<i64>().unwrap_or(0) as i32;
                set(&x as *const i32 as *const c_void);
            }
            b's' | b'S' => {
                let x: i16 = v.parse::<i64>().unwrap_or(0) as i16;
                set(&x as *const i16 as *const c_void);
            }
            b'f' => {
                let x: f32 = v.parse().unwrap_or(0.0);
                set(&x as *const f32 as *const c_void);
            }
            b'd' => {
                let x: f64 = v.parse().unwrap_or(0.0);
                set(&x as *const f64 as *const c_void);
            }
            b':' => {
                let x: Sel = sel(v);
                set(&x as *const Sel as *const c_void);
            }
            _ => {
                let x: i64 = v.parse().unwrap_or(0);
                set(&x as *const i64 as *const c_void);
            }
        }
    }
    msg0::<()>(inv, "retainArguments");
    msg0::<()>(inv, "invoke");
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
        add_method(app, "connectedScenes", app_connected_scenes as Imp, "@@:");
        add_method(app, "openSessions", app_open_sessions as Imp, "@@:");
        add_method(app, "supportsMultipleScenes", app_false as Imp, "B@:");
        add_method(app, "applicationState", app_state as Imp, "q@:");
        add_method(app, "isIdleTimerDisabled", app_idle as Imp, "B@:");
        add_method(app, "setIdleTimerDisabled:", app_set_idle as Imp, "v@:B");
        add_method(app, "statusBarOrientation", app_orient as Imp, "q@:");
        add_method(app, "supportedInterfaceOrientationsForWindow:", app_supported_orientations as Imp, "Q@:@");
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
        add_method(app, "registerUserNotificationSettings:", app_reg_notif_settings as Imp, "v@:@");
        add_method(app, "currentUserNotificationSettings", app_cur_notif_settings as Imp, "@@:");
        add_method(app, "beginBackgroundTaskWithExpirationHandler:", app_begin_bg as Imp, "Q@:@?");
        add_method(app, "beginBackgroundTaskWithName:expirationHandler:", app_begin_bg_name as Imp, "Q@:@@?");
        add_method(app, "endBackgroundTask:", app_noop_u as Imp, "v@:Q");
        add_method(app, "setMinimumBackgroundFetchInterval:", app_noop_f as Imp, "v@:d");
        add_method(app, "sendAction:to:from:forEvent:", app_send_action as Imp, "B@::@@@");
        add_method(app, "beginReceivingRemoteControlEvents", app_noop as Imp, "v@:");
        add_method(app, "endReceivingRemoteControlEvents", app_noop as Imp, "v@:");
        add_method(app, "isIgnoringInteractionEvents", app_false as Imp, "B@:");
        add_method(app, "beginIgnoringInteractionEvents", app_noop as Imp, "v@:");
        add_method(app, "endIgnoringInteractionEvents", app_noop as Imp, "v@:");
        add_method(app, "backgroundRefreshStatus", app_bg_refresh_status as Imp, "q@:");
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
        add_method(tv, "inputAccessoryView", tv_input_accessory as Imp, "@@:");
        add_method(tv, "setInputAccessoryView:", tv_set_input_accessory as Imp, "v@:@");
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
    let sb = cls("UIStoryboard");
    if !sb.is_null() {
        add_class_method(sb, "storyboardWithName:bundle:", storyboard_with_name as Imp, "@@:@@");
        add_method(sb, "instantiateInitialViewController", storyboard_initial as Imp, "@@:");
        add_method(sb, "instantiateInitialViewControllerWithCreator:", storyboard_initial_creator as Imp, "@@:@?");
        add_method(sb, "instantiateViewControllerWithIdentifier:", storyboard_instantiate as Imp, "@@:@");
    }
    let vc = cls("UIViewController");
    if !vc.is_null() {
        add_method(vc, "storyboard", vc_storyboard as Imp, "@@:");
        add_method(vc, "setStoryboard:", vc_set_storyboard as Imp, "v@:@");
    }

    scene_imp::register();
}

mod scene_imp {
    use super::*;

    static WS_GEOM_KEY: u8 = 0;

    unsafe extern "C" fn nil_ret(_s: Id, _c: Sel) -> Id { NIL }
    unsafe extern "C" fn empty_set(_s: Id, _c: Sel) -> Id { msg0(cls("NSSet"), "set") }

    unsafe extern "C" fn scene_del(s: Id, _c: Sel) -> Id { assoc_get(s, &SCENE_DEL_KEY) }
    unsafe extern "C" fn scene_set_del(s: Id, _c: Sel, d: Id) { assoc_set(s, &SCENE_DEL_KEY, d); }
    unsafe extern "C" fn scene_sess(s: Id, _c: Sel) -> Id { assoc_get(s, &SCENE_SESSION_KEY) }
    unsafe extern "C" fn scene_act_state(_s: Id, _c: Sel) -> isize { 0 }
    unsafe extern "C" fn scene_title(s: Id, _c: Sel) -> Id {
        let t = assoc_get(s, &SCENE_TITLE_KEY);
        if t.is_null() { nsstring("") } else { t }
    }
    unsafe extern "C" fn scene_set_title(s: Id, _c: Sel, v: Id) {
        let copied = if v.is_null() { NIL } else { msg0(v, "copy") };
        assoc_set(s, &SCENE_TITLE_KEY, copied);
        if !copied.is_null() {
            objc_release(copied);
        }
    }

    unsafe extern "C" fn wscene_screen(_s: Id, _c: Sel) -> Id { msg0(cls("UIScreen"), "mainScreen") }
    unsafe extern "C" fn wscene_windows(_s: Id, _c: Sel) -> Id { PearUIKitWindows() }
    unsafe extern "C" fn wscene_key_window(_s: Id, _c: Sel) -> Id { PearUIKitKeyWindow() }
    unsafe extern "C" fn wscene_coord_space(_s: Id, _c: Sel) -> Id {
        msg0(msg0(cls("UIScreen"), "mainScreen"), "coordinateSpace")
    }
    unsafe extern "C" fn wscene_traits(_s: Id, _c: Sel) -> Id {
        msg0(msg0(cls("UIScreen"), "mainScreen"), "traitCollection")
    }
    unsafe extern "C" fn wscene_effective_geometry(s: Id, _c: Sel) -> Id {
        let g = assoc_get(s, &WS_GEOM_KEY);
        if !g.is_null() {
            return g;
        }
        let geom: Id = msg0(cls("UIWindowSceneGeometry"), "new");
        assoc_set(s, &WS_GEOM_KEY, geom);
        if !geom.is_null() {
            objc_release(geom);
        }
        geom
    }
    unsafe extern "C" fn geom_system_frame(_s: Id, _c: Sel) -> CGRect {
        msg0(msg0(cls("UIScreen"), "mainScreen"), "bounds")
    }
    unsafe extern "C" fn geom_resizing(_s: Id, _c: Sel) -> isize { 0 }

    unsafe extern "C" fn sess_role(s: Id, _c: Sel) -> Id {
        let r = assoc_get(s, &SESSION_ROLE_KEY);
        if r.is_null() { nsstring("UIWindowSceneSessionRoleApplication") } else { r }
    }
    unsafe extern "C" fn sess_cfg(s: Id, _c: Sel) -> Id { assoc_get(s, &SESSION_CONFIG_KEY) }
    unsafe extern "C" fn sess_scene(s: Id, _c: Sel) -> Id { pear_objc::assoc_get_weak(s, &SESSION_SCENE_KEY) }
    unsafe extern "C" fn sess_pid(_s: Id, _c: Sel) -> Id { nsstring("pear-main-scene") }

    unsafe extern "C" fn scfg_with_name_role(cls_obj: Id, _c: Sel, name: Id, role: Id) -> Id {
        let obj: Id = msg2(msg0(cls_obj, "alloc"), "initWithName:sessionRole:", name, role);
        objc_autorelease(obj)
    }
    unsafe extern "C" fn scfg_init_name_role(s: Id, _c: Sel, name: Id, role: Id) -> Id {
        let o: Id = super0(s, "UISceneConfiguration", "init");
        if !o.is_null() {
            assoc_set(o, &SCFG_NAME_KEY, name);
            let r = if role.is_null() { nsstring("UIWindowSceneSessionRoleApplication") } else { role };
            assoc_set(o, &SCFG_ROLE_KEY, r);
        }
        o
    }
    unsafe extern "C" fn scfg_name(s: Id, _c: Sel) -> Id { assoc_get(s, &SCFG_NAME_KEY) }
    unsafe extern "C" fn scfg_role(s: Id, _c: Sel) -> Id {
        let r = assoc_get(s, &SCFG_ROLE_KEY);
        if r.is_null() { nsstring("UIWindowSceneSessionRoleApplication") } else { r }
    }
    unsafe extern "C" fn scfg_get_cls(s: Id, key: &u8, default: Class) -> Class {
        let v = assoc_get(s, key);
        if v.is_null() { default } else { msg0(v, "pointerValue") }
    }
    unsafe extern "C" fn scfg_set_cls(s: Id, key: &u8, c: Class) {
        let val = if c.is_null() {
            NIL
        } else {
            msg1::<*const c_void, Id>(cls("NSValue"), "valueWithPointer:", c as *const c_void)
        };
        assoc_set(s, key, val);
    }
    unsafe extern "C" fn scfg_del_cls(s: Id, _c: Sel) -> Class { scfg_get_cls(s, &SCFG_DELEGATE_CLS_KEY, std::ptr::null_mut()) }
    unsafe extern "C" fn scfg_set_del_cls(s: Id, _c: Sel, c: Class) { scfg_set_cls(s, &SCFG_DELEGATE_CLS_KEY, c); }
    unsafe extern "C" fn scfg_scene_cls(s: Id, _c: Sel) -> Class { scfg_get_cls(s, &SCFG_SCENE_CLS_KEY, cls("UIWindowScene")) }
    unsafe extern "C" fn scfg_set_scene_cls(s: Id, _c: Sel, c: Class) { scfg_set_cls(s, &SCFG_SCENE_CLS_KEY, c); }

    pub unsafe fn register() {
        let sc = cls("UIScene");
        if !sc.is_null() {
            add_method(sc, "delegate", scene_del as Imp, "@@:");
            add_method(sc, "setDelegate:", scene_set_del as Imp, "v@:@");
            add_method(sc, "session", scene_sess as Imp, "@@:");
            add_method(sc, "activationState", scene_act_state as Imp, "q@:");
            add_method(sc, "title", scene_title as Imp, "@@:");
            add_method(sc, "setTitle:", scene_set_title as Imp, "v@:@");
        }

        let wsc = cls("UIWindowScene");
        if !wsc.is_null() {
            add_method(wsc, "screen", wscene_screen as Imp, "@@:");
            add_method(wsc, "windows", wscene_windows as Imp, "@@:");
            add_method(wsc, "keyWindow", wscene_key_window as Imp, "@@:");
            add_method(wsc, "interfaceOrientation", app_orient as Imp, "q@:");
            add_method(wsc, "coordinateSpace", wscene_coord_space as Imp, "@@:");
            add_method(wsc, "traitCollection", wscene_traits as Imp, "@@:");
            add_method(wsc, "effectiveGeometry", wscene_effective_geometry as Imp, "@@:");
            add_method(wsc, "statusBarManager", nil_ret as Imp, "@@:");
            add_method(wsc, "sizeRestrictions", nil_ret as Imp, "@@:");
        }

        let geom_cls = cls("UIWindowSceneGeometry");
        if !geom_cls.is_null() {
            add_method(geom_cls, "coordinateSpace", wscene_coord_space as Imp, "@@:");
            add_method(geom_cls, "systemFrame", geom_system_frame as Imp, "{CGRect={CGPoint=dd}{CGSize=dd}}@:");
            add_method(geom_cls, "interfaceOrientation", app_orient as Imp, "q@:");
            add_method(geom_cls, "resizingRestrictions", geom_resizing as Imp, "q@:");
        }

        let sess = cls("UISceneSession");
        if !sess.is_null() {
            add_method(sess, "role", sess_role as Imp, "@@:");
            add_method(sess, "configuration", sess_cfg as Imp, "@@:");
            add_method(sess, "scene", sess_scene as Imp, "@@:");
            add_method(sess, "persistentIdentifier", sess_pid as Imp, "@@:");
        }

        let opts = cls("UISceneConnectionOptions");
        if !opts.is_null() {
            add_method(opts, "URLContexts", empty_set as Imp, "@@:");
            add_method(opts, "userActivities", empty_set as Imp, "@@:");
            add_method(opts, "sourceApplication", nil_ret as Imp, "@@:");
            add_method(opts, "handoffUserActivityType", nil_ret as Imp, "@@:");
            add_method(opts, "notificationResponse", nil_ret as Imp, "@@:");
            add_method(opts, "shortcutItem", nil_ret as Imp, "@@:");
        }

        let cfg = cls("UISceneConfiguration");
        if !cfg.is_null() {
            add_class_method(cfg, "configurationWithName:sessionRole:", scfg_with_name_role as Imp, "@@:@@");
            add_method(cfg, "initWithName:sessionRole:", scfg_init_name_role as Imp, "@@:@@");
            add_method(cfg, "name", scfg_name as Imp, "@@:");
            add_method(cfg, "role", scfg_role as Imp, "@@:");
            add_method(cfg, "delegateClass", scfg_del_cls as Imp, "#@:");
            add_method(cfg, "setDelegateClass:", scfg_set_del_cls as Imp, "v@:#");
            add_method(cfg, "sceneClass", scfg_scene_cls as Imp, "#@:");
            add_method(cfg, "setSceneClass:", scfg_set_scene_cls as Imp, "v@:#");
            add_method(cfg, "storyboard", nil_ret as Imp, "@@:");
        }
    }
}
