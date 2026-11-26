//! Clean-room: Mach-O ARM64 Compact Unwind (`__unwind_info` / `__eh_frame`)
//! and Itanium C++ Exception Handling ABI (`__cxa_*`, `__gxx_personality_v0`,
//! `_Unwind_*`) for cross-platform guest execution on macOS and Linux.

use std::cell::RefCell;
use std::ffi::{c_char, c_int, c_void, CStr};
use std::sync::Mutex;

pub const _URC_NO_REASON: c_int = 0;
pub const _URC_END_OF_STACK: c_int = 5;
pub const _URC_HANDLER_FOUND: c_int = 6;
pub const _URC_INSTALL_CONTEXT: c_int = 7;
pub const _URC_CONTINUE_UNWIND: c_int = 8;

pub const _UA_SEARCH_PHASE: c_int = 1;
pub const _UA_CLEANUP_PHASE: c_int = 2;
pub const _UA_HANDLER_FRAME: c_int = 4;
pub const _UA_FORCE_UNWIND: c_int = 8;

const UNWIND_SECOND_LEVEL_REGULAR: u32 = 2;
const UNWIND_SECOND_LEVEL_COMPRESSED: u32 = 3;

const UNWIND_IS_NOT_FUNCTION_START: u32 = 0x8000_0000;
const UNWIND_HAS_LSDA: u32 = 0x4000_0000;
const UNWIND_PERSONALITY_MASK: u32 = 0x3000_0000;
const UNWIND_ARM64_MODE_MASK: u32 = 0x0F00_0000;
const UNWIND_ARM64_MODE_FRAMELESS: u32 = 0x0200_0000;
const UNWIND_ARM64_MODE_DWARF: u32 = 0x0300_0000;
const UNWIND_ARM64_MODE_FRAME: u32 = 0x0400_0000;

const VA_MASK: usize = 0x0000_FFFF_FFFF_FFFF;
const CXX_EXC_CLASS: u64 = 0x434C_4E47_432B_2B00; // "CLNGC++\0"

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

#[derive(Clone, Copy)]
struct UnwindImage {
    base: usize,
    end: usize,
    unwind_info: usize,
    unwind_info_len: usize,
    eh_frame: usize,
    eh_frame_len: usize,
}

static UNWIND_IMAGES: Mutex<Vec<UnwindImage>> = Mutex::new(Vec::new());

pub fn clear_images() {
    UNWIND_IMAGES.lock().unwrap().clear();
}

pub fn register_image(img: &pear_dyld::LoadedImage) {
    let base = img.header() as usize;
    let end = img
        .sections
        .iter()
        .map(|s| img.address(s.addr.saturating_add(s.size)) as usize)
        .max()
        .unwrap_or(base)
        .max(base);
    let (unwind_info, unwind_info_len) = img
        .section("__TEXT", "__unwind_info")
        .map(|s| (img.address(s.addr) as usize, s.size as usize))
        .unwrap_or((0, 0));
    let (eh_frame, eh_frame_len) = img
        .section("__TEXT", "__eh_frame")
        .map(|s| (img.address(s.addr) as usize, s.size as usize))
        .unwrap_or((0, 0));
    if unwind_info == 0 && eh_frame == 0 {
        return;
    }
    UNWIND_IMAGES.lock().unwrap().push(UnwindImage {
        base,
        end,
        unwind_info,
        unwind_info_len,
        eh_frame,
        eh_frame_len,
    });
}

#[derive(Clone, Copy, Debug)]
pub struct FrameUnwindInfo {
    pub func_start: usize,
    pub encoding: u32,
    pub personality: usize,
    pub lsda: usize,
    pub eh_frame: usize,
    pub eh_frame_len: usize,
}

#[inline]
unsafe fn read_u16_le(ptr: usize) -> u16 {
    std::ptr::read_unaligned(ptr as *const u16).to_le()
}

#[inline]
unsafe fn read_u32_le(ptr: usize) -> u32 {
    std::ptr::read_unaligned(ptr as *const u32).to_le()
}

pub fn lookup_unwind_info(pc: usize) -> Option<FrameUnwindInfo> {
    let pc = pc & VA_MASK;
    let imgs = UNWIND_IMAGES.lock().unwrap();
    let img = imgs
        .iter()
        .copied()
        .find(|i| pc >= i.base && pc < i.end)?;
    drop(imgs);

    unsafe { lookup_in_image(&img, pc) }
}

