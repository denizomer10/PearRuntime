//! objc_msgSend and friends (arm64). A nil receiver returns zero; otherwise
//! the IMP is found (sending +initialize first if needed) with every
//! argument register preserved, and the call tail-jumps to it. A selector
//! the class does not implement goes through dynamic method resolution
//! (+resolveInstanceMethod: / +resolveClassMethod:) and then
//! forwardingTargetForSelector:; forwardInvocation: needs NSInvocation
//! (Foundation) and is not available yet, so the send then aborts with
//! objc4's message.

use crate::abi::*;
use crate::runtime;

/// What the assembly gets back: the IMP to jump to and the receiver to
/// pass it (a forwarding target may replace the original one).
#[repr(C)]
pub struct Dispatch {
    pub imp: Imp,
    pub receiver: Id,
}

#[cold]
fn unrecognized(receiver: Id, cls: *const ClassT, s: Sel) -> ! {
    let meta = runtime::is_meta(cls);
    let name = runtime::class(cls).map_or_else(|| format!("{cls:p}"), |c| c.name.clone());
    eprintln!(
        "[PearRuntime][objc] {}[{name} {}]: unrecognized selector sent to {} {receiver:p}",
        if meta { "+" } else { "-" },
        runtime::sel_name(s),
        if meta { "class" } else { "instance" },
    );
    std::process::abort()
}

/// The IMP of a selector a class responds to (Rust-side sends).
pub unsafe fn imp_for(receiver: Id, s: Sel) -> Imp {
    let d = lookup_for_send(receiver, s);
    d.imp
}

/// Dispatch for sending `s` to `receiver` (not nil).
pub extern "C" fn lookup_for_send(receiver: Id, s: Sel) -> Dispatch {
    unsafe {
        let cls = runtime::object_class(receiver);
        let target = if runtime::is_meta(cls) { receiver as *const ClassT } else { cls };
        runtime::ensure_initialized(target);
        match runtime::lookup(cls, s) {
            Some(imp) => {
                if !is_tagged_pointer(receiver) {
                    runtime::fast_cache_insert(cls, s, imp);
                }
                Dispatch { imp, receiver }
            }
            None => forward(receiver, cls, s, 0),
        }
    }
}

/// Dispatch for objc_msgSendSuper2: lookup starts at `current_class`'s
/// superclass.
pub extern "C" fn lookup_for_super2(sup: *const ObjcSuper, s: Sel) -> Dispatch {
    unsafe {
        let start = runtime::superclass((*sup).current_class);
        match runtime::lookup(start, s) {
            Some(imp) => Dispatch { imp, receiver: (*sup).receiver },
            None => forward((*sup).receiver, start, s, 0),
        }
    }
}

/// Dispatch for objc_msgSendSuper: lookup starts at `current_class` itself.
pub extern "C" fn lookup_for_super(sup: *const ObjcSuper, s: Sel) -> Dispatch {
    unsafe {
        let start = (*sup).current_class;
        match runtime::lookup(start, s) {
            Some(imp) => Dispatch { imp, receiver: (*sup).receiver },
            None => forward((*sup).receiver, start, s, 0),
        }
    }
}

/// Dispatch for _objc_msgForward used as an IMP: forward `s` as if the
/// receiver's class did not implement it.
pub extern "C" fn lookup_for_forward(receiver: Id, s: Sel) -> Dispatch {
    unsafe { forward(receiver, runtime::object_class(receiver), s, 1) }
}

unsafe fn send_bool(receiver: Id, name: &str, arg: Sel) -> bool {
    let s = runtime::sel(name);
    match runtime::lookup(runtime::object_class(receiver), s) {
        Some(imp) => {
            let f: extern "C" fn(Id, Sel, Sel) -> bool = std::mem::transmute(imp);
            f(receiver, s, arg)
        }
        None => false,
    }
}

/// Method resolution and fast forwarding for an unimplemented selector.
/// `skip_resolve`: the IMP called was _objc_msgForward itself.
unsafe fn forward(receiver: Id, cls: *const ClassT, s: Sel, skip_resolve: u8) -> Dispatch {
    let meta = runtime::is_meta(cls);
    if skip_resolve == 0 {
        // +resolveInstanceMethod: / +resolveClassMethod: may add the method.
        let class_obj = if meta { receiver } else { cls as Id };
        let resolver = if meta { "resolveClassMethod:" } else { "resolveInstanceMethod:" };
        if send_bool(class_obj, resolver, s) {
            if let Some(imp) = runtime::lookup(cls, s) {
                return Dispatch { imp, receiver };
            }
        }
    }
    // forwardingTargetForSelector: hands the message to another object.
    let fwd = runtime::sel("forwardingTargetForSelector:");
    if let Some(imp) = runtime::lookup(cls, fwd) {
        let f: extern "C" fn(Id, Sel, Sel) -> Id = std::mem::transmute(imp);
        let target = f(receiver, fwd, s);
        if !target.is_null() && target != receiver {
            return lookup_for_send(target, s);
        }
    }
    unrecognized(receiver, cls, s)
}

