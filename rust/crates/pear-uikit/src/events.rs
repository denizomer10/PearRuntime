//! UIKit as pear-host's event sink: hosts deliver pointer, key, text, window
//! and lifecycle events here (`pear_host::events()`), never to UIKit's C
//! entry points (`PearUIKitDeliver*`, kept exported for the C ABI).

use crate::app::{
    PearUIKitApplicationDidBecomeActive, PearUIKitApplicationDidEnterBackground,
    PearUIKitApplicationWillEnterForeground, PearUIKitApplicationWillResignActive,
    PearUIKitApplicationWillTerminate, PearUIKitDeliverEditCommand, PearUIKitDeliverKey, PearUIKitDeliverText,
    PearUIKitSetKeyWindow, PearUIKitTextInputActive,
};
use crate::basics::PearUIKitDeliverPointer;
use crate::view::PearUIKitWindowDidResize;
use pear_host::{EditCommand, GuestEvents, GuestObject, KeyEvent, Lifecycle, Size, TouchPhase};
use pear_objc::{nsstring, CGPoint, CGSize, Id};

pub struct UIKitEvents;

impl GuestEvents for UIKitEvents {
    fn pointer(&self, window: GuestObject, phase: TouchPhase, x: f64, y: f64, timestamp: f64) {
        PearUIKitDeliverPointer(window.0 as Id, phase as isize, CGPoint { x, y }, timestamp);
    }

    fn key(&self, event: KeyEvent) {
        unsafe {
            PearUIKitDeliverKey(
                event.hid_usage as isize,
                nsstring(&event.characters),
                nsstring(&event.characters_ignoring_modifiers),
                event.modifiers as isize,
                event.down,
                event.timestamp,
            );
        }
    }

    fn text(&self, text: &str) {
        unsafe { PearUIKitDeliverText(nsstring(text)) };
    }

    fn edit_command(&self, command: EditCommand) {
        PearUIKitDeliverEditCommand(command as i32);
    }

    fn text_input_active(&self) -> bool {
        PearUIKitTextInputActive()
    }

    fn window_resized(&self, window: GuestObject, points: Size, scale: f64) {
        PearUIKitWindowDidResize(window.0 as Id, CGSize { width: points.width, height: points.height }, scale);
    }

    fn window_became_key(&self, window: GuestObject) {
        PearUIKitSetKeyWindow(window.0 as Id);
    }

    fn lifecycle(&self, event: Lifecycle) {
        match event {
            Lifecycle::DidBecomeActive => PearUIKitApplicationDidBecomeActive(),
            Lifecycle::WillResignActive => PearUIKitApplicationWillResignActive(),
            Lifecycle::DidEnterBackground => PearUIKitApplicationDidEnterBackground(),
            Lifecycle::WillEnterForeground => PearUIKitApplicationWillEnterForeground(),
            Lifecycle::WillTerminate => PearUIKitApplicationWillTerminate(),
        }
    }
}

/// The host's native surface behind a guest layer, for graphics frameworks
/// in other images (OpenGLES.framework links UIKit, which owns the host).
/// Writes `*out` and returns true when the host can present `layer`.
#[no_mangle]
pub unsafe extern "C" fn PearHostSurfaceForLayer(layer: *mut std::ffi::c_void, out: *mut pear_host::SurfaceInfo) -> bool {
    match pear_host::host().surface(pear_host::LayerRef(layer)) {
        Some(info) if !out.is_null() => {
            out.write(info);
            true
        }
        _ => false,
    }
}
