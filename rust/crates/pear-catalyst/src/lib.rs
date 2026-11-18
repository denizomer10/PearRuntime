//! PearRuntime for Mac Catalyst (`libPearRuntime.dylib`), in Rust.
//!
//! Each module mirrors `modules/<module>/` and installs itself from the image
//! initializer below, gated by PEAR_RUNTIME_DISABLE / PEAR_RUNTIME_ONLY
//! exactly as the Objective-C++ modules are.

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
    pub mod web_input;
}
pub mod performance;
pub mod services {
    pub mod beep;
    pub mod network;
    pub mod odr;
    pub mod tracking;
}
pub mod device {
    pub mod profile;
}
pub mod graphics {
    pub mod fps_hud;
    pub mod screen;
    pub mod window_chrome;
}
pub mod graphics_metal {
    pub mod diagnostics;
}
pub mod storage {
    pub mod credential;
    pub mod path;
}

/// Runs the Rust modules' initializers in module order.
extern "C" fn pear_catalyst_load() {
    unsafe {
        pear_objc::autorelease_pool(|| {
            core::foundation::init();
            input::legacy_touch::init();
            input::pointer_lock::init();
            input::web_input::init();
            storage::credential::init();
            storage::path::init();
            services::beep::init();
            services::network::init();
            services::odr::init();
            services::tracking::init();
            graphics::screen::init();
            graphics::window_chrome::init();
            performance::init();
            graphics_metal::diagnostics::init();
        });
    }
}

#[used]
#[link_section = "__DATA,__mod_init_func,mod_init_funcs"]
static PEAR_CATALYST_INIT: extern "C" fn() = pear_catalyst_load;
