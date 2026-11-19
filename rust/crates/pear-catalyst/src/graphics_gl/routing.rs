//! Opt-in import-slot router (public Mach-O bind opcodes and chained fixups).
//! Only the main app and its embedded images are modified, never system
//! images; every GL entry point resolves to ANGLE, never to system GL.

use super::backend::{self, display, trace};
use super::context::{self, ctx};
use super::{eagl, glkit};
use crate::rt;
use pear_objc::{msg0, nsstring, Class, Id};
use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, Once, OnceLock};

type P = *mut c_void;

static READY: AtomicBool = AtomicBool::new(false);
static APP_ROOT: OnceLock<String> = OnceLock::new();
static BRIDGE_PATH: Mutex<String> = Mutex::new(String::new());
/// Library directory for lazy initialization (the app does not link OpenGL ES
/// itself): ANGLE starts on its first GL use, a Metal-only game pays nothing.
static LAZY_DIRECTORY: Mutex<String> = Mutex::new(String::new());

#[export_name = "_Z27PearANGLEGameRoutingEnabledv"]
pub extern "C" fn routing_enabled() -> bool {
    READY.load(Ordering::Acquire)
}

fn is_gl_name(name: &[u8]) -> bool {
    name.len() > 2 && name[0] == b'g' && name[1] == b'l' && name[2].is_ascii_uppercase()
}

extern "C" {
    fn objc_getClass(name: *const c_char) -> Class;
    fn objc_lookUpClass(name: *const c_char) -> Class;
    fn objc_getMetaClass(name: *const c_char) -> Class;
    fn object_getClass(obj: Id) -> Class;
    fn NSClassFromString(name: Id) -> Class;
    fn dlsym(handle: P, name: *const c_char) -> P;
    fn dlopen(path: *const c_char, mode: c_int) -> P;
    fn CFBundleGetFunctionPointerForName(bundle: P, name: Id) -> P;
    fn CFStringGetCString(s: Id, buffer: *mut c_char, size: isize, encoding: u32) -> bool;
}

unsafe fn routed_class_name(name: &[u8]) -> Class {
    match name {
        b"EAGLContext" => eagl::context_class(),
        b"EAGLSharegroup" => eagl::sharegroup_class(),
        // GLKit's view classes run on the system OpenGL ES; these run on ANGLE.
        b"GLKView" => glkit::view_class(),
        b"GLKViewController" => glkit::view_controller_class(),
        _ => std::ptr::null_mut(),
    }
}

unsafe fn bytes<'a>(p: *const c_char) -> &'a [u8] {
    CStr::from_ptr(p).to_bytes()
}

unsafe extern "C" fn routed_version(major: *mut u32, minor: *mut u32) {
    if !major.is_null() {
        *major = 1;
    }
    if !minor.is_null() {
        *minor = 0;
    }
}
unsafe extern "C" fn routed_proc(name: *const c_char) -> P {
    game_symbol(name)
}
unsafe extern "C" fn routed_ns_class(name: Id) -> Class {
    if !name.is_null() {
        let c = routed_class_name(rt::ns_string(name).as_bytes());
        if !c.is_null() {
            return c;
        }
    }
    NSClassFromString(name)
}
unsafe extern "C" fn routed_get_class(name: *const c_char) -> Class {
    let c = if name.is_null() { std::ptr::null_mut() } else { routed_class_name(bytes(name)) };
    if c.is_null() { objc_getClass(name) } else { c }
}
unsafe extern "C" fn routed_lookup_class(name: *const c_char) -> Class {
    let c = if name.is_null() { std::ptr::null_mut() } else { routed_class_name(bytes(name)) };
    if c.is_null() { objc_lookUpClass(name) } else { c }
}
unsafe extern "C" fn routed_meta_class(name: *const c_char) -> Class {
    let c = if name.is_null() { std::ptr::null_mut() } else { routed_class_name(bytes(name)) };
    if c.is_null() { objc_getMetaClass(name) } else { object_getClass(c) }
}

unsafe fn first_proc(preferred: &CStr, fallback: &CStr) -> P {
    let f = backend::get_proc_address(preferred.as_ptr());
    if f.is_null() { backend::get_proc_address(fallback.as_ptr()) } else { f }
}

macro_rules! cached {
    ($ty:ty, $e:expr) => {{
        static C: OnceLock<usize> = OnceLock::new();
        let v = *C.get_or_init(|| $e as usize);
        if v == 0 { None } else { Some(std::mem::transmute::<usize, $ty>(v)) }
    }};
}

// GL_APPLE_framebuffer_multisample is iOS-only; ANGLE offers the same through
// GL_ANGLE_framebuffer_multisample/blit (ES2) or ES3 core.
unsafe extern "C" fn storage_multisample_apple(target: u32, samples: i32, format: u32, width: i32, height: i32) {
    if let Some(f) = cached!(unsafe extern "C" fn(u32, i32, u32, i32, i32), first_proc(c"glRenderbufferStorageMultisampleANGLE", c"glRenderbufferStorageMultisample")) {
        f(target, samples, format, width, height);
    }
}

/// Size of the renderbuffer attached as colour 0 of `framebuffer`, if any.
unsafe fn color_renderbuffer_size(framebuffer: u32) -> Option<(i32, i32)> {
    let attachment = cached!(unsafe extern "C" fn(u32, u32, u32, *mut i32), backend::get_proc_address(c"glGetFramebufferAttachmentParameteriv".as_ptr()))?;
    let integer = cached!(unsafe extern "C" fn(u32, *mut i32), backend::get_proc_address(c"glGetIntegerv".as_ptr()))?;
    let bind = cached!(unsafe extern "C" fn(u32, u32), backend::get_proc_address(c"glBindRenderbuffer".as_ptr()))?;
    let parameter = cached!(unsafe extern "C" fn(u32, u32, *mut i32), backend::get_proc_address(c"glGetRenderbufferParameteriv".as_ptr()))?;
    let (mut kind, mut name, mut previous, mut w, mut h) = (0, 0, 0, 0, 0);
    attachment(framebuffer, 0x8CE0, 0x8CD0, &mut kind);
    if kind != 0x8D41 {
        return None;
    }
    attachment(framebuffer, 0x8CE0, 0x8CD1, &mut name);
    integer(0x8CA7, &mut previous);
    bind(0x8D41, name as u32);
    parameter(0x8D41, 0x8D42, &mut w);
    parameter(0x8D41, 0x8D43, &mut h);
    bind(0x8D41, previous as u32);
    (w > 0 && h > 0).then_some((w, h))
}

/// Resolves READ_FRAMEBUFFER_APPLE into the draw framebuffer, colour only,
/// over the whole surface; the scissor test applies as on iOS.
unsafe extern "C" fn resolve_multisample_apple() {
    let Some(blit) = cached!(unsafe extern "C" fn(i32, i32, i32, i32, i32, i32, i32, i32, u32, u32), first_proc(c"glBlitFramebufferANGLE", c"glBlitFramebuffer")) else { return };
    let Some((mut w, mut h)) = color_renderbuffer_size(0x8CA8) else { return };
    if let Some((dw, dh)) = color_renderbuffer_size(0x8CA9) {
        w = w.min(dw);
        h = h.min(dh);
    }
    blit(0, 0, w, h, 0, 0, w, h, 0x4000, 0x2600);
}

