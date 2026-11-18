//! PearRuntime for Mac Catalyst (`libPearRuntime.dylib`), in Rust.
//!
//! Each module mirrors `modules/<module>/` and installs itself from the image
//! initializer below, gated by PEAR_RUNTIME_DISABLE / PEAR_RUNTIME_ONLY
//! exactly as the Objective-C++ modules are.

pub mod rt;

pub mod core {
    pub mod registry;
}
pub mod services {
    pub mod beep;
    pub mod tracking;
}
pub mod graphics_metal {
    pub mod diagnostics;
}

/// Runs the Rust modules' initializers in module order.
extern "C" fn pear_catalyst_load() {
    unsafe {
        pear_objc::autorelease_pool(|| {
            services::beep::init();
            services::tracking::init();
            graphics_metal::diagnostics::init();
        });
    }
}

#[used]
#[link_section = "__DATA,__mod_init_func,mod_init_funcs"]
static PEAR_CATALYST_INIT: extern "C" fn() = pear_catalyst_load;
