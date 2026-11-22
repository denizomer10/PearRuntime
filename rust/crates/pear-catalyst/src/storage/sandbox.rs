//! An iOS-like view of the host filesystem, plus file-mode repairs.
//!
//! PEAR_IOS_SANDBOX=1 (set by every launch) applies a sandbox profile that
//! hides the host userland (tools, /Applications, jailbreak paths) while
//! system frameworks, the app bundle and its container stay available; denied
//! paths then look absent (ENOENT) rather than forbidden. Files a port creates
//! with a mode it cannot reopen get owner read (and, for the managed data
//! home only, owner write), and umask never masks the owner. PEAR_DEBUG_PATHS
//! traces host-path, process and write probes without changing results.
//!
//! These replacements run on any thread, often before anything else is set
//! up: they use no Foundation and do not allocate.

use crate::rt::{self, Interpose};
use std::ffi::{c_char, c_int, c_long, c_uint, c_void, CStr};
use std::fmt::Write as _;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Mutex, Once};

type Mode = u16;

#[repr(C)]
struct Stat {
    dev: i32,
    mode: u16,
    nlink: u16,
    ino: u64,
    uid: u32,
    gid: u32,
    rdev: i32,
    _times: [i64; 9],
    size: i64,
    blocks: i64,
    blksize: i32,
    flags: u32,
    gen: u32,
    lspare: i32,
    qspare: [i64; 2],
}

const PATH_MAX: usize = 1024;
const AT_FDCWD: c_int = -2;
const AT_SYMLINK_NOFOLLOW: c_int = 0x20;
const O_WRONLY: c_int = 0x1;
const O_RDWR: c_int = 0x2;
const O_ACCMODE: c_int = 0x3;
const O_NONBLOCK: c_int = 0x4;
const O_APPEND: c_int = 0x8;
const O_NOFOLLOW: c_int = 0x100;
const O_CREAT: c_int = 0x200;
const O_EVTONLY: c_int = 0x8000;
const O_CLOEXEC: c_int = 0x0100_0000;
const F_GETPATH: c_int = 50;
const EPERM: c_int = 1;
const ENOENT: c_int = 2;
const EACCES: c_int = 13;
const S_IFMT: u16 = 0o170000;
const S_IFREG: u16 = 0o100000;
const ALLPERMS: u16 = 0o7777;
const S_IRUSR: u16 = 0o400;
const S_IWUSR: u16 = 0o200;
const SYS_SYSCTL: c_int = 202;

extern "C" {
    fn __error() -> *mut c_int;
    fn getenv(name: *const c_char) -> *mut c_char;
    fn access(path: *const c_char, mode: c_int) -> c_int;
    fn stat(path: *const c_char, info: *mut Stat) -> c_int;
    fn lstat(path: *const c_char, info: *mut Stat) -> c_int;
    fn fstat(fd: c_int, info: *mut Stat) -> c_int;
    fn fstatat(dirfd: c_int, path: *const c_char, info: *mut Stat, flags: c_int) -> c_int;
    fn readlink(path: *const c_char, buf: *mut c_char, size: usize) -> isize;
    fn opendir(path: *const c_char) -> *mut c_void;
    fn open(path: *const c_char, flags: c_int, ...) -> c_int;
    fn openat(dirfd: c_int, path: *const c_char, flags: c_int, ...) -> c_int;
    fn creat(path: *const c_char, mode: Mode) -> c_int;
    fn fopen(path: *const c_char, mode: *const c_char) -> *mut c_void;
    fn fileno(file: *mut c_void) -> c_int;
    fn umask(mask: Mode) -> Mode;
    fn fork() -> c_int;
    fn popen(command: *const c_char, mode: *const c_char) -> *mut c_void;
    fn chmod(path: *const c_char, mode: Mode) -> c_int;
    fn fchmod(fd: c_int, mode: Mode) -> c_int;
    fn fchmodat(dirfd: c_int, path: *const c_char, mode: Mode, flags: c_int) -> c_int;
    fn faccessat(dirfd: c_int, path: *const c_char, mode: c_int, flags: c_int) -> c_int;
    fn statfs(path: *const c_char, info: *mut c_void) -> c_int;
    fn csops(pid: c_int, ops: c_uint, address: *mut c_void, size: usize) -> c_int;
    fn syscall(number: c_int, ...) -> c_int;
    fn fcntl(fd: c_int, cmd: c_int, ...) -> c_int;
    fn realpath(path: *const c_char, resolved: *mut c_char) -> *mut c_char;
    fn close(fd: c_int) -> c_int;
    fn write(fd: c_int, buf: *const c_void, len: usize) -> isize;
    fn geteuid() -> u32;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn strpbrk(s: *const c_char, chars: *const c_char) -> *const c_char;
    // DeviceShim: raw sysctl syscalls go through the hardware identity layer.
    fn PearSysctlMIB(name: *mut c_int, namelen: c_uint, oldp: *mut c_void, oldlenp: *mut usize, newp: *mut c_void, newlen: usize) -> c_int;
}