// Apple GPUs never advertise desktop texture compression; games pick their
// asset pack from the extension list and iOS bundles ship only PVR/ETC packs.
// PEAR_GL_ALL_EXTENSIONS=1 shows ANGLE's full list.
fn hidden_extension(name: &[u8]) -> bool {
    const HIDDEN: [&[u8]; 10] = [
        b"GL_EXT_texture_compression_dxt1",
        b"GL_EXT_texture_compression_s3tc",
        b"GL_EXT_texture_compression_s3tc_srgb",
        b"GL_ANGLE_texture_compression_dxt3",
        b"GL_ANGLE_texture_compression_dxt5",
        b"GL_EXT_texture_compression_rgtc",
        b"GL_EXT_texture_compression_bptc",
        b"GL_AMD_compressed_ATC_texture",
        b"GL_NV_texture_compression_s3tc",
        b"GL_S3_s3tc",
    ];
    static EVERYTHING: OnceLock<bool> = OnceLock::new();
    if *EVERYTHING.get_or_init(|| rt::env_set("PEAR_GL_ALL_EXTENSIONS")) {
        return false;
    }
    HIDDEN.contains(&name)
}

unsafe extern "C" fn routed_get_string(which: u32) -> *const u8 {
    let Some(original) = cached!(unsafe extern "C" fn(u32) -> *const u8, backend::get_proc_address(c"glGetString".as_ptr())) else { return std::ptr::null() };
    let value = original(which);
    if which != 0x1F03 || value.is_null() {
        return value;
    }
    // ANGLE keeps one extension string per context: filter each once.
    static FILTERED: Mutex<Option<HashMap<usize, CString>>> = Mutex::new(None);
    let mut guard = FILTERED.lock().unwrap_or_else(|e| e.into_inner());
    let map = guard.get_or_insert_with(HashMap::new);
    let entry = map.entry(value as usize).or_insert_with(|| {
        let all = CStr::from_ptr(value as *const c_char).to_bytes();
        let kept: Vec<&[u8]> = all.split(|b| *b == b' ').filter(|e| !e.is_empty() && !hidden_extension(e)).collect();
        CString::new(kept.join(&b' ')).unwrap_or_default()
    });
    entry.as_ptr() as *const u8
}

/// ES3 indexed extension queries, renumbered without the hidden entries.
unsafe fn visible_extension(wanted: i32, name: Option<&mut *const u8>) -> i32 {
    let integer = cached!(unsafe extern "C" fn(u32, *mut i32), backend::get_proc_address(c"glGetIntegerv".as_ptr()));
    let indexed = cached!(unsafe extern "C" fn(u32, u32) -> *const u8, backend::get_proc_address(c"glGetStringi".as_ptr()));
    let (Some(integer), Some(indexed)) = (integer, indexed) else { return 0 };
    let mut total = 0;
    integer(0x821D, &mut total);
    let mut visible = 0;
    let mut name = name;
    for index in 0..total {
        let entry = indexed(0x1F03, index as u32);
        if entry.is_null() || hidden_extension(CStr::from_ptr(entry as *const c_char).to_bytes()) {
            continue;
        }
        if visible == wanted {
            if let Some(n) = name.as_deref_mut() {
                *n = entry;
                return visible;
            }
        }
        visible += 1;
    }
    visible
}

unsafe extern "C" fn routed_get_stringi(which: u32, index: u32) -> *const u8 {
    let Some(original) = cached!(unsafe extern "C" fn(u32, u32) -> *const u8, backend::get_proc_address(c"glGetStringi".as_ptr())) else { return std::ptr::null() };
    if which != 0x1F03 {
        return original(which, index);
    }
    let mut name: *const u8 = std::ptr::null();
    visible_extension(index as i32, Some(&mut name));
    // Out of range: let ANGLE record GL_INVALID_VALUE as usual.
    if name.is_null() { original(which, 0x7fffffff) } else { name }
}

unsafe extern "C" fn routed_get_integerv(which: u32, value: *mut i32) {
    let Some(original) = cached!(unsafe extern "C" fn(u32, *mut i32), backend::get_proc_address(c"glGetIntegerv".as_ptr())) else { return };
    original(which, value);
    if which == 0x821D && !value.is_null() {
        *value = visible_extension(-1, None);
    }
}

fn current_version() -> isize {
    unsafe { ctx(context::current()).map(|c| c.client_version).unwrap_or(0) }
}

// Shipped iOS clients import unsuffixed framebuffer names in GLES1 contexts;
// ANGLE validates GLES2 core names strictly: dispatch to OES under GLES1.
macro_rules! framebuffer_alias {
    ($fname:ident, $core:literal, ($($a:ident: $t:ty),*) $(-> $r:ty)?) => {
        unsafe extern "C" fn $fname($($a: $t),*) $(-> $r)? {
            type F = unsafe extern "C" fn($($t),*) $(-> $r)?;
            let core = cached!(F, backend::get_proc_address(concat!($core, "\0").as_ptr() as *const c_char));
            let oes = cached!(F, backend::get_proc_address(concat!($core, "OES\0").as_ptr() as *const c_char));
            let f = if current_version() == 1 { oes } else { core };
            (f.expect(concat!($core, " unavailable")))($($a),*)
        }
    };
}
framebuffer_alias!(compat_gen_renderbuffers, "glGenRenderbuffers", (n: i32, names: *mut u32));
framebuffer_alias!(compat_bind_renderbuffer, "glBindRenderbuffer", (target: u32, name: u32));
framebuffer_alias!(compat_is_renderbuffer, "glIsRenderbuffer", (name: u32) -> u8);
framebuffer_alias!(compat_renderbuffer_storage, "glRenderbufferStorage", (target: u32, format: u32, w: i32, h: i32));
framebuffer_alias!(compat_get_renderbuffer_parameteriv, "glGetRenderbufferParameteriv", (target: u32, p: u32, v: *mut i32));
framebuffer_alias!(compat_gen_framebuffers, "glGenFramebuffers", (n: i32, names: *mut u32));
framebuffer_alias!(compat_bind_framebuffer, "glBindFramebuffer", (target: u32, name: u32));
framebuffer_alias!(compat_delete_framebuffers, "glDeleteFramebuffers", (n: i32, names: *const u32));
framebuffer_alias!(compat_is_framebuffer, "glIsFramebuffer", (name: u32) -> u8);
framebuffer_alias!(compat_framebuffer_renderbuffer, "glFramebufferRenderbuffer", (target: u32, attachment: u32, kind: u32, name: u32));
framebuffer_alias!(compat_framebuffer_texture_2d, "glFramebufferTexture2D", (target: u32, attachment: u32, kind: u32, name: u32, level: i32));
framebuffer_alias!(compat_get_framebuffer_attachment_parameteriv, "glGetFramebufferAttachmentParameteriv", (target: u32, attachment: u32, p: u32, v: *mut i32));

unsafe extern "C" fn compat_check_framebuffer_status(target: u32) -> u32 {
    type F = unsafe extern "C" fn(u32) -> u32;
    let core = cached!(F, backend::get_proc_address(c"glCheckFramebufferStatus".as_ptr()));
    let oes = cached!(F, backend::get_proc_address(c"glCheckFramebufferStatusOES".as_ptr()));
    let f = if current_version() == 1 { oes } else { core };
    let result = (f.expect("glCheckFramebufferStatus unavailable"))(target);
    static FAILURES: AtomicU32 = AtomicU32::new(0);
    if result != 0x8CD5 && rt::env_set("PEAR_DEBUG_GL") && FAILURES.fetch_add(1, Ordering::Relaxed) < 32 {
        trace(&format!("[PearRuntime][angle-validation] framebuffer target=0x{target:x} status=0x{result:x} GLES{}\n", current_version()));
    }
    result
}