macro_rules! dispatching_stub {
    ($lookup:path) => {
        core::arch::naked_asm!(
            "cbz x0, 9f",
            "stp x29, x30, [sp, #-16]!",
            "mov x29, sp",
            "sub sp, sp, #208",
            "stp x0, x1, [sp, #0]",
            "stp x2, x3, [sp, #16]",
            "stp x4, x5, [sp, #32]",
            "stp x6, x7, [sp, #48]",
            "str x8, [sp, #64]",
            "stp q0, q1, [sp, #80]",
            "stp q2, q3, [sp, #112]",
            "stp q4, q5, [sp, #144]",
            "stp q6, q7, [sp, #176]",
            "bl {lookup}",
            // x0 = IMP, x1 = receiver
            "mov x17, x0",
            "mov x16, x1",
            "ldp x0, x1, [sp, #0]",
            "ldp x2, x3, [sp, #16]",
            "ldp x4, x5, [sp, #32]",
            "ldp x6, x7, [sp, #48]",
            "ldr x8, [sp, #64]",
            "ldp q0, q1, [sp, #80]",
            "ldp q2, q3, [sp, #112]",
            "ldp q4, q5, [sp, #144]",
            "ldp q6, q7, [sp, #176]",
            "mov sp, x29",
            "ldp x29, x30, [sp], #16",
            "mov x0, x16",
            "br x17",
            // nil receiver: zero the integer and float result registers.
            "9:",
            "mov x1, #0",
            "movi d0, #0",
            "movi d1, #0",
            "movi d2, #0",
            "movi d3, #0",
            "ret",
            lookup = sym $lookup,
        )
    };
}

macro_rules! dispatching_stub_cached {
    ($lookup:path) => {
        core::arch::naked_asm!(
            "cbz x0, 9f",
            "tst x0, #1",
            "b.ne 1f",
            "tbnz x0, #63, 1f",
            "ldr x16, [x0]",
            "ldr x11, [x16, #16]",
            "ldr x10, [x11]",
            "cbz x10, 1f",
            "ldr w12, [x11, #8]",
            "lsr x13, x1, #3",
            "and x13, x13, x12",
            "add x14, x10, x13, lsl #4",
            "ldp x15, x17, [x14]",
            "cmp x15, x1",
            "b.ne 1f",
            "cbz x17, 1f",
            "br x17",
            "1:",
            "stp x29, x30, [sp, #-16]!",
            "mov x29, sp",
            "sub sp, sp, #208",
            "stp x0, x1, [sp, #0]",
            "stp x2, x3, [sp, #16]",
            "stp x4, x5, [sp, #32]",
            "stp x6, x7, [sp, #48]",
            "str x8, [sp, #64]",
            "stp q0, q1, [sp, #80]",
            "stp q2, q3, [sp, #112]",
            "stp q4, q5, [sp, #144]",
            "stp q6, q7, [sp, #176]",
            "bl {lookup}",
            // x0 = IMP, x1 = receiver
            "mov x17, x0",
            "mov x16, x1",
            "ldp x0, x1, [sp, #0]",
            "ldp x2, x3, [sp, #16]",
            "ldp x4, x5, [sp, #32]",
            "ldp x6, x7, [sp, #48]",
            "ldr x8, [sp, #64]",
            "ldp q0, q1, [sp, #80]",
            "ldp q2, q3, [sp, #112]",
            "ldp q4, q5, [sp, #144]",
            "ldp q6, q7, [sp, #176]",
            "mov sp, x29",
            "ldp x29, x30, [sp], #16",
            "mov x0, x16",
            "br x17",
            // nil receiver: zero the integer and float result registers.
            "9:",
            "mov x1, #0",
            "movi d0, #0",
            "movi d1, #0",
            "movi d2, #0",
            "movi d3, #0",
            "ret",
            lookup = sym $lookup,
        )
    };
}

#[unsafe(naked)]
pub extern "C" fn objc_msg_send() {
    dispatching_stub_cached!(lookup_for_send)
}

/// x0 is a `struct objc_super *` (never nil).
#[unsafe(naked)]
pub extern "C" fn objc_msg_send_super2() {
    dispatching_stub!(lookup_for_super2)
}

#[unsafe(naked)]
pub extern "C" fn objc_msg_send_super() {
    dispatching_stub!(lookup_for_super)
}

#[unsafe(naked)]
pub extern "C" fn objc_msg_forward() {
    dispatching_stub!(lookup_for_forward)
}

/// objc_retain_xN / objc_release_xN: newer toolchains call these to retain
/// or release the object in register xN (result in x0, as objc_retain).
macro_rules! register_variants {
    ($($name:ident, $target:path, $reg:literal);* $(;)?) => {$(
        #[unsafe(naked)]
        pub extern "C" fn $name() {
            core::arch::naked_asm!(concat!("mov x0, ", $reg), "b {f}", f = sym $target)
        }
    )*};
}

