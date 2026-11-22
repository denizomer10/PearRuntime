//! Darwin thread-local variables for loaded images.
//!
//! Each `__thread_vars` entry is a descriptor `{thunk, key, offset}`; code
//! calls `thunk(descriptor)` and gets the variable's address for the calling
//! thread in x0. Callers may assume every register but x0, x16 and x17 is
//! preserved (dyld's `tlv_get_addr` contract), so the thunk is assembly that
//! saves the rest around the Rust lookup. A thread's block for an image is
//! allocated on first use from the image's template (`__thread_data`
//! contents followed by zeroed `__thread_bss`) and freed when the thread exits.

use std::cell::RefCell;
use std::sync::RwLock;

#[repr(C)]
pub struct Descriptor {
    pub thunk: usize,
    pub key: usize,
    pub offset: usize,
}

/// Per-image templates + optional TLV initializer function pointers; `key` is the index + 1.
static TEMPLATES: RwLock<Vec<(Vec<u8>, Vec<usize>)>> = RwLock::new(Vec::new());

/// Registers an image's template and TLV initializer functions; returns its key.
pub fn register(template: Vec<u8>, init_fns: Vec<usize>) -> usize {
    let mut t = TEMPLATES.write().unwrap();
    t.push((template, init_fns));
    t.len()
}

struct Blocks(Vec<Option<Box<[u8]>>>);

thread_local! {
    static BLOCKS: RefCell<Blocks> = const { RefCell::new(Blocks(Vec::new())) };
}

/// The calling thread's address for `descriptor`.
extern "C" fn lookup(descriptor: *const Descriptor) -> *mut u8 {
    let (key, offset) = unsafe { ((*descriptor).key, (*descriptor).offset) };
    let mut need_init: Option<Vec<usize>> = None;
    let ptr = BLOCKS.with(|blocks| {
        let mut blocks = blocks.borrow_mut();
        if blocks.0.len() < key {
            blocks.0.resize_with(key, || None);
        }
        let slot = &mut blocks.0[key - 1];
        if slot.is_none() {
            let templates = TEMPLATES.read().unwrap();
            let (tmpl, inits) = &templates[key - 1];
            *slot = Some(tmpl.clone().into_boxed_slice());
            if !inits.is_empty() {
                need_init = Some(inits.clone());
            }
        }
        unsafe { slot.as_mut().unwrap().as_mut_ptr().add(offset) }
    });
    if let Some(inits) = need_init {
        for addr in inits {
            let f: extern "C" fn() = unsafe { std::mem::transmute(addr) };
            f();
        }
    }
    ptr
}

/// The descriptors' thunk: `x0 = lookup(x0)`, every other register (except
/// x16/x17) preserved, including x18 and all vector registers.
#[cfg(target_arch = "aarch64")]
#[unsafe(naked)]
pub extern "C" fn thunk() {
    core::arch::naked_asm!(
        "stp x29, x30, [sp, #-16]!",
        "mov x29, sp",
        "sub sp, sp, #(16 * 10 + 32 * 16)",
        "stp x1, x2, [sp, #0]",
        "stp x3, x4, [sp, #16]",
        "stp x5, x6, [sp, #32]",
        "stp x7, x8, [sp, #48]",
        "stp x9, x10, [sp, #64]",
        "stp x11, x12, [sp, #80]",
        "stp x13, x14, [sp, #96]",
        "stp x15, x18, [sp, #112]",
        "add x16, sp, #160",
        "stp q0, q1, [x16, #0]",
        "stp q2, q3, [x16, #32]",
        "stp q4, q5, [x16, #64]",
        "stp q6, q7, [x16, #96]",
        "stp q8, q9, [x16, #128]",
        "stp q10, q11, [x16, #160]",
        "stp q12, q13, [x16, #192]",
        "stp q14, q15, [x16, #224]",
        "stp q16, q17, [x16, #256]",
        "stp q18, q19, [x16, #288]",
        "stp q20, q21, [x16, #320]",
        "stp q22, q23, [x16, #352]",
        "stp q24, q25, [x16, #384]",
        "stp q26, q27, [x16, #416]",
        "stp q28, q29, [x16, #448]",
        "stp q30, q31, [x16, #480]",
        "bl {lookup}",
        "add x16, sp, #160",
        "ldp q0, q1, [x16, #0]",
        "ldp q2, q3, [x16, #32]",
        "ldp q4, q5, [x16, #64]",
        "ldp q6, q7, [x16, #96]",
        "ldp q8, q9, [x16, #128]",
        "ldp q10, q11, [x16, #160]",
        "ldp q12, q13, [x16, #192]",
        "ldp q14, q15, [x16, #224]",
        "ldp q16, q17, [x16, #256]",
        "ldp q18, q19, [x16, #288]",
        "ldp q20, q21, [x16, #320]",
        "ldp q22, q23, [x16, #352]",
        "ldp q24, q25, [x16, #384]",
        "ldp q26, q27, [x16, #416]",
        "ldp q28, q29, [x16, #448]",
        "ldp q30, q31, [x16, #480]",
        "ldp x1, x2, [sp, #0]",
        "ldp x3, x4, [sp, #16]",
        "ldp x5, x6, [sp, #32]",
        "ldp x7, x8, [sp, #48]",
        "ldp x9, x10, [sp, #64]",
        "ldp x11, x12, [sp, #80]",
        "ldp x13, x14, [sp, #96]",
        "ldp x15, x18, [sp, #112]",
        "mov sp, x29",
        "ldp x29, x30, [sp], #16",
        "ret",
        lookup = sym lookup,
    )
}