unsafe fn info_log(f: unsafe extern "C" fn(u32, i32, *mut i32, *mut c_char), object: u32) -> String {
    let mut log = [0u8; 2048];
    let mut length = 0;
    f(object, (log.len() - 1) as i32, &mut length, log.as_mut_ptr() as *mut c_char);
    String::from_utf8_lossy(&log[..(length.clamp(0, 2047)) as usize]).into_owned()
}

unsafe extern "C" fn diagnostic_compile_shader(shader: u32) {
    let compile = cached!(unsafe extern "C" fn(u32), backend::get_proc_address(c"glCompileShader".as_ptr())).unwrap();
    let query = cached!(unsafe extern "C" fn(u32, u32, *mut i32), backend::get_proc_address(c"glGetShaderiv".as_ptr())).unwrap();
    let info = cached!(unsafe extern "C" fn(u32, i32, *mut i32, *mut c_char), backend::get_proc_address(c"glGetShaderInfoLog".as_ptr())).unwrap();
    compile(shader);
    let mut ok = 0;
    query(shader, 0x8B81, &mut ok);
    if ok == 0 {
        trace(&format!("[PearRuntime][angle-validation] shader {shader} compile failed: {}\n", info_log(info, shader)));
    }
}

unsafe extern "C" fn diagnostic_link_program(program: u32) {
    let link = cached!(unsafe extern "C" fn(u32), backend::get_proc_address(c"glLinkProgram".as_ptr())).unwrap();
    let query = cached!(unsafe extern "C" fn(u32, u32, *mut i32), backend::get_proc_address(c"glGetProgramiv".as_ptr())).unwrap();
    let info = cached!(unsafe extern "C" fn(u32, i32, *mut i32, *mut c_char), backend::get_proc_address(c"glGetProgramInfoLog".as_ptr())).unwrap();
    link(program);
    let mut ok = 0;
    query(program, 0x8B82, &mut ok);
    if ok == 0 {
        trace(&format!("[PearRuntime][angle-validation] program {program} link failed: {}\n", info_log(info, program)));
    }
}

unsafe extern "C" fn diagnostic_uniform_location(program: u32, name: *const c_char) -> i32 {
    let original = cached!(unsafe extern "C" fn(u32, *const c_char) -> i32, backend::get_proc_address(c"glGetUniformLocation".as_ptr())).unwrap();
    let location = original(program, name);
    static COUNT: AtomicU32 = AtomicU32::new(0);
    if COUNT.fetch_add(1, Ordering::Relaxed) < 128 {
        let n = if name.is_null() { "(null)".into() } else { CStr::from_ptr(name).to_string_lossy().into_owned() };
        trace(&format!("[PearRuntime][angle-validation] uniform program={program} name={n} location={location}\n"));
    }
    location
}

unsafe extern "C" fn diagnostic_map_buffer_oes(target: u32, access: u32) -> P {
    let original = cached!(unsafe extern "C" fn(u32, u32) -> P, backend::get_proc_address(c"glMapBufferOES".as_ptr())).unwrap();
    let result = original(target, access);
    static COUNT: AtomicU32 = AtomicU32::new(0);
    if COUNT.fetch_add(1, Ordering::Relaxed) < 64 {
        trace(&format!(
            "[PearRuntime][angle-validation] mapBuffer target=0x{target:x} access=0x{access:x} null={} GLES{}\n",
            result.is_null() as i32,
            current_version()
        ));
    }
    result
}

// iOS accepts READ/DRAW_FRAMEBUFFER(_APPLE) in glDiscardFramebufferEXT; ANGLE's
// extension accepts only GL_FRAMEBUFFER. Those targets go to ES3
// glInvalidateFramebuffer; discard is a hint, so without one it is dropped.
unsafe extern "C" fn routed_discard_framebuffer(target: u32, count: i32, attachments: *const u32) {
    type F = unsafe extern "C" fn(u32, i32, *const u32);
    let discard = cached!(F, backend::get_proc_address(c"glDiscardFramebufferEXT".as_ptr()));
    let invalidate = cached!(F, backend::get_proc_address(c"glInvalidateFramebuffer".as_ptr()));
    let integer = cached!(unsafe extern "C" fn(u32, *mut i32), backend::get_proc_address(c"glGetIntegerv".as_ptr()));
    if target != 0x8CA8 && target != 0x8CA9 {
        if let Some(d) = discard {
            d(target, count, attachments);
        }
        return;
    }
    let (Some(invalidate), Some(integer)) = (invalidate, integer) else { return };
    if count <= 0 || count > 16 || attachments.is_null() {
        return;
    }
    let mut bound = 0;
    integer(if target == 0x8CA8 { 0x8CAA } else { 0x8CA6 }, &mut bound);
    let mut mapped = [0u32; 16];
    for i in 0..count as usize {
        let value = *attachments.add(i);
        mapped[i] = if bound == 0 {
            value
        } else {
            match value {
                0x1800 => 0x8CE0,
                0x1801 => 0x8D00,
                0x1802 => 0x8D20,
                v => v,
            }
        };
    }
    let mut major = 0;
    integer(0x821B, &mut major);
    if major >= 3 {
        invalidate(target, count, mapped.as_ptr());
    }
}

unsafe extern "C" fn routed_delete_renderbuffers(count: i32, names: *const u32) {
    let current = context::current();
    if !current.is_null() {
        super::drawable::forget_imp(current, std::ptr::null_mut(), names, count);
    }
    type F = unsafe extern "C" fn(i32, *const u32);
    let core = cached!(F, backend::get_proc_address(c"glDeleteRenderbuffers".as_ptr()));
    let oes = cached!(F, backend::get_proc_address(c"glDeleteRenderbuffersOES".as_ptr()));
    if let Some(f) = if current_version() == 1 { oes } else { core } {
        f(count, names);
    }
}

/// (replacement, handled) for the dlsym/dlopen trampolines.
#[repr(C)]
struct Decision {
    value: P,
    handled: usize,
}

unsafe extern "C" fn decide_dlsym(_handle: P, name: *const c_char) -> Decision {
    let replacement = game_symbol(name);
    // An unavailable GL function never silently resolves into system GL.
    let handled = !replacement.is_null() || (!name.is_null() && is_gl_name(bytes(name)));
    Decision { value: replacement, handled: handled as usize }
}

unsafe extern "C" fn decide_dlopen(path: *const c_char, mode: c_int) -> Decision {
    if path.is_null() {
        return Decision { value: std::ptr::null_mut(), handled: 0 };
    }
    let p = bytes(path);
    let opengl = p == b"OpenGLES" || p.windows(b"OpenGLES.framework/".len()).any(|w| w == b"OpenGLES.framework/");
    if !opengl {
        return Decision { value: std::ptr::null_mut(), handled: 0 };
    }
    let bridge = BRIDGE_PATH.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if bridge.is_empty() {
        trace("[PearRuntime][angle-route] missing ANGLE ABI bridge; system OpenGLES load refused\n");
        return Decision { value: std::ptr::null_mut(), handled: 1 };
    }
    let c = CString::new(bridge).unwrap_or_default();
    Decision { value: dlopen(c.as_ptr(), mode), handled: 1 }
}

