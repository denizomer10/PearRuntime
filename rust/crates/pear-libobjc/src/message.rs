//! objc_msgSend and objc_msgSendSuper2 (arm64): nil receivers return zero;
//! otherwise the IMP is looked up (sending +initialize first if needed) with
//! every argument register preserved, and the call tail-jumps to it.
//! Unrecognized selectors abort with objc4's message.

use crate::abi::*;
use crate::runtime;

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

/// The IMP for sending `s` to `receiver` (not nil).
pub extern "C" fn lookup_for_send(receiver: Id, s: Sel) -> Imp {
    unsafe {
        let cls = runtime::object_class(receiver);
        // Messaging a class initializes it; so does messaging an instance
        // whose class was created without a message (rare).
        let target = if runtime::is_meta(cls) { receiver as *const ClassT } else { cls };
        runtime::ensure_initialized(target);
        runtime::lookup(cls, s).unwrap_or_else(|| unrecognized(receiver, cls, s))
    }
}

/// The IMP for a super send: lookup starts at `current_class`'s superclass.
pub extern "C" fn lookup_for_super(sup: *const ObjcSuper, s: Sel) -> Imp {
    unsafe {
        let start = runtime::superclass((*sup).current_class);
        runtime::lookup(start, s).unwrap_or_else(|| unrecognized((*sup).receiver, start, s))
    }
}

macro_rules! save_restore_call {
    ($lookup:path, $after:literal) => {
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
            "mov x17, x0",
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
            $after,
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
    save_restore_call!(lookup_for_send, "nop")
}

/// x0 is a `struct objc_super *`; the method gets its receiver in x0.
#[unsafe(naked)]
pub extern "C" fn objc_msg_send_super2() {
    save_restore_call!(lookup_for_super, "ldr x0, [x0]")
}
