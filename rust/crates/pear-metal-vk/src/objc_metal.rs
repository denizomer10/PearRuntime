//! Clean-room Metal Objective-C API surface registered onto `pear-libobjc`,
//! backed by `mtlb`, `spirv`, `vk`, and real CPU buffer/texture memory.

use crate::mtlb;
use crate::spirv;
use crate::vk::VkContext;
use pear_libobjc::abi::{ClassT, Id, Imp, Sel};
use pear_libobjc::message::lookup_for_send;
use pear_libobjc::object::{alloc, EMPTY_CACHE, NSOBJECT_CLASS, NSOBJECT_META, StaticClass};
use pear_libobjc::runtime as objc_rt;
use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, Once, OnceLock};

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct MTLOrigin {
    pub x: usize,
    pub y: usize,
    pub z: usize,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct MTLSize {
    pub width: usize,
    pub height: usize,
    pub depth: usize,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct MTLRegion {
    pub origin: MTLOrigin,
    pub size: MTLSize,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct MTLClearColor {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
    pub alpha: f64,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct MTLViewport {
    pub origin_x: f64,
    pub origin_y: f64,
    pub width: f64,
    pub height: f64,
    pub znear: f64,
    pub zfar: f64,
}

macro_rules! metal_class {
    ($cls:ident, $meta:ident) => {
        static $meta: StaticClass = StaticClass(UnsafeCell::new(ClassT {
            isa: &raw const NSOBJECT_META as *const ClassT,
            superclass: &raw const NSOBJECT_META as *const ClassT,
            cache: &raw const EMPTY_CACHE as *const c_void,
            vtable: std::ptr::null(),
            bits: 0,
        }));
        pub static $cls: StaticClass = StaticClass(UnsafeCell::new(ClassT {
            isa: &raw const $meta as *const ClassT,
            superclass: &raw const NSOBJECT_CLASS as *const ClassT,
            cache: &raw const EMPTY_CACHE as *const c_void,
            vtable: std::ptr::null(),
            bits: 0,
        }));
    };
}

metal_class!(MTLDEVICE_CLASS, MTLDEVICE_META);
metal_class!(MTLCOMMANDQUEUE_CLASS, MTLCOMMANDQUEUE_META);
metal_class!(MTLCOMMANDBUFFER_CLASS, MTLCOMMANDBUFFER_META);
metal_class!(MTLRENDERCOMMANDENCODER_CLASS, MTLRENDERCOMMANDENCODER_META);
metal_class!(MTLCOMPUTECOMMANDENCODER_CLASS, MTLCOMPUTECOMMANDENCODER_META);
metal_class!(MTLBLITCOMMANDENCODER_CLASS, MTLBLITCOMMANDENCODER_META);
metal_class!(MTLBUFFER_CLASS, MTLBUFFER_META);
metal_class!(MTLTEXTURE_CLASS, MTLTEXTURE_META);
metal_class!(MTLLIBRARY_CLASS, MTLLIBRARY_META);
metal_class!(MTLFUNCTION_CLASS, MTLFUNCTION_META);
metal_class!(MTLRENDERPIPELINESTATE_CLASS, MTLRENDERPIPELINESTATE_META);
metal_class!(MTLCOMPUTEPIPELINESTATE_CLASS, MTLCOMPUTEPIPELINESTATE_META);
metal_class!(MTLDEPTHSTENCILSTATE_CLASS, MTLDEPTHSTENCILSTATE_META);
metal_class!(MTLSAMPLERSTATE_CLASS, MTLSAMPLERSTATE_META);
metal_class!(MTLDRAWABLE_CLASS, MTLDRAWABLE_META);
metal_class!(MTLHEAP_CLASS, MTLHEAP_META);
metal_class!(MTLFENCE_CLASS, MTLFENCE_META);
metal_class!(MTLEVENT_CLASS, MTLEVENT_META);
metal_class!(MTLSHAREDEVENT_CLASS, MTLSHAREDEVENT_META);
metal_class!(MTLTEXTUREDESCRIPTOR_CLASS, MTLTEXTUREDESCRIPTOR_META);
metal_class!(MTLRENDERPIPELINEDESCRIPTOR_CLASS, MTLRENDERPIPELINEDESCRIPTOR_META);
metal_class!(MTLRENDERPASSDESCRIPTOR_CLASS, MTLRENDERPASSDESCRIPTOR_META);
metal_class!(MTLRENDERPASSCOLORARRAY_CLASS, MTLRENDERPASSCOLORARRAY_META);
metal_class!(MTLRENDERPASSCOLORATTACH_CLASS, MTLRENDERPASSCOLORATTACH_META);
metal_class!(MTLRENDERPASSDEPTHATTACH_CLASS, MTLRENDERPASSDEPTHATTACH_META);
metal_class!(MTLPIPELINE_COLOR_ARRAY_CLASS, MTLPIPELINE_COLOR_ARRAY_META);
metal_class!(MTLPIPELINE_COLOR_ATTACH_CLASS, MTLPIPELINE_COLOR_ATTACH_META);
metal_class!(MTLDEPTHSTENCILDESCRIPTOR_CLASS, MTLDEPTHSTENCILDESCRIPTOR_META);
metal_class!(MTLSAMPLERDESCRIPTOR_CLASS, MTLSAMPLERDESCRIPTOR_META);
metal_class!(MTLVERTEXDESCRIPTOR_CLASS, MTLVERTEXDESCRIPTOR_META);
metal_class!(MTLVERTEXATTRARRAY_CLASS, MTLVERTEXATTRARRAY_META);
metal_class!(MTLVERTEXATTRDESC_CLASS, MTLVERTEXATTRDESC_META);
metal_class!(MTLVERTEXLAYOUTARRAY_CLASS, MTLVERTEXLAYOUTARRAY_META);
metal_class!(MTLVERTEXLAYOUTDESC_CLASS, MTLVERTEXLAYOUTDESC_META);
metal_class!(MTLCOMPUTEPIPELINEDESC_CLASS, MTLCOMPUTEPIPELINEDESC_META);
metal_class!(MTLHEAPDESCRIPTOR_CLASS, MTLHEAPDESCRIPTOR_META);
metal_class!(MTLCOMPILEOPTIONS_CLASS, MTLCOMPILEOPTIONS_META);
metal_class!(MTLFUNCTIONCONSTANTS_CLASS, MTLFUNCTIONCONSTANTS_META);
metal_class!(MTLCAPTUREMANAGER_CLASS, MTLCAPTUREMANAGER_META);
metal_class!(MTLCAPTUREDESCRIPTOR_CLASS, MTLCAPTUREDESCRIPTOR_META);
metal_class!(MTLSTENCILDESCRIPTOR_CLASS, MTLSTENCILDESCRIPTOR_META);

struct BufferState {
    bytes: Vec<u8>,
}

struct TextureState {
    width: usize,
    height: usize,
    depth: usize,
    pixel_format: usize,
    texture_type: usize,
    mipmap_levels: usize,
    usage: usize,
    pixels: Vec<u8>,
}

struct LibraryState {
    functions: HashMap<String, mtlb::MtlbFunction>,
    order: Vec<String>,
}

struct FunctionState {
    name: String,
    stage: mtlb::ShaderStage,
    spirv: Vec<u32>,
    vk_module: u64,
}

#[derive(Default)]
struct CommandBufferState {
    status: usize, // 4 = Completed
    draw_calls: usize,
    compute_dispatches: usize,
    blit_ops: usize,
    completed_handlers: Vec<usize>,
    scheduled_handlers: Vec<usize>,
}

static VK_CTX: OnceLock<Option<VkContext>> = OnceLock::new();
static DEFAULT_DEVICE: OnceLock<usize> = OnceLock::new();
static BUFFERS: OnceLock<Mutex<HashMap<usize, BufferState>>> = OnceLock::new();
static TEXTURES: OnceLock<Mutex<HashMap<usize, TextureState>>> = OnceLock::new();
static LIBRARIES: OnceLock<Mutex<HashMap<usize, LibraryState>>> = OnceLock::new();
static FUNCTIONS: OnceLock<Mutex<HashMap<usize, FunctionState>>> = OnceLock::new();
static CMD_BUFFERS: OnceLock<Mutex<HashMap<usize, CommandBufferState>>> = OnceLock::new();
static ENCODER_TO_CB: OnceLock<Mutex<HashMap<usize, usize>>> = OnceLock::new();
static DRAWABLE_TO_TEX: OnceLock<Mutex<HashMap<usize, usize>>> = OnceLock::new();
static DRAWABLE_TO_LAYER: OnceLock<Mutex<HashMap<usize, usize>>> = OnceLock::new();
static TEX_CLEAR_COLOR: OnceLock<Mutex<HashMap<usize, [f32; 4]>>> = OnceLock::new();
static LAST_PRESENTED_FRAME: OnceLock<Mutex<Option<(usize, usize, Vec<u8>)>>> = OnceLock::new();
static LAYER_DRAWABLES: OnceLock<Mutex<HashMap<usize, usize>>> = OnceLock::new();
static SUBSCRIPT_SLOTS: OnceLock<Mutex<HashMap<(usize, usize), usize>>> = OnceLock::new();
static PROPS_ID: OnceLock<Mutex<HashMap<(usize, usize), usize>>> = OnceLock::new();
static PROPS_USIZE: OnceLock<Mutex<HashMap<(usize, usize), usize>>> = OnceLock::new();
static PROPS_SIZE: OnceLock<Mutex<HashMap<(usize, usize), MTLSize>>> = OnceLock::new();
static PROPS_COLOR: OnceLock<Mutex<HashMap<(usize, usize), MTLClearColor>>> = OnceLock::new();
static PROPS_F64: OnceLock<Mutex<HashMap<(usize, usize), f64>>> = OnceLock::new();

static TOTAL_MTLB_LIBRARIES: AtomicUsize = AtomicUsize::new(0);
static TOTAL_MTLB_FUNCTIONS: AtomicUsize = AtomicUsize::new(0);
static TOTAL_DRAW_CALLS: AtomicUsize = AtomicUsize::new(0);
static TOTAL_COMPUTE_DISPATCHES: AtomicUsize = AtomicUsize::new(0);
static TOTAL_PRESENTS: AtomicUsize = AtomicUsize::new(0);

/// Returns `(mtlb_libraries, mtlb_functions, draw_calls, compute_dispatches, presents)`.
pub fn render_stats() -> (usize, usize, usize, usize, usize) {
    (
        TOTAL_MTLB_LIBRARIES.load(Ordering::Relaxed),
        TOTAL_MTLB_FUNCTIONS.load(Ordering::Relaxed),
        TOTAL_DRAW_CALLS.load(Ordering::Relaxed),
        TOTAL_COMPUTE_DISPATCHES.load(Ordering::Relaxed),
        TOTAL_PRESENTS.load(Ordering::Relaxed),
    )
}

fn vk_context() -> Option<&'static VkContext> {
    VK_CTX.get_or_init(VkContext::try_create).as_ref()
}

fn buffers() -> &'static Mutex<HashMap<usize, BufferState>> {
    BUFFERS.get_or_init(Default::default)
}
fn textures() -> &'static Mutex<HashMap<usize, TextureState>> {
    TEXTURES.get_or_init(Default::default)
}
fn libraries() -> &'static Mutex<HashMap<usize, LibraryState>> {
    LIBRARIES.get_or_init(Default::default)
}
fn functions() -> &'static Mutex<HashMap<usize, FunctionState>> {
    FUNCTIONS.get_or_init(Default::default)
}
fn cmd_buffers() -> &'static Mutex<HashMap<usize, CommandBufferState>> {
    CMD_BUFFERS.get_or_init(Default::default)
}
fn encoder_to_cb() -> &'static Mutex<HashMap<usize, usize>> {
    ENCODER_TO_CB.get_or_init(Default::default)
}
fn drawable_to_tex() -> &'static Mutex<HashMap<usize, usize>> {
    DRAWABLE_TO_TEX.get_or_init(Default::default)
}
fn drawable_to_layer() -> &'static Mutex<HashMap<usize, usize>> {
    DRAWABLE_TO_LAYER.get_or_init(Default::default)
}
fn tex_clear_color() -> &'static Mutex<HashMap<usize, [f32; 4]>> {
    TEX_CLEAR_COLOR.get_or_init(Default::default)
}
fn last_presented_frame_slot() -> &'static Mutex<Option<(usize, usize, Vec<u8>)>> {
    LAST_PRESENTED_FRAME.get_or_init(|| Mutex::new(None))
}
/// Returns a copy of the most recently presented `(width, height, rgba8)` framebuffer.
pub fn last_presented_frame() -> Option<(usize, usize, Vec<u8>)> {
    last_presented_frame_slot().lock().unwrap().clone()
}
fn layer_drawables() -> &'static Mutex<HashMap<usize, usize>> {
    LAYER_DRAWABLES.get_or_init(Default::default)
}
fn subscript_slots() -> &'static Mutex<HashMap<(usize, usize), usize>> {
    SUBSCRIPT_SLOTS.get_or_init(Default::default)
}
fn props_id() -> &'static Mutex<HashMap<(usize, usize), usize>> {
    PROPS_ID.get_or_init(Default::default)
}
fn props_usize() -> &'static Mutex<HashMap<(usize, usize), usize>> {
    PROPS_USIZE.get_or_init(Default::default)
}
fn props_size() -> &'static Mutex<HashMap<(usize, usize), MTLSize>> {
    PROPS_SIZE.get_or_init(Default::default)
}
fn props_color() -> &'static Mutex<HashMap<(usize, usize), MTLClearColor>> {
    PROPS_COLOR.get_or_init(Default::default)
}
fn props_f64() -> &'static Mutex<HashMap<(usize, usize), f64>> {
    PROPS_F64.get_or_init(Default::default)
}