/// `dlsym` replacement. Unrelated lookups tail-call the real dlsym with the
/// caller's return address intact, preserving RTLD_NEXT/RTLD_SELF semantics.
#[unsafe(naked)]
unsafe extern "C" fn routed_dlsym() {
    core::arch::naked_asm!(
        "stp x29, x30, [sp, #-32]!",
        "mov x29, sp",
        "stp x0, x1, [sp, #16]",
        "bl {decide}",
        "cbnz x1, 2f",
        "ldp x0, x1, [sp, #16]",
        "ldp x29, x30, [sp], #32",
        "b {real}",
        "2:",
        "ldp x29, x30, [sp], #32",
        "ret",
        decide = sym decide_dlsym,
        real = sym dlsym,
    )
}

/// `dlopen` replacement: OpenGLES loads get the ANGLE bridge; everything else
/// tail-calls dlopen so @rpath/@loader_path resolve against the caller.
#[unsafe(naked)]
unsafe extern "C" fn routed_dlopen() {
    core::arch::naked_asm!(
        "stp x29, x30, [sp, #-32]!",
        "mov x29, sp",
        "stp x0, x1, [sp, #16]",
        "bl {decide}",
        "cbnz x1, 2f",
        "ldp x0, x1, [sp, #16]",
        "ldp x29, x30, [sp], #32",
        "b {real}",
        "2:",
        "ldp x29, x30, [sp], #32",
        "ret",
        decide = sym decide_dlopen,
        real = sym dlopen,
    )
}

unsafe extern "C" fn routed_bundle_proc(bundle: P, name: Id) -> P {
    let mut symbol = [0 as c_char; 256];
    if !name.is_null() && CFStringGetCString(name, symbol.as_mut_ptr(), 256, 0x08000100) {
        let replacement = game_symbol(symbol.as_ptr());
        if !replacement.is_null() || is_gl_name(bytes(symbol.as_ptr())) {
            return replacement;
        }
    }
    CFBundleGetFunctionPointerForName(bundle, name)
}

fn ensure_angle() {
    if !display().is_null() || LAZY_DIRECTORY.lock().unwrap_or_else(|e| e.into_inner()).is_empty() {
        return;
    }
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        pear_objc::autorelease_pool(|| {
            let mut error: Id = std::ptr::null_mut();
            let dir = LAZY_DIRECTORY.lock().unwrap_or_else(|e| e.into_inner()).clone();
            if !backend::initialize(nsstring(&dir), &mut error) {
                // No system OpenGL ES fallback: refuse instead of mixing backends.
                let reason = if error.is_null() { "unknown".into() } else { rt::ns_string(msg0(error, "localizedDescription")) };
                trace(&format!("[PearRuntime][angle-route] lazy ANGLE initialization failed: {reason}\n"));
                libc_exit(78);
            }
            trace("[PearRuntime][angle-route] ANGLE initialized on first GL use\n");
        })
    });
}

fn libc_exit(code: c_int) -> ! {
    extern "C" {
        fn _exit(code: c_int) -> !;
    }
    unsafe { _exit(code) }
}

/// Routed C/ObjC lookup functions need no GL themselves.
fn is_lookup_wrapper(name: &[u8]) -> bool {
    matches!(name, b"dlsym" | b"dlopen" | b"CFBundleGetFunctionPointerForName" | b"NSClassFromString" | b"objc_getClass" | b"objc_lookUpClass" | b"objc_getMetaClass")
}

/// ANGLE GL/EAGL replacements (null for unrelated symbols).
#[export_name = "_Z19PearANGLEGameSymbolPKc"]
pub unsafe extern "C" fn game_symbol(name: *const c_char) -> P {
    if name.is_null() || !routing_enabled() {
        return std::ptr::null_mut();
    }
    let n = bytes(name);
    if !is_lookup_wrapper(n) && (is_gl_name(n) || n.starts_with(b"OBJC_") || n.starts_with(b"EAGL") || n == b"eglGetProcAddress") {
        ensure_angle();
    }
    let class_ptr = |c: Class| c as P;
    match n {
        b"OBJC_CLASS_$_EAGLContext" => return class_ptr(eagl::context_class()),
        b"OBJC_CLASS_$_EAGLSharegroup" => return class_ptr(eagl::sharegroup_class()),
        b"OBJC_METACLASS_$_EAGLContext" => return object_getClass(eagl::context_class() as Id) as P,
        b"OBJC_METACLASS_$_EAGLSharegroup" => return object_getClass(eagl::sharegroup_class() as Id) as P,
        b"OBJC_CLASS_$_GLKView" => return class_ptr(glkit::view_class()),
        b"OBJC_METACLASS_$_GLKView" => return object_getClass(glkit::view_class() as Id) as P,
        b"OBJC_CLASS_$_GLKViewController" => return class_ptr(glkit::view_controller_class()),
        b"OBJC_METACLASS_$_GLKViewController" => return object_getClass(glkit::view_controller_class() as Id) as P,
        b"glCheckFramebufferStatusOES" => return compat_check_framebuffer_status as P,
        _ => {}
    }
    if rt::env_set("PEAR_DEBUG_GL") {
        match n {
            b"glCompileShader" => return diagnostic_compile_shader as P,
            b"glLinkProgram" => return diagnostic_link_program as P,
            b"glGetUniformLocation" => return diagnostic_uniform_location as P,
            b"glMapBufferOES" => return diagnostic_map_buffer_oes as P,
            _ => {}
        }
    }
    let f: P = match n {
        b"glGenRenderbuffers" => compat_gen_renderbuffers as P,
        b"glBindRenderbuffer" => compat_bind_renderbuffer as P,
        b"glIsRenderbuffer" => compat_is_renderbuffer as P,
        b"glRenderbufferStorage" => compat_renderbuffer_storage as P,
        b"glGetRenderbufferParameteriv" => compat_get_renderbuffer_parameteriv as P,
        b"glGenFramebuffers" => compat_gen_framebuffers as P,
        b"glBindFramebuffer" => compat_bind_framebuffer as P,
        b"glDeleteFramebuffers" => compat_delete_framebuffers as P,
        b"glIsFramebuffer" => compat_is_framebuffer as P,
        b"glCheckFramebufferStatus" => compat_check_framebuffer_status as P,
        b"glFramebufferRenderbuffer" => compat_framebuffer_renderbuffer as P,
        b"glFramebufferTexture2D" => compat_framebuffer_texture_2d as P,
        b"glGetFramebufferAttachmentParameteriv" => compat_get_framebuffer_attachment_parameteriv as P,
        b"glDeleteRenderbuffers" | b"glDeleteRenderbuffersOES" => routed_delete_renderbuffers as P,
        b"glGetString" => routed_get_string as P,
        b"glDiscardFramebufferEXT" => routed_discard_framebuffer as P,
        b"glGetStringi" => routed_get_stringi as P,
        b"glGetIntegerv" => routed_get_integerv as P,
        b"glRenderbufferStorageMultisampleAPPLE" => storage_multisample_apple as P,
        b"glResolveMultisampleFramebufferAPPLE" => resolve_multisample_apple as P,
        b"EAGLGetVersion" => routed_version as P,
        b"EAGLGetProcAddress" | b"eglGetProcAddress" => routed_proc as P,
        b"NSClassFromString" => routed_ns_class as P,
        b"objc_getClass" => routed_get_class as P,
        b"objc_lookUpClass" => routed_lookup_class as P,
        b"objc_getMetaClass" => routed_meta_class as P,
        b"dlsym" => routed_dlsym as P,
        b"dlopen" => routed_dlopen as P,
        b"CFBundleGetFunctionPointerForName" => routed_bundle_proc as P,
        _ => std::ptr::null_mut(),
    };
    if !f.is_null() {
        return f;
    }
    if is_gl_name(n) {
        return backend::get_proc_address(name);
    }
    std::ptr::null_mut()
}