// The libc calls the file-mode repair makes. A smoke build lets the fixture
// (scripts/filesystem-smoke.mm) inject metadata, failures and races here.
unsafe fn sys_fstat(fd: c_int, info: *mut Stat) -> c_int {
    #[cfg(feature = "smoke")]
    if let Some(f) = crate::smoke::hook::<unsafe extern "C" fn(c_int, *mut Stat) -> c_int>(&crate::smoke::FS_FSTAT) {
        return f(fd, info);
    }
    fstat(fd, info)
}

unsafe fn sys_fchmod(fd: c_int, mode: Mode) -> c_int {
    #[cfg(feature = "smoke")]
    if let Some(f) = crate::smoke::hook::<unsafe extern "C" fn(c_int, Mode) -> c_int>(&crate::smoke::FS_FCHMOD) {
        return f(fd, mode);
    }
    fchmod(fd, mode)
}

unsafe fn sys_fcntl_getpath(fd: c_int, path: *mut c_char) -> c_int {
    #[cfg(feature = "smoke")]
    if let Some(f) = crate::smoke::hook::<unsafe extern "C" fn(c_int, c_int, *mut c_char) -> c_int>(&crate::smoke::FS_GETPATH) {
        return f(fd, F_GETPATH, path);
    }
    fcntl(fd, F_GETPATH, path)
}

unsafe fn sys_openat(dirfd: c_int, path: *const c_char, flags: c_int, mode: c_int) -> c_int {
    #[cfg(feature = "smoke")]
    if let Some(f) = crate::smoke::hook::<unsafe extern "C" fn(c_int, *const c_char, c_int, ...) -> c_int>(&crate::smoke::FS_OPENAT) {
        return f(dirfd, path, flags, mode);
    }
    openat(dirfd, path, flags, mode)
}

unsafe fn sys_open(path: *const c_char, flags: c_int, mode: c_int) -> c_int {
    #[cfg(feature = "smoke")]
    if let Some(f) = crate::smoke::hook::<unsafe extern "C" fn(*const c_char, c_int, ...) -> c_int>(&crate::smoke::FS_OPEN) {
        return f(path, flags, mode);
    }
    open(path, flags, mode)
}

pub(crate) fn trace_enabled() -> bool {
    TRACE_ENABLED.load(Ordering::Relaxed)
}

fn errno() -> c_int {
    unsafe { *__error() }
}

fn set_errno(e: c_int) {
    unsafe { *__error() = e };
}

unsafe fn env_is(name: &CStr) -> bool {
    !getenv(name.as_ptr()).is_null()
}

// ---- diagnostic tracing (PEAR_DEBUG_PATHS) ---------------------------------

static TRACE_ENABLED: AtomicBool = AtomicBool::new(false);
static TRACE_ALL: AtomicBool = AtomicBool::new(false);
static mut TRACE_FILE: [u8; PATH_MAX] = [0; PATH_MAX];
static TRACE_FD: AtomicI32 = AtomicI32::new(-1);
static TRACE_LOCK: Mutex<()> = Mutex::new(());

