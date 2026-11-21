//! Clean-room: DWARF Exception Handling / Itanium C++ ABI & objc4
//! `__objc_personality_v0` LSDA specification (APSL 2.0 / Itanium ABI,
//! reference only, nothing copied).
//!
//! Real Objective-C exception runtime (`objc_exception_throw`,
//! `objc_exception_rethrow`, `objc_begin_catch`, `objc_end_catch`,
//! `objc_terminate`, `objc_setExceptionPreprocessor`, and DWARF LSDA
//! `___objc_personality_v0`).

use crate::abi::*;
use crate::runtime::{self, object_class, superclass};
use std::ffi::{c_char, c_int, c_void};
use std::sync::atomic::{AtomicUsize, Ordering};

pub const _URC_NO_REASON: c_int = 0;
pub const _URC_HANDLER_FOUND: c_int = 6;
pub const _URC_INSTALL_CONTEXT: c_int = 7;
pub const _URC_CONTINUE_UNWIND: c_int = 8;

pub const _UA_SEARCH_PHASE: c_int = 1;
pub const _UA_CLEANUP_PHASE: c_int = 2;
pub const _UA_HANDLER_FRAME: c_int = 4;
pub const _UA_FORCE_UNWIND: c_int = 8;

const DW_EH_PE_OMIT: u8 = 0xFF;
const DW_EH_PE_ULEB128: u8 = 0x01;
const DW_EH_PE_UDATA2: u8 = 0x02;
const DW_EH_PE_UDATA4: u8 = 0x03;
const DW_EH_PE_UDATA8: u8 = 0x04;
const DW_EH_PE_SLEB128: u8 = 0x09;
const DW_EH_PE_SDATA2: u8 = 0x0A;
const DW_EH_PE_SDATA4: u8 = 0x0B;
const DW_EH_PE_SDATA8: u8 = 0x0C;
const DW_EH_PE_PCREL: u8 = 0x10;
const DW_EH_PE_INDIRECT: u8 = 0x80;

const OBJC_EXC_CLASS: u64 = 0x4F42_4A43_0000_0000; // "OBJC\0\0\0\0"

#[repr(C)]
pub struct ObjcEhtype {
    pub vtable: *const c_void,
    pub name: *const c_char,
    pub cls: *const ClassT,
}
unsafe impl Sync for ObjcEhtype {}

#[repr(transparent)]
pub struct StaticVtable(pub [usize; 4]);
unsafe impl Sync for StaticVtable {}

pub static OBJC_EMPTY_VTABLE: StaticVtable = StaticVtable([0; 4]);
pub static OBJC_EHTYPE_VTABLE: StaticVtable = StaticVtable([0; 4]);
pub static OBJC_EHTYPE_ID: ObjcEhtype = ObjcEhtype {
    vtable: &raw const OBJC_EHTYPE_VTABLE as *const c_void,
    name: c"id".as_ptr(),
    cls: std::ptr::null(),
};

#[repr(C)]
pub struct ObjcUnwindException {
    pub exception_class: u64,
    pub exception_cleanup: Option<extern "C" fn(c_int, *mut ObjcUnwindException)>,
    pub private_1: usize,
    pub private_2: usize,
    pub obj: Id,
}

static EXCEPTION_PREPROCESSOR: AtomicUsize = AtomicUsize::new(0);
thread_local! {
    static ACTIVE_EXC: std::cell::Cell<*mut ObjcUnwindException> = const { std::cell::Cell::new(std::ptr::null_mut()) };
}

extern "C" {
    fn _Unwind_RaiseException(exc: *mut ObjcUnwindException) -> c_int;
    fn _Unwind_GetLanguageSpecificData(ctx: *mut c_void) -> *const u8;
    fn _Unwind_GetRegionStart(ctx: *mut c_void) -> usize;
    fn _Unwind_GetIP(ctx: *mut c_void) -> usize;
    fn _Unwind_SetGR(ctx: *mut c_void, index: c_int, value: usize);
    fn _Unwind_SetIP(ctx: *mut c_void, value: usize);
}