fn routing_candidate(name: &[u8]) -> bool {
    let Some(name) = name.strip_prefix(b"_") else { return false };
    is_gl_name(name)
        || name.starts_with(b"EAGL")
        || matches!(
            name,
            b"eglGetProcAddress"
                | b"OBJC_CLASS_$_EAGLContext"
                | b"OBJC_CLASS_$_EAGLSharegroup"
                | b"NSClassFromString"
                | b"OBJC_METACLASS_$_EAGLContext"
                | b"OBJC_METACLASS_$_EAGLSharegroup"
                | b"objc_getClass"
                | b"objc_lookUpClass"
                | b"objc_getMetaClass"
                | b"OBJC_CLASS_$_GLKView"
                | b"OBJC_METACLASS_$_GLKView"
                | b"OBJC_CLASS_$_GLKViewController"
                | b"OBJC_METACLASS_$_GLKViewController"
                | b"dlsym"
                | b"dlopen"
                | b"CFBundleGetFunctionPointerForName"
        )
}

// ---------------------------------------------------------------------------
// Mach-O import patching

struct Patch {
    slot: *mut usize,
    replacement: usize,
}

#[repr(C)]
struct Segment {
    cmd: u32,
    cmdsize: u32,
    segname: [u8; 16],
    vmaddr: u64,
    vmsize: u64,
    fileoff: u64,
    filesize: u64,
    maxprot: i32,
    initprot: i32,
    nsects: u32,
    flags: u32,
}

#[repr(C)]
struct DyldInfo {
    cmd: u32,
    cmdsize: u32,
    rebase_off: u32,
    rebase_size: u32,
    bind_off: u32,
    bind_size: u32,
    weak_bind_off: u32,
    weak_bind_size: u32,
    lazy_bind_off: u32,
    lazy_bind_size: u32,
    export_off: u32,
    export_size: u32,
}

#[repr(C)]
struct LinkeditData {
    cmd: u32,
    cmdsize: u32,
    dataoff: u32,
    datasize: u32,
}

#[repr(C)]
struct MachHeader64 {
    magic: u32,
    cputype: i32,
    cpusubtype: i32,
    filetype: u32,
    ncmds: u32,
    sizeofcmds: u32,
    flags: u32,
    reserved: u32,
}

const LC_SEGMENT_64: u32 = 0x19;
const LC_DYLD_INFO: u32 = 0x22;
const LC_DYLD_INFO_ONLY: u32 = 0x8000_0022;
const LC_DYLD_CHAINED_FIXUPS: u32 = 0x8000_0034;
const DYLIB_COMMANDS: [u32; 5] = [0xC, 0x8000_0018, 0x8000_001F, 0x8000_0023, 0x20];
const CPU_TYPE_ARM64: i32 = 0x0100_000C;
const CPU_SUBTYPE_MASK: i32 = 0xff00_0000u32 as i32;
const CPU_SUBTYPE_ARM64E: i32 = 2;