unsafe fn send0<R>(obj: Id, name: &str) -> R {
    let s = objc_rt::sel(name);
    let d = lookup_for_send(obj, s);
    let f: unsafe extern "C" fn(Id, Sel) -> R = std::mem::transmute(d.imp);
    f(d.receiver, s)
}

unsafe fn send1<A1, R>(obj: Id, name: &str, a1: A1) -> R {
    let s = objc_rt::sel(name);
    let d = lookup_for_send(obj, s);
    let f: unsafe extern "C" fn(Id, Sel, A1) -> R = std::mem::transmute(d.imp);
    f(d.receiver, s, a1)
}

unsafe fn invoke_cb_handler(block_ptr: usize, cb: Id) {
    if block_ptr == 0 {
        return;
    }
    let block = block_ptr as *const c_void;
    let invoke = *((block as *const usize).add(2));
    if invoke != 0 {
        let f: unsafe extern "C" fn(*const c_void, Id) = std::mem::transmute(invoke);
        f(block, cb);
    }
}

unsafe fn make_nsarray(items: &[Id]) -> Id {
    let Some(cls) = objc_rt::class_named("NSArray") else {
        return std::ptr::null_mut();
    };
    let s = objc_rt::sel("arrayWithObjects:count:");
    let d = lookup_for_send(cls as Id, s);
    let f: unsafe extern "C" fn(Id, Sel, *const Id, usize) -> Id = std::mem::transmute(d.imp);
    f(d.receiver, s, items.as_ptr(), items.len())
}

// ---- Generic property helpers ----
extern "C" fn prop_get_id(this: Id, cmd: Sel) -> Id {
    props_id()
        .lock()
        .unwrap()
        .get(&(this as usize, cmd as usize))
        .copied()
        .unwrap_or(0) as Id
}

fn getter_sel_for_setter(cmd: Sel) -> Option<usize> {
    let name = unsafe { std::ffi::CStr::from_ptr(cmd) }.to_str().ok()?;
    let rest = name.strip_prefix("set")?.strip_suffix(':')?;
    let mut chars = rest.chars();
    let first = chars.next()?;
    let getter = format!("{}{}", first.to_ascii_lowercase(), chars.as_str());
    Some(objc_rt::sel(&getter) as usize)
}

fn store_retained_slot<K: Eq + std::hash::Hash>(
    map: &Mutex<HashMap<K, usize>>,
    key: K,
    val: Id,
) {
    if !val.is_null() {
        pear_libobjc::object::retain(val);
    }
    let prev = {
        let mut g = map.lock().unwrap();
        if val.is_null() {
            g.remove(&key)
        } else {
            g.insert(key, val as usize)
        }
    };
    if let Some(old) = prev {
        if old != 0 {
            pear_libobjc::object::release(old as Id);
        }
    }
}

extern "C" fn prop_set_id(this: Id, cmd: Sel, val: Id) {
    if let Some(gk) = getter_sel_for_setter(cmd) {
        store_retained_slot(props_id(), (this as usize, gk), val);
    }
}
extern "C" fn prop_get_usize(this: Id, cmd: Sel) -> usize {
    props_usize()
        .lock()
        .unwrap()
        .get(&(this as usize, cmd as usize))
        .copied()
        .unwrap_or(0)
}
extern "C" fn prop_set_usize(this: Id, cmd: Sel, val: usize) {
    if let Some(gk) = getter_sel_for_setter(cmd) {
        props_usize().lock().unwrap().insert((this as usize, gk), val);
    }
}
extern "C" fn prop_get_size(this: Id, cmd: Sel) -> MTLSize {
    props_size()
        .lock()
        .unwrap()
        .get(&(this as usize, cmd as usize))
        .copied()
        .unwrap_or(MTLSize { width: 1, height: 1, depth: 1 })
}
extern "C" fn prop_set_size(this: Id, cmd: Sel, val: MTLSize) {
    if let Some(gk) = getter_sel_for_setter(cmd) {
        props_size().lock().unwrap().insert((this as usize, gk), val);
    }
}
extern "C" fn prop_get_color(this: Id, cmd: Sel) -> MTLClearColor {
    props_color()
        .lock()
        .unwrap()
        .get(&(this as usize, cmd as usize))
        .copied()
        .unwrap_or_default()
}
extern "C" fn prop_set_color(this: Id, cmd: Sel, val: MTLClearColor) {
    if let Some(gk) = getter_sel_for_setter(cmd) {
        props_color().lock().unwrap().insert((this as usize, gk), val);
    }
}
extern "C" fn prop_get_f64_default_one(this: Id, cmd: Sel) -> f64 {
    props_f64()
        .lock()
        .unwrap()
        .get(&(this as usize, cmd as usize))
        .copied()
        .unwrap_or(1.0)
}
extern "C" fn prop_set_f64(this: Id, cmd: Sel, val: f64) {
    if let Some(gk) = getter_sel_for_setter(cmd) {
        props_f64().lock().unwrap().insert((this as usize, gk), val);
    }
}
extern "C" fn ds_is_alpha_to_coverage(this: Id, _cmd: Sel) -> usize {
    let g = props_usize().lock().unwrap();
    g.get(&(this as usize, objc_rt::sel("isAlphaToCoverageEnabled") as usize))
        .or_else(|| g.get(&(this as usize, objc_rt::sel("alphaToCoverageEnabled") as usize)))
        .copied()
        .unwrap_or(0)
}

extern "C" fn mtl_ret_self(this: Id, _cmd: Sel) -> Id { this }
extern "C" fn mtl_true(_this: Id, _cmd: Sel) -> bool { true }
extern "C" fn mtl_false(_this: Id, _cmd: Sel) -> bool { false }
extern "C" fn mtl_true1(_this: Id, _cmd: Sel, _a: usize) -> bool { true }
extern "C" fn mtl_noop0(_this: Id, _cmd: Sel) {}
extern "C" fn mtl_noop1(_this: Id, _cmd: Sel, _a: usize) {}
extern "C" fn mtl_noop2(_this: Id, _cmd: Sel, _a: usize, _b: usize) {}
extern "C" fn mtl_noop3(_this: Id, _cmd: Sel, _a: usize, _b: usize, _c: usize) {}
extern "C" fn mtl_noop4(_this: Id, _cmd: Sel, _a: usize, _b: usize, _c: usize, _d: usize) {}
extern "C" fn mtl_null0(_this: Id, _cmd: Sel) -> Id { std::ptr::null_mut() }
extern "C" fn mtl_one(_this: Id, _cmd: Sel) -> usize { 1 }
extern "C" fn mtl_zero(_this: Id, _cmd: Sel) -> usize { 0 }

// ---- MTLDevice ----
pub extern "C" fn create_system_default_device() -> Id {
    init();
    *DEFAULT_DEVICE.get_or_init(|| {
        let _ = vk_context();
        unsafe { alloc(MTLDEVICE_CLASS.ptr()) as usize }
    }) as Id
}

pub extern "C" fn copy_all_devices() -> Id {
    let dev = create_system_default_device();
    unsafe { make_nsarray(&[dev]) }
}

extern "C" fn dev_name(_this: Id, _cmd: Sel) -> Id {
    let name = vk_context()
        .map(|ctx| ctx.device_name().to_string())
        .unwrap_or_else(|| "PearRuntime Metal-Vulkan GPU".to_string());
    pear_foundation::nsstring_from_rust(&name)
}

extern "C" fn dev_max_threads_per_tg(_this: Id, _cmd: Sel) -> MTLSize {
    MTLSize { width: 1024, height: 1024, depth: 64 }
}

extern "C" fn dev_max_buffer_len(_this: Id, _cmd: Sel) -> usize {
    1 << 30
}

extern "C" fn dev_supports_family(_this: Id, _cmd: Sel, _family: isize) -> bool {
    true
}

extern "C" fn dev_new_command_queue(_this: Id, _cmd: Sel) -> Id {
    unsafe { alloc(MTLCOMMANDQUEUE_CLASS.ptr()) }
}

extern "C" fn dev_new_command_queue_max(_this: Id, _cmd: Sel, _max: usize) -> Id {
    unsafe { alloc(MTLCOMMANDQUEUE_CLASS.ptr()) }
}

