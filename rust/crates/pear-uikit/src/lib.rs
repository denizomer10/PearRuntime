//! PearHost UIKit implementation in Rust.
//!
//! Exports all C/ObjC functions and registers Objective-C methods onto the
//! UIKit class stubs in `UIKitClasses.m` at framework load time.

mod accessibility;
mod app;
mod basics;
mod graphics;
mod view;

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
