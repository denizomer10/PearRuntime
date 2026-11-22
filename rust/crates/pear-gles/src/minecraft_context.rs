//! Minecraft creates its EAGL drawable on the main thread and later starts
//! issuing framebuffer commands from its rendering pool with no current context.

use super::{eagl_current_context, eagl_set_current_context, gl_symbols::CURRENT_API};
use pear_objc::{cls, msg0, nsstring_to_string, objc_release, objc_retain, sel, Id, NIL};
use std::ffi::{c_char, c_int, c_void, CStr};
use std::sync::{Mutex, OnceLock};

struct Handoff {
    context: usize,
    detach_queued: bool,
    detached: bool,
}

static HANDOFF: Mutex<Option<Handoff>> = Mutex::new(None);

extern "C" {
    static _dispatch_main_q: c_void;
    fn dispatch_async_f(
        queue: *const c_void,
        context: *mut c_void,
        callback: unsafe extern "C" fn(*mut c_void),
    );
    fn pthread_main_np() -> c_int;
    fn pthread_self() -> *mut c_void;
    fn pthread_getname_np(thread: *mut c_void, name: *mut c_char, len: usize) -> c_int;
    fn PearHostLogMessage(area: *const c_char, message: *const c_char);
}

fn enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| unsafe {
        let bundle: Id = msg0(cls("NSBundle"), "mainBundle");
        nsstring_to_string(msg0(bundle, "bundleIdentifier")).as_deref() == Some("com.mojang.minecraftpe")
    })
}

pub unsafe fn remember_drawable_context(context: Id) {
    if pthread_main_np() == 0 || !enabled() {
        return;
    }
    let mut handoff = HANDOFF.lock().unwrap();
    if handoff.as_ref().is_some_and(|state| state.context == context as usize) {
        return;
    }
    let previous = handoff.replace(Handoff {
        context: objc_retain(context) as usize,
        detach_queued: false,
        detached: false,
    });
    drop(handoff);
    if let Some(previous) = previous {
        objc_release(previous.context as Id);
    }
}

unsafe extern "C" fn detach_from_main(context: *mut c_void) {
    let context = context as Id;
    let mut handoff = HANDOFF.lock().unwrap();
    if let Some(state) = handoff.as_mut().filter(|state| state.context == context as usize) {
        let current = eagl_current_context(NIL, sel("currentContext"));
        state.detached = current != context
            || eagl_set_current_context(NIL, sel("setCurrentContext:"), NIL);
        state.detach_queued = false;
    }
    drop(handoff);
    objc_release(context);
}

pub unsafe fn bind_for_framebuffer() {
    if CURRENT_API.with(|api| api.get()) != 0 || !enabled() || pthread_main_np() != 0 {
        return;
    }
    let mut name = [0; 64];
    if pthread_getname_np(pthread_self(), name.as_mut_ptr(), name.len()) != 0
        || !CStr::from_ptr(name.as_ptr()).to_bytes().starts_with(b"Rendering Pool(")
    {
        return;
    }
    let mut handoff = HANDOFF.lock().unwrap();
    let Some(state) = handoff.as_mut() else { return };
    let context = state.context as Id;
    if !state.detached {
        if !state.detach_queued {
            state.detach_queued = true;
            // The game's main thread can wait for the render worker. Queue the
            // detach without waiting, then pick it up on the next bind.
            dispatch_async_f(&raw const _dispatch_main_q, objc_retain(context) as *mut c_void, detach_from_main);
        }
        return;
    }
    if eagl_set_current_context(NIL, sel("setCurrentContext:"), context) {
        *handoff = None;
        drop(handoff);
        objc_release(context);
        PearHostLogMessage(c"gl".as_ptr(), c"Minecraft drawable context bound on its render worker".as_ptr());
    }
}