extern "C" fn dev_new_buffer_len(_this: Id, _cmd: Sel, length: usize, _opts: usize) -> Id {
    let obj = unsafe { alloc(MTLBUFFER_CLASS.ptr()) };
    buffers().lock().unwrap().insert(obj as usize, BufferState {
        bytes: vec![0u8; length],
    });
    obj
}

extern "C" fn dev_new_buffer_bytes(_this: Id, _cmd: Sel, ptr: *const u8, length: usize, _opts: usize) -> Id {
    let obj = unsafe { alloc(MTLBUFFER_CLASS.ptr()) };
    let bytes = if ptr.is_null() || length == 0 {
        vec![0u8; length]
    } else {
        unsafe { std::slice::from_raw_parts(ptr, length).to_vec() }
    };
    buffers().lock().unwrap().insert(obj as usize, BufferState { bytes });
    obj
}

extern "C" fn dev_new_buffer_nocopy(
    this: Id,
    cmd: Sel,
    ptr: *const u8,
    length: usize,
    opts: usize,
    _dealloc: *const c_void,
) -> Id {
    dev_new_buffer_bytes(this, cmd, ptr, length, opts)
}

extern "C" fn dev_new_texture_desc(_this: Id, _cmd: Sel, desc: Id) -> Id {
    let (w, h, d, pf, tt, mips, usage) = if desc.is_null() {
        (1, 1, 1, 80, 2, 1, 1)
    } else {
        unsafe {
            (
                send0::<usize>(desc, "width").max(1),
                send0::<usize>(desc, "height").max(1),
                send0::<usize>(desc, "depth").max(1),
                send0::<usize>(desc, "pixelFormat"),
                send0::<usize>(desc, "textureType"),
                send0::<usize>(desc, "mipmapLevelCount").max(1),
                send0::<usize>(desc, "usage"),
            )
        }
    };
    create_texture_object(w, h, d, pf, tt, mips, usage)
}

fn create_texture_object(
    width: usize,
    height: usize,
    depth: usize,
    pixel_format: usize,
    texture_type: usize,
    mipmap_levels: usize,
    usage: usize,
) -> Id {
    let obj = unsafe { alloc(MTLTEXTURE_CLASS.ptr()) };
    let byte_len = width.saturating_mul(height).saturating_mul(depth).saturating_mul(4).min(64 << 20);
    textures().lock().unwrap().insert(obj as usize, TextureState {
        width,
        height,
        depth,
        pixel_format,
        texture_type,
        mipmap_levels,
        usage,
        pixels: vec![0u8; byte_len],
    });
    obj
}

unsafe fn extract_bytes_from_data(data: Id) -> Vec<u8> {
    if data.is_null() {
        return Vec::new();
    }
    let len: usize = send0(data, "length");
    let ptr: *const u8 = send0(data, "bytes");
    if ptr.is_null() || len == 0 {
        Vec::new()
    } else {
        std::slice::from_raw_parts(ptr, len).to_vec()
    }
}

extern "C" fn dev_new_library_with_data(_this: Id, _cmd: Sel, data: Id, _err: *mut Id) -> Id {
    let raw = unsafe { extract_bytes_from_data(data) };
    let mut map = HashMap::new();
    let mut order = Vec::new();
    if let Ok(parsed) = mtlb::parse_mtlb(&raw) {
        TOTAL_MTLB_LIBRARIES.fetch_add(1, Ordering::Relaxed);
        for f in parsed.functions {
            order.push(f.name.clone());
            map.insert(f.name.clone(), f);
        }
    }
    let obj = unsafe { alloc(MTLLIBRARY_CLASS.ptr()) };
    libraries().lock().unwrap().insert(obj as usize, LibraryState {
        functions: map,
        order,
    });
    obj
}

extern "C" fn dev_new_library_with_source(_this: Id, _cmd: Sel, src_id: Id, _opts: Id, _err: *mut Id) -> Id {
    let src = pear_foundation::rust_str_from_nsstring(src_id).unwrap_or_default();
    let mut map = HashMap::new();
    let mut order = Vec::new();
    for line in src.lines() {
        let t = line.trim();
        let stage = if t.starts_with("vertex ") {
            Some(mtlb::ShaderStage::Vertex)
        } else if t.starts_with("fragment ") {
            Some(mtlb::ShaderStage::Fragment)
        } else if t.starts_with("kernel ") {
            Some(mtlb::ShaderStage::Kernel)
        } else {
            None
        };
        if let Some(st) = stage {
            if let Some(paren) = t.find('(') {
                let before = t[..paren].trim();
                if let Some(name) = before.split_whitespace().last() {
                    let clean = name.trim_matches('*').to_string();
                    if !clean.is_empty() && !map.contains_key(&clean) {
                        order.push(clean.clone());
                        map.insert(clean.clone(), mtlb::MtlbFunction {
                            name: clean,
                            stage: st,
                            hash: [0u8; 32],
                            bitcode_offset: 0,
                            bitcode_size: 0,
                            bitcode: Vec::new(),
                        });
                    }
                }
            }
        }
    }
    let obj = unsafe { alloc(MTLLIBRARY_CLASS.ptr()) };
    libraries().lock().unwrap().insert(obj as usize, LibraryState {
        functions: map,
        order,
    });
    obj
}

extern "C" fn dev_new_default_library(_this: Id, _cmd: Sel) -> Id {
    let obj = unsafe { alloc(MTLLIBRARY_CLASS.ptr()) };
    libraries().lock().unwrap().insert(obj as usize, LibraryState {
        functions: HashMap::new(),
        order: Vec::new(),
    });
    obj
}

extern "C" fn dev_new_render_pipeline(_this: Id, _cmd: Sel, _desc: Id, _err: *mut Id) -> Id {
    unsafe { alloc(MTLRENDERPIPELINESTATE_CLASS.ptr()) }
}
extern "C" fn dev_new_compute_pipeline(_this: Id, _cmd: Sel, _fn: Id, _err: *mut Id) -> Id {
    unsafe { alloc(MTLCOMPUTEPIPELINESTATE_CLASS.ptr()) }
}
extern "C" fn dev_new_compute_pipeline_desc(_this: Id, _cmd: Sel, _desc: Id, _opts: usize, _refl: *mut Id, _err: *mut Id) -> Id {
    unsafe { alloc(MTLCOMPUTEPIPELINESTATE_CLASS.ptr()) }
}
extern "C" fn dev_new_depth_stencil(_this: Id, _cmd: Sel, _desc: Id) -> Id {
    unsafe { alloc(MTLDEPTHSTENCILSTATE_CLASS.ptr()) }
}
extern "C" fn dev_new_sampler(_this: Id, _cmd: Sel, _desc: Id) -> Id {
    unsafe { alloc(MTLSAMPLERSTATE_CLASS.ptr()) }
}
extern "C" fn dev_new_heap(_this: Id, _cmd: Sel, _desc: Id) -> Id {
    unsafe { alloc(MTLHEAP_CLASS.ptr()) }
}
extern "C" fn dev_new_fence(_this: Id, _cmd: Sel) -> Id {
    unsafe { alloc(MTLFENCE_CLASS.ptr()) }
}
extern "C" fn dev_new_event(_this: Id, _cmd: Sel) -> Id {
    unsafe { alloc(MTLEVENT_CLASS.ptr()) }
}
extern "C" fn dev_new_shared_event(_this: Id, _cmd: Sel) -> Id {
    unsafe { alloc(MTLSHAREDEVENT_CLASS.ptr()) }
}

// ---- MTLBuffer ----
extern "C" fn buf_contents(this: Id, _cmd: Sel) -> *mut c_void {
    let mut g = buffers().lock().unwrap();
    g.get_mut(&(this as usize))
        .map(|b| b.bytes.as_mut_ptr() as *mut c_void)
        .unwrap_or(std::ptr::null_mut())
}
extern "C" fn buf_length(this: Id, _cmd: Sel) -> usize {
    buffers().lock().unwrap().get(&(this as usize)).map(|b| b.bytes.len()).unwrap_or(0)
}

// ---- MTLTexture ----
extern "C" fn tex_width(this: Id, _cmd: Sel) -> usize {
    textures().lock().unwrap().get(&(this as usize)).map(|t| t.width).unwrap_or(1)
}
extern "C" fn tex_height(this: Id, _cmd: Sel) -> usize {
    textures().lock().unwrap().get(&(this as usize)).map(|t| t.height).unwrap_or(1)
}
extern "C" fn tex_depth(this: Id, _cmd: Sel) -> usize {
    textures().lock().unwrap().get(&(this as usize)).map(|t| t.depth).unwrap_or(1)
}
extern "C" fn tex_pixel_format(this: Id, _cmd: Sel) -> usize {
    textures().lock().unwrap().get(&(this as usize)).map(|t| t.pixel_format).unwrap_or(80)
}
extern "C" fn tex_type(this: Id, _cmd: Sel) -> usize {
    textures().lock().unwrap().get(&(this as usize)).map(|t| t.texture_type).unwrap_or(2)
}
extern "C" fn tex_mips(this: Id, _cmd: Sel) -> usize {
    textures().lock().unwrap().get(&(this as usize)).map(|t| t.mipmap_levels).unwrap_or(1)
}
extern "C" fn tex_usage(this: Id, _cmd: Sel) -> usize {
    textures().lock().unwrap().get(&(this as usize)).map(|t| t.usage).unwrap_or(1)
}
extern "C" fn tex_new_view(this: Id, _cmd: Sel, _pf: usize) -> Id {
    this
}
extern "C" fn tex_replace_region(
    this: Id,
    _cmd: Sel,
    region: MTLRegion,
    _level: usize,
    bytes: *const u8,
    bytes_per_row: usize,
) {
    if bytes.is_null() || region.size.width == 0 || region.size.height == 0 {
        return;
    }
    let mut g = textures().lock().unwrap();
    let Some(t) = g.get_mut(&(this as usize)) else { return };
    if t.pixels.is_empty() {
        return;
    }
    let bpp = 4usize;
    let src_row_stride = if bytes_per_row > 0 { bytes_per_row } else { region.size.width * bpp };
    let dst_row_stride = t.width * bpp;
    for row in 0..region.size.height {
        let dy = region.origin.y + row;
        if dy >= t.height {
            break;
        }
        let copy_cols = region.size.width.min(t.width.saturating_sub(region.origin.x));
        let copy_bytes = copy_cols * bpp;
        let dst_off = dy * dst_row_stride + region.origin.x * bpp;
        if dst_off + copy_bytes > t.pixels.len() {
            break;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(
                bytes.add(row * src_row_stride),
                t.pixels.as_mut_ptr().add(dst_off),
                copy_bytes,
            );
        }
    }
}
extern "C" fn tex_replace_region_slice(
    this: Id,
    cmd: Sel,
    region: MTLRegion,
    level: usize,
    _slice: usize,
    bytes: *const u8,
    bytes_per_row: usize,
    _bytes_per_image: usize,
) {
    tex_replace_region(this, cmd, region, level, bytes, bytes_per_row);
}