/// Fixed-size line buffer: formatting without allocation.
struct Line {
    buf: [u8; 2048],
    len: usize,
}

impl Line {
    fn new() -> Self {
        Line { buf: [0; 2048], len: 0 }
    }
}

impl std::fmt::Write for Line {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        let n = s.len().min(self.buf.len() - 1 - self.len);
        self.buf[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]);
        self.len += n;
        Ok(())
    }
}

unsafe fn cstr_lossy<'a>(p: *const c_char) -> &'a str {
    if p.is_null() { "(null)" } else { std::str::from_utf8_unchecked(CStr::from_ptr(p).to_bytes()) }
}

unsafe fn trace_write(api: &str, path: &str, result: c_long, error: c_int) {
    let _guard = TRACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if TRACE_FD.load(Ordering::Acquire) < 0 {
        // openat keeps this out of the interposed open().
        let file = std::ptr::addr_of!(TRACE_FILE) as *const c_char;
        TRACE_FD.store(sys_openat(AT_FDCWD, file, O_WRONLY | O_CREAT | O_APPEND, 0o644 as c_int), Ordering::Release);
    }
    let fd = TRACE_FD.load(Ordering::Acquire);
    if fd >= 0 {
        let mut line = Line::new();
        let _ = writeln!(line, "{api} {path} -> {result} (errno {error})");
        write(fd, line.buf.as_ptr() as *const c_void, line.len);
    }
}

static mut SUPPORT_PREFIX: [u8; PATH_MAX] = [0; PATH_MAX];
static SUPPORT_ONCE: Once = Once::new();

/// Host paths worth tracing: not the system/runtime noise, not our own storage.
unsafe fn trace_interesting(path: *const c_char) -> bool {
    if TRACE_ALL.load(Ordering::Relaxed) && !path.is_null() {
        return true;
    }
    if path.is_null() || *path != b'/' as c_char {
        return false;
    }
    SUPPORT_ONCE.call_once(|| {
        let mut line = Line::new();
        let root = getenv(c"PEAR_SUPPORT_ROOT".as_ptr());
        if !root.is_null() && *root != 0 {
            let _ = write!(line, "{}/", cstr_lossy(root));
        } else {
            let mut home = getenv(c"HOME".as_ptr()) as *const c_char;
            if home.is_null() || *home == 0 {
                home = c"/tmp".as_ptr();
            }
            let _ = write!(line, "{}/Library/Application Support/PearSilicon/", cstr_lossy(home));
        }
        let n = line.len.min(PATH_MAX - 1);
        (&mut *std::ptr::addr_of_mut!(SUPPORT_PREFIX))[..n].copy_from_slice(&line.buf[..n]);
    });
    let p = CStr::from_ptr(path).to_bytes();
    const BORING: [&[u8]; 11] = [
        b"/System/", b"/usr/lib/", b"/usr/share/", b"/Library/Frameworks/", b"/private/var/folders/", b"/dev/",
        b"/private/tmp/", b"/tmp/", b"/private/var/db/", b"/private/var/run/", b"/private/var/log/",
    ];
    if BORING.iter().any(|b| p.starts_with(b)) {
        return false;
    }
    let prefix = CStr::from_ptr(std::ptr::addr_of!(SUPPORT_PREFIX) as *const c_char).to_bytes();
    !(!prefix.is_empty() && p.starts_with(prefix))
}

unsafe fn trace_path(api: &str, path: *const c_char, result: c_long, error: c_int) {
    if TRACE_ENABLED.load(Ordering::Relaxed) && trace_interesting(path) {
        trace_write(api, cstr_lossy(path), result, error);
    }
}

fn sandbox_enabled() -> bool {
    unsafe {
        let v = getenv(c"PEAR_IOS_SANDBOX".as_ptr());
        !v.is_null() && *v != 0 && CStr::from_ptr(v).to_bytes() != b"0"
    }
}

