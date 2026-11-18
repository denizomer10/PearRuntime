//! PearHost UIKit implementation in Rust.
//!
//! Exports all C/ObjC functions and registers Objective-C methods onto the
//! UIKit class stubs in `UIKitClasses.m` at framework load time.

// pear-core (logging, missing-call reporting, forwarding hooks) is reached only
// through its C exports; link it into this static library explicitly.
extern crate pear_core;

mod accessibility;
mod app;
mod basics;
mod graphics;
mod view;

extern "C" {
    fn PearMissingInstallForImage(address_in_image: *const std::ffi::c_void);
    fn PearHostLogMessage(area: *const std::ffi::c_char, message: *const std::ffi::c_char);
}

/// Image initializer: registers the Rust implementations on the UIKit class
/// symbols and installs the missing-call reporter before the app's first message.
unsafe extern "C" fn keychain_log(message: *const std::ffi::c_char) {
    PearHostLogMessage(std::ptr::null(), message);
}

extern "C" fn pear_uikit_load() {
    // Keychain API over a local store (pear-keychain), interposed process-wide.
    pear_keychain::set_logger(keychain_log);
    let version = option_env!("PEAR_RUNTIME_VERSION").unwrap_or("dev");
    let msg = std::ffi::CString::new(format!("PearHost UIKit {version} (no iOSSupport) loading")).unwrap_or_default();
    unsafe {
        PearHostLogMessage(c"".as_ptr(), msg.as_ptr());
        pear_uikit_register_classes();
        PearMissingInstallForImage(pear_uikit_load as *const std::ffi::c_void);
    }
}

#[used]
#[link_section = "__DATA,__mod_init_func,mod_init_funcs"]
static PEAR_UIKIT_INIT: extern "C" fn() = pear_uikit_load;

#[no_mangle]
pub extern "C" fn pear_uikit_register_classes() {
    unsafe {
        accessibility::register();
        graphics::register();
        basics::register_classes();
        view::register_classes();
        app::register_classes();
    }
}