fn read_uleb(p: &mut &[u8]) -> Option<u64> {
    let mut value = 0u64;
    let mut shift = 0;
    while shift < 64 {
        let (&byte, rest) = p.split_first()?;
        *p = rest;
        if shift == 63 && byte & 0x7e != 0 {
            return None;
        }
        value |= ((byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return Some(value);
        }
        shift += 7;
    }
    None
}

/// Reads an SLEB addend; Some(nonzero).
fn read_addend(p: &mut &[u8]) -> Option<bool> {
    let mut nonzero = false;
    for index in 0..10 {
        let (&byte, rest) = p.split_first()?;
        *p = rest;
        if index == 9 && byte != 0 && byte != 0x7f {
            return None;
        }
        nonzero |= byte & 0x7f != 0;
        if byte & 0x80 == 0 {
            return Some(nonzero);
        }
    }
    None
}

fn bridged(ordinal: i64, dylibs: &[String]) -> bool {
    ordinal > 0 && (ordinal as usize) <= dylibs.len() && dylibs[ordinal as usize - 1].contains("libPearOpenGLES.dylib")
}

unsafe fn read_bindings(mut p: &[u8], lazy: bool, segments: &[*const Segment], slide: isize, dylibs: &[String], patches: &mut Vec<Patch>) -> bool {
    let debug = rt::env_set("PEAR_DEBUG_ANGLE_ROUTE");
    let begin_len = p.len();
    let (mut segment, mut kind, mut offset, mut has_addend, mut ordinal) = (u32::MAX, 1u8, 0u64, false, 0i64);
    let mut symbol: &[u8] = b"";
    let mut bind = |segment: u32, offset: u64, kind: u8, has_addend: bool, ordinal: i64, symbol: &[u8]| -> bool {
        if !routing_candidate(symbol) {
            return true;
        }
        // A converted image already links our typed exports; keep those slots.
        if bridged(ordinal, dylibs) {
            return true;
        }
        let name = String::from_utf8_lossy(symbol);
        if segment as usize >= segments.len() || kind != 1 || has_addend {
            trace(&format!("[PearRuntime][angle-route] invalid binding {name} segment={segment} type={kind} addend={}\n", has_addend as i32));
            return false;
        }
        let s = &*segments[segment as usize];
        if offset > s.vmsize || s.vmsize - offset < 8 || offset % 8 != 0 {
            trace(&format!("[PearRuntime][angle-route] invalid slot {name} offset={offset} size={}\n", s.vmsize));
            return false;
        }
        let c = CString::new(&symbol[1..]).unwrap_or_default();
        let replacement = game_symbol(c.as_ptr());
        if replacement.is_null() {
            trace(&format!("[PearRuntime][angle-route] unsupported import {name}\n"));
            return false;
        }
        patches.push(Patch { slot: (slide + s.vmaddr as isize + offset as isize) as *mut usize, replacement: replacement as usize });
        true
    };
    while let Some((&instruction, rest)) = p.split_first() {
        if debug {
            trace(&format!("[angle-bind] {}: {instruction:02x}\n", begin_len - p.len()));
        }
        p = rest;
        let (opcode, imm) = (instruction & 0xF0, instruction & 0x0F);
        match opcode {
            0x00 => {
                if !lazy {
                    return true;
                }
                segment = u32::MAX;
                offset = 0;
                symbol = b"";
                kind = 1;
                has_addend = false;
                ordinal = 0;
            }
            0x10 => ordinal = imm as i64,
            0x30 => ordinal = if imm != 0 { (imm | 0xf0) as i8 as i64 } else { 0 },
            0x20 => match read_uleb(&mut p) {
                Some(v) if v <= i32::MAX as u64 => ordinal = v as i64,
                _ => return false,
            },
            0x40 => {
                let Some(end) = p.iter().position(|b| *b == 0) else { return false };
                symbol = &p[..end];
                p = &p[end + 1..];
            }
            0x50 => kind = imm,
            0x60 => match read_addend(&mut p) {
                Some(n) => has_addend = n,
                None => return false,
            },
            0x70 => {
                segment = imm as u32;
                match read_uleb(&mut p) {
                    Some(v) => offset = v,
                    None => return false,
                }
            }
            0x80 => match read_uleb(&mut p) {
                // Backwards moves are two's-complement ULEBs: wrap like dyld.
                Some(v) => offset = offset.wrapping_add(v),
                None => return false,
            },
            0x90 => {
                if !bind(segment, offset, kind, has_addend, ordinal, symbol) {
                    return false;
                }
                offset = offset.wrapping_add(8);
            }
            0xA0 => {
                if !bind(segment, offset, kind, has_addend, ordinal, symbol) {
                    return false;
                }
                let Some(v) = read_uleb(&mut p) else { return false };
                offset = offset.wrapping_add(v).wrapping_add(8);
            }
            0xB0 => {
                if !bind(segment, offset, kind, has_addend, ordinal, symbol) {
                    return false;
                }
                offset = offset.wrapping_add((imm as u64 + 1) * 8);
            }
            0xC0 => {
                let (Some(count), Some(skip)) = (read_uleb(&mut p), read_uleb(&mut p)) else { return false };
                if count > 1_000_000 {
                    return false;
                }
                for _ in 0..count {
                    if !bind(segment, offset, kind, has_addend, ordinal, symbol) {
                        return false;
                    }
                    offset = offset.wrapping_add(skip).wrapping_add(8);
                }
            }
            _ => {
                trace(&format!("[PearRuntime][angle-route] unsupported bind opcode 0x{opcode:x}\n"));
                return false; // chained/threaded formats need the separate decoder
            }
        }
    }
    true
}

extern "C" {
    fn open(path: *const c_char, flags: c_int, ...) -> c_int;
    fn close(fd: c_int) -> c_int;
    fn fstat(fd: c_int, st: *mut [u8; 144]) -> c_int;
    fn mmap(addr: P, len: usize, prot: c_int, flags: c_int, fd: c_int, offset: i64) -> P;
    fn munmap(addr: P, len: usize) -> c_int;
}

/// Import slots of an image linked with chained fixups. dyld overwrites the
/// chains while loading, so they are walked in the file on disk; the import
/// table is read from the mapped __LINKEDIT. Plain 64-bit formats only.
unsafe fn read_chained_bindings(path: &CStr, fixups: &LinkeditData, linkedit_base: usize, segments: &[*const Segment], slide: isize, dylibs: &[String], patches: &mut Vec<Patch>) -> bool {
    let size = fixups.datasize as usize;
    if size < 28 {
        return false;
    }
    let data = std::slice::from_raw_parts((linkedit_base + fixups.dataoff as usize) as *const u8, size);
    let u32_at = |d: &[u8], o: usize| -> Option<u32> { d.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())) };
    let (version, starts_offset, imports_offset, symbols_offset, imports_count, imports_format, symbols_format) = (
        u32_at(data, 0).unwrap(),
        u32_at(data, 4).unwrap() as usize,
        u32_at(data, 8).unwrap() as usize,
        u32_at(data, 12).unwrap() as usize,
        u32_at(data, 16).unwrap(),
        u32_at(data, 20).unwrap(),
        u32_at(data, 24).unwrap(),
    );
    if version != 0 || symbols_format != 0 || starts_offset >= size || imports_offset >= size || symbols_offset >= size {
        trace("[PearRuntime][angle-route] unsupported chained fixups header\n");
        return false;
    }
    let import_size = match imports_format {
        1 => 4,
        2 => 8,
        3 => 16,
        _ => 0,
    };
    if import_size == 0 || imports_offset as u64 + imports_count as u64 * import_size as u64 > size as u64 {
        return false;
    }
    let symbols = &data[symbols_offset..];
    // Import ordinal -> (library ordinal, name).
    let import = |index: u32| -> Option<(i64, &[u8])> {
        if index >= imports_count {
            return None;
        }
        let entry = &data[imports_offset + index as usize * import_size..];
        let (library, name_offset) = if imports_format == 3 {
            let raw = u64::from_le_bytes(entry[..8].try_into().ok()?);
            let addend = u64::from_le_bytes(entry[8..16].try_into().ok()?);
            if addend != 0 {
                return None;
            }
            ((raw & 0xffff) as u16 as i16 as i64, (raw >> 32) as usize)
        } else {
            let raw = u32::from_le_bytes(entry[..4].try_into().ok()?);
            if imports_format == 2 && i32::from_le_bytes(entry[4..8].try_into().ok()?) != 0 {
                return None;
            }
            ((raw & 0xff) as u8 as i8 as i64, (raw >> 9) as usize)
        };
        let rest = symbols.get(name_offset..)?;
        let end = rest.iter().position(|b| *b == 0)?;
        Some((library, &rest[..end]))
    };

    // The original chains live in the arm64 slice of the file on disk.
    let fd = open(path.as_ptr(), 0x0000 | 0x0100_0000); // O_RDONLY | O_CLOEXEC
    if fd < 0 {
        return false;
    }
    let mut st = [0u8; 144];
    if fstat(fd, &mut st) != 0 {
        close(fd);
        return false;
    }
    let file_size = i64::from_le_bytes(st[96..104].try_into().unwrap()) as usize; // st_size
    if file_size < 32 {
        close(fd);
        return false;
    }
    let mapping = mmap(std::ptr::null_mut(), file_size, 1, 2, fd, 0); // PROT_READ, MAP_PRIVATE
    close(fd);
    if mapping as isize == -1 {
        return false;
    }
    struct Unmap(P, usize);
    impl Drop for Unmap {
        fn drop(&mut self) {
            unsafe { munmap(self.0, self.1) };
        }
    }
    let _unmap = Unmap(mapping, file_size);
    let file = std::slice::from_raw_parts(mapping as *const u8, file_size);
    let (mut slice, mut slice_size) = (0usize, file_size);
    let magic = u32::from_le_bytes(file[..4].try_into().unwrap());
    if magic == 0xbebafeca || magic == 0xcafebabe {
        let be = |o: usize| u32::from_be_bytes(file[o..o + 4].try_into().unwrap());
        let count = be(4);
        let mut found = false;
        for index in 0..count as usize {
            let at = 8 + index * 20;
            if at + 20 > file_size {
                break;
            }
            if be(at) as i32 == CPU_TYPE_ARM64 && (be(at + 4) as i32 & !CPU_SUBTYPE_MASK) != CPU_SUBTYPE_ARM64E {
                slice = be(at + 8) as usize;
                slice_size = be(at + 12) as usize;
                found = true;
                break;
            }
        }
        if !found || slice + slice_size > file_size {
            return false;
        }
    }

    let starts = &data[starts_offset..];
    let Some(seg_count) = u32_at(starts, 0) else { return false };
    if seg_count as usize > segments.len() || 4 * (1 + seg_count as usize) > starts.len() {
        return false;
    }
    for segment_index in 0..seg_count as usize {
        let info_offset = u32_at(starts, 4 + 4 * segment_index).unwrap() as usize;
        if info_offset == 0 {
            continue;
        }
        let Some(chain) = starts.get(info_offset..) else { return false };
        if chain.len() < 22 {
            return false;
        }
        let page_size = u16::from_le_bytes(chain[4..6].try_into().unwrap()) as u64;
        let pointer_format = u16::from_le_bytes(chain[6..8].try_into().unwrap());
        let page_count = u16::from_le_bytes(chain[20..22].try_into().unwrap()) as usize;
        if chain.len() < 22 + 2 * page_count {
            return false;
        }
        if pointer_format != 2 && pointer_format != 6 {
            trace(&format!("[PearRuntime][angle-route] unsupported chained pointer format {pointer_format}\n"));
            return false;
        }
        let segment = &*segments[segment_index];
        for page in 0..page_count {
            let start = u16::from_le_bytes(chain[22 + 2 * page..24 + 2 * page].try_into().unwrap());
            if start == 0xFFFF {
                continue;
            }
            if start & 0x8000 != 0 {
                return false; // 32-bit formats only
            }
            let mut offset = page as u64 * page_size + start as u64;
            for _ in 0..1_000_000 {
                if offset + 8 > segment.filesize || offset + 8 > segment.vmsize {
                    return false;
                }
                let file_offset = (segment.fileoff + offset) as usize;
                if file_offset + 8 > slice_size {
                    return false;
                }
                let raw = u64::from_le_bytes(file[slice + file_offset..slice + file_offset + 8].try_into().unwrap());
                if raw >> 63 != 0 {
                    let ordinal = (raw & 0xFF_FFFF) as u32;
                    let addend = (raw >> 24) & 0xFF;
                    let Some((library, name)) = import(ordinal) else {
                        trace(&format!("[PearRuntime][angle-route] invalid chained import {ordinal}\n"));
                        return false;
                    };
                    if routing_candidate(name) && !bridged(library, dylibs) {
                        if addend != 0 || offset % 8 != 0 {
                            return false;
                        }
                        let c = CString::new(&name[1..]).unwrap_or_default();
                        let replacement = game_symbol(c.as_ptr());
                        if replacement.is_null() {
                            trace(&format!("[PearRuntime][angle-route] unsupported import {}\n", String::from_utf8_lossy(name)));
                            return false;
                        }
                        patches.push(Patch { slot: (slide + segment.vmaddr as isize + offset as isize) as *mut usize, replacement: replacement as usize });
                    }
                }
                let next = (raw >> 51) & 0xfff;
                if next == 0 {
                    break;
                }
                offset += next * 4;
            }
        }
    }
    true
}