const PROFILE: &CStr = c"(version 1)(allow default)(deny file-read* (subpath \"/bin\"))(deny file-read* (subpath \"/sbin\"))(deny file-read* (subpath \"/usr/bin\"))(deny file-read* (subpath \"/usr/sbin\"))(deny file-read* (subpath \"/usr/libexec\"))(deny file-read* (subpath \"/Applications\"))(deny file-read* (subpath \"/Library/MobileSubstrate\"))(deny file-read* (subpath \"/var/jb\"))(deny file-read* (subpath \"/private/var/jb\"))(deny file-read* (subpath \"/private/var/lib/apt\"))(deny file-read* (subpath \"/private/var/lib/cydia\"))(deny file-read* (subpath \"/private/var/stash\"))(deny file-read* (subpath \"/private/var/tmp/cydia.log\"))(deny file-read* (subpath \"/private/var/mobile/Library/SBSettings\"))(deny file-read* (subpath \"/System/Library/LaunchDaemons/com.saurik.Cydia.Startup.plist\"))(deny file-read* (subpath \"/System/Library/LaunchDaemons/com.ikey.bbot.plist\"))(deny file-read* (subpath \"/usr/lib/libsubstrate.dylib\"))(deny file-read* (subpath \"/usr/lib/libhooker.dylib\"))(deny file-read* (subpath \"/usr/lib/libellekit.dylib\"))";

unsafe fn apply_ios_sandbox() {
    if !sandbox_enabled() {
        return;
    }
    const RTLD_DEFAULT: *mut c_void = -2isize as *mut c_void;
    let init = dlsym(RTLD_DEFAULT, c"sandbox_init".as_ptr());
    if init.is_null() {
        rt::log("[PearRuntime][sandbox] sandbox_init unavailable");
        return;
    }
    let init: unsafe extern "C" fn(*const c_char, u64, *mut *mut c_char) -> c_int = std::mem::transmute(init);
    let mut error: *mut c_char = std::ptr::null_mut();
    if init(PROFILE.as_ptr(), 0, &mut error) == 0 {
        rt::log("[PearRuntime][sandbox] iOS-like filesystem view installed");
    } else {
        rt::log(&format!("[PearRuntime][sandbox] profile failed: {}", if error.is_null() { "unknown" } else { cstr_lossy(error) }));
    }
    if !error.is_null() {
        let free_error = dlsym(RTLD_DEFAULT, c"sandbox_free_error".as_ptr());
        if !free_error.is_null() {
            let free_error: unsafe extern "C" fn(*mut c_char) = std::mem::transmute(free_error);
            free_error(error);
        }
    }
}

pub fn init() {
    if !rt::module_enabled("storage") {
        return;
    }
    unsafe {
        let file = getenv(c"PEAR_FS_LOG_FILE".as_ptr());
        let dst = std::ptr::addr_of_mut!(TRACE_FILE) as *mut u8;
        let src: &[u8] = if !file.is_null() && *file == b'/' as c_char { CStr::from_ptr(file).to_bytes() } else { b"/tmp/pear-fs.log" };
        let n = src.len().min(PATH_MAX - 1);
        std::ptr::copy_nonoverlapping(src.as_ptr(), dst, n);
        *dst.add(n) = 0;
        TRACE_ALL.store(env_is(c"PEAR_DEBUG_ALL_PATHS"), Ordering::Relaxed);
        TRACE_ENABLED.store(env_is(c"PEAR_DEBUG_PATHS"), Ordering::Relaxed);
        apply_ios_sandbox();
    }
}

// ---- sandbox denials look like absence ---------------------------------

fn map_sandbox_error() {
    if sandbox_enabled() && matches!(errno(), EPERM | EACCES) {
        set_errno(ENOENT);
    }
}