unsafe fn lookup_in_image(img: &UnwindImage, pc: usize) -> Option<FrameUnwindInfo> {
    if img.unwind_info == 0 || img.unwind_info_len < 28 {
        return None;
    }
    let ui = img.unwind_info;
    let ui_end = ui + img.unwind_info_len;
    let version = read_u32_le(ui);
    if version != 1 {
        return None;
    }
    let common_off = read_u32_le(ui + 4) as usize;
    let common_cnt = read_u32_le(ui + 8) as usize;
    let pers_off = read_u32_le(ui + 12) as usize;
    let pers_cnt = read_u32_le(ui + 16) as usize;
    let idx_off = read_u32_le(ui + 20) as usize;
    let idx_cnt = read_u32_le(ui + 24) as usize;
    if idx_cnt < 2 || ui + idx_off + idx_cnt * 12 > ui_end {
        return None;
    }

    let rel_pc = (pc - img.base) as u32;
    let first_start = read_u32_le(ui + idx_off);
    let last_end = read_u32_le(ui + idx_off + (idx_cnt - 1) * 12);
    if rel_pc < first_start || rel_pc >= last_end {
        return None;
    }

    // Binary search first-level index for entry[i].functionOffset <= rel_pc < entry[i+1].functionOffset
    let mut lo = 0usize;
    let mut hi = idx_cnt - 2;
    while lo < hi {
        let mid = lo + (hi - lo + 1) / 2;
        let off = read_u32_le(ui + idx_off + mid * 12);
        if off <= rel_pc {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }

    let entry_addr = ui + idx_off + lo * 12;
    let next_entry_addr = entry_addr + 12;
    let lvl1_func_off = read_u32_le(entry_addr);
    let second_level_off = read_u32_le(entry_addr + 4) as usize;
    let lsda_start_off = read_u32_le(entry_addr + 8) as usize;
    let lsda_end_off = read_u32_le(next_entry_addr + 8) as usize;
    if second_level_off == 0 || ui + second_level_off + 8 > ui_end {
        return None;
    }

    let page = ui + second_level_off;
    let kind = read_u32_le(page);
    let (func_off, encoding) = match kind {
        UNWIND_SECOND_LEVEL_REGULAR => {
            let entry_off = read_u16_le(page + 4) as usize;
            let entry_cnt = read_u16_le(page + 6) as usize;
            if entry_cnt == 0 || page + entry_off + entry_cnt * 8 > ui_end {
                return None;
            }
            let table = page + entry_off;
            if rel_pc < read_u32_le(table) {
                return None;
            }
            let mut l = 0usize;
            let mut r = entry_cnt - 1;
            while l < r {
                let m = l + (r - l + 1) / 2;
                if read_u32_le(table + m * 8) <= rel_pc {
                    l = m;
                } else {
                    r = m - 1;
                }
            }
            (
                read_u32_le(table + l * 8),
                read_u32_le(table + l * 8 + 4),
            )
        }
        UNWIND_SECOND_LEVEL_COMPRESSED => {
            let entry_off = read_u16_le(page + 4) as usize;
            let entry_cnt = read_u16_le(page + 6) as usize;
            let enc_off = read_u16_le(page + 8) as usize;
            let enc_cnt = read_u16_le(page + 10) as usize;
            if entry_cnt == 0 || page + entry_off + entry_cnt * 4 > ui_end {
                return None;
            }
            let table = page + entry_off;
            let entry_fn = |idx: usize| -> (u32, usize) {
                let w = read_u32_le(table + idx * 4);
                (lvl1_func_off + (w & 0x00FF_FFFF), (w >> 24) as usize)
            };
            if rel_pc < entry_fn(0).0 {
                return None;
            }
            let mut l = 0usize;
            let mut r = entry_cnt - 1;
            while l < r {
                let m = l + (r - l + 1) / 2;
                if entry_fn(m).0 <= rel_pc {
                    l = m;
                } else {
                    r = m - 1;
                }
            }
            let (f_off, enc_idx) = entry_fn(l);
            let enc = if enc_idx < common_cnt {
                let addr = ui + common_off + enc_idx * 4;
                if addr + 4 > ui_end { return None; }
                read_u32_le(addr)
            } else {
                let pi = enc_idx - common_cnt;
                if pi >= enc_cnt {
                    return None;
                }
                let addr = page + enc_off + pi * 4;
                if addr + 4 > ui_end { return None; }
                read_u32_le(addr)
            };
            (f_off, enc)
        }
        _ => return None,
    };

    if (encoding & UNWIND_IS_NOT_FUNCTION_START) != 0 && encoding == 0 {
        return None;
    }

    let mut personality = 0usize;
    let pers_idx = ((encoding & UNWIND_PERSONALITY_MASK) >> 28) as usize;
    if pers_idx > 0 && pers_idx <= pers_cnt && ui + pers_off + pers_idx * 4 <= ui_end {
        let got_off = read_u32_le(ui + pers_off + (pers_idx - 1) * 4) as usize;
        if got_off != 0 {
            let slot_addr = img.base + got_off;
            personality = *(slot_addr as *const usize);
        }
    }

    let mut lsda = 0usize;
    if (encoding & UNWIND_HAS_LSDA) != 0 && lsda_end_off > lsda_start_off && ui + lsda_end_off <= ui_end {
        let cnt = (lsda_end_off - lsda_start_off) / 8;
        let lsda_tbl = ui + lsda_start_off;
        if cnt > 0 {
            let mut l = 0usize;
            let mut r = cnt - 1;
            while l <= r {
                let m = l + (r - l) / 2;
                let f = read_u32_le(lsda_tbl + m * 8);
                if f == func_off {
                    let loff = read_u32_le(lsda_tbl + m * 8 + 4) as usize;
                    if loff != 0 {
                        lsda = img.base + loff;
                    }
                    break;
                } else if f < func_off {
                    l = m + 1;
                } else {
                    if m == 0 { break; }
                    r = m - 1;
                }
            }
        }
    }

    if (encoding & UNWIND_ARM64_MODE_MASK) == UNWIND_ARM64_MODE_DWARF
        && img.eh_frame != 0
        && (personality == 0 || lsda == 0)
    {
        let fde_off = (encoding & 0x00FF_FFFF) as usize;
        if fde_off < img.eh_frame_len {
            if let Some((eh_pers, eh_lsda)) = parse_eh_frame_fde(img.eh_frame + fde_off) {
                if personality == 0 {
                    personality = eh_pers;
                }
                if lsda == 0 {
                    lsda = eh_lsda;
                }
            }
        }
    }

    Some(FrameUnwindInfo {
        func_start: img.base + func_off as usize,
        encoding,
        personality,
        lsda,
        eh_frame: img.eh_frame,
        eh_frame_len: img.eh_frame_len,
    })
}

unsafe fn parse_eh_frame_fde(fde_addr: usize) -> Option<(usize, usize)> {
    let mut p = fde_addr as *const u8;
    let fde_len = read_u32_le(p as usize) as usize;
    if fde_len == 0 || fde_len == u32::MAX as usize {
        return None;
    }
    p = p.add(4);
    let cie_off_addr = p as usize;
    let cie_back = read_u32_le(p as usize) as usize;
    if cie_back == 0 {
        return None;
    }
    let cie_addr = cie_off_addr.checked_sub(cie_back)?;
    let mut cp = cie_addr as *const u8;
    let cie_len = read_u32_le(cp as usize);
    if cie_len < 8 {
        return None;
    }
    cp = cp.add(8); // skip length + cie_id(0)
    let _version = *cp;
    cp = cp.add(1);
    let aug_start = cp;
    while *cp != 0 {
        cp = cp.add(1);
    }
    let aug = std::slice::from_raw_parts(aug_start, (cp as usize) - (aug_start as usize));
    cp = cp.add(1); // skip NUL
    let _code_align = read_uleb128(&mut cp);
    let _data_align = read_sleb128(&mut cp);
    let _ret_reg = read_uleb128(&mut cp);

    let mut pers = 0usize;
    let mut lsda_enc = DW_EH_PE_OMIT;
    let mut fde_enc = 0u8;
    if aug.first() == Some(&b'z') {
        let _aug_len = read_uleb128(&mut cp);
        for &c in &aug[1..] {
            match c {
                b'L' => {
                    lsda_enc = *cp;
                    cp = cp.add(1);
                }
                b'P' => {
                    let penc = *cp;
                    cp = cp.add(1);
                    pers = read_encoded(&mut cp, penc, cie_addr);
                }
                b'R' => {
                    fde_enc = *cp;
                    cp = cp.add(1);
                }
                _ => {}
            }
        }
    }

    p = p.add(4); // move past cie_pointer to PC start
    let _pc_start = read_encoded(&mut p, fde_enc, fde_addr);
    let _pc_range = read_encoded(&mut p, fde_enc & 0x0F, fde_addr);
    let mut lsda = 0usize;
    if aug.first() == Some(&b'z') {
        let fde_aug_len = read_uleb128(&mut p);
        if fde_aug_len > 0 && lsda_enc != DW_EH_PE_OMIT {
            lsda = read_encoded(&mut p, lsda_enc, fde_addr);
        }
    }
    Some((pers, lsda))
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct UnwindContext {
    pub x: [usize; 31],
    pub sp: usize,
    pub pc: usize,
    pub d: [u64; 8],
    pub func_start: usize,
    pub lsda: usize,
    pub pad: usize,
}

#[repr(C)]
pub struct UnwindException {
    pub exception_class: u64,
    pub exception_cleanup: Option<extern "C" fn(c_int, *mut UnwindException)>,
    pub private_1: usize,
    pub private_2: usize,
}

#[repr(C)]
struct CxaException {
    reserve: usize,
    unexpected_handler: usize,
    terminate_handler: usize,
    next_exception: *mut CxaException,
    handler_count: i32,
    handler_switch_value: i32,
    action_record: *const u8,
    language_specific_data: *const u8,
    catch_temp: *mut c_void,
    adjusted_ptr: *mut c_void,
    exception_type: *const c_void,
    exception_destructor: Option<extern "C" fn(*mut c_void)>,
    unwind_header: UnwindException,
}

unsafe fn step_frame(ctx: &mut UnwindContext, info: Option<&FrameUnwindInfo>) -> bool {
    let mode = info.map(|i| i.encoding & UNWIND_ARM64_MODE_MASK).unwrap_or(0);
    let enc = info.map(|i| i.encoding).unwrap_or(0);

    if mode == UNWIND_ARM64_MODE_FRAMELESS {
        let stack_size = (((enc >> 12) & 0xFFF) as usize) * 16;
        let next_pc = ctx.x[30] & VA_MASK;
        if next_pc == 0 {
            return false;
        }
        ctx.sp = ctx.sp.wrapping_add(stack_size);
        ctx.pc = next_pc;
        return true;
    }

    let fp = ctx.x[29] & VA_MASK;
    if fp == 0 || (fp & 7) != 0 {
        return false;
    }

    if mode == UNWIND_ARM64_MODE_FRAME {
        // Callee-saved pairs sit below fp (each pair is 16 bytes, lowest bit first downward)
        let mut ptr = fp;
        if (enc & 0x001) != 0 {
            ptr -= 16;
            ctx.x[20] = *(ptr as *const usize);
            ctx.x[19] = *((ptr + 8) as *const usize);
        }
        if (enc & 0x002) != 0 {
            ptr -= 16;
            ctx.x[22] = *(ptr as *const usize);
            ctx.x[21] = *((ptr + 8) as *const usize);
        }
        if (enc & 0x004) != 0 {
            ptr -= 16;
            ctx.x[24] = *(ptr as *const usize);
            ctx.x[23] = *((ptr + 8) as *const usize);
        }
        if (enc & 0x008) != 0 {
            ptr -= 16;
            ctx.x[26] = *(ptr as *const usize);
            ctx.x[25] = *((ptr + 8) as *const usize);
        }
        if (enc & 0x010) != 0 {
            ptr -= 16;
            ctx.x[28] = *(ptr as *const usize);
            ctx.x[27] = *((ptr + 8) as *const usize);
        }
        if (enc & 0x100) != 0 {
            ptr -= 16;
            ctx.d[1] = *(ptr as *const u64);
            ctx.d[0] = *((ptr + 8) as *const u64);
        }
        if (enc & 0x200) != 0 {
            ptr -= 16;
            ctx.d[3] = *(ptr as *const u64);
            ctx.d[2] = *((ptr + 8) as *const u64);
        }
        if (enc & 0x400) != 0 {
            ptr -= 16;
            ctx.d[5] = *(ptr as *const u64);
            ctx.d[4] = *((ptr + 8) as *const u64);
        }
        if (enc & 0x800) != 0 {
            ptr -= 16;
            ctx.d[7] = *(ptr as *const u64);
            ctx.d[6] = *((ptr + 8) as *const u64);
        }
    }

    let prev_fp = (*(fp as *const usize)) & VA_MASK;
    let prev_lr = (*((fp + 8) as *const usize)) & VA_MASK;
    if prev_lr == 0 {
        return false;
    }
    if prev_fp != 0 && prev_fp <= fp {
        return false;
    }
    ctx.x[29] = prev_fp;
    ctx.x[30] = prev_lr;
    ctx.sp = fp + 16;
    ctx.pc = prev_lr;
    true
}

type PersonalityFn = unsafe extern "C" fn(c_int, c_int, u64, *mut UnwindException, *mut c_void) -> c_int;

#[cfg(target_arch = "aarch64")]
#[unsafe(naked)]
pub unsafe extern "C" fn unwind_install_context(_ctx: *const UnwindContext) -> ! {
    core::arch::naked_asm!(
        "ldr x16, [x0, #256]",
        "ldr x17, [x0, #248]",
        "ldp d8, d9, [x0, #264]",
        "ldp d10, d11, [x0, #280]",
        "ldp d12, d13, [x0, #296]",
        "ldp d14, d15, [x0, #312]",
        "ldp x19, x20, [x0, #152]",
        "ldp x21, x22, [x0, #168]",
        "ldp x23, x24, [x0, #184]",
        "ldp x25, x26, [x0, #200]",
        "ldp x27, x28, [x0, #216]",
        "ldp x29, x30, [x0, #232]",
        "ldp x0, x1, [x0, #0]",
        "mov sp, x17",
        "br x16",
    );
}

#[cfg(not(target_arch = "aarch64"))]
pub unsafe extern "C" fn unwind_install_context(_ctx: *const UnwindContext) -> ! {
    std::process::abort()
}

unsafe extern "C" fn unwind_raise_impl(exc: *mut UnwindException, init_ctx: *mut UnwindContext) -> c_int {
    if exc.is_null() || init_ctx.is_null() {
        return _URC_END_OF_STACK;
    }
    let exc_class = (*exc).exception_class;

    // Phase 1: Search for a catching handler frame
    let mut ctx = *init_ctx;
    let mut handler_sp = 0usize;
    let mut found = false;

    for _ in 0..512 {
        let lookup_pc = ctx.pc.saturating_sub(1);
        let info = lookup_unwind_info(lookup_pc);
        if let Some(ref fi) = info {
            if fi.personality != 0 {
                ctx.func_start = fi.func_start;
                ctx.lsda = fi.lsda;
                let pfn: PersonalityFn = std::mem::transmute(fi.personality);
                let rc = pfn(
                    1,
                    _UA_SEARCH_PHASE,
                    exc_class,
                    exc,
                    (&raw mut ctx) as *mut c_void,
                );
                if rc == _URC_HANDLER_FOUND {
                    handler_sp = ctx.sp;
                    found = true;
                    break;
                } else if rc != _URC_CONTINUE_UNWIND {
                    return rc;
                }
            }
        }
        if !step_frame(&mut ctx, info.as_ref()) {
            break;
        }
    }

    if !found {
        return _URC_END_OF_STACK;
    }

    // Phase 2: Cleanup + install handler context
    (*exc).private_1 = handler_sp;
    unwind_phase2(exc, init_ctx, handler_sp)
}

unsafe extern "C" fn unwind_resume_impl(exc: *mut UnwindException, init_ctx: *mut UnwindContext) -> ! {
    let handler_sp = if !exc.is_null() { (*exc).private_1 } else { 0 };
    let mut ctx = *init_ctx;
    // Resume starts from caller of the cleanup pad that called _Unwind_Resume
    let first_info = lookup_unwind_info(ctx.pc.saturating_sub(1));
    if !step_frame(&mut ctx, first_info.as_ref()) {
        crate::dump_guest_backtrace("_Unwind_Resume failed to step out of cleanup frame");
        std::process::abort();
    }
    let _ = unwind_phase2(exc, &raw mut ctx, handler_sp);
    crate::dump_guest_backtrace("_Unwind_Resume reached end of stack");
    std::process::abort();
}

unsafe fn unwind_phase2(
    exc: *mut UnwindException,
    start_ctx: *mut UnwindContext,
    handler_sp: usize,
) -> c_int {
    let exc_class = (*exc).exception_class;
    let mut ctx = *start_ctx;

    for _ in 0..512 {
        let lookup_pc = ctx.pc.saturating_sub(1);
        let info = lookup_unwind_info(lookup_pc);
        if let Some(ref fi) = info {
            if fi.personality != 0 {
                ctx.func_start = fi.func_start;
                ctx.lsda = fi.lsda;
                let mut actions = _UA_CLEANUP_PHASE;
                if handler_sp != 0 && ctx.sp == handler_sp {
                    actions |= _UA_HANDLER_FRAME;
                }
                let pfn: PersonalityFn = std::mem::transmute(fi.personality);
                let rc = pfn(
                    1,
                    actions,
                    exc_class,
                    exc,
                    (&raw mut ctx) as *mut c_void,
                );
                if rc == _URC_INSTALL_CONTEXT {
                    unwind_install_context(&raw const ctx);
                } else if rc != _URC_CONTINUE_UNWIND {
                    return rc;
                }
            }
        }
        if !step_frame(&mut ctx, info.as_ref()) {
            break;
        }
    }
    _URC_END_OF_STACK
}

#[cfg(target_arch = "aarch64")]
#[unsafe(naked)]
pub unsafe extern "C" fn unwind_raise_exception(_exc: *mut UnwindException) -> c_int {
    core::arch::naked_asm!(
        "sub sp, sp, #352",
        "stp x0, x1, [sp, #0]",
        "stp x2, x3, [sp, #16]",
        "stp x4, x5, [sp, #32]",
        "stp x6, x7, [sp, #48]",
        "stp x8, x9, [sp, #64]",
        "stp x10, x11, [sp, #80]",
        "stp x12, x13, [sp, #96]",
        "stp x14, x15, [sp, #112]",
        "stp x16, x17, [sp, #128]",
        "str x18, [sp, #144]",
        "stp x19, x20, [sp, #152]",
        "stp x21, x22, [sp, #168]",
        "stp x23, x24, [sp, #184]",
        "stp x25, x26, [sp, #200]",
        "stp x27, x28, [sp, #216]",
        "stp x29, x30, [sp, #232]",
        "add x9, sp, #352",
        "str x9, [sp, #248]",
        "str x30, [sp, #256]",
        "stp d8, d9, [sp, #264]",
        "stp d10, d11, [sp, #280]",
        "stp d12, d13, [sp, #296]",
        "stp d14, d15, [sp, #312]",
        "mov x1, sp",
        "bl {raise_impl}",
        "ldp x29, x30, [sp, #232]",
        "add sp, sp, #352",
        "ret",
        raise_impl = sym unwind_raise_impl,
    );
}

#[cfg(not(target_arch = "aarch64"))]
pub unsafe extern "C" fn unwind_raise_exception(_exc: *mut UnwindException) -> c_int {
    _URC_END_OF_STACK
}

#[cfg(target_arch = "aarch64")]
#[unsafe(naked)]
pub unsafe extern "C" fn unwind_resume(_exc: *mut UnwindException) -> ! {
    core::arch::naked_asm!(
        "sub sp, sp, #352",
        "stp x0, x1, [sp, #0]",
        "stp x19, x20, [sp, #152]",
        "stp x21, x22, [sp, #168]",
        "stp x23, x24, [sp, #184]",
        "stp x25, x26, [sp, #200]",
        "stp x27, x28, [sp, #216]",
        "stp x29, x30, [sp, #232]",
        "add x9, sp, #352",
        "str x9, [sp, #248]",
        "str x30, [sp, #256]",
        "stp d8, d9, [sp, #264]",
        "stp d10, d11, [sp, #280]",
        "stp d12, d13, [sp, #296]",
        "stp d14, d15, [sp, #312]",
        "mov x1, sp",
        "bl {resume_impl}",
        resume_impl = sym unwind_resume_impl,
    );
}

#[cfg(not(target_arch = "aarch64"))]
pub unsafe extern "C" fn unwind_resume(_exc: *mut UnwindException) -> ! {
    std::process::abort()
}

pub unsafe extern "C" fn unwind_get_ip(ctx: *mut c_void) -> usize {
    if ctx.is_null() { 0 } else { (*(ctx as *const UnwindContext)).pc }
}

pub unsafe extern "C" fn unwind_get_region_start(ctx: *mut c_void) -> usize {
    if ctx.is_null() { 0 } else { (*(ctx as *const UnwindContext)).func_start }
}

pub unsafe extern "C" fn unwind_get_lsda(ctx: *mut c_void) -> *const u8 {
    if ctx.is_null() { std::ptr::null() } else { (*(ctx as *const UnwindContext)).lsda as *const u8 }
}

pub unsafe extern "C" fn unwind_set_gr(ctx: *mut c_void, idx: c_int, val: usize) {
    if !ctx.is_null() && idx >= 0 && (idx as usize) < 31 {
        (*(ctx as *mut UnwindContext)).x[idx as usize] = val;
    }
}

pub unsafe extern "C" fn unwind_set_ip(ctx: *mut c_void, val: usize) {
    if !ctx.is_null() {
        (*(ctx as *mut UnwindContext)).pc = val;
    }
}

pub unsafe extern "C" fn unwind_delete_exception(exc: *mut UnwindException) {
    if !exc.is_null() {
        if let Some(cleanup) = (*exc).exception_cleanup {
            cleanup(_URC_END_OF_STACK, exc);
        }
    }
}

// ---- Itanium C++ ABI (`libc++abi`) -----------------------------------------

thread_local! {
    static CAUGHT_STACK: RefCell<Vec<*mut CxaException>> = const { RefCell::new(Vec::new()) };
}

#[inline]
unsafe fn cxa_from_thrown(thrown: *mut c_void) -> *mut CxaException {
    (thrown as *mut CxaException).sub(1)
}

#[inline]
unsafe fn cxa_from_unwind(ue: *mut UnwindException) -> *mut CxaException {
    // `unwind_header` is the last field of `CxaException`
    let off = std::mem::offset_of!(CxaException, unwind_header);
    ((ue as usize) - off) as *mut CxaException
}

#[inline]
unsafe fn thrown_from_cxa(cxa: *mut CxaException) -> *mut c_void {
    cxa.add(1) as *mut c_void
}

extern "C" fn cxa_cleanup(_reason: c_int, ue: *mut UnwindException) {
    if ue.is_null() {
        return;
    }
    unsafe {
        let cxa = cxa_from_unwind(ue);
        let thrown = thrown_from_cxa(cxa);
        if let Some(dtor) = (*cxa).exception_destructor {
            dtor(thrown);
        }
        cxa_free_exception(thrown);
    }
}

pub unsafe extern "C" fn cxa_allocate_exception(thrown_size: usize) -> *mut c_void {
    let total = std::mem::size_of::<CxaException>() + thrown_size.max(8);
    let align = 16usize;
    let layout = std::alloc::Layout::from_size_align(total, align).unwrap();
    let ptr = std::alloc::alloc_zeroed(layout);
    if ptr.is_null() {
        std::process::abort();
    }
    let cxa = ptr as *mut CxaException;
    (*cxa).reserve = total;
    thrown_from_cxa(cxa)
}

pub unsafe extern "C" fn cxa_free_exception(thrown: *mut c_void) {
    if thrown.is_null() {
        return;
    }
    let cxa = cxa_from_thrown(thrown);
    let total = (*cxa).reserve.max(std::mem::size_of::<CxaException>());
    let layout = std::alloc::Layout::from_size_align(total, 16).unwrap();
    std::alloc::dealloc(cxa as *mut u8, layout);
}

unsafe extern "C" fn cxa_prepare_throw(
    thrown: *mut c_void,
    tinfo: *const c_void,
    dtor: Option<extern "C" fn(*mut c_void)>,
) -> *mut UnwindException {
    let cxa = cxa_from_thrown(thrown);
    (*cxa).exception_type = tinfo;
    (*cxa).exception_destructor = dtor;
    (*cxa).unwind_header.exception_class = CXX_EXC_CLASS;
    (*cxa).unwind_header.exception_cleanup = Some(cxa_cleanup);
    (*cxa).unwind_header.private_1 = 0;
    (*cxa).unwind_header.private_2 = 0;
    &raw mut (*cxa).unwind_header
}

unsafe fn type_name_from_tinfo(tinfo: *const c_void) -> Option<&'static str> {
    if tinfo.is_null() {
        return None;
    }
    let raw_name_ptr = *((tinfo as usize + 8) as *const usize);
    let ptr = (raw_name_ptr & 0x7FFF_FFFF_FFFF_FFFF) as *const c_char;
    if ptr.is_null() {
        return None;
    }
    CStr::from_ptr(ptr).to_str().ok()
}

#[cold]
unsafe extern "C" fn report_uncaught_cxa(exc: *mut UnwindException) -> ! {
    let tname = if !exc.is_null() && (*exc).exception_class == CXX_EXC_CLASS {
        let cxa = cxa_from_unwind(exc);
        type_name_from_tinfo((*cxa).exception_type).unwrap_or("<unknown C++ exception>")
    } else {
        "<foreign exception>"
    };
    crate::dump_guest_backtrace(&format!("uncaught C++ exception of type {tname}"));
    std::process::abort();
}

#[cfg(target_arch = "aarch64")]
#[unsafe(naked)]
pub unsafe extern "C" fn cxa_throw(
    _thrown: *mut c_void,
    _tinfo: *const c_void,
    _dtor: Option<extern "C" fn(*mut c_void)>,
) -> ! {
    core::arch::naked_asm!(
        "stp x29, x30, [sp, #-16]!",
        "bl {prepare}",
        "ldp x29, x30, [sp], #16",
        "stp x19, x30, [sp, #-16]!",
        "mov x19, x0",
        "bl {raise}",
        "mov x0, x19",
        "bl {uncaught}",
        prepare = sym cxa_prepare_throw,
        raise = sym unwind_raise_exception,
        uncaught = sym report_uncaught_cxa,
    );
}

#[cfg(not(target_arch = "aarch64"))]
pub unsafe extern "C" fn cxa_throw(
    thrown: *mut c_void,
    tinfo: *const c_void,
    dtor: Option<extern "C" fn(*mut c_void)>,
) -> ! {
    let ue = cxa_prepare_throw(thrown, tinfo, dtor);
    report_uncaught_cxa(ue);
}

pub unsafe extern "C" fn cxa_begin_catch(ue_ptr: *mut c_void) -> *mut c_void {
    if ue_ptr.is_null() {
        return std::ptr::null_mut();
    }
    let ue = ue_ptr as *mut UnwindException;
    if (*ue).exception_class == CXX_EXC_CLASS {
        let cxa = cxa_from_unwind(ue);
        // Flip sign if rethrown (negative handler_count in Itanium ABI)
        let hc = (*cxa).handler_count;
        (*cxa).handler_count = if hc < 0 { -hc + 1 } else { hc + 1 };
        CAUGHT_STACK.with(|st| {
            let mut s = st.borrow_mut();
            if s.last().copied() != Some(cxa) {
                s.push(cxa);
            }
        });
        let adj = (*cxa).adjusted_ptr;
        if !adj.is_null() { adj } else { thrown_from_cxa(cxa) }
    } else {
        ue_ptr
    }
}

pub unsafe extern "C" fn cxa_end_catch() {
    let popped = CAUGHT_STACK.with(|st| st.borrow_mut().pop());
    let Some(cxa) = popped else {
        return;
    };
    if (*cxa).handler_count < 0 {
        // Pending rethrow; caller still owns it
        return;
    }
    (*cxa).handler_count -= 1;
    if (*cxa).handler_count <= 0 {
        let thrown = thrown_from_cxa(cxa);
        if let Some(dtor) = (*cxa).exception_destructor {
            dtor(thrown);
        }
        cxa_free_exception(thrown);
    }
}

unsafe extern "C" fn cxa_prepare_rethrow() -> *mut UnwindException {
    let top = CAUGHT_STACK.with(|st| st.borrow().last().copied());
    match top {
        Some(cxa) => {
            (*cxa).handler_count = -(*cxa).handler_count.abs();
            &raw mut (*cxa).unwind_header
        }
        None => {
            crate::dump_guest_backtrace("__cxa_rethrow called with no active exception");
            std::process::abort();
        }
    }
}

#[cfg(target_arch = "aarch64")]
#[unsafe(naked)]
pub unsafe extern "C" fn cxa_rethrow() -> ! {
    core::arch::naked_asm!(
        "stp x29, x30, [sp, #-16]!",
        "bl {prepare}",
        "ldp x29, x30, [sp], #16",
        "stp x19, x30, [sp, #-16]!",
        "mov x19, x0",
        "bl {raise}",
        "mov x0, x19",
        "bl {uncaught}",
        prepare = sym cxa_prepare_rethrow,
        raise = sym unwind_raise_exception,
        uncaught = sym report_uncaught_cxa,
    );
}

#[cfg(not(target_arch = "aarch64"))]
pub unsafe extern "C" fn cxa_rethrow() -> ! {
    let ue = cxa_prepare_rethrow();
    report_uncaught_cxa(ue);
}

pub unsafe extern "C" fn cxa_current_exception_type() -> *const c_void {
    CAUGHT_STACK
        .with(|st| st.borrow().last().copied())
        .map_or(std::ptr::null(), |cxa| (*cxa).exception_type)
}

// ---- DWARF LSDA & `__gxx_personality_v0` -----------------------------------

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

unsafe fn matches_cxx_type(catch_tinfo: usize, thrown_tinfo: *const c_void) -> bool {
    if catch_tinfo == 0 {
        return true; // catch (...)
    }
    if thrown_tinfo.is_null() {
        return false;
    }
    if catch_tinfo == thrown_tinfo as usize {
        return true;
    }
    match (
        type_name_from_tinfo(catch_tinfo as *const c_void),
        type_name_from_tinfo(thrown_tinfo),
    ) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

pub unsafe fn eval_cxx_lsda(
    lsda: *const u8,
    region_start: usize,
    pc: usize,
    thrown_tinfo: *const c_void,
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
                return Some((landing_pad, 0));
            }
            let mut ap = action_table.add(action - 1);
            loop {
                let ttype_idx = read_sleb128(&mut ap);
                let mut next_p = ap;
                let next_off = read_sleb128(&mut next_p);
                if ttype_idx > 0 && ttype_end != 0 {
                    let stride = encoded_size(ttype_enc);
                    let mut entry_p = (ttype_end - (ttype_idx as usize) * stride) as *const u8;
                    let tinfo_ptr = read_encoded(&mut entry_p, ttype_enc, ttype_end);
                    if matches_cxx_type(tinfo_ptr, thrown_tinfo) {
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

pub unsafe extern "C" fn gxx_personality_v0(
    version: c_int,
    actions: c_int,
    exception_class: u64,
    unwind_exception: *mut UnwindException,
    context: *mut c_void,
) -> c_int {
    if version != 1 || context.is_null() {
        return _URC_CONTINUE_UNWIND;
    }
    let lsda = unwind_get_lsda(context);
    let region_start = unwind_get_region_start(context);
    let pc = unwind_get_ip(context).saturating_sub(1);

    let thrown_tinfo = if !unwind_exception.is_null() && exception_class == CXX_EXC_CLASS {
        (*cxa_from_unwind(unwind_exception)).exception_type
    } else {
        std::ptr::null()
    };

    let Some((landing_pad, selector_idx)) = eval_cxx_lsda(lsda, region_start, pc, thrown_tinfo)
    else {
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
        if !unwind_exception.is_null() && exception_class == CXX_EXC_CLASS && selector_idx > 0 {
            let cxa = cxa_from_unwind(unwind_exception);
            (*cxa).adjusted_ptr = thrown_from_cxa(cxa);
            (*cxa).handler_switch_value = selector_idx as i32;
        }
        unwind_set_gr(context, 0, unwind_exception as usize);
        unwind_set_gr(context, 1, selector_idx as usize);
        unwind_set_ip(context, landing_pad);
        return _URC_INSTALL_CONTEXT;
    }

    _URC_CONTINUE_UNWIND
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::offset_of;

    #[test]
    fn unwind_context_layout_and_lsda_catch() {
        assert_eq!(std::mem::size_of::<UnwindContext>(), 352);
        assert_eq!(offset_of!(UnwindContext, sp), 248);
        assert_eq!(offset_of!(UnwindContext, pc), 256);
        assert_eq!(offset_of!(UnwindContext, d), 264);

        unsafe {
            let mem = cxa_allocate_exception(16);
            assert!(!mem.is_null());
            let mut fake_tinfo = [0usize; 2];
            fake_tinfo[1] = c"22Il2CppExceptionWrapper".as_ptr() as usize;
            let ue = cxa_prepare_throw(mem, fake_tinfo.as_ptr() as *const c_void, None);
            assert_eq!((*ue).exception_class, CXX_EXC_CLASS);

            // Synthetic LSDA matching 0x1010..0x1030 with catch-all ttype=0 -> sel=1
            let mut lsda = Vec::<u8>::new();
            lsda.push(DW_EH_PE_OMIT);
            lsda.push(DW_EH_PE_UDATA4);
            lsda.push(21);
            lsda.push(DW_EH_PE_UDATA4);
            lsda.push(13);
            lsda.extend_from_slice(&0x10u32.to_le_bytes());
            lsda.extend_from_slice(&0x20u32.to_le_bytes());
            lsda.extend_from_slice(&0x40u32.to_le_bytes());
            lsda.push(1);
            lsda.push(1);
            lsda.push(0);
            lsda.extend_from_slice(&0u32.to_le_bytes());

            let hit = eval_cxx_lsda(lsda.as_ptr(), 0x1000, 0x1018, fake_tinfo.as_ptr() as *const c_void);
            assert_eq!(hit, Some((0x1040, 1)));

            let got = cxa_begin_catch(ue as *mut c_void);
            assert_eq!(got, mem);
            assert_eq!(cxa_current_exception_type(), fake_tinfo.as_ptr() as *const c_void);
            cxa_end_catch();
        }
    }
}