extern "C" {
    static mach_task_self_: u32;
    fn mach_vm_region(task: u32, address: *mut u64, size: *mut u64, flavor: i32, info: *mut u32, count: *mut u32, object: *mut u32) -> i32;
    fn mach_vm_protect(task: u32, address: u64, size: u64, set_maximum: i32, protection: i32) -> i32;
    fn mach_port_deallocate(task: u32, name: u32) -> i32;
    fn getpagesize() -> c_int;
}

unsafe fn apply_patches(patches: &[Patch]) -> bool {
    let task = mach_task_self_;
    let page_size = getpagesize() as u64;
    let mut pages: Vec<(u64, i32)> = Vec::new();
    let mut ready = true;
    for patch in patches {
        let page = patch.slot as u64 & !(page_size - 1);
        if pages.iter().any(|(p, _)| *p == page) {
            continue;
        }
        let (mut region, mut region_size) = (page, 0u64);
        let mut info = [0u32; 9];
        let mut count = 9u32;
        let mut object = 0u32;
        let status = mach_vm_region(task, &mut region, &mut region_size, 9, info.as_mut_ptr(), &mut count, &mut object);
        if object != 0 {
            mach_port_deallocate(task, object);
        }
        if status != 0 || region > page || region_size < page - region + page_size {
            trace(&format!("[PearRuntime][angle-route] region query failed {status}\n"));
            ready = false;
            break;
        }
        let protection = info[0] as i32;
        let status = mach_vm_protect(task, page, page_size, 0, protection | 0x2 | 0x10);
        if status != 0 {
            trace(&format!("[PearRuntime][angle-route] import page protection failed {status}\n"));
            ready = false;
            break;
        }
        pages.push((page, protection));
    }
    if ready {
        for patch in patches {
            std::sync::atomic::AtomicUsize::from_ptr(patch.slot).store(patch.replacement, Ordering::Release);
        }
    }
    for (page, protection) in pages {
        if mach_vm_protect(task, page, page_size, 0, protection) != 0 {
            ready = false;
        }
    }
    ready
}

#[repr(C)]
struct DlInfo {
    fname: *const c_char,
    fbase: P,
    sname: *const c_char,
    saddr: P,
}
extern "C" {
    fn dladdr(addr: *const c_void, info: *mut DlInfo) -> c_int;
    fn _dyld_image_count() -> u32;
    fn _dyld_get_image_name(index: u32) -> *const c_char;
    fn _dyld_register_func_for_add_image(f: unsafe extern "C" fn(*const MachHeader64, isize));
    fn _NSGetExecutablePath(buf: *mut c_char, size: *mut u32) -> c_int;
    fn realpath(path: *const c_char, resolved: *mut c_char) -> *mut c_char;
    fn access(path: *const c_char, mode: c_int) -> c_int;
}

unsafe extern "C" fn route_image(header: *const MachHeader64, slide: isize) {
    let mut image = DlInfo { fname: std::ptr::null(), fbase: std::ptr::null_mut(), sname: std::ptr::null(), saddr: std::ptr::null_mut() };
    if dladdr(header as *const c_void, &mut image) == 0 || image.fname.is_null() {
        return;
    }
    let Some(root) = APP_ROOT.get() else { return };
    let path_c = CStr::from_ptr(image.fname);
    let path = path_c.to_string_lossy().into_owned();
    if path != *root && !path.starts_with(&format!("{root}/")) {
        return;
    }
    let leaf = path.rsplit('/').next().unwrap_or("");
    if matches!(leaf, "libPearRuntime.dylib" | "libcxxcompat.dylib" | "libswiftUIKit.dylib" | "libEGL.dylib" | "libGLESv2.dylib" | "libPearOpenGLES.dylib")
        || path.contains("MarketplaceKit.framework/")
    {
        return;
    }
    let fail = || -> ! {
        trace(&format!("[PearRuntime][angle-route] REFUSED image {path}; no mixed GL fallback\n"));
        libc_exit(78)
    };
    let h = &*header;
    if h.magic != 0xfeedfacf {
        fail();
    }
    if h.cputype != CPU_TYPE_ARM64 || (h.cpusubtype & !CPU_SUBTYPE_MASK) == CPU_SUBTYPE_ARM64E {
        fail();
    }
    let mut cursor = (header as *const u8).add(std::mem::size_of::<MachHeader64>());
    let end = cursor.add(h.sizeofcmds as usize);
    let mut segments: Vec<*const Segment> = Vec::new();
    let mut dylibs: Vec<String> = Vec::new();
    let (mut linkedit, mut dyld, mut chained): (*const Segment, *const DyldInfo, *const LinkeditData) = (std::ptr::null(), std::ptr::null(), std::ptr::null());
    for _ in 0..h.ncmds {
        if cursor.add(8) > end {
            fail();
        }
        let cmd = *(cursor as *const u32);
        let cmdsize = *(cursor.add(4) as *const u32) as usize;
        if cmdsize < 8 || cmdsize > end as usize - cursor as usize {
            fail();
        }
        if cmd == LC_SEGMENT_64 && cmdsize >= std::mem::size_of::<Segment>() {
            let s = cursor as *const Segment;
            segments.push(s);
            if (*s).segname.starts_with(b"__LINKEDIT") {
                linkedit = s;
            }
        }
        if (cmd == LC_DYLD_INFO || cmd == LC_DYLD_INFO_ONLY) && cmdsize >= std::mem::size_of::<DyldInfo>() {
            dyld = cursor as *const DyldInfo;
        }
        if cmd == LC_DYLD_CHAINED_FIXUPS && cmdsize >= std::mem::size_of::<LinkeditData>() {
            chained = cursor as *const LinkeditData;
        }
        if DYLIB_COMMANDS.contains(&cmd) && cmdsize >= 24 {
            let name_offset = *(cursor.add(8) as *const u32) as usize;
            if name_offset >= cmdsize {
                fail();
            }
            let bytes = std::slice::from_raw_parts(cursor.add(name_offset), cmdsize - name_offset);
            let Some(n) = bytes.iter().position(|b| *b == 0) else { fail() };
            dylibs.push(String::from_utf8_lossy(&bytes[..n]).into_owned());
        }
        cursor = cursor.add(cmdsize);
    }
    if linkedit.is_null() || (chained.is_null() && dyld.is_null()) {
        trace(&format!(
            "[PearRuntime][angle-route] metadata chained={} linkedit={} classic={}\n",
            (!chained.is_null()) as i32,
            (!linkedit.is_null()) as i32,
            (!dyld.is_null()) as i32
        ));
        fail();
    }
    let le = &*linkedit;
    let base = (slide + le.vmaddr as isize - le.fileoff as isize) as usize;
    let mut patches = Vec::new();
    if !chained.is_null() {
        let c = &*chained;
        if (c.dataoff as u64) < le.fileoff
            || c.dataoff as u64 + c.datasize as u64 > le.fileoff + le.filesize
            || !read_chained_bindings(path_c, c, base, &segments, slide, &dylibs, &mut patches)
        {
            trace("[PearRuntime][angle-route] chained fixups could not be routed\n");
            fail();
        }
        if !apply_patches(&patches) {
            fail();
        }
        trace(&format!("[PearRuntime][angle-route] bound {} imports in {path} (chained fixups)\n", patches.len()));
        return;
    }
    let d = &*dyld;
    let read = |offset: u32, size: u32, lazy: bool, patches: &mut Vec<Patch>| -> bool {
        if size == 0 {
            return true;
        }
        if (offset as u64) < le.fileoff || offset as u64 + size as u64 > le.fileoff + le.filesize {
            trace(&format!("[PearRuntime][angle-route] invalid bind range {offset}+{size} linkedit={}+{}\n", le.fileoff, le.filesize));
            return false;
        }
        read_bindings(std::slice::from_raw_parts((base + offset as usize) as *const u8, size as usize), lazy, &segments, slide, &dylibs, patches)
    };
    if !read(d.bind_off, d.bind_size, false, &mut patches) {
        trace(&format!(
            "[PearRuntime][angle-route] regular bindings failed off={} size={} linkeditVM={:x} file={} slide={:x}\n",
            d.bind_off, d.bind_size, le.vmaddr, le.fileoff, slide as usize
        ));
        fail();
    }
    if !read(d.lazy_bind_off, d.lazy_bind_size, true, &mut patches) {
        trace("[PearRuntime][angle-route] lazy bindings failed\n");
        fail();
    }
    if !read(d.weak_bind_off, d.weak_bind_size, false, &mut patches) {
        trace("[PearRuntime][angle-route] weak bindings failed\n");
        fail();
    }
    if !apply_patches(&patches) {
        fail();
    }
    trace(&format!("[PearRuntime][angle-route] bound {} imports in {path}\n", patches.len()));
}