/// `access` is not filtered like stat/open/opendir; reconcile the two so an
/// existence check never reports a path stat denies.
#[no_mangle]
pub unsafe extern "C" fn PearSandboxAccess(path: *const c_char, mode: c_int) -> c_int {
    if !sandbox_enabled() || path.is_null() {
        let result = access(path, mode);
        if result != 0 {
            trace_path("access", path, result as c_long, errno());
        }
        return result;
    }
    let mut info: Stat = std::mem::zeroed();
    if stat(path, &mut info) != 0 {
        if matches!(errno(), EPERM | EACCES) {
            set_errno(ENOENT);
        }
        trace_path("access", path, -1, errno());
        return -1;
    }
    let result = access(path, mode);
    if result != 0 {
        trace_path("access", path, result as c_long, errno());
    } else {
        trace_path("access-ok", path, mode as c_long, 0);
    }
    result
}

#[no_mangle]
pub unsafe extern "C" fn PearSandboxStat(path: *const c_char, info: *mut Stat) -> c_int {
    let result = stat(path, info);
    if result != 0 {
        map_sandbox_error();
        trace_path("stat", path, result as c_long, errno());
    }
    result
}

#[no_mangle]
pub unsafe extern "C" fn PearSandboxLstat(path: *const c_char, info: *mut Stat) -> c_int {
    let result = lstat(path, info);
    if result != 0 {
        map_sandbox_error();
        trace_path("lstat", path, result as c_long, errno());
    }
    result
}

#[no_mangle]
pub unsafe extern "C" fn PearSandboxReadlink(path: *const c_char, buffer: *mut c_char, size: usize) -> isize {
    let result = readlink(path, buffer, size);
    if result < 0 {
        map_sandbox_error();
        trace_path("readlink", path, result as c_long, errno());
    }
    result
}

#[no_mangle]
pub unsafe extern "C" fn PearSandboxOpendir(path: *const c_char) -> *mut c_void {
    let result = opendir(path);
    if result.is_null() {
        map_sandbox_error();
        trace_path("opendir", path, 0, errno());
    }
    result
}

// ---- file-mode repair --------------------------------------------------

/// Owner read is the minimum any app needs for data in its own container;
/// execute/write bits are preserved, mode 0 becomes 0644.
fn safe_mode(mode: Mode) -> Mode {
    if mode == 0 { 0o644 } else { mode | 0o400 }
}

fn open_is_writable(flags: c_int) -> bool {
    matches!(flags & O_ACCMODE, O_WRONLY | O_RDWR)
}

fn path_is_below(path: &[u8], root: &[u8]) -> bool {
    path.len() > root.len() && path.starts_with(root) && path[root.len()] == b'/'
}

/// Only the launcher-selected data home (PEAR_HOME_DIR) is a write-repair
/// capability, never HOME or the support root (installed bundles live there).
unsafe fn descriptor_in_managed_home(fd: c_int) -> bool {
    let home = getenv(c"PEAR_HOME_DIR".as_ptr());
    let mut root = [0 as c_char; PATH_MAX];
    let mut path = [0 as c_char; PATH_MAX];
    let mut resolved = [0 as c_char; PATH_MAX];
    if home.is_null() || *home != b'/' as c_char || realpath(home, root.as_mut_ptr()).is_null() {
        return false;
    }
    let root_b = CStr::from_ptr(root.as_ptr()).to_bytes();
    if root_b == b"/" {
        return false;
    }
    if sys_fcntl_getpath(fd, path.as_mut_ptr()) != 0 || !path_is_below(CStr::from_ptr(path.as_ptr()).to_bytes(), root_b) {
        return false;
    }
    !realpath(path.as_ptr(), resolved.as_mut_ptr()).is_null() && path_is_below(CStr::from_ptr(resolved.as_ptr()).to_bytes(), root_b)
}

unsafe fn repair_file_mode(fd: c_int, writable: bool) -> bool {
    let mut info: Stat = std::mem::zeroed();
    if sys_fstat(fd, &mut info) != 0 || info.mode & S_IFMT != S_IFREG || info.uid != geteuid() {
        return false;
    }
    let perms = info.mode & ALLPERMS;
    let mut repaired = perms | S_IRUSR;
    // A writable creation can succeed with a read-only mode; hard links could
    // alias data outside the home, so they never gain write access here.
    if writable && perms & S_IWUSR == 0 && info.nlink == 1 && descriptor_in_managed_home(fd) {
        repaired |= S_IWUSR;
    }
    repaired != perms && sys_fchmod(fd, repaired) == 0
}

