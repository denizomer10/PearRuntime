//! Anonymous memory and instruction-cache maintenance, per host OS, with no
//! libc crate: the few POSIX calls are declared here.

use std::ffi::{c_int, c_void};

pub const PROT_READ: c_int = 1;
pub const PROT_WRITE: c_int = 2;
pub const PROT_EXEC: c_int = 4;
const MAP_PRIVATE: c_int = 0x02;
#[cfg(target_vendor = "apple")]
const MAP_ANON: c_int = 0x1000;
#[cfg(not(target_vendor = "apple"))]
const MAP_ANON: c_int = 0x20;
const MAP_FAILED: *mut c_void = !0usize as *mut c_void;

extern "C" {
    fn mmap(addr: *mut c_void, len: usize, prot: c_int, flags: c_int, fd: c_int, off: i64) -> *mut c_void;
    fn munmap(addr: *mut c_void, len: usize) -> c_int;
    fn mprotect(addr: *mut c_void, len: usize, prot: c_int) -> c_int;
    #[cfg(target_vendor = "apple")]
    fn sys_icache_invalidate(start: *mut c_void, len: usize);
}

/// Mach-O segments are 16 KiB aligned; every supported host page size
/// (4 KiB, 16 KiB) divides this.
pub const SEGMENT_ALIGN: usize = 0x4000;

/// A zero-filled, read-write reservation of `size` bytes whose start is
/// `SEGMENT_ALIGN`-aligned. Unmapped on drop.
pub struct Region {
    mapping: *mut u8,
    mapping_len: usize,
    pub start: *mut u8,
}

unsafe impl Send for Region {}
unsafe impl Sync for Region {}

impl Region {
    /// Images are placed low, from `FIRST_HINT` up and always below 2^47,
    /// as on Darwin: iOS code may rely on it (objc4 masks class pointers to
    /// 47 bits; engines tag pointers above it), while Linux arm64 otherwise
    /// maps near the top of its 48-bit space.
    pub fn reserve(size: usize) -> Option<Region> {
        use std::sync::atomic::{AtomicUsize, Ordering};
        const FIRST_HINT: usize = 0x2_0000_0000;
        const LIMIT: usize = 1 << 47;
        static NEXT_HINT: AtomicUsize = AtomicUsize::new(FIRST_HINT);
        let len = (size + SEGMENT_ALIGN - 1) & !(SEGMENT_ALIGN - 1);
        let mapping_len = len + SEGMENT_ALIGN;
        let mut hint = NEXT_HINT.load(Ordering::Relaxed);
        for _ in 0..64 {
            let p = unsafe {
                mmap(hint as *mut c_void, mapping_len, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANON, -1, 0)
            };
            if p == MAP_FAILED || p.is_null() {
                return None;
            }
            let end = p as usize + mapping_len;
            if end <= LIMIT {
                NEXT_HINT.fetch_max(end, Ordering::Relaxed);
                let start = ((p as usize + SEGMENT_ALIGN - 1) & !(SEGMENT_ALIGN - 1)) as *mut u8;
                return Some(Region { mapping: p as *mut u8, mapping_len, start });
            }
            // The kernel ignored the hint (range busy): try further up.
            unsafe { munmap(p, mapping_len) };
            hint += mapping_len.max(1 << 30);
        }
        None
    }

    /// Sets the protection of `[offset, offset + len)` (aligned to pages).
    pub fn protect(&self, offset: usize, len: usize, prot: c_int) -> bool {
        unsafe { mprotect(self.start.add(offset) as *mut c_void, len, prot) == 0 }
    }
}

impl Drop for Region {
    fn drop(&mut self) {
        unsafe {
            munmap(self.mapping as *mut c_void, self.mapping_len);
        }
    }
}

/// Makes instructions written to `[start, start + len)` visible to
/// instruction fetch.
pub fn flush_icache(start: *mut u8, len: usize) {
    #[cfg(target_vendor = "apple")]
    unsafe {
        sys_icache_invalidate(start as *mut c_void, len)
    }
    #[cfg(all(not(target_vendor = "apple"), target_arch = "aarch64"))]
    unsafe {
        // Clean data cache lines to the point of unification, then
        // invalidate instruction cache lines (CTR_EL0 gives line sizes).
        let ctr: u64;
        std::arch::asm!("mrs {}, ctr_el0", out(reg) ctr);
        let dline = 4usize << ((ctr >> 16) & 0xf);
        let iline = 4usize << (ctr & 0xf);
        let (begin, end) = (start as usize, start as usize + len);
        let mut a = begin & !(dline - 1);
        while a < end {
            std::arch::asm!("dc cvau, {}", in(reg) a);
            a += dline;
        }
        std::arch::asm!("dsb ish");
        let mut a = begin & !(iline - 1);
        while a < end {
            std::arch::asm!("ic ivau, {}", in(reg) a);
            a += iline;
        }
        std::arch::asm!("dsb ish", "isb");
    }
    #[cfg(all(not(target_vendor = "apple"), not(target_arch = "aarch64")))]
    {
        let _ = (start, len);
    }
}