extern "C" fn cleanup_exc(_reason: c_int, exc: *mut ObjcUnwindException) {
    if !exc.is_null() {
        unsafe {
            let boxed = Box::from_raw(exc);
            crate::object::release(boxed.obj);
        }
    }
}

pub extern "C" fn objc_set_exception_preprocessor(new_fn: *const c_void) -> *const c_void {
    EXCEPTION_PREPROCESSOR.swap(new_fn as usize, Ordering::AcqRel) as *const c_void
}

pub unsafe extern "C" fn objc_exception_throw(mut exception: Id) -> ! {
    let pre = EXCEPTION_PREPROCESSOR.load(Ordering::Acquire);
    if pre != 0 {
        let f: extern "C" fn(Id) -> Id = std::mem::transmute(pre);
        exception = f(exception);
    }
    crate::object::retain(exception);
    let ue = Box::into_raw(Box::new(ObjcUnwindException {
        exception_class: OBJC_EXC_CLASS,
        exception_cleanup: Some(cleanup_exc),
        private_1: 0,
        private_2: 0,
        obj: exception,
    }));
    ACTIVE_EXC.with(|c| c.set(ue));
    let _ = _Unwind_RaiseException(ue);
    let cls_name = if exception.is_null() {
        "nil".to_string()
    } else {
        let c = object_class(exception);
        runtime::class(c).map_or_else(|| "?".to_string(), |ci| ci.name.clone())
    };
    eprintln!("[pear-libobjc] uncaught Objective-C exception <{cls_name} {:p}>", exception);
    std::process::abort();
}

pub unsafe extern "C" fn objc_exception_rethrow() -> ! {
    let ue = ACTIVE_EXC.with(|c| c.get());
    if !ue.is_null() {
        let _ = _Unwind_RaiseException(ue);
    }
    eprintln!("[pear-libobjc] objc_exception_rethrow: uncaught");
    std::process::abort();
}

pub unsafe extern "C" fn objc_begin_catch(exc_buf: *mut c_void) -> Id {
    if exc_buf.is_null() {
        return std::ptr::null_mut();
    }
    let ue = exc_buf as *mut ObjcUnwindException;
    if (*ue).exception_class == OBJC_EXC_CLASS {
        ACTIVE_EXC.with(|c| c.set(ue));
        (*ue).obj
    } else {
        exc_buf as Id
    }
}

pub extern "C" fn objc_end_catch() {
    let ue = ACTIVE_EXC.with(|c| c.replace(std::ptr::null_mut()));
    if !ue.is_null() {
        cleanup_exc(0, ue);
    }
}

pub extern "C" fn objc_terminate() -> ! {
    eprintln!("[pear-libobjc] objc_terminate");
    std::process::abort();
}

// ---- DWARF LSDA reader -----------------------------------------------------

unsafe fn read_uleb128(p: &mut *const u8) -> usize {
    let mut res = 0usize;
    let mut shift = 0u32;
    loop {
        let b = **p;
        *p = (*p).add(1);
        res |= ((b & 0x7F) as usize) << shift;
        if (b & 0x80) == 0 {
            return res;
        }
        shift += 7;
    }
}

unsafe fn read_sleb128(p: &mut *const u8) -> isize {
    let mut res = 0isize;
    let mut shift = 0u32;
    let mut b: u8;
    loop {
        b = **p;
        *p = (*p).add(1);
        res |= ((b & 0x7F) as isize) << shift;
        shift += 7;
        if (b & 0x80) == 0 {
            break;
        }
    }
    if (shift < usize::BITS) && ((b & 0x40) != 0) {
        res |= !0isize << shift;
    }
    res
}