extern "C" fn tex_get_bytes(
    this: Id,
    _cmd: Sel,
    dst: *mut u8,
    bytes_per_row: usize,
    region: MTLRegion,
    _level: usize,
) {
    if dst.is_null() || region.size.width == 0 || region.size.height == 0 {
        return;
    }
    let g = textures().lock().unwrap();
    let Some(t) = g.get(&(this as usize)) else { return };
    let bpp = 4usize;
    let dst_row_stride = if bytes_per_row > 0 { bytes_per_row } else { region.size.width * bpp };
    let src_row_stride = t.width * bpp;
    for row in 0..region.size.height {
        let sy = region.origin.y + row;
        if sy >= t.height {
            break;
        }
        let copy_cols = region.size.width.min(t.width.saturating_sub(region.origin.x));
        let copy_bytes = copy_cols * bpp;
        let src_off = sy * src_row_stride + region.origin.x * bpp;
        if src_off + copy_bytes > t.pixels.len() {
            break;
        }
        unsafe {
            std::ptr::copy_nonoverlapping(
                t.pixels.as_ptr().add(src_off),
                dst.add(row * dst_row_stride),
                copy_bytes,
            );
        }
    }
}

// ---- MTLLibrary / MTLFunction ----
extern "C" fn lib_function_names(this: Id, _cmd: Sel) -> Id {
    let names: Vec<Id> = libraries()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|l| l.order.iter().map(|s| pear_foundation::nsstring_from_rust(s)).collect())
        .unwrap_or_default();
    unsafe { make_nsarray(&names) }
}

fn instantiate_function(mtlb_fn: mtlb::MtlbFunction) -> Id {
    TOTAL_MTLB_FUNCTIONS.fetch_add(1, Ordering::Relaxed);
    let spirv = spirv::compile_metal_function_to_spirv(&mtlb_fn);
    let vk_module = vk_context()
        .and_then(|ctx| ctx.create_shader_module(&spirv))
        .unwrap_or(0);
    let obj = unsafe { alloc(MTLFUNCTION_CLASS.ptr()) };
    functions().lock().unwrap().insert(obj as usize, FunctionState {
        name: mtlb_fn.name,
        stage: mtlb_fn.stage,
        spirv,
        vk_module,
    });
    obj
}

extern "C" fn lib_new_function(this: Id, _cmd: Sel, name_id: Id) -> Id {
    let name = pear_foundation::rust_str_from_nsstring(name_id).unwrap_or_default();
    let found = libraries()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .and_then(|l| l.functions.get(&name).cloned());
    let func_data = found.unwrap_or_else(|| mtlb::MtlbFunction {
        name,
        stage: if pear_foundation::rust_str_from_nsstring(name_id)
            .unwrap_or_default()
            .to_ascii_lowercase()
            .contains("frag")
        {
            mtlb::ShaderStage::Fragment
        } else {
            mtlb::ShaderStage::Vertex
        },
        hash: [0u8; 32],
        bitcode_offset: 0,
        bitcode_size: 0,
        bitcode: Vec::new(),
    });
    instantiate_function(func_data)
}

extern "C" fn lib_new_function_consts(this: Id, cmd: Sel, name_id: Id, _vals: Id, _err: *mut Id) -> Id {
    lib_new_function(this, cmd, name_id)
}

extern "C" fn fn_name(this: Id, _cmd: Sel) -> Id {
    let name = functions()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|f| f.name.clone())
        .unwrap_or_default();
    pear_foundation::nsstring_from_rust(&name)
}

extern "C" fn fn_type(this: Id, _cmd: Sel) -> usize {
    functions()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|f| match f.stage {
            mtlb::ShaderStage::Vertex => 1,   // MTLFunctionTypeVertex
            mtlb::ShaderStage::Fragment => 2, // MTLFunctionTypeFragment
            mtlb::ShaderStage::Kernel => 3,   // MTLFunctionTypeKernel
            _ => 1,
        })
        .unwrap_or(1)
}

// ---- Command Queue / Command Buffer / Encoders ----
extern "C" fn cq_command_buffer(_this: Id, _cmd: Sel) -> Id {
    let cb = unsafe { alloc(MTLCOMMANDBUFFER_CLASS.ptr()) };
    cmd_buffers().lock().unwrap().insert(cb as usize, CommandBufferState {
        status: 0, // NotEnqueued
        ..Default::default()
    });
    cb
}

