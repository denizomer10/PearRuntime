// PearHost SafariServices.framework (`SFSafariViewController`,
// `SFSafariViewControllerConfiguration`, `SFAuthenticationSession`).

use pear_objc::{
    assoc_get, assoc_get_weak, assoc_set, assoc_set_bool, assoc_set_i64, assoc_set_weak,
    assoc_bool, assoc_i64, class_getSuperclass, msg0, msg2, objc_msgSendSuper, objc_release,
    replace_method as add_method, sel, Class, Id, Imp, ObjcSuper, Sel, NIL,
};
use std::ffi::c_void;

pear_objc::image_info!();

extern "C" {
    #[link_name = "OBJC_CLASS_$_NSObject"]
    static NSObject_CLASS: c_void;
    #[link_name = "OBJC_METACLASS_$_NSObject"]
    static NSObject_META: c_void;
    #[link_name = "OBJC_CLASS_$_UIViewController"]
    static UIViewController_CLASS: c_void;
    #[link_name = "OBJC_METACLASS_$_UIViewController"]
    static UIViewController_META: c_void;
    fn PearMissingInstallForImage(address_in_image: *const c_void);
}

pear_objc::empty_class!(SFAuthenticationSession : &raw const super::NSObject_CLASS, &raw const super::NSObject_META);
pear_objc::empty_class!(SFSafariViewController : &raw const super::UIViewController_CLASS, &raw const super::UIViewController_META);
pear_objc::empty_class!(SFSafariViewControllerConfiguration : &raw const super::NSObject_CLASS, &raw const super::NSObject_META);

static CFG_READER_KEY: u8 = 0;
static CFG_BAR_COLLAPSING_KEY: u8 = 0;

unsafe extern "C" fn cfg_reader(self_obj: Id, _cmd: Sel) -> bool {
    assoc_bool(self_obj, &CFG_READER_KEY, false)
}
unsafe extern "C" fn cfg_set_reader(self_obj: Id, _cmd: Sel, v: bool) {
    assoc_set_bool(self_obj, &CFG_READER_KEY, v);
}
unsafe extern "C" fn cfg_bar_collapsing(self_obj: Id, _cmd: Sel) -> bool {
    assoc_bool(self_obj, &CFG_BAR_COLLAPSING_KEY, true)
}
unsafe extern "C" fn cfg_set_bar_collapsing(self_obj: Id, _cmd: Sel, v: bool) {
    assoc_set_bool(self_obj, &CFG_BAR_COLLAPSING_KEY, v);
}

static SVC_URL_KEY: u8 = 0;
static SVC_CFG_KEY: u8 = 0;
static SVC_DELEGATE_KEY: u8 = 0;
static SVC_BAR_TINT_KEY: u8 = 0;
static SVC_CTRL_TINT_KEY: u8 = 0;
static SVC_DISMISS_STYLE_KEY: u8 = 0;

unsafe extern "C" fn svc_init_url_cfg(self_obj: Id, _cmd: Sel, url: Id, cfg: Id) -> Id {
    let mut sup = ObjcSuper {
        receiver: self_obj,
        super_class: class_getSuperclass(SFSafariViewController::CLASS.ptr() as Class),
    };
    let send: unsafe extern "C" fn(*mut ObjcSuper, Sel, Id, Id) -> Id =
        std::mem::transmute(objc_msgSendSuper as *const c_void);
    let obj = send(&mut sup, sel("initWithNibName:bundle:"), NIL, NIL);
    if !obj.is_null() {
        assoc_set(obj, &SVC_URL_KEY, url);
        if !cfg.is_null() {
            assoc_set(obj, &SVC_CFG_KEY, cfg);
        } else {
            let def_cfg: Id = msg0(SFSafariViewControllerConfiguration::CLASS.ptr() as Id, "new");
            assoc_set(obj, &SVC_CFG_KEY, def_cfg);
            objc_release(def_cfg);
        }
    }
    obj
}

unsafe extern "C" fn svc_init_url(self_obj: Id, _cmd: Sel, url: Id) -> Id {
    msg2(self_obj, "initWithURL:configuration:", url, NIL)
}

unsafe extern "C" fn svc_init_url_reader(self_obj: Id, _cmd: Sel, url: Id, reader: bool) -> Id {
    let cfg: Id = msg0(SFSafariViewControllerConfiguration::CLASS.ptr() as Id, "new");
    if !cfg.is_null() {
        pear_objc::msg1::<bool, ()>(cfg, "setEntersReaderIfAvailable:", reader);
    }
    let obj = msg2(self_obj, "initWithURL:configuration:", url, cfg);
    if !cfg.is_null() {
        objc_release(cfg);
    }
    obj
}

unsafe extern "C" fn svc_configuration(self_obj: Id, _cmd: Sel) -> Id {
    assoc_get(self_obj, &SVC_CFG_KEY)
}