/// Legacy owner-read recovery for files with neither owner read nor write;
/// never copies an owner-write bit onto a pathname that could be swapped.
unsafe fn repair_owner_read_at(dirfd: c_int, path: *const c_char) -> bool {
    let mut info: Stat = std::mem::zeroed();
    if fstatat(dirfd, path, &mut info, AT_SYMLINK_NOFOLLOW) != 0 || info.mode & S_IFMT != S_IFREG || info.uid != geteuid() {
        return false;
    }
    let perms = info.mode & ALLPERMS;
    if perms & (S_IRUSR | S_IWUSR) != 0 {
        return false;
    }
    fchmodat(dirfd, path, perms | S_IRUSR, AT_SYMLINK_NOFOLLOW) == 0
}

/// Pins the real target (no final symlink, no create/truncate, caller's
/// dirfd) and repairs through that descriptor only.
unsafe fn repair_file_mode_at(dirfd: c_int, path: *const c_char, writable: bool) -> bool {
    if path.is_null() {
        return false;
    }
    let probe = O_EVTONLY | O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK;
    let mut fd = sys_openat(dirfd, path, probe, 0 as c_int);
    if fd < 0 && errno() == EACCES {
        fd = sys_openat(dirfd, path, probe | O_WRONLY, 0 as c_int);
    }
    let mut read_repaired = false;
    if fd < 0 && errno() == EACCES && repair_owner_read_at(dirfd, path) {
        read_repaired = true;
        fd = sys_openat(dirfd, path, probe, 0 as c_int);
    }
    if fd < 0 {
        return read_repaired;
    }
    let repaired = repair_file_mode(fd, writable);
    close(fd);
    repaired || read_repaired
}

/// Darwin arm64 passes the variadic mode on the stack: keep libc's ABI both
/// ways and never read a mode that a non-creating open did not pass.
#[no_mangle]
pub unsafe extern "C" fn PearTraceOpen(path: *const c_char, flags: c_int, mut args: ...) -> c_int {
    let mode: c_int = if flags & O_CREAT != 0 { args.next_arg::<c_int>() } else { 0 };
    open_common(AT_FDCWD, path, flags, mode, false)
}

#[no_mangle]
pub unsafe extern "C" fn PearTraceOpenAt(dirfd: c_int, path: *const c_char, flags: c_int, mut args: ...) -> c_int {
    let mode: c_int = if flags & O_CREAT != 0 { args.next_arg::<c_int>() } else { 0 };
    // The path trace writes through openat; do not route it back here.
    if !path.is_null() && CStr::from_ptr(path).to_bytes() == b"/tmp/pear-fs.log" {
        return sys_openat(dirfd, path, flags, mode);
    }
    open_common(dirfd, path, flags, mode, true)
}

unsafe fn open_common(dirfd: c_int, path: *const c_char, flags: c_int, mode: c_int, at: bool) -> c_int {
    let creating = flags & O_CREAT != 0;
    let writable = open_is_writable(flags);
    let call = || if at { sys_openat(dirfd, path, flags, mode) } else { sys_open(path, flags, mode) };
    let mut result = call();
    let mut saved = if result < 0 { errno() } else { 0 };
    if result < 0 && saved == EACCES && repair_file_mode_at(dirfd, path, writable) {
        result = call();
        saved = if result < 0 { errno() } else { 0 };
    }
    if result >= 0 && (creating || writable) {
        repair_file_mode(result, writable);
    }
    if result < 0 {
        trace_path(if at { "openat" } else { "open" }, path, result as c_long, saved);
    } else if flags & (O_CREAT | O_WRONLY | O_RDWR) != 0 {
        trace_path(if at { "openat-write" } else { "open-write" }, path, result as c_long, 0);
    }
    set_errno(saved);
    result
}