extern "C" fn cb_render_encoder(this: Id, _cmd: Sel, pass: Id) -> Id {
    if !pass.is_null() {
        unsafe {
            let atts: Id = send0(pass, "colorAttachments");
            if !atts.is_null() {
                let att0: Id = send1(atts, "objectAtIndexedSubscript:", 0usize);
                if !att0.is_null() {
                    let load_action: usize = send0(att0, "loadAction");
                    let tex: Id = send0(att0, "texture");
                    if !tex.is_null() && load_action == 2 {
                        let cc: MTLClearColor = send0(att0, "clearColor");
                        let rgba_f = [
                            cc.red.clamp(0.0, 1.0) as f32,
                            cc.green.clamp(0.0, 1.0) as f32,
                            cc.blue.clamp(0.0, 1.0) as f32,
                            cc.alpha.clamp(0.0, 1.0) as f32,
                        ];
                        let r8 = (rgba_f[0] * 255.0).round() as u8;
                        let g8 = (rgba_f[1] * 255.0).round() as u8;
                        let b8 = (rgba_f[2] * 255.0).round() as u8;
                        let a8 = (rgba_f[3] * 255.0).round() as u8;
                        tex_clear_color().lock().unwrap().insert(tex as usize, rgba_f);
                        if let Some(t) = textures().lock().unwrap().get_mut(&(tex as usize)) {
                            let is_bgra = t.pixel_format == 80 || t.pixel_format == 81;
                            let px = if is_bgra { [b8, g8, r8, a8] } else { [r8, g8, b8, a8] };
                            if t.pixels.len() >= 4 {
                                for chunk in t.pixels.chunks_exact_mut(4) {
                                    chunk.copy_from_slice(&px);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let enc = unsafe { alloc(MTLRENDERCOMMANDENCODER_CLASS.ptr()) };
    encoder_to_cb().lock().unwrap().insert(enc as usize, this as usize);
    enc
}
extern "C" fn cb_compute_encoder(this: Id, _cmd: Sel) -> Id {
    let enc = unsafe { alloc(MTLCOMPUTECOMMANDENCODER_CLASS.ptr()) };
    encoder_to_cb().lock().unwrap().insert(enc as usize, this as usize);
    enc
}
extern "C" fn cb_compute_encoder_disp(this: Id, cmd: Sel, _disp: usize) -> Id {
    cb_compute_encoder(this, cmd)
}
extern "C" fn cb_blit_encoder(this: Id, _cmd: Sel) -> Id {
    let enc = unsafe { alloc(MTLBLITCOMMANDENCODER_CLASS.ptr()) };
    encoder_to_cb().lock().unwrap().insert(enc as usize, this as usize);
    enc
}

extern "C" fn cb_add_completed_handler(this: Id, _cmd: Sel, block: *const c_void) {
    if block.is_null() {
        return;
    }
    if let Some(st) = cmd_buffers().lock().unwrap().get_mut(&(this as usize)) {
        if st.status == 4 {
            unsafe { invoke_cb_handler(block as usize, this) };
        } else {
            st.completed_handlers.push(block as usize);
        }
    }
}

extern "C" fn cb_add_scheduled_handler(this: Id, _cmd: Sel, block: *const c_void) {
    if block.is_null() {
        return;
    }
    if let Some(st) = cmd_buffers().lock().unwrap().get_mut(&(this as usize)) {
        if st.status == 4 {
            unsafe { invoke_cb_handler(block as usize, this) };
        } else {
            st.scheduled_handlers.push(block as usize);
        }
    }
}

extern "C" fn cb_commit(this: Id, _cmd: Sel) {
    let (sched, comp) = {
        let mut g = cmd_buffers().lock().unwrap();
        let st = g.entry(this as usize).or_default();
        st.status = 4; // MTLCommandBufferStatusCompleted
        (
            std::mem::take(&mut st.scheduled_handlers),
            std::mem::take(&mut st.completed_handlers),
        )
    };
    unsafe {
        for h in sched {
            invoke_cb_handler(h, this);
        }
        for h in comp {
            invoke_cb_handler(h, this);
        }
    }
}

extern "C" fn cb_status(this: Id, _cmd: Sel) -> usize {
    cmd_buffers().lock().unwrap().get(&(this as usize)).map(|s| s.status).unwrap_or(4)
}

fn present_drawable_common(drawable: Id) {
    TOTAL_PRESENTS.fetch_add(1, Ordering::Relaxed);
    if drawable.is_null() {
        return;
    }
    let tex = drawable_to_tex()
        .lock()
        .unwrap()
        .get(&(drawable as usize))
        .copied()
        .unwrap_or(0);
    if tex == 0 {
        return;
    }
    let layer = drawable_to_layer()
        .lock()
        .unwrap()
        .get(&(drawable as usize))
        .copied()
        .unwrap_or(0);

    let snapshot = {
        let g = textures().lock().unwrap();
        g.get(&tex).map(|t| {
            let is_bgra = t.pixel_format == 80 || t.pixel_format == 81;
            let expected = t.width * t.height * 4;
            let mut rgba = Vec::with_capacity(expected);
            for chunk in t.pixels.chunks_exact(4).take(t.width * t.height) {
                if is_bgra {
                    rgba.extend_from_slice(&[chunk[2], chunk[1], chunk[0], chunk[3]]);
                } else {
                    rgba.extend_from_slice(chunk);
                }
            }
            if rgba.len() < expected {
                rgba.resize(expected, 0);
            }
            (t.width, t.height, rgba)
        })
    };
    let Some((w, h, rgba)) = snapshot else {
        return;
    };

    let clear_rgba = tex_clear_color()
        .lock()
        .unwrap()
        .get(&tex)
        .copied()
        .unwrap_or_else(|| {
            if rgba.len() >= 4 {
                [
                    rgba[0] as f32 / 255.0,
                    rgba[1] as f32 / 255.0,
                    rgba[2] as f32 / 255.0,
                    rgba[3] as f32 / 255.0,
                ]
            } else {
                [0.0, 0.0, 0.0, 1.0]
            }
        });

    if let Ok(path) = std::env::var("PEAR_DUMP_FRAME") {
        if !path.is_empty() && w > 0 && h > 0 {
            let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
            ppm.reserve(w * h * 3);
            for px in rgba.chunks_exact(4) {
                ppm.extend_from_slice(&px[..3]);
            }
            let _ = std::fs::write(path, ppm);
        }
    }

    *last_presented_frame_slot().lock().unwrap() = Some((w, h, rgba));

    if layer != 0 {
        if let Some(ctx) = vk_context() {
            if let Some(info) = pear_host::host().surface(pear_host::LayerRef(layer as *mut c_void)) {
                if let pear_host::NativeSurface::Xlib { display, window } = info.surface {
                    if !display.is_null() && window != 0 {
                        let _ = ctx.present_xlib_clear(
                            display,
                            window,
                            w as u32,
                            h as u32,
                            clear_rgba,
                        );
                    }
                }
            }
        }
    }
}

extern "C" fn cb_present_drawable(_this: Id, _cmd: Sel, drawable: Id) {
    present_drawable_common(drawable);
}
extern "C" fn drawable_present0(this: Id, _cmd: Sel) {
    present_drawable_common(this);
}
extern "C" fn drawable_present1(this: Id, _cmd: Sel, _time: f64) {
    present_drawable_common(this);
}

extern "C" fn enc_draw_primitives(this: Id, _cmd: Sel, _ptype: usize, _start: usize, _count: usize) {
    TOTAL_DRAW_CALLS.fetch_add(1, Ordering::Relaxed);
    if let Some(&cb) = encoder_to_cb().lock().unwrap().get(&(this as usize)) {
        if let Some(st) = cmd_buffers().lock().unwrap().get_mut(&cb) {
            st.draw_calls += 1;
        }
    }
}
extern "C" fn enc_draw_indexed(
    this: Id,
    _cmd: Sel,
    _ptype: usize,
    _icount: usize,
    _itype: usize,
    _ibuf: Id,
    _ioff: usize,
) {
    TOTAL_DRAW_CALLS.fetch_add(1, Ordering::Relaxed);
    if let Some(&cb) = encoder_to_cb().lock().unwrap().get(&(this as usize)) {
        if let Some(st) = cmd_buffers().lock().unwrap().get_mut(&cb) {
            st.draw_calls += 1;
        }
    }
}
extern "C" fn comp_dispatch(this: Id, _cmd: Sel, _tgs: MTLSize, _tptg: MTLSize) {
    TOTAL_COMPUTE_DISPATCHES.fetch_add(1, Ordering::Relaxed);
    if let Some(&cb) = encoder_to_cb().lock().unwrap().get(&(this as usize)) {
        if let Some(st) = cmd_buffers().lock().unwrap().get_mut(&cb) {
            st.compute_dispatches += 1;
        }
    }
}

extern "C" fn blit_copy_buffer(
    this: Id,
    _cmd: Sel,
    src_buf: Id,
    src_off: usize,
    dst_buf: Id,
    dst_off: usize,
    size: usize,
) {
    if let Some(&cb) = encoder_to_cb().lock().unwrap().get(&(this as usize)) {
        if let Some(st) = cmd_buffers().lock().unwrap().get_mut(&cb) {
            st.blit_ops += 1;
        }
    }
    if src_buf.is_null() || dst_buf.is_null() || size == 0 {
        return;
    }
    let data_copy = {
        let g = buffers().lock().unwrap();
        let Some(src) = g.get(&(src_buf as usize)) else { return };
        if src_off.saturating_add(size) > src.bytes.len() {
            return;
        }
        src.bytes[src_off..src_off + size].to_vec()
    };
    let mut g = buffers().lock().unwrap();
    if let Some(dst) = g.get_mut(&(dst_buf as usize)) {
        if dst_off.saturating_add(size) <= dst.bytes.len() {
            dst.bytes[dst_off..dst_off + size].copy_from_slice(&data_copy);
        }
    }
}

// ---- Subscript array & descriptor helpers ----
fn subscript_for(arr: Id, idx: usize, elem_cls: &StaticClass) -> Id {
    let mut g = subscript_slots().lock().unwrap();
    if let Some(&existing) = g.get(&(arr as usize, idx)) {
        return existing as Id;
    }
    let obj = unsafe { alloc(elem_cls.ptr()) };
    g.insert((arr as usize, idx), obj as usize);
    obj
}

extern "C" fn subscript_set(this: Id, _cmd: Sel, obj: Id, idx: usize) {
    store_retained_slot(subscript_slots(), (this as usize, idx), obj);
}

extern "C" fn color_attach_at(this: Id, _cmd: Sel, idx: usize) -> Id {
    subscript_for(this, idx, &MTLRENDERPASSCOLORATTACH_CLASS)
}
extern "C" fn pipe_color_attach_at(this: Id, _cmd: Sel, idx: usize) -> Id {
    subscript_for(this, idx, &MTLPIPELINE_COLOR_ATTACH_CLASS)
}
extern "C" fn vertex_attr_at(this: Id, _cmd: Sel, idx: usize) -> Id {
    subscript_for(this, idx, &MTLVERTEXATTRDESC_CLASS)
}
extern "C" fn vertex_layout_at(this: Id, _cmd: Sel, idx: usize) -> Id {
    subscript_for(this, idx, &MTLVERTEXLAYOUTDESC_CLASS)
}

fn lazy_prop_alloc(this: Id, sel_name: &str, cls: &StaticClass) -> Id {
    let key = objc_rt::sel(sel_name) as usize;
    let mut g = props_id().lock().unwrap();
    if let Some(&v) = g.get(&(this as usize, key)) {
        return v as Id;
    }
    let obj = unsafe { alloc(cls.ptr()) };
    g.insert((this as usize, key), obj as usize);
    obj
}

extern "C" fn render_pass_color_attachments(this: Id, _cmd: Sel) -> Id {
    lazy_prop_alloc(this, "colorAttachments", &MTLRENDERPASSCOLORARRAY_CLASS)
}
extern "C" fn render_pass_depth_attachment(this: Id, _cmd: Sel) -> Id {
    lazy_prop_alloc(this, "depthAttachment", &MTLRENDERPASSDEPTHATTACH_CLASS)
}
extern "C" fn render_pass_stencil_attachment(this: Id, _cmd: Sel) -> Id {
    lazy_prop_alloc(this, "stencilAttachment", &MTLRENDERPASSDEPTHATTACH_CLASS)
}
extern "C" fn ds_front_face_stencil(this: Id, _cmd: Sel) -> Id {
    lazy_prop_alloc(this, "frontFaceStencil", &MTLSTENCILDESCRIPTOR_CLASS)
}
extern "C" fn ds_back_face_stencil(this: Id, _cmd: Sel) -> Id {
    lazy_prop_alloc(this, "backFaceStencil", &MTLSTENCILDESCRIPTOR_CLASS)
}
extern "C" fn pipe_color_attachments(this: Id, _cmd: Sel) -> Id {
    let key = objc_rt::sel("colorAttachments") as usize;
    let mut g = props_id().lock().unwrap();
    if let Some(&v) = g.get(&(this as usize, key)) {
        return v as Id;
    }
    let arr = unsafe { alloc(MTLPIPELINE_COLOR_ARRAY_CLASS.ptr()) };
    g.insert((this as usize, key), arr as usize);
    arr
}
extern "C" fn vdesc_attributes(this: Id, _cmd: Sel) -> Id {
    let key = objc_rt::sel("attributes") as usize;
    let mut g = props_id().lock().unwrap();
    if let Some(&v) = g.get(&(this as usize, key)) {
        return v as Id;
    }
    let arr = unsafe { alloc(MTLVERTEXATTRARRAY_CLASS.ptr()) };
    g.insert((this as usize, key), arr as usize);
    arr
}
extern "C" fn vdesc_layouts(this: Id, _cmd: Sel) -> Id {
    let key = objc_rt::sel("layouts") as usize;
    let mut g = props_id().lock().unwrap();
    if let Some(&v) = g.get(&(this as usize, key)) {
        return v as Id;
    }
    let arr = unsafe { alloc(MTLVERTEXLAYOUTARRAY_CLASS.ptr()) };
    g.insert((this as usize, key), arr as usize);
    arr
}

extern "C" fn desc_new(cls_obj: Id, _cmd: Sel) -> Id {
    unsafe { alloc(cls_obj as *const ClassT) }
}
extern "C" fn tex_desc_2d(_cls: Id, _cmd: Sel, pf: usize, w: usize, h: usize, mip: bool) -> Id {
    let obj = unsafe { alloc(MTLTEXTUREDESCRIPTOR_CLASS.ptr()) };
    let mut u = props_usize().lock().unwrap();
    u.insert((obj as usize, objc_rt::sel("pixelFormat") as usize), pf);
    u.insert((obj as usize, objc_rt::sel("width") as usize), w.max(1));
    u.insert((obj as usize, objc_rt::sel("height") as usize), h.max(1));
    u.insert((obj as usize, objc_rt::sel("depth") as usize), 1);
    u.insert((obj as usize, objc_rt::sel("textureType") as usize), 2);
    u.insert((obj as usize, objc_rt::sel("mipmapLevelCount") as usize), if mip { 4 } else { 1 });
    u.insert((obj as usize, objc_rt::sel("usage") as usize), 1);
    obj
}
extern "C" fn capture_shared(_cls: Id, _cmd: Sel) -> Id {
    static SHARED: OnceLock<usize> = OnceLock::new();
    *SHARED.get_or_init(|| unsafe { alloc(MTLCAPTUREMANAGER_CLASS.ptr()) as usize }) as Id
}

// ---- CAMetalDrawable ----
extern "C" fn drawable_texture(this: Id, _cmd: Sel) -> Id {
    drawable_to_tex()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .copied()
        .unwrap_or(0) as Id
}

pub fn next_drawable_for_layer(layer: Id, width: usize, height: usize, pixel_format: usize) -> Id {
    init();
    let w = width.max(1);
    let h = height.max(1);
    let pf = if pixel_format == 0 { 80 } else { pixel_format }; // BGRA8Unorm
    let mut ld = layer_drawables().lock().unwrap();
    let drawable = if let Some(&existing) = ld.get(&(layer as usize)) {
        existing as Id
    } else {
        let d = unsafe { alloc(MTLDRAWABLE_CLASS.ptr()) };
        ld.insert(layer as usize, d as usize);
        d
    };
    drop(ld);
    drawable_to_layer()
        .lock()
        .unwrap()
        .insert(drawable as usize, layer as usize);

    let tex = drawable_to_tex().lock().unwrap().get(&(drawable as usize)).copied().unwrap_or(0) as Id;
    let needs_new = if tex.is_null() {
        true
    } else {
        let g = textures().lock().unwrap();
        g.get(&(tex as usize)).map(|t| t.width != w || t.height != h).unwrap_or(true)
    };
    if needs_new {
        let new_tex = create_texture_object(w, h, 1, pf, 2, 1, 5);
        drawable_to_tex().lock().unwrap().insert(drawable as usize, new_tex as usize);
    }
    drawable
}

/// Inspects compiled SPIR-V words for a `PearMTLFunction` (used in tests and diagnostics).
pub fn function_spirv(func_obj: Id) -> Option<Vec<u32>> {
    functions().lock().unwrap().get(&(func_obj as usize)).map(|f| f.spirv.clone())
}

/// Inspects recorded counts (`draw_calls, compute_dispatches, blit_ops`) for a `PearMTLCommandBuffer`.
pub fn command_buffer_counts(cb_obj: Id) -> Option<(usize, usize, usize)> {
    cmd_buffers()
        .lock()
        .unwrap()
        .get(&(cb_obj as usize))
        .map(|s| (s.draw_calls, s.compute_dispatches, s.blit_ops))
}

extern "C" fn mtl_set_purgeable(_this: Id, _cmd: Sel, state: usize) -> usize {
    if state == 1 { 2 } else { state }
}

pub fn register_classes() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let i = |p: *const ()| p as Imp;

        objc_rt::register_builtin(
            MTLDEVICE_CLASS.ptr(),
            MTLDEVICE_META.ptr(),
            "PearMTLDevice",
            32,
            &[
                ("name", i(dev_name as *const ())),
                ("maxThreadsPerThreadgroup", i(dev_max_threads_per_tg as *const ())),
                ("maxBufferLength", i(dev_max_buffer_len as *const ())),
                ("supportsFamily:", i(dev_supports_family as *const ())),
                ("supportsFeatureSet:", i(dev_supports_family as *const ())),
                ("supportsTextureSampleCount:", i(mtl_true1 as *const ())),
                ("hasUnifiedMemory", i(mtl_true as *const ())),
                ("isLowPower", i(mtl_false as *const ())),
                ("isHeadless", i(mtl_false as *const ())),
                ("isRemovable", i(mtl_false as *const ())),
                ("recommendedMaxWorkingSetSize", i(dev_max_buffer_len as *const ())),
                ("currentAllocatedSize", i(mtl_zero as *const ())),
                ("newCommandQueue", i(dev_new_command_queue as *const ())),
                ("newCommandQueueWithMaxCommandBufferCount:", i(dev_new_command_queue_max as *const ())),
                ("newBufferWithLength:options:", i(dev_new_buffer_len as *const ())),
                ("newBufferWithBytes:length:options:", i(dev_new_buffer_bytes as *const ())),
                ("newBufferWithBytesNoCopy:length:options:deallocator:", i(dev_new_buffer_nocopy as *const ())),
                ("newTextureWithDescriptor:", i(dev_new_texture_desc as *const ())),
                ("newLibraryWithData:error:", i(dev_new_library_with_data as *const ())),
                ("newLibraryWithSource:options:error:", i(dev_new_library_with_source as *const ())),
                ("newDefaultLibrary", i(dev_new_default_library as *const ())),
                ("newDefaultLibraryWithBundle:error:", i(dev_new_default_library as *const ())),
                ("newRenderPipelineStateWithDescriptor:error:", i(dev_new_render_pipeline as *const ())),
                ("newComputePipelineStateWithFunction:error:", i(dev_new_compute_pipeline as *const ())),
                ("newComputePipelineStateWithDescriptor:options:reflection:error:", i(dev_new_compute_pipeline_desc as *const ())),
                ("newDepthStencilStateWithDescriptor:", i(dev_new_depth_stencil as *const ())),
                ("newSamplerStateWithDescriptor:", i(dev_new_sampler as *const ())),
                ("newHeapWithDescriptor:", i(dev_new_heap as *const ())),
                ("newFence", i(dev_new_fence as *const ())),
                ("newEvent", i(dev_new_event as *const ())),
                ("newSharedEvent", i(dev_new_shared_event as *const ())),
            ],
            &[],
        );

        objc_rt::register_builtin(
            MTLCOMMANDQUEUE_CLASS.ptr(),
            MTLCOMMANDQUEUE_META.ptr(),
            "PearMTLCommandQueue",
            32,
            &[
                ("commandBuffer", i(cq_command_buffer as *const ())),
                ("commandBufferWithUnretainedReferences", i(cq_command_buffer as *const ())),
                ("device", i(create_system_default_device as *const ())),
                ("label", i(prop_get_id as *const ())),
                ("setLabel:", i(prop_set_id as *const ())),
            ],
            &[],
        );

        objc_rt::register_builtin(
            MTLCOMMANDBUFFER_CLASS.ptr(),
            MTLCOMMANDBUFFER_META.ptr(),
            "PearMTLCommandBuffer",
            32,
            &[
                ("renderCommandEncoderWithDescriptor:", i(cb_render_encoder as *const ())),
                ("parallelRenderCommandEncoderWithDescriptor:", i(cb_render_encoder as *const ())),
                ("computeCommandEncoder", i(cb_compute_encoder as *const ())),
                ("computeCommandEncoderWithDispatchType:", i(cb_compute_encoder_disp as *const ())),
                ("blitCommandEncoder", i(cb_blit_encoder as *const ())),
                ("addScheduledHandler:", i(cb_add_scheduled_handler as *const ())),
                ("addCompletedHandler:", i(cb_add_completed_handler as *const ())),
                ("presentDrawable:", i(cb_present_drawable as *const ())),
                ("enqueue", i(mtl_noop0 as *const ())),
                ("commit", i(cb_commit as *const ())),
                ("waitUntilScheduled", i(mtl_noop0 as *const ())),
                ("waitUntilCompleted", i(mtl_noop0 as *const ())),
                ("status", i(cb_status as *const ())),
                ("error", i(mtl_null0 as *const ())),
                ("label", i(prop_get_id as *const ())),
                ("setLabel:", i(prop_set_id as *const ())),
            ],
            &[],
        );

        objc_rt::register_builtin(
            MTLRENDERCOMMANDENCODER_CLASS.ptr(),
            MTLRENDERCOMMANDENCODER_META.ptr(),
            "PearMTLRenderCommandEncoder",
            32,
            &[
                ("setRenderPipelineState:", i(mtl_noop1 as *const ())),
                ("renderCommandEncoder", i(mtl_ret_self as *const ())),
                ("setDepthStencilState:", i(mtl_noop1 as *const ())),
                ("setStencilReferenceValue:", i(mtl_noop1 as *const ())),
                ("setCullMode:", i(mtl_noop1 as *const ())),
                ("setFrontFacingWinding:", i(mtl_noop1 as *const ())),
                ("setTriangleFillMode:", i(mtl_noop1 as *const ())),
                ("setDepthClipMode:", i(mtl_noop1 as *const ())),
                ("setDepthBias:slopeScale:clamp:", i(mtl_noop3 as *const ())),
                ("setViewport:", i(mtl_noop1 as *const ())),
                ("setViewports:count:", i(mtl_noop2 as *const ())),
                ("setScissorRect:", i(mtl_noop1 as *const ())),
                ("setScissorRects:count:", i(mtl_noop2 as *const ())),
                ("setBlendColorRed:green:blue:alpha:", i(mtl_noop4 as *const ())),
                ("setVertexBuffer:offset:atIndex:", i(mtl_noop3 as *const ())),
                ("setVertexBytes:length:atIndex:", i(mtl_noop3 as *const ())),
                ("setVertexTexture:atIndex:", i(mtl_noop2 as *const ())),
                ("setVertexSamplerState:atIndex:", i(mtl_noop2 as *const ())),
                ("setFragmentBuffer:offset:atIndex:", i(mtl_noop3 as *const ())),
                ("setFragmentBytes:length:atIndex:", i(mtl_noop3 as *const ())),
                ("setFragmentTexture:atIndex:", i(mtl_noop2 as *const ())),
                ("setFragmentSamplerState:atIndex:", i(mtl_noop2 as *const ())),
                ("useResource:usage:", i(mtl_noop2 as *const ())),
                ("useHeap:", i(mtl_noop1 as *const ())),
                ("drawPrimitives:vertexStart:vertexCount:", i(enc_draw_primitives as *const ())),
                ("drawPrimitives:vertexStart:vertexCount:instanceCount:", i(enc_draw_primitives as *const ())),
                ("drawIndexedPrimitives:indexCount:indexType:indexBuffer:indexBufferOffset:", i(enc_draw_indexed as *const ())),
                ("drawIndexedPrimitives:indexCount:indexType:indexBuffer:indexBufferOffset:instanceCount:", i(enc_draw_indexed as *const ())),
                ("endEncoding", i(mtl_noop0 as *const ())),
                ("pushDebugGroup:", i(mtl_noop1 as *const ())),
                ("popDebugGroup", i(mtl_noop0 as *const ())),
            ],
            &[],
        );

        objc_rt::register_builtin(
            MTLCOMPUTECOMMANDENCODER_CLASS.ptr(),
            MTLCOMPUTECOMMANDENCODER_META.ptr(),
            "PearMTLComputeCommandEncoder",
            32,
            &[
                ("setComputePipelineState:", i(mtl_noop1 as *const ())),
                ("setBuffer:offset:atIndex:", i(mtl_noop3 as *const ())),
                ("setBytes:length:atIndex:", i(mtl_noop3 as *const ())),
                ("setTexture:atIndex:", i(mtl_noop2 as *const ())),
                ("setSamplerState:atIndex:", i(mtl_noop2 as *const ())),
                ("setThreadgroupMemoryLength:atIndex:", i(mtl_noop2 as *const ())),
                ("dispatchThreadgroups:threadsPerThreadgroup:", i(comp_dispatch as *const ())),
                ("dispatchThreads:threadsPerThreadgroup:", i(comp_dispatch as *const ())),
                ("endEncoding", i(mtl_noop0 as *const ())),
            ],
            &[],
        );

        objc_rt::register_builtin(
            MTLBLITCOMMANDENCODER_CLASS.ptr(),
            MTLBLITCOMMANDENCODER_META.ptr(),
            "PearMTLBlitCommandEncoder",
            32,
            &[
                ("copyFromBuffer:sourceOffset:toBuffer:destinationOffset:size:", i(blit_copy_buffer as *const ())),
                ("copyFromTexture:sourceSlice:sourceLevel:sourceOrigin:sourceSize:toTexture:destinationSlice:destinationLevel:destinationOrigin:", i(mtl_noop4 as *const ())),
                ("fillBuffer:range:value:", i(mtl_noop3 as *const ())),
                ("generateMipmapsForTexture:", i(mtl_noop1 as *const ())),
                ("synchronizeResource:", i(mtl_noop1 as *const ())),
                ("endEncoding", i(mtl_noop0 as *const ())),
            ],
            &[],
        );

        objc_rt::register_builtin(
            MTLBUFFER_CLASS.ptr(),
            MTLBUFFER_META.ptr(),
            "PearMTLBuffer",
            32,
            &[
                ("contents", i(buf_contents as *const ())),
                ("length", i(buf_length as *const ())),
                ("didModifyRange:", i(mtl_noop2 as *const ())),
                ("label", i(prop_get_id as *const ())),
                ("setLabel:", i(prop_set_id as *const ())),
                ("setPurgeableState:", i(mtl_set_purgeable as *const ())),
            ],
            &[],
        );

        objc_rt::register_builtin(
            MTLTEXTURE_CLASS.ptr(),
            MTLTEXTURE_META.ptr(),
            "PearMTLTexture",
            32,
            &[
                ("width", i(tex_width as *const ())),
                ("height", i(tex_height as *const ())),
                ("depth", i(tex_depth as *const ())),
                ("pixelFormat", i(tex_pixel_format as *const ())),
                ("textureType", i(tex_type as *const ())),
                ("mipmapLevelCount", i(tex_mips as *const ())),
                ("arrayLength", i(mtl_one as *const ())),
                ("sampleCount", i(mtl_one as *const ())),
                ("storageMode", i(mtl_zero as *const ())),
                ("usage", i(tex_usage as *const ())),
                ("newTextureViewWithPixelFormat:", i(tex_new_view as *const ())),
                ("replaceRegion:mipmapLevel:withBytes:bytesPerRow:", i(tex_replace_region as *const ())),
                ("replaceRegion:mipmapLevel:slice:withBytes:bytesPerRow:bytesPerImage:", i(tex_replace_region_slice as *const ())),
                ("getBytes:bytesPerRow:fromRegion:mipmapLevel:", i(tex_get_bytes as *const ())),
                ("label", i(prop_get_id as *const ())),
                ("setLabel:", i(prop_set_id as *const ())),
                ("setPurgeableState:", i(mtl_set_purgeable as *const ())),
            ],
            &[],
        );

        objc_rt::register_builtin(
            MTLLIBRARY_CLASS.ptr(),
            MTLLIBRARY_META.ptr(),
            "PearMTLLibrary",
            32,
            &[
                ("functionNames", i(lib_function_names as *const ())),
                ("newFunctionWithName:", i(lib_new_function as *const ())),
                ("newFunctionWithName:constantValues:error:", i(lib_new_function_consts as *const ())),
                ("label", i(prop_get_id as *const ())),
                ("setLabel:", i(prop_set_id as *const ())),
            ],
            &[],
        );

        objc_rt::register_builtin(
            MTLFUNCTION_CLASS.ptr(),
            MTLFUNCTION_META.ptr(),
            "PearMTLFunction",
            32,
            &[
                ("name", i(fn_name as *const ())),
                ("functionType", i(fn_type as *const ())),
                ("label", i(prop_get_id as *const ())),
                ("setLabel:", i(prop_set_id as *const ())),
            ],
            &[],
        );

        for (cls, meta, name) in [
            (&MTLRENDERPIPELINESTATE_CLASS, &MTLRENDERPIPELINESTATE_META, "PearMTLRenderPipelineState"),
            (&MTLCOMPUTEPIPELINESTATE_CLASS, &MTLCOMPUTEPIPELINESTATE_META, "PearMTLComputePipelineState"),
            (&MTLDEPTHSTENCILSTATE_CLASS, &MTLDEPTHSTENCILSTATE_META, "PearMTLDepthStencilState"),
            (&MTLSAMPLERSTATE_CLASS, &MTLSAMPLERSTATE_META, "PearMTLSamplerState"),
            (&MTLHEAP_CLASS, &MTLHEAP_META, "PearMTLHeap"),
            (&MTLFENCE_CLASS, &MTLFENCE_META, "PearMTLFence"),
            (&MTLEVENT_CLASS, &MTLEVENT_META, "PearMTLEvent"),
            (&MTLSHAREDEVENT_CLASS, &MTLSHAREDEVENT_META, "PearMTLSharedEvent"),
        ] {
            objc_rt::register_builtin(cls.ptr(), meta.ptr(), name, 32, &[
                ("label", i(prop_get_id as *const ())),
                ("setLabel:", i(prop_set_id as *const ())),
                ("maxTotalThreadsPerThreadgroup", i(mtl_one as *const ())),
                ("threadExecutionWidth", i(mtl_one as *const ())),
            ], &[]);
        }

        objc_rt::register_builtin(
            MTLDRAWABLE_CLASS.ptr(),
            MTLDRAWABLE_META.ptr(),
            "PearMTLDrawable",
            32,
            &[
                ("texture", i(drawable_texture as *const ())),
                ("present", i(drawable_present0 as *const ())),
                ("presentAtTime:", i(drawable_present1 as *const ())),
            ],
            &[],
        );

        objc_rt::register_builtin(
            MTLTEXTUREDESCRIPTOR_CLASS.ptr(),
            MTLTEXTUREDESCRIPTOR_META.ptr(),
            "MTLTextureDescriptor",
            32,
            &[
                ("textureType", i(prop_get_usize as *const ())),
                ("setTextureType:", i(prop_set_usize as *const ())),
                ("pixelFormat", i(prop_get_usize as *const ())),
                ("setPixelFormat:", i(prop_set_usize as *const ())),
                ("width", i(prop_get_usize as *const ())),
                ("setWidth:", i(prop_set_usize as *const ())),
                ("height", i(prop_get_usize as *const ())),
                ("setHeight:", i(prop_set_usize as *const ())),
                ("depth", i(prop_get_usize as *const ())),
                ("setDepth:", i(prop_set_usize as *const ())),
                ("mipmapLevelCount", i(prop_get_usize as *const ())),
                ("setMipmapLevelCount:", i(prop_set_usize as *const ())),
                ("usage", i(prop_get_usize as *const ())),
                ("setUsage:", i(prop_set_usize as *const ())),
                ("storageMode", i(prop_get_usize as *const ())),
                ("setStorageMode:", i(prop_set_usize as *const ())),
                ("sampleCount", i(prop_get_usize as *const ())),
                ("setSampleCount:", i(prop_set_usize as *const ())),
                ("arrayLength", i(prop_get_usize as *const ())),
                ("setArrayLength:", i(prop_set_usize as *const ())),
            ],
            &[
                ("texture2DDescriptorWithPixelFormat:width:height:mipmapped:", i(tex_desc_2d as *const ())),
            ],
        );

        objc_rt::register_builtin(
            MTLRENDERPIPELINEDESCRIPTOR_CLASS.ptr(),
            MTLRENDERPIPELINEDESCRIPTOR_META.ptr(),
            "MTLRenderPipelineDescriptor",
            32,
            &[
                ("label", i(prop_get_id as *const ())),
                ("setLabel:", i(prop_set_id as *const ())),
                ("vertexFunction", i(prop_get_id as *const ())),
                ("setVertexFunction:", i(prop_set_id as *const ())),
                ("fragmentFunction", i(prop_get_id as *const ())),
                ("setFragmentFunction:", i(prop_set_id as *const ())),
                ("vertexDescriptor", i(prop_get_id as *const ())),
                ("setVertexDescriptor:", i(prop_set_id as *const ())),
                ("colorAttachments", i(pipe_color_attachments as *const ())),
                ("depthAttachmentPixelFormat", i(prop_get_usize as *const ())),
                ("setDepthAttachmentPixelFormat:", i(prop_set_usize as *const ())),
                ("stencilAttachmentPixelFormat", i(prop_get_usize as *const ())),
                ("setStencilAttachmentPixelFormat:", i(prop_set_usize as *const ())),
                ("sampleCount", i(prop_get_usize as *const ())),
                ("setSampleCount:", i(prop_set_usize as *const ())),
                ("rasterSampleCount", i(prop_get_usize as *const ())),
                ("setRasterSampleCount:", i(prop_set_usize as *const ())),
                ("alphaToCoverageEnabled", i(prop_get_usize as *const ())),
                ("isAlphaToCoverageEnabled", i(ds_is_alpha_to_coverage as *const ())),
                ("setAlphaToCoverageEnabled:", i(prop_set_usize as *const ())),
                ("reset", i(mtl_noop0 as *const ())),
            ],
            &[],
        );

        objc_rt::register_builtin(
            MTLRENDERPASSDESCRIPTOR_CLASS.ptr(),
            MTLRENDERPASSDESCRIPTOR_META.ptr(),
            "MTLRenderPassDescriptor",
            32,
            &[
                ("colorAttachments", i(render_pass_color_attachments as *const ())),
                ("depthAttachment", i(render_pass_depth_attachment as *const ())),
                ("setDepthAttachment:", i(prop_set_id as *const ())),
                ("stencilAttachment", i(render_pass_stencil_attachment as *const ())),
                ("setStencilAttachment:", i(prop_set_id as *const ())),
                ("visibilityResultBuffer", i(prop_get_id as *const ())),
                ("setVisibilityResultBuffer:", i(prop_set_id as *const ())),
                ("setSamplePositions:count:", i(mtl_noop2 as *const ())),
                ("renderTargetWidth", i(prop_get_usize as *const ())),
                ("setRenderTargetWidth:", i(prop_set_usize as *const ())),
                ("renderTargetHeight", i(prop_get_usize as *const ())),
                ("setRenderTargetHeight:", i(prop_set_usize as *const ())),
            ],
            &[("renderPassDescriptor", i(desc_new as *const ()))],
        );

        objc_rt::register_builtin(
            MTLRENDERPASSCOLORARRAY_CLASS.ptr(),
            MTLRENDERPASSCOLORARRAY_META.ptr(),
            "MTLRenderPassColorAttachmentDescriptorArray",
            32,
            &[
                ("objectAtIndexedSubscript:", i(color_attach_at as *const ())),
                ("setObject:atIndexedSubscript:", i(subscript_set as *const ())),
            ],
            &[],
        );
        objc_rt::register_builtin(
            MTLPIPELINE_COLOR_ARRAY_CLASS.ptr(),
            MTLPIPELINE_COLOR_ARRAY_META.ptr(),
            "MTLRenderPipelineColorAttachmentDescriptorArray",
            32,
            &[
                ("objectAtIndexedSubscript:", i(pipe_color_attach_at as *const ())),
                ("setObject:atIndexedSubscript:", i(subscript_set as *const ())),
            ],
            &[],
        );
        objc_rt::register_builtin(
            MTLRENDERPASSCOLORATTACH_CLASS.ptr(),
            MTLRENDERPASSCOLORATTACH_META.ptr(),
            "MTLRenderPassColorAttachmentDescriptor",
            32,
            &[
                ("texture", i(prop_get_id as *const ())),
                ("setTexture:", i(prop_set_id as *const ())),
                ("resolveTexture", i(prop_get_id as *const ())),
                ("setResolveTexture:", i(prop_set_id as *const ())),
                ("level", i(prop_get_usize as *const ())),
                ("setLevel:", i(prop_set_usize as *const ())),
                ("slice", i(prop_get_usize as *const ())),
                ("setSlice:", i(prop_set_usize as *const ())),
                ("depthPlane", i(prop_get_usize as *const ())),
                ("setDepthPlane:", i(prop_set_usize as *const ())),
                ("loadAction", i(prop_get_usize as *const ())),
                ("setLoadAction:", i(prop_set_usize as *const ())),
                ("storeAction", i(prop_get_usize as *const ())),
                ("setStoreAction:", i(prop_set_usize as *const ())),
                ("clearColor", i(prop_get_color as *const ())),
                ("setClearColor:", i(prop_set_color as *const ())),
            ],
            &[],
        );
        objc_rt::register_builtin(
            MTLRENDERPASSDEPTHATTACH_CLASS.ptr(),
            MTLRENDERPASSDEPTHATTACH_META.ptr(),
            "MTLRenderPassDepthAttachmentDescriptor",
            32,
            &[
                ("texture", i(prop_get_id as *const ())),
                ("setTexture:", i(prop_set_id as *const ())),
                ("level", i(prop_get_usize as *const ())),
                ("setLevel:", i(prop_set_usize as *const ())),
                ("slice", i(prop_get_usize as *const ())),
                ("setSlice:", i(prop_set_usize as *const ())),
                ("depthPlane", i(prop_get_usize as *const ())),
                ("setDepthPlane:", i(prop_set_usize as *const ())),
                ("loadAction", i(prop_get_usize as *const ())),
                ("setLoadAction:", i(prop_set_usize as *const ())),
                ("storeAction", i(prop_get_usize as *const ())),
                ("setStoreAction:", i(prop_set_usize as *const ())),
                ("clearDepth", i(prop_get_f64_default_one as *const ())),
                ("setClearDepth:", i(prop_set_f64 as *const ())),
            ],
            &[],
        );
        objc_rt::register_builtin(
            MTLPIPELINE_COLOR_ATTACH_CLASS.ptr(),
            MTLPIPELINE_COLOR_ATTACH_META.ptr(),
            "MTLRenderPipelineColorAttachmentDescriptor",
            32,
            &[
                ("pixelFormat", i(prop_get_usize as *const ())),
                ("setPixelFormat:", i(prop_set_usize as *const ())),
                ("isBlendingEnabled", i(prop_get_usize as *const ())),
                ("setBlendingEnabled:", i(prop_set_usize as *const ())),
                ("rgbBlendOperation", i(prop_get_usize as *const ())),
                ("setRgbBlendOperation:", i(prop_set_usize as *const ())),
                ("alphaBlendOperation", i(prop_get_usize as *const ())),
                ("setAlphaBlendOperation:", i(prop_set_usize as *const ())),
                ("sourceRGBBlendFactor", i(prop_get_usize as *const ())),
                ("setSourceRGBBlendFactor:", i(prop_set_usize as *const ())),
                ("sourceAlphaBlendFactor", i(prop_get_usize as *const ())),
                ("setSourceAlphaBlendFactor:", i(prop_set_usize as *const ())),
                ("destinationRGBBlendFactor", i(prop_get_usize as *const ())),
                ("setDestinationRGBBlendFactor:", i(prop_set_usize as *const ())),
                ("destinationAlphaBlendFactor", i(prop_get_usize as *const ())),
                ("setDestinationAlphaBlendFactor:", i(prop_set_usize as *const ())),
                ("writeMask", i(prop_get_usize as *const ())),
                ("setWriteMask:", i(prop_set_usize as *const ())),
            ],
            &[],
        );

        objc_rt::register_builtin(
            MTLVERTEXDESCRIPTOR_CLASS.ptr(),
            MTLVERTEXDESCRIPTOR_META.ptr(),
            "MTLVertexDescriptor",
            32,
            &[
                ("attributes", i(vdesc_attributes as *const ())),
                ("layouts", i(vdesc_layouts as *const ())),
                ("reset", i(mtl_noop0 as *const ())),
            ],
            &[("vertexDescriptor", i(desc_new as *const ()))],
        );
        objc_rt::register_builtin(
            MTLVERTEXATTRARRAY_CLASS.ptr(),
            MTLVERTEXATTRARRAY_META.ptr(),
            "MTLVertexAttributeDescriptorArray",
            32,
            &[
                ("objectAtIndexedSubscript:", i(vertex_attr_at as *const ())),
                ("setObject:atIndexedSubscript:", i(subscript_set as *const ())),
            ],
            &[],
        );
        objc_rt::register_builtin(
            MTLVERTEXLAYOUTARRAY_CLASS.ptr(),
            MTLVERTEXLAYOUTARRAY_META.ptr(),
            "MTLVertexBufferLayoutDescriptorArray",
            32,
            &[
                ("objectAtIndexedSubscript:", i(vertex_layout_at as *const ())),
                ("setObject:atIndexedSubscript:", i(subscript_set as *const ())),
            ],
            &[],
        );
        objc_rt::register_builtin(
            MTLVERTEXATTRDESC_CLASS.ptr(),
            MTLVERTEXATTRDESC_META.ptr(),
            "MTLVertexAttributeDescriptor",
            32,
            &[
                ("format", i(prop_get_usize as *const ())),
                ("setFormat:", i(prop_set_usize as *const ())),
                ("offset", i(prop_get_usize as *const ())),
                ("setOffset:", i(prop_set_usize as *const ())),
                ("bufferIndex", i(prop_get_usize as *const ())),
                ("setBufferIndex:", i(prop_set_usize as *const ())),
            ],
            &[],
        );
        objc_rt::register_builtin(
            MTLVERTEXLAYOUTDESC_CLASS.ptr(),
            MTLVERTEXLAYOUTDESC_META.ptr(),
            "MTLVertexBufferLayoutDescriptor",
            32,
            &[
                ("stride", i(prop_get_usize as *const ())),
                ("setStride:", i(prop_set_usize as *const ())),
                ("stepFunction", i(prop_get_usize as *const ())),
                ("setStepFunction:", i(prop_set_usize as *const ())),
                ("stepRate", i(prop_get_usize as *const ())),
                ("setStepRate:", i(prop_set_usize as *const ())),
            ],
            &[],
        );

        objc_rt::register_builtin(
            MTLDEPTHSTENCILDESCRIPTOR_CLASS.ptr(),
            MTLDEPTHSTENCILDESCRIPTOR_META.ptr(),
            "MTLDepthStencilDescriptor",
            32,
            &[
                ("depthCompareFunction", i(prop_get_usize as *const ())),
                ("setDepthCompareFunction:", i(prop_set_usize as *const ())),
                ("isDepthWriteEnabled", i(prop_get_usize as *const ())),
                ("setDepthWriteEnabled:", i(prop_set_usize as *const ())),
                ("frontFaceStencil", i(ds_front_face_stencil as *const ())),
                ("setFrontFaceStencil:", i(prop_set_id as *const ())),
                ("backFaceStencil", i(ds_back_face_stencil as *const ())),
                ("setBackFaceStencil:", i(prop_set_id as *const ())),
                ("label", i(prop_get_id as *const ())),
                ("setLabel:", i(prop_set_id as *const ())),
            ],
            &[],
        );
        objc_rt::register_builtin(
            MTLSAMPLERDESCRIPTOR_CLASS.ptr(),
            MTLSAMPLERDESCRIPTOR_META.ptr(),
            "MTLSamplerDescriptor",
            32,
            &[
                ("minFilter", i(prop_get_usize as *const ())),
                ("setMinFilter:", i(prop_set_usize as *const ())),
                ("magFilter", i(prop_get_usize as *const ())),
                ("setMagFilter:", i(prop_set_usize as *const ())),
                ("mipFilter", i(prop_get_usize as *const ())),
                ("setMipFilter:", i(prop_set_usize as *const ())),
                ("sAddressMode", i(prop_get_usize as *const ())),
                ("setSAddressMode:", i(prop_set_usize as *const ())),
                ("tAddressMode", i(prop_get_usize as *const ())),
                ("setTAddressMode:", i(prop_set_usize as *const ())),
                ("rAddressMode", i(prop_get_usize as *const ())),
                ("setRAddressMode:", i(prop_set_usize as *const ())),
                ("maxAnisotropy", i(prop_get_usize as *const ())),
                ("setMaxAnisotropy:", i(prop_set_usize as *const ())),
                ("label", i(prop_get_id as *const ())),
                ("setLabel:", i(prop_set_id as *const ())),
            ],
            &[],
        );
        objc_rt::register_builtin(
            MTLCOMPUTEPIPELINEDESC_CLASS.ptr(),
            MTLCOMPUTEPIPELINEDESC_META.ptr(),
            "MTLComputePipelineDescriptor",
            32,
            &[
                ("computeFunction", i(prop_get_id as *const ())),
                ("setComputeFunction:", i(prop_set_id as *const ())),
                ("label", i(prop_get_id as *const ())),
                ("setLabel:", i(prop_set_id as *const ())),
            ],
            &[],
        );
        objc_rt::register_builtin(
            MTLHEAPDESCRIPTOR_CLASS.ptr(),
            MTLHEAPDESCRIPTOR_META.ptr(),
            "MTLHeapDescriptor",
            32,
            &[
                ("size", i(prop_get_usize as *const ())),
                ("setSize:", i(prop_set_usize as *const ())),
            ],
            &[],
        );
        objc_rt::register_builtin(
            MTLCOMPILEOPTIONS_CLASS.ptr(),
            MTLCOMPILEOPTIONS_META.ptr(),
            "MTLCompileOptions",
            32,
            &[
                ("fastMathEnabled", i(prop_get_usize as *const ())),
                ("setFastMathEnabled:", i(prop_set_usize as *const ())),
                ("languageVersion", i(prop_get_usize as *const ())),
                ("setLanguageVersion:", i(prop_set_usize as *const ())),
            ],
            &[],
        );
        objc_rt::register_builtin(
            MTLFUNCTIONCONSTANTS_CLASS.ptr(),
            MTLFUNCTIONCONSTANTS_META.ptr(),
            "MTLFunctionConstantValues",
            32,
            &[("setConstantValue:type:atIndex:", i(mtl_noop3 as *const ()))],
            &[],
        );
        objc_rt::register_builtin(
            MTLCAPTUREMANAGER_CLASS.ptr(),
            MTLCAPTUREMANAGER_META.ptr(),
            "MTLCaptureManager",
            32,
            &[("isCapturing", i(mtl_false as *const ()))],
            &[("sharedCaptureManager", i(capture_shared as *const ()))],
        );
        objc_rt::register_builtin(
            MTLCAPTUREDESCRIPTOR_CLASS.ptr(),
            MTLCAPTUREDESCRIPTOR_META.ptr(),
            "MTLCaptureDescriptor",
            32,
            &[("captureObject", i(prop_get_id as *const ())), ("setCaptureObject:", i(prop_set_id as *const ()))],
            &[],
        );
        objc_rt::register_builtin(
            MTLSTENCILDESCRIPTOR_CLASS.ptr(),
            MTLSTENCILDESCRIPTOR_META.ptr(),
            "MTLStencilDescriptor",
            32,
            &[
                ("stencilCompareFunction", i(prop_get_usize as *const ())),
                ("setStencilCompareFunction:", i(prop_set_usize as *const ())),
                ("stencilFailureOperation", i(prop_get_usize as *const ())),
                ("setStencilFailureOperation:", i(prop_set_usize as *const ())),
                ("depthFailureOperation", i(prop_get_usize as *const ())),
                ("setDepthFailureOperation:", i(prop_set_usize as *const ())),
                ("depthStencilPassOperation", i(prop_get_usize as *const ())),
                ("setDepthStencilPassOperation:", i(prop_set_usize as *const ())),
                ("readMask", i(prop_get_usize as *const ())),
                ("setReadMask:", i(prop_set_usize as *const ())),
                ("writeMask", i(prop_get_usize as *const ())),
                ("setWriteMask:", i(prop_set_usize as *const ())),
            ],
            &[],
        );
    });
}

pub fn init() {
    register_classes();
}