fn exists(path: &str) -> bool {
    CString::new(path).map(|c| unsafe { access(c.as_ptr(), 4) == 0 }).unwrap_or(false)
}

/// Arms routing at load (no module guard: an app linking GL must not start
/// with the system OpenGL ES).
pub fn enable() {
    unsafe {
        let backend_env = std::env::var("PEAR_GRAPHICS_BACKEND").ok();
        let mut bridge = String::new();
        for index in 0.._dyld_image_count() {
            let p = _dyld_get_image_name(index);
            if p.is_null() {
                continue;
            }
            let p = CStr::from_ptr(p).to_string_lossy();
            if p.rsplit('/').next() == Some("libPearOpenGLES.dylib") {
                bridge = p.into_owned();
                break;
            }
        }
        // An app reaching OpenGL ES only at run time does not load the bridge
        // at launch: arm with the bridge next to the runtime, so a later
        // dlopen("OpenGLES") gets it, never the system framework.
        let mut runtime_directory = String::new();
        let mut me = DlInfo { fname: std::ptr::null(), fbase: std::ptr::null_mut(), sname: std::ptr::null(), saddr: std::ptr::null_mut() };
        if dladdr(enable as *const c_void, &mut me) != 0 && !me.fname.is_null() {
            let f = CStr::from_ptr(me.fname).to_string_lossy().into_owned();
            runtime_directory = f.rfind('/').map(|i| f[..i].to_string()).unwrap_or_default();
        }
        let mut lazy = false;
        if bridge.is_empty() && !runtime_directory.is_empty() {
            let sibling = format!("{runtime_directory}/libPearOpenGLES.dylib");
            if exists(&sibling) {
                bridge = sibling;
                lazy = backend_env.is_none();
            }
        }
        *BRIDGE_PATH.lock().unwrap_or_else(|e| e.into_inner()) = bridge.clone();
        if backend_env.as_deref().is_some_and(|b| b != "angle") {
            trace("[PearRuntime][angle-route] unsupported backend; only ANGLE is available\n");
            libc_exit(78);
        }
        if backend_env.is_none() && bridge.is_empty() {
            return;
        }
        if !rt::module_enabled("graphics-gl") {
            if lazy {
                // The app does not link GL: a core-only or Metal session runs on.
                trace("[PearRuntime][angle-route] graphics-gl disabled; GL routing not armed\n");
                return;
            }
            trace("[PearRuntime][angle-route] ANGLE requires the graphics-gl module\n");
            libc_exit(78);
        }
        pear_objc::autorelease_pool(|| {
            let mut executable = [0 as c_char; 1024];
            let mut length = executable.len() as u32;
            if _NSGetExecutablePath(executable.as_mut_ptr(), &mut length) != 0 {
                libc_exit(78);
            }
            let mut canonical = [0 as c_char; 1024];
            if realpath(executable.as_ptr(), canonical.as_mut_ptr()).is_null() {
                libc_exit(78);
            }
            let mut root = CStr::from_ptr(canonical.as_ptr()).to_string_lossy().into_owned();
            if let Some(app) = root.find(".app/") {
                root.truncate(app + 4);
            }
            let _ = APP_ROOT.set(root.clone());
            // ANGLE lives next to the runtime (Frameworks/PearANGLE); the app
            // root layout is the fallback, PEAR_ANGLE_LIB_DIR the override.
            let libraries = if let Ok(d) = std::env::var("PEAR_ANGLE_LIB_DIR") {
                d
            } else if !runtime_directory.is_empty() && exists(&format!("{runtime_directory}/PearANGLE/libEGL.dylib")) {
                format!("{runtime_directory}/PearANGLE")
            } else {
                rt::path_join(&root, "Frameworks/PearANGLE")
            };
            if lazy {
                *LAZY_DIRECTORY.lock().unwrap_or_else(|e| e.into_inner()) = libraries;
            } else {
                let mut error: Id = std::ptr::null_mut();
                if !backend::initialize(nsstring(&libraries), &mut error) {
                    let reason = if error.is_null() { "(null)".into() } else { rt::ns_string(msg0(error, "localizedDescription")) };
                    trace(&format!("[PearRuntime][angle-route] initialization failed: {reason}\n"));
                    libc_exit(78);
                }
            }
            READY.store(true, Ordering::Release);
            _dyld_register_func_for_add_image(route_image);
            super::shim::install_geometry_hooks();
            trace(if lazy {
                "[PearRuntime][angle-route] ANGLE routing armed; starts on first GL use\n"
            } else {
                "[PearRuntime][angle-route] ANGLE is the only GL backend\n"
            });
        });
    }
}
