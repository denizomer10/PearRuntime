//! PearANGLE smoke test: EGL display on Metal, ES3 context on a pbuffer,
//! clear to a known color, read it back.
//!
//!   ANGLE=build/host/PearANGLE-src
//!   RUSTFLAGS="-L $ANGLE -C link-arg=-Wl,-rpath,$PWD/$ANGLE" \
//!     cargo run --manifest-path rust/Cargo.toml -p pear-gles --example pearangle-smoke

use std::ffi::{c_char, c_void, CStr};

type EGLDisplay = *mut c_void;
type EGLConfig = *mut c_void;
type EGLSurface = *mut c_void;
type EGLContext = *mut c_void;
type EGLint = i32;

const EGL_NONE: EGLint = 0x3038;
const EGL_RED_SIZE: EGLint = 0x3024;
const EGL_GREEN_SIZE: EGLint = 0x3023;
const EGL_BLUE_SIZE: EGLint = 0x3022;
const EGL_ALPHA_SIZE: EGLint = 0x3021;
const EGL_SURFACE_TYPE: EGLint = 0x3033;
const EGL_PBUFFER_BIT: EGLint = 0x0001;
const EGL_RENDERABLE_TYPE: EGLint = 0x3040;
const EGL_OPENGL_ES3_BIT: EGLint = 0x0040;
const EGL_WIDTH: EGLint = 0x3057;
const EGL_HEIGHT: EGLint = 0x3056;
const EGL_CONTEXT_CLIENT_VERSION: EGLint = 0x3098;
const EGL_PLATFORM_ANGLE_ANGLE: u32 = 0x3202;
const EGL_PLATFORM_ANGLE_TYPE_ANGLE: EGLint = 0x3203;
const EGL_PLATFORM_ANGLE_TYPE_METAL_ANGLE: EGLint = 0x3489;
const GL_COLOR_BUFFER_BIT: u32 = 0x4000;
const GL_RGBA: u32 = 0x1908;
const GL_UNSIGNED_BYTE: u32 = 0x1401;
const GL_RENDERER: u32 = 0x1F01;
const GL_VERSION: u32 = 0x1F02;

#[link(name = "EGL")]
extern "C" {
    fn eglGetPlatformDisplay(platform: u32, native: *mut c_void, attributes: *const isize) -> EGLDisplay;
    fn eglGetProcAddress(name: *const c_char) -> *mut c_void;
    fn eglInitialize(display: EGLDisplay, major: *mut EGLint, minor: *mut EGLint) -> u32;
    fn eglChooseConfig(display: EGLDisplay, attributes: *const EGLint, configs: *mut EGLConfig, size: EGLint, count: *mut EGLint) -> u32;
    fn eglCreatePbufferSurface(display: EGLDisplay, config: EGLConfig, attributes: *const EGLint) -> EGLSurface;
    fn eglCreateContext(display: EGLDisplay, config: EGLConfig, share: EGLContext, attributes: *const EGLint) -> EGLContext;
    fn eglMakeCurrent(display: EGLDisplay, draw: EGLSurface, read: EGLSurface, context: EGLContext) -> u32;
    fn eglGetError() -> EGLint;
    fn eglTerminate(display: EGLDisplay) -> u32;
}

#[link(name = "GLESv2")]
extern "C" {
    fn glClearColor(r: f32, g: f32, b: f32, a: f32);
    fn glClear(mask: u32);
    fn glReadPixels(x: i32, y: i32, width: i32, height: i32, format: u32, kind: u32, pixels: *mut c_void);
    fn glGetString(name: u32) -> *const c_char;
}

fn main() {
    unsafe {
        let display_attributes = [EGL_PLATFORM_ANGLE_TYPE_ANGLE, EGL_PLATFORM_ANGLE_TYPE_METAL_ANGLE, EGL_NONE];
        let mut d = eglGetPlatformDisplay(EGL_PLATFORM_ANGLE_ANGLE, std::ptr::null_mut(), std::ptr::null());
        let get_ext = eglGetProcAddress(c"eglGetPlatformDisplayEXT".as_ptr());
        if !get_ext.is_null() {
            let get_ext: unsafe extern "C" fn(u32, *mut c_void, *const EGLint) -> EGLDisplay = std::mem::transmute(get_ext);
            d = get_ext(EGL_PLATFORM_ANGLE_ANGLE, std::ptr::null_mut(), display_attributes.as_ptr());
        }
        let (mut major, mut minor) = (0, 0);
        if eglInitialize(d, &mut major, &mut minor) == 0 {
            println!("FAIL eglInitialize 0x{:x}", eglGetError());
            std::process::exit(1);
        }
        let config_attributes = [
            EGL_RED_SIZE, 8, EGL_GREEN_SIZE, 8, EGL_BLUE_SIZE, 8, EGL_ALPHA_SIZE, 8,
            EGL_SURFACE_TYPE, EGL_PBUFFER_BIT, EGL_RENDERABLE_TYPE, EGL_OPENGL_ES3_BIT, EGL_NONE,
        ];
        let mut config: EGLConfig = std::ptr::null_mut();
        let mut n = 0;
        if eglChooseConfig(d, config_attributes.as_ptr(), &mut config, 1, &mut n) == 0 || n < 1 {
            println!("FAIL eglChooseConfig");
            std::process::exit(1);
        }
        let surface_attributes = [EGL_WIDTH, 64, EGL_HEIGHT, 64, EGL_NONE];
        let s = eglCreatePbufferSurface(d, config, surface_attributes.as_ptr());
        let context_attributes = [EGL_CONTEXT_CLIENT_VERSION, 3, EGL_NONE];
        let c = eglCreateContext(d, config, std::ptr::null_mut(), context_attributes.as_ptr());
        if s.is_null() || c.is_null() || eglMakeCurrent(d, s, s, c) == 0 {
            println!("FAIL context 0x{:x}", eglGetError());
            std::process::exit(1);
        }
        glClearColor(0.25, 0.5, 0.75, 1.0);
        glClear(GL_COLOR_BUFFER_BIT);
        let mut px = [0u8; 4];
        glReadPixels(32, 32, 1, 1, GL_RGBA, GL_UNSIGNED_BYTE, px.as_mut_ptr() as *mut c_void);
        let text = |p: *const c_char| if p.is_null() { "(null)".into() } else { CStr::from_ptr(p).to_string_lossy() };
        println!("EGL {major}.{minor}\nGL_RENDERER: {}\nGL_VERSION: {}", text(glGetString(GL_RENDERER)), text(glGetString(GL_VERSION)));
        println!("pixel: {} {} {} {}", px[0], px[1], px[2], px[3]);
        let ok = (62..=66).contains(&px[0]) && (126..=130).contains(&px[1]) && (189..=193).contains(&px[2]) && px[3] == 255;
        println!("{}", if ok { "PASS" } else { "FAIL pixel" });
        eglTerminate(d);
        std::process::exit(if ok { 0 } else { 1 });
    }
}