unsafe fn read_encoded(p: &mut *const u8, enc: u8, base: usize) -> usize {
    if enc == DW_EH_PE_OMIT {
        return 0;
    }
    let addr_before = *p as usize;
    let val: usize = match enc & 0x0F {
        0x00 => {
            let v = std::ptr::read_unaligned(*p as *const usize);
            *p = (*p).add(std::mem::size_of::<usize>());
            v
        }
        DW_EH_PE_ULEB128 => read_uleb128(p),
        DW_EH_PE_UDATA2 => {
            let v = std::ptr::read_unaligned(*p as *const u16) as usize;
            *p = (*p).add(2);
            v
        }
        DW_EH_PE_UDATA4 => {
            let v = std::ptr::read_unaligned(*p as *const u32) as usize;
            *p = (*p).add(4);
            v
        }
        DW_EH_PE_UDATA8 => {
            let v = std::ptr::read_unaligned(*p as *const u64) as usize;
            *p = (*p).add(8);
            v
        }
        DW_EH_PE_SLEB128 => read_sleb128(p) as usize,
        DW_EH_PE_SDATA2 => {
            let v = std::ptr::read_unaligned(*p as *const i16) as isize as usize;
            *p = (*p).add(2);
            v
        }
        DW_EH_PE_SDATA4 => {
            let v = std::ptr::read_unaligned(*p as *const i32) as isize as usize;
            *p = (*p).add(4);
            v
        }
        DW_EH_PE_SDATA8 => {
            let v = std::ptr::read_unaligned(*p as *const i64) as isize as usize;
            *p = (*p).add(8);
            v
        }
        _ => 0,
    };
    if val == 0 {
        return 0;
    }
    let mut out = if (enc & 0x70) == DW_EH_PE_PCREL {
        addr_before.wrapping_add(val)
    } else {
        let _ = base;
        val
    };
    if (enc & DW_EH_PE_INDIRECT) != 0 && out != 0 {
        out = *(out as *const usize);
    }
    out
}

fn encoded_size(enc: u8) -> usize {
    match enc & 0x0F {
        DW_EH_PE_UDATA2 | DW_EH_PE_SDATA2 => 2,
        DW_EH_PE_UDATA4 | DW_EH_PE_SDATA4 => 4,
        DW_EH_PE_UDATA8 | DW_EH_PE_SDATA8 | 0x00 => 8,
        _ => 8,
    }
}

unsafe fn matches_catch_type(eh_ptr: usize, exc: Id) -> bool {
    if eh_ptr == 0 {
        return true; // @catch(...)
    }
    if eh_ptr == (&raw const OBJC_EHTYPE_ID as usize) {
        return true; // @catch(id)
    }
    let eh = &*(eh_ptr as *const ObjcEhtype);
    if eh.cls.is_null() {
        return true;
    }
    let mut c = object_class(exc);
    while !c.is_null() {
        if c == eh.cls {
            return true;
        }
        c = superclass(c);
    }
    false
}

pub unsafe fn eval_lsda(
    lsda: *const u8,
    region_start: usize,
    pc: usize,
    exc: Id,
) -> Option<(usize, isize)> {
    if lsda.is_null() {
        return None;
    }
    let mut p = lsda;
    let lp_enc = *p;
    p = p.add(1);
    let lp_start = if lp_enc == DW_EH_PE_OMIT {
        region_start
    } else {
        read_encoded(&mut p, lp_enc, region_start)
    };
    let ttype_enc = *p;
    p = p.add(1);
    let ttype_end = if ttype_enc == DW_EH_PE_OMIT {
        0
    } else {
        let off = read_uleb128(&mut p);
        (p as usize) + off
    };
    let cs_enc = *p;
    p = p.add(1);
    let cs_len = read_uleb128(&mut p);
    let cs_end = p.add(cs_len);
    let action_table = cs_end;

    while p < cs_end {
        let start = read_encoded(&mut p, cs_enc, region_start);
        let len = read_encoded(&mut p, cs_enc, region_start);
        let lp = read_encoded(&mut p, cs_enc, region_start);
        let action = read_uleb128(&mut p);
        if pc >= region_start + start && pc < region_start + start + len {
            if lp == 0 {
                return None;
            }
            let landing_pad = lp_start + lp;
            if action == 0 {
                return Some((landing_pad, 0)); // cleanup only
            }
            let mut ap = action_table.add(action - 1);
            loop {
                let ttype_idx = read_sleb128(&mut ap);
                let mut next_p = ap;
                let next_off = read_sleb128(&mut next_p);
                if ttype_idx > 0 && ttype_end != 0 {
                    let stride = encoded_size(ttype_enc);
                    let mut entry_p = (ttype_end - (ttype_idx as usize) * stride) as *const u8;
                    let eh_ptr = read_encoded(&mut entry_p, ttype_enc, ttype_end);
                    if matches_catch_type(eh_ptr, exc) {
                        return Some((landing_pad, ttype_idx));
                    }
                } else if ttype_idx == 0 {
                    return Some((landing_pad, 0));
                }
                if next_off == 0 {
                    break;
                }
                ap = ap.offset(next_off);
            }
            return None;
        }
    }
    None
}