#[no_mangle]
pub unsafe extern "C" fn PearTraceCreat(path: *const c_char, mode: Mode) -> c_int {
    let mut result = creat(path, mode);
    let mut saved = if result < 0 { errno() } else { 0 };
    if result < 0 && saved == EACCES && repair_file_mode_at(AT_FDCWD, path, true) {
        result = creat(path, mode);
        saved = if result < 0 { errno() } else { 0 };
    }
    if result >= 0 {
        repair_file_mode(result, true);
    }
    trace_path("creat", path, result as c_long, saved);
    set_errno(saved);
    result
}

#[no_mangle]
pub unsafe extern "C" fn PearTraceFopen(path: *const c_char, mode: *const c_char) -> *mut c_void {
    let writable = !mode.is_null() && !strpbrk(mode, c"wa+".as_ptr()).is_null();
    let mut result = fopen(path, mode);
    let mut saved = if result.is_null() { errno() } else { 0 };
    if result.is_null() && saved == EACCES && repair_file_mode_at(AT_FDCWD, path, writable) {
        result = fopen(path, mode);
        saved = if result.is_null() { errno() } else { 0 };
    }
    if !result.is_null() && writable {
        let fd = fileno(result);
        if fd >= 0 {
            repair_file_mode(fd, true);
        }
    }
    if result.is_null() {
        trace_path("fopen", path, 0, saved);
    }
    set_errno(saved);
    result
}

/// Never mask the owner's own access (a 0777 umask leaves unreadable files).
#[no_mangle]
pub unsafe extern "C" fn PearTraceUmask(mask: Mode) -> Mode {
    umask(mask & 0o077)
}

/// The same owner-read floor for an explicit chmod/fchmod.
#[no_mangle]
pub unsafe extern "C" fn PearTraceChmod(path: *const c_char, mode: Mode) -> c_int {
    chmod(path, safe_mode(mode))
}

#[no_mangle]
pub unsafe extern "C" fn PearTraceFchmod(fd: c_int, mode: Mode) -> c_int {
    sys_fchmod(fd, safe_mode(mode))
}

// ---- process and environment probes (traced, results unchanged) ---------

fn env_interesting(name: &[u8]) -> bool {
    const NAMES: [&[u8]; 8] = [
        b"DYLD_INSERT_LIBRARIES", b"DYLD_LIBRARY_PATH", b"DYLD_FRAMEWORK_PATH", b"DYLD_FALLBACK_LIBRARY_PATH",
        b"DYLD_FALLBACK_FRAMEWORK_PATH", b"SIMULATOR_UDID", b"SIMULATOR_MODEL_IDENTIFIER", b"XCTestConfigurationFilePath",
    ];
    NAMES.contains(&name)
}

/// A raw sysctl syscall skips the interposed libc wrappers: route it through
/// the device layer so hardware queries stay consistent.
#[no_mangle]
pub unsafe extern "C" fn PearTraceSyscall(number: c_int, mut args: ...) -> c_long {
    let mut a = [0i64; 6];
    for v in a.iter_mut() {
        *v = args.next_arg::<i64>();
    }
    if number == SYS_SYSCTL && a[0] != 0 {
        let result = PearSysctlMIB(a[0] as *mut c_int, a[1] as c_uint, a[2] as *mut c_void, a[3] as *mut usize, a[4] as *mut c_void, a[5] as usize) as c_long;
        if TRACE_ENABLED.load(Ordering::Relaxed) {
            trace_write("syscall-sysctl", "", result, errno());
        }
        return result;
    }
    let result = syscall(number, a[0], a[1], a[2], a[3], a[4], a[5]) as c_long;
    let saved = errno();
    if TRACE_ENABLED.load(Ordering::Relaxed) {
        let mut label = Line::new();
        let _ = write!(label, "nr={number} a1=0x{:x} a2=0x{:x} a3=0x{:x}", a[0], a[1], a[2]);
        trace_write("syscall", std::str::from_utf8_unchecked(&label.buf[..label.len]), result, saved);
    }
    set_errno(saved);
    result
}