register_variants! {
    retain_x1, crate::objc_retain_entry, "x1"; retain_x2, crate::objc_retain_entry, "x2";
    retain_x3, crate::objc_retain_entry, "x3"; retain_x4, crate::objc_retain_entry, "x4";
    retain_x5, crate::objc_retain_entry, "x5"; retain_x6, crate::objc_retain_entry, "x6";
    retain_x7, crate::objc_retain_entry, "x7"; retain_x8, crate::objc_retain_entry, "x8";
    retain_x9, crate::objc_retain_entry, "x9"; retain_x10, crate::objc_retain_entry, "x10";
    retain_x11, crate::objc_retain_entry, "x11"; retain_x12, crate::objc_retain_entry, "x12";
    retain_x13, crate::objc_retain_entry, "x13"; retain_x14, crate::objc_retain_entry, "x14";
    retain_x15, crate::objc_retain_entry, "x15"; retain_x19, crate::objc_retain_entry, "x19";
    retain_x20, crate::objc_retain_entry, "x20"; retain_x21, crate::objc_retain_entry, "x21";
    retain_x22, crate::objc_retain_entry, "x22"; retain_x23, crate::objc_retain_entry, "x23";
    retain_x24, crate::objc_retain_entry, "x24"; retain_x25, crate::objc_retain_entry, "x25";
    retain_x26, crate::objc_retain_entry, "x26"; retain_x27, crate::objc_retain_entry, "x27";
    retain_x28, crate::objc_retain_entry, "x28";
    release_x1, crate::objc_release_entry, "x1"; release_x2, crate::objc_release_entry, "x2";
    release_x3, crate::objc_release_entry, "x3"; release_x4, crate::objc_release_entry, "x4";
    release_x5, crate::objc_release_entry, "x5"; release_x6, crate::objc_release_entry, "x6";
    release_x7, crate::objc_release_entry, "x7"; release_x8, crate::objc_release_entry, "x8";
    release_x9, crate::objc_release_entry, "x9"; release_x10, crate::objc_release_entry, "x10";
    release_x11, crate::objc_release_entry, "x11"; release_x12, crate::objc_release_entry, "x12";
    release_x13, crate::objc_release_entry, "x13"; release_x14, crate::objc_release_entry, "x14";
    release_x15, crate::objc_release_entry, "x15"; release_x19, crate::objc_release_entry, "x19";
    release_x20, crate::objc_release_entry, "x20"; release_x21, crate::objc_release_entry, "x21";
    release_x22, crate::objc_release_entry, "x22"; release_x23, crate::objc_release_entry, "x23";
    release_x24, crate::objc_release_entry, "x24"; release_x25, crate::objc_release_entry, "x25";
    release_x26, crate::objc_release_entry, "x26"; release_x27, crate::objc_release_entry, "x27";
    release_x28, crate::objc_release_entry, "x28";
}

/// Looks up a register variant by its Mach-O name.
pub fn register_variant(name: &str) -> Option<usize> {
    let (kind, reg) = name.strip_prefix("_objc_retain_x").map(|r| ("retain", r))
        .or_else(|| name.strip_prefix("_objc_release_x").map(|r| ("release", r)))?;
    let f: extern "C" fn() = match (kind, reg) {
        ("retain", "1") => retain_x1 as extern "C" fn(), ("retain", "2") => retain_x2, ("retain", "3") => retain_x3,
        ("retain", "4") => retain_x4, ("retain", "5") => retain_x5, ("retain", "6") => retain_x6,
        ("retain", "7") => retain_x7, ("retain", "8") => retain_x8, ("retain", "9") => retain_x9,
        ("retain", "10") => retain_x10, ("retain", "11") => retain_x11, ("retain", "12") => retain_x12,
        ("retain", "13") => retain_x13, ("retain", "14") => retain_x14, ("retain", "15") => retain_x15,
        ("retain", "19") => retain_x19, ("retain", "20") => retain_x20, ("retain", "21") => retain_x21,
        ("retain", "22") => retain_x22, ("retain", "23") => retain_x23, ("retain", "24") => retain_x24,
        ("retain", "25") => retain_x25, ("retain", "26") => retain_x26, ("retain", "27") => retain_x27,
        ("retain", "28") => retain_x28,
        ("release", "1") => release_x1, ("release", "2") => release_x2, ("release", "3") => release_x3,
        ("release", "4") => release_x4, ("release", "5") => release_x5, ("release", "6") => release_x6,
        ("release", "7") => release_x7, ("release", "8") => release_x8, ("release", "9") => release_x9,
        ("release", "10") => release_x10, ("release", "11") => release_x11, ("release", "12") => release_x12,
        ("release", "13") => release_x13, ("release", "14") => release_x14, ("release", "15") => release_x15,
        ("release", "19") => release_x19, ("release", "20") => release_x20, ("release", "21") => release_x21,
        ("release", "22") => release_x22, ("release", "23") => release_x23, ("release", "24") => release_x24,
        ("release", "25") => release_x25, ("release", "26") => release_x26, ("release", "27") => release_x27,
        ("release", "28") => release_x28,
        _ => return None,
    };
    Some(f as *const () as usize)
}
