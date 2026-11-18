//! PearRuntime for Mac Catalyst (`libPearRuntime.dylib`), in Rust.
//!
//! Each module mirrors `modules/<module>/` and installs itself from the image
//! initializer below, gated by PEAR_RUNTIME_DISABLE / PEAR_RUNTIME_ONLY
//! exactly as the Objective-C++ modules are.

// Variadic libc replacements (open, openat, syscall) must keep the variadic
// ABI; build.sh already builds this crate with RUSTC_BOOTSTRAP (std from source).
#![feature(c_variadic)]

pub mod rt;
pub mod geometry;

pub mod core {
    pub mod foundation;
    pub mod host;
    pub mod registry;
}
pub mod input {
    pub mod legacy_touch;
    pub mod mouse_touch;
    pub mod pointer_lock;
    pub mod touch_synth;
    pub mod key_names;
    pub mod virtual_gamepad;
    pub mod web_input;
}
pub mod performance;
pub mod services {
    pub mod beep;
    pub mod consent;
    pub mod network;
    pub mod odr;
    pub mod platform_services;
    pub mod tracking;
}
pub mod device {
    pub mod identity;
    pub mod platform;
    pub mod profile;
}
pub mod graphics {
    pub mod fps_hud;
    pub mod legacy_window;
    pub mod screen;
    pub mod window_chrome;
}
pub mod graphics_metal {
    pub mod diagnostics;
    pub mod layer;
}
pub mod storage {
    pub mod credential;
    pub mod sandbox;
    pub mod path;
    pub mod cloud;
}

unsafe extern "C" fn keychain_log(message: *const std::ffi::c_char) {
    rt::log(rt::cstr(message));
}

/// Runs the Rust modules' initializers in module order.
extern "C" fn pear_catalyst_load() {
    // The keychain store (shared with PearHost) is interposed for the whole
    // process; it only needs the runtime's logger.
    pear_keychain::set_logger(keychain_log);
    unsafe {
        pear_objc::autorelease_pool(|| {
            core::foundation::init();
            input::touch_synth::init();
            input::legacy_touch::init();
            input::pointer_lock::init();
            input::web_input::init();
            device::identity::init();
            device::platform::init();
            storage::credential::init();
            storage::path::init();
            storage::sandbox::init();
            storage::cloud::init();
            services::beep::init();
            services::consent::init();
            services::network::init();
            services::odr::init();
            services::platform_services::init();
            services::tracking::init();
            graphics::screen::init();
            graphics::window_chrome::init();
            performance::init();
            graphics_metal::diagnostics::init();
            graphics_metal::layer::init();
        });
    }
}

#[used]
#[link_section = "__DATA,__mod_init_func,mod_init_funcs"]
static PEAR_CATALYST_INIT: extern "C" fn() = pear_catalyst_load;
