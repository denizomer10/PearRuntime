//! The macOS host (AppKit): pear-host's `HostPlatform` over the AppKit backend
//! functions in app.rs (`PearBackend*`, still exported for the C ABI).
//! The portable UIKit layer reaches these only through `pear_host::host()`.

use crate::app::{
    PearBackendCreateWindow, PearBackendDisplayLink, PearBackendInitialize, PearBackendOpenURL,
    PearBackendPresentAlert, PearBackendRun, PearBackendScreenInfo, PearBackendSetIdleTimerDisabled,
    PearBackendSetRootLayer, PearBackendShowWindow,
};
use pear_host::{GuestObject, GuestSelector, HostPlatform, LayerRef, Rect, ScreenInfo, Size, WindowHandle};
use pear_objc::{cls, msg1, nsstring, CGPoint, CGRect, CGSize, Id};
use std::ffi::{c_char, c_void, CString};

pub struct MacHost;

extern "C" fn alert_reply(ctx: *mut c_void, index: isize) {
    let reply = unsafe { Box::from_raw(ctx as *mut Box<dyn FnOnce(isize) + Send>) };
    reply(index);
}

extern "C" fn did_finish(ctx: *mut c_void) {
    let f = unsafe { Box::from_raw(ctx as *mut Box<dyn FnOnce()>) };
    f();
}

impl HostPlatform for MacHost {
    fn initialize(&self) {
        PearBackendInitialize();
    }

    fn screen(&self) -> ScreenInfo {
        let info = PearBackendScreenInfo();
        ScreenInfo {
            points: Size { width: info.point_size.width, height: info.point_size.height },
            scale: info.scale,
            max_fps: info.max_fps.max(0) as u32,
        }
    }

    fn create_window(&self, owner: GuestObject, frame: Rect) -> WindowHandle {
        let rect = CGRect {
            origin: CGPoint { x: frame.x, y: frame.y },
            size: CGSize { width: frame.width, height: frame.height },
        };
        WindowHandle(PearBackendCreateWindow(owner.0 as Id, rect))
    }

    fn set_root_layer(&self, window: WindowHandle, layer: LayerRef) {
        PearBackendSetRootLayer(window.0, layer.0 as Id);
    }

    fn show_window(&self, window: WindowHandle, key: bool) {
        PearBackendShowWindow(window.0, key);
    }

    fn display_link(&self, target: GuestObject, selector: GuestSelector) -> GuestObject {
        GuestObject(PearBackendDisplayLink(target.0 as Id, selector.0 as pear_objc::Sel))
    }

    fn open_url(&self, url: &str) -> bool {
        unsafe {
            let ns_url: Id = msg1(cls("NSURL"), "URLWithString:", nsstring(url));
            PearBackendOpenURL(ns_url)
        }
    }

    fn set_idle_timer_disabled(&self, disabled: bool) {
        PearBackendSetIdleTimerDisabled(disabled);
    }

    fn present_alert(&self, title: &str, message: &str, buttons: &[String], reply: Box<dyn FnOnce(isize) + Send>) {
        let title = CString::new(title.replace('\0', "")).unwrap_or_default();
        let message = CString::new(message.replace('\0', "")).unwrap_or_default();
        let owned: Vec<CString> = buttons.iter().map(|b| CString::new(b.replace('\0', "")).unwrap_or_default()).collect();
        let pointers: Vec<*const c_char> = owned.iter().map(|b| b.as_ptr()).collect();
        let ctx = Box::into_raw(Box::new(reply)) as *mut c_void;
        // The backend copies the strings before returning.
        PearBackendPresentAlert(title.as_ptr(), message.as_ptr(), pointers.as_ptr(), pointers.len(), ctx, alert_reply);
    }

    fn run(&self, did_finish_launching: Box<dyn FnOnce()>) {
        let ctx = Box::into_raw(Box::new(did_finish_launching)) as *mut c_void;
        PearBackendRun(ctx, did_finish);
    }
}