unsafe extern "C" fn svc_delegate(self_obj: Id, _cmd: Sel) -> Id {
    assoc_get_weak(self_obj, &SVC_DELEGATE_KEY)
}

unsafe extern "C" fn svc_set_delegate(self_obj: Id, _cmd: Sel, del: Id) {
    assoc_set_weak(self_obj, &SVC_DELEGATE_KEY, del);
}

unsafe extern "C" fn svc_bar_tint(self_obj: Id, _cmd: Sel) -> Id {
    assoc_get(self_obj, &SVC_BAR_TINT_KEY)
}

unsafe extern "C" fn svc_set_bar_tint(self_obj: Id, _cmd: Sel, color: Id) {
    assoc_set(self_obj, &SVC_BAR_TINT_KEY, color);
}

unsafe extern "C" fn svc_ctrl_tint(self_obj: Id, _cmd: Sel) -> Id {
    assoc_get(self_obj, &SVC_CTRL_TINT_KEY)
}

unsafe extern "C" fn svc_set_ctrl_tint(self_obj: Id, _cmd: Sel, color: Id) {
    assoc_set(self_obj, &SVC_CTRL_TINT_KEY, color);
}

unsafe extern "C" fn svc_dismiss_style(self_obj: Id, _cmd: Sel) -> isize {
    assoc_i64(self_obj, &SVC_DISMISS_STYLE_KEY, 0) as isize
}

unsafe extern "C" fn svc_set_dismiss_style(self_obj: Id, _cmd: Sel, style: isize) {
    assoc_set_i64(self_obj, &SVC_DISMISS_STYLE_KEY, style as i64);
}

unsafe extern "C" fn auth_init(self_obj: Id, _cmd: Sel, _url: Id, _scheme: Id, _handler: Id) -> Id {
    let mut sup = ObjcSuper {
        receiver: self_obj,
        super_class: class_getSuperclass(SFAuthenticationSession::CLASS.ptr() as Class),
    };
    let send: unsafe extern "C" fn(*mut ObjcSuper, Sel) -> Id =
        std::mem::transmute(objc_msgSendSuper as *const c_void);
    send(&mut sup, sel("init"))
}

unsafe extern "C" fn auth_start(_self_obj: Id, _cmd: Sel) -> bool {
    false
}

unsafe extern "C" fn auth_cancel(_self_obj: Id, _cmd: Sel) {}

extern "C" fn pear_host_framework_load() {
    unsafe {
        let cfg = SFSafariViewControllerConfiguration::CLASS.ptr() as Class;
        add_method(cfg, "entersReaderIfAvailable", cfg_reader as Imp, "B@:");
        add_method(cfg, "setEntersReaderIfAvailable:", cfg_set_reader as Imp, "v@:B");
        add_method(cfg, "barCollapsingEnabled", cfg_bar_collapsing as Imp, "B@:");
        add_method(cfg, "setBarCollapsingEnabled:", cfg_set_bar_collapsing as Imp, "v@:B");

        let svc = SFSafariViewController::CLASS.ptr() as Class;
        add_method(svc, "initWithURL:configuration:", svc_init_url_cfg as Imp, "@@:@@");
        add_method(svc, "initWithURL:", svc_init_url as Imp, "@@:@");
        add_method(svc, "initWithURL:entersReaderIfAvailable:", svc_init_url_reader as Imp, "@@:@B");
        add_method(svc, "configuration", svc_configuration as Imp, "@@:");
        add_method(svc, "delegate", svc_delegate as Imp, "@@:");
        add_method(svc, "setDelegate:", svc_set_delegate as Imp, "v@:@");
        add_method(svc, "preferredBarTintColor", svc_bar_tint as Imp, "@@:");
        add_method(svc, "setPreferredBarTintColor:", svc_set_bar_tint as Imp, "v@:@");
        add_method(svc, "preferredControlTintColor", svc_ctrl_tint as Imp, "@@:");
        add_method(svc, "setPreferredControlTintColor:", svc_set_ctrl_tint as Imp, "v@:@");
        add_method(svc, "dismissButtonStyle", svc_dismiss_style as Imp, "q@:");
        add_method(svc, "setDismissButtonStyle:", svc_set_dismiss_style as Imp, "v@:q");

        let auth = SFAuthenticationSession::CLASS.ptr() as Class;
        add_method(auth, "initWithURL:callbackURLScheme:completionHandler:", auth_init as Imp, "@@:@@@?");
        add_method(auth, "start", auth_start as Imp, "B@:");
        add_method(auth, "cancel", auth_cancel as Imp, "v@:");

        PearMissingInstallForImage(pear_host_framework_load as *const c_void);
    }
}

#[used]
#[link_section = "__DATA,__mod_init_func,mod_init_funcs"]
static PEAR_HOST_FRAMEWORK_INIT: extern "C" fn() = pear_host_framework_load;