#[no_mangle]
pub unsafe extern "C" fn PearTraceGetenv(name: *const c_char) -> *mut c_char {
    let value = getenv(name);
    if TRACE_ENABLED.load(Ordering::Relaxed) && !name.is_null() && env_interesting(CStr::from_ptr(name).to_bytes()) {
        trace_write("getenv", cstr_lossy(name), (!value.is_null()) as c_long, 0);
    }
    value
}

#[no_mangle]
pub unsafe extern "C" fn PearTraceFaccessat(dirfd: c_int, path: *const c_char, mode: c_int, flags: c_int) -> c_int {
    let result = faccessat(dirfd, path, mode, flags);
    let saved = errno();
    if result != 0 {
        trace_path("faccessat", path, result as c_long, saved);
    } else {
        trace_path("faccessat-ok", path, mode as c_long, 0);
    }
    set_errno(saved);
    result
}

#[no_mangle]
pub unsafe extern "C" fn PearTraceStatfs(path: *const c_char, info: *mut c_void) -> c_int {
    let result = statfs(path, info);
    let saved = errno();
    if result == 0 || saved != ENOENT {
        trace_path("statfs", path, result as c_long, saved);
    }
    set_errno(saved);
    result
}

#[no_mangle]
pub unsafe extern "C" fn PearTraceCsops(pid: c_int, ops: c_uint, address: *mut c_void, size: usize) -> c_int {
    let result = csops(pid, ops, address, size);
    let saved = errno();
    if TRACE_ENABLED.load(Ordering::Relaxed) {
        let mut label = Line::new();
        let _ = write!(label, "pid={pid} ops=0x{ops:x} size={size}");
        if result == 0 && !address.is_null() && size > 0 {
            let _ = write!(label, " data=");
            for i in 0..size.min(24) {
                let _ = write!(label, "{:02x}", *(address as *const u8).add(i));
            }
        }
        trace_write("csops", std::str::from_utf8_unchecked(&label.buf[..label.len]), result as c_long, saved);
    }
    set_errno(saved);
    result
}

#[no_mangle]
pub unsafe extern "C" fn PearTraceFork() -> c_int {
    if TRACE_ENABLED.load(Ordering::Relaxed) {
        trace_write("fork", "", 0, 0);
    }
    fork()
}

#[no_mangle]
pub unsafe extern "C" fn PearTracePopen(command: *const c_char, mode: *const c_char) -> *mut c_void {
    if TRACE_ENABLED.load(Ordering::Relaxed) {
        trace_write("popen", cstr_lossy(command), 0, 0);
    }
    popen(command, mode)
}

macro_rules! interpose {
    ($($replacement:ident => $replacee:ident),* $(,)?) => {
        #[used]
        #[cfg_attr(not(feature = "smoke"), link_section = "__DATA,__interpose")]
        static INTERPOSE: [Interpose; [$(stringify!($replacee)),*].len()] = [
            $(Interpose { replacement: $replacement as *const c_void, replacee: $replacee as *const c_void }),*
        ];
    };
}

interpose! {
    PearSandboxAccess => access,
    PearSandboxStat => stat,
    PearSandboxLstat => lstat,
    PearSandboxReadlink => readlink,
    PearSandboxOpendir => opendir,
    PearTraceOpen => open,
    PearTraceOpenAt => openat,
    PearTraceCreat => creat,
    PearTraceFopen => fopen,
    PearTraceUmask => umask,
    PearTraceFork => fork,
    PearTracePopen => popen,
    PearTraceChmod => chmod,
    PearTraceFchmod => fchmod,
    PearTraceFaccessat => faccessat,
    PearTraceStatfs => statfs,
    PearTraceCsops => csops,
    PearTraceSyscall => syscall,
    PearTraceGetenv => getenv,
}