pub unsafe extern "C" fn objc_personality_v0(
    version: c_int,
    actions: c_int,
    exception_class: u64,
    unwind_exception: *mut ObjcUnwindException,
    context: *mut c_void,
) -> c_int {
    if version != 1 || context.is_null() {
        return _URC_CONTINUE_UNWIND;
    }
    let lsda = _Unwind_GetLanguageSpecificData(context);
    let region_start = _Unwind_GetRegionStart(context);
    let pc = _Unwind_GetIP(context).saturating_sub(1);
    let exc_obj = if !unwind_exception.is_null() && exception_class == OBJC_EXC_CLASS {
        (*unwind_exception).obj
    } else {
        std::ptr::null_mut()
    };

    let Some((landing_pad, selector_idx)) = eval_lsda(lsda, region_start, pc, exc_obj) else {
        return _URC_CONTINUE_UNWIND;
    };

    if (actions & _UA_SEARCH_PHASE) != 0 {
        return if selector_idx > 0 {
            _URC_HANDLER_FOUND
        } else {
            _URC_CONTINUE_UNWIND
        };
    }

    if (actions & _UA_CLEANUP_PHASE) != 0 {
        if selector_idx > 0 && (actions & _UA_HANDLER_FRAME) == 0 {
            return _URC_CONTINUE_UNWIND;
        }
        _Unwind_SetGR(context, 0, unwind_exception as usize);
        _Unwind_SetGR(context, 1, selector_idx as usize);
        _Unwind_SetIP(context, landing_pad);
        return _URC_INSTALL_CONTEXT;
    }

    _URC_CONTINUE_UNWIND
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsda_matches_id_catch_and_cleanup() {
        // Minimal synthetic LSDA:
        // lp_enc = omit (0xff), ttype_enc = udata4 (0x03), ttype_off = 9
        // cs_enc = udata4 (0x03), cs_len = 13 (1 entry: udata4*3 + uleb128 = 13)
        // call_site: start=0x10, len=0x20, lp=0x40, action=1
        // action: ttype_idx=1, next=0
        // ttype table (end-4): OBJC_EHTYPE_ID (or 0 = catch-all)
        let mut lsda = Vec::<u8>::new();
        lsda.push(DW_EH_PE_OMIT); // lp_enc
        lsda.push(DW_EH_PE_UDATA4); // ttype_enc
        // cs_enc (1) + cs_len (1) + call_site_table (13) + action_table (2) + ttype (4) = 21 bytes after uleb
        lsda.push(21); // ttype_base_offset -> ttype_end = after ttype_table
        lsda.push(DW_EH_PE_UDATA4); // cs_enc
        lsda.push(13); // cs_len
        lsda.extend_from_slice(&0x10u32.to_le_bytes()); // start
        lsda.extend_from_slice(&0x20u32.to_le_bytes()); // len
        lsda.extend_from_slice(&0x40u32.to_le_bytes()); // lp
        lsda.push(1); // action = 1
        lsda.push(1); // ttype_idx = 1 (sleb128)
        lsda.push(0); // next_off = 0 (sleb128)
        lsda.extend_from_slice(&0u32.to_le_bytes()); // catch-all ttype = 0

        let hit = unsafe { eval_lsda(lsda.as_ptr(), 0x1000, 0x1018, std::ptr::null_mut()) };
        assert_eq!(hit, Some((0x1040, 1)));
        let miss = unsafe { eval_lsda(lsda.as_ptr(), 0x1000, 0x1050, std::ptr::null_mut()) };
        assert_eq!(miss, None);
    }
}
