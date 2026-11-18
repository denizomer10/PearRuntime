//! PearRuntime for Mac Catalyst (`libPearRuntime.dylib`), in Rust.
//!
//! Each module mirrors `modules/<module>/` and installs itself from the image
//! initializer below, gated by PEAR_RUNTIME_DISABLE / PEAR_RUNTIME_ONLY
//! exactly as the Objective-C++ modules are.

pub mod rt;

pub mod core {
    pub mod registry;
}
pub mod input {
    pub mod legacy_touch;
    pub mod pointer_lock;
}
pub mod performance;
pub mod services {
    pub mod beep;
    pub mod tracking;
}
pub mod graphics {
    pub mod fps_hud;
    pub mod window_chrome;
}
pub mod graphics_metal {
    pub mod diagnostics;
}
pub mod storage {
    pub mod credential;
}

/// Runs the Rust modules' initializers in module order.
extern "C" fn pear_catalyst_load() {
    unsafe {
        pear_objc::autorelease_pool(|| {
            input::legacy_touch::init();
            input::pointer_lock::init();
            storage::credential::init();
            services::beep::init();
            services::tracking::init();
            graphics::window_chrome::init();
            performance::init();
            graphics_metal::diagnostics::init();
        });
    }
}

#[used]
#[link_section = "__DATA,__mod_init_func,mod_init_funcs"]
static PEAR_CATALYST_INIT: extern "C" fn() = pear_catalyst_load;
