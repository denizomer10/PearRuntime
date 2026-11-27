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
use std::sync::{Arc, Mutex, Once, OnceLock};

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
    pixels: Arc<Vec<u8>>,
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

#[derive(Clone, Default)]
struct VertexAttrInfo {
    buffer_idx: usize,
    offset: usize,
    format: usize,
}

#[derive(Clone, Default)]
struct PipelineStateInfo {
    attrs: Vec<VertexAttrInfo>,
    strides: HashMap<usize, usize>,
    blend_enabled: bool,
}

#[derive(Clone, Default)]
struct RenderEncoderState {
    color_tex: usize,
    pipeline: usize,
    viewport: Option<MTLViewport>,
    vertex_buffers: HashMap<usize, (usize, usize)>,
    vertex_bytes: HashMap<usize, Vec<u8>>,
    fragment_textures: HashMap<usize, usize>,
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
static PIPELINES: OnceLock<Mutex<HashMap<usize, PipelineStateInfo>>> = OnceLock::new();
static RENDER_ENCODERS: OnceLock<Mutex<HashMap<usize, RenderEncoderState>>> = OnceLock::new();
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
static MTL_TRACE_ENABLED: OnceLock<bool> = OnceLock::new();
static MTL_TRACE_COUNT: AtomicUsize = AtomicUsize::new(0);

fn metal_trace_enabled() -> bool {
    *MTL_TRACE_ENABLED.get_or_init(|| std::env::var("PEAR_METAL_TRACE").as_deref() == Ok("1"))
}

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
fn pipelines() -> &'static Mutex<HashMap<usize, PipelineStateInfo>> {
    PIPELINES.get_or_init(Default::default)
}
fn render_encoders() -> &'static Mutex<HashMap<usize, RenderEncoderState>> {
    RENDER_ENCODERS.get_or_init(Default::default)
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

unsafe fn invoke_obj_err_handler(handler: Id, obj: Id, err: Id) {
    if handler.is_null() {
        return;
    }
    let block = handler as *const c_void;
    let invoke = *((block as *const usize).add(2));
    if invoke != 0 {
        let f: unsafe extern "C" fn(*const c_void, Id, Id) = std::mem::transmute(invoke);
        f(block, obj, err);
    }
}

unsafe fn invoke_obj_refl_err_handler(handler: Id, obj: Id, refl: Id, err: Id) {
    if handler.is_null() {
        return;
    }
    let block = handler as *const c_void;
    let invoke = *((block as *const usize).add(2));
    if invoke != 0 {
        let f: unsafe extern "C" fn(*const c_void, Id, Id, Id) = std::mem::transmute(invoke);
        f(block, obj, refl, err);
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
extern "C" fn prop_get_bool_is(this: Id, cmd: Sel) -> usize {
    let g = props_usize().lock().unwrap();
    if let Some(&v) = g.get(&(this as usize, cmd as usize)) {
        return v;
    }
    if let Ok(name) = unsafe { std::ffi::CStr::from_ptr(cmd) }.to_str() {
        if let Some(rest) = name.strip_prefix("is") {
            let mut chars = rest.chars();
            if let Some(first) = chars.next() {
                if first.is_ascii_uppercase() {
                    let alt = format!("{}{}", first.to_ascii_lowercase(), chars.as_str());
                    if let Some(&v) = g.get(&(this as usize, objc_rt::sel(&alt) as usize)) {
                        return v;
                    }
                }
            }
        }
    }
    0
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
        pixels: Arc::new(vec![0u8; byte_len]),
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

extern "C" fn dev_new_library_with_source_async(
    this: Id,
    cmd: Sel,
    src_id: Id,
    opts: Id,
    handler: Id,
) {
    let lib = dev_new_library_with_source(this, cmd, src_id, opts, std::ptr::null_mut());
    unsafe { invoke_obj_err_handler(handler, lib, std::ptr::null_mut()) };
}

extern "C" fn dev_new_default_library(_this: Id, _cmd: Sel) -> Id {
    let obj = unsafe { alloc(MTLLIBRARY_CLASS.ptr()) };
    libraries().lock().unwrap().insert(obj as usize, LibraryState {
        functions: HashMap::new(),
        order: Vec::new(),
    });
    obj
}

extern "C" fn dev_new_render_pipeline(_this: Id, _cmd: Sel, desc: Id, _err: *mut Id) -> Id {
    let mut info = PipelineStateInfo::default();
    if !desc.is_null() {
        unsafe {
            let vdesc: Id = send0(desc, "vertexDescriptor");
            if !vdesc.is_null() {
                let attrs_arr: Id = send0(vdesc, "attributes");
                if !attrs_arr.is_null() {
                    for idx in 0..31usize {
                        let attr: Id = send1(attrs_arr, "objectAtIndexedSubscript:", idx);
                        if !attr.is_null() {
                            let format: usize = send0(attr, "format");
                            if format != 0 {
                                let offset: usize = send0(attr, "offset");
                                let buffer_idx: usize = send0(attr, "bufferIndex");
                                info.attrs.push(VertexAttrInfo {
                                    buffer_idx,
                                    offset,
                                    format,
                                });
                            }
                        }
                    }
                }
                let layouts_arr: Id = send0(vdesc, "layouts");
                if !layouts_arr.is_null() {
                    for idx in 0..31usize {
                        let layout: Id = send1(layouts_arr, "objectAtIndexedSubscript:", idx);
                        if !layout.is_null() {
                            let stride: usize = send0(layout, "stride");
                            if stride != 0 {
                                info.strides.insert(idx, stride);
                            }
                        }
                    }
                }
            }
            let catts: Id = send0(desc, "colorAttachments");
            if !catts.is_null() {
                let ca0: Id = send1(catts, "objectAtIndexedSubscript:", 0usize);
                if !ca0.is_null() {
                    info.blend_enabled = send0::<usize>(ca0, "isBlendingEnabled") != 0;
                }
            }
        }
    }
    let obj = unsafe { alloc(MTLRENDERPIPELINESTATE_CLASS.ptr()) };
    pipelines().lock().unwrap().insert(obj as usize, info);
    obj
}
extern "C" fn dev_new_compute_pipeline(_this: Id, _cmd: Sel, _fn: Id, _err: *mut Id) -> Id {
    unsafe { alloc(MTLCOMPUTEPIPELINESTATE_CLASS.ptr()) }
}
extern "C" fn dev_new_compute_pipeline_desc(_this: Id, _cmd: Sel, _desc: Id, _opts: usize, _refl: *mut Id, _err: *mut Id) -> Id {
    unsafe { alloc(MTLCOMPUTEPIPELINESTATE_CLASS.ptr()) }
}
extern "C" fn dev_new_render_pipeline_opts_sync(
    this: Id,
    cmd: Sel,
    desc: Id,
    _opts: usize,
    refl: *mut Id,
    err: *mut Id,
) -> Id {
    if !refl.is_null() {
        unsafe { *refl = std::ptr::null_mut(); }
    }
    if !err.is_null() {
        unsafe { *err = std::ptr::null_mut(); }
    }
    dev_new_render_pipeline(this, cmd, desc, err)
}
extern "C" fn dev_new_render_pipeline_async(this: Id, cmd: Sel, desc: Id, handler: Id) {
    let pso = dev_new_render_pipeline(this, cmd, desc, std::ptr::null_mut());
    unsafe { invoke_obj_err_handler(handler, pso, std::ptr::null_mut()) };
}
extern "C" fn dev_new_render_pipeline_opts_async(
    this: Id,
    cmd: Sel,
    desc: Id,
    _opts: usize,
    handler: Id,
) {
    let pso = dev_new_render_pipeline(this, cmd, desc, std::ptr::null_mut());
    unsafe { invoke_obj_refl_err_handler(handler, pso, std::ptr::null_mut(), std::ptr::null_mut()) };
}
extern "C" fn dev_new_compute_pipeline_async(this: Id, cmd: Sel, func: Id, handler: Id) {
    let cps = dev_new_compute_pipeline(this, cmd, func, std::ptr::null_mut());
    unsafe { invoke_obj_err_handler(handler, cps, std::ptr::null_mut()) };
}
extern "C" fn dev_new_compute_pipeline_opts_async(
    this: Id,
    cmd: Sel,
    func: Id,
    _opts: usize,
    handler: Id,
) {
    let cps = dev_new_compute_pipeline(this, cmd, func, std::ptr::null_mut());
    unsafe { invoke_obj_refl_err_handler(handler, cps, std::ptr::null_mut(), std::ptr::null_mut()) };
}
extern "C" fn dev_new_compute_pipeline_desc_async(
    this: Id,
    cmd: Sel,
    desc: Id,
    opts: usize,
    handler: Id,
) {
    let cps = dev_new_compute_pipeline_desc(
        this,
        cmd,
        desc,
        opts,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
    );
    unsafe { invoke_obj_refl_err_handler(handler, cps, std::ptr::null_mut(), std::ptr::null_mut()) };
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
fn f16_to_f32(bits: u16) -> f32 {
    let sign = ((bits as u32) & 0x8000) << 16;
    let exp = ((bits >> 10) & 0x1f) as u32;
    let frac = (bits & 0x03ff) as u32;
    if exp == 0 {
        if frac == 0 {
            f32::from_bits(sign)
        } else {
            let v = (frac as f32) / 1024.0 * 2.0f32.powi(-14);
            if sign != 0 { -v } else { v }
        }
    } else if exp == 31 {
        let bits32 = sign | 0x7f80_0000 | (frac << 13);
        f32::from_bits(bits32)
    } else {
        let bits32 = sign | ((exp + 112) << 23) | (frac << 13);
        f32::from_bits(bits32)
    }
}

fn tex_src_bpp(pixel_format: usize) -> usize {
    match pixel_format {
        1 | 10 | 11 | 12 => 1,
        30 | 31 => 2,
        115 => 8,
        125 => 16,
        _ => 4,
    }
}

fn upload_texture_region_bytes(tex_id: usize, region: MTLRegion, src: &[u8], bytes_per_row: usize) {
    if src.is_empty() || region.size.width == 0 || region.size.height == 0 {
        return;
    }
    let mut g = textures().lock().unwrap();
    let Some(t) = g.get_mut(&tex_id) else { return };
    let tw = t.width;
    let th = t.height;
    let pf = t.pixel_format;
    if t.pixels.is_empty() || region.origin.x >= tw || region.origin.y >= th {
        return;
    }
    let dst_row_stride = tw * 4;
    let copy_cols = region.size.width.min(tw.saturating_sub(region.origin.x));
    let copy_rows = region.size.height.min(th.saturating_sub(region.origin.y));

    if let Some((bw, bh)) = crate::astc::astc_block_dims(pf) {
        let decoded = crate::astc::decode_astc_surface(
            src,
            bytes_per_row,
            region.size.width,
            region.size.height,
            bw,
            bh,
        );
        let copy_bytes = copy_cols * 4;
        let dst_pixels = Arc::make_mut(&mut t.pixels);
        for row in 0..copy_rows {
            let dy = region.origin.y + row;
            let s_off = row * region.size.width * 4;
            let d_off = dy * dst_row_stride + region.origin.x * 4;
            if let (Some(sr), Some(dr)) = (
                decoded.get(s_off..s_off + copy_bytes),
                dst_pixels.get_mut(d_off..d_off + copy_bytes),
            ) {
                dr.copy_from_slice(sr);
            }
        }
        return;
    }

    let bpp = tex_src_bpp(pf);
    let src_row_stride = if bytes_per_row >= region.size.width * bpp {
        bytes_per_row
    } else {
        region.size.width * bpp
    };
    let dst_pixels = Arc::make_mut(&mut t.pixels);
    for row in 0..copy_rows {
        let dy = region.origin.y + row;
        for col in 0..copy_cols {
            let s_off = row * src_row_stride + col * bpp;
            let d_off = dy * dst_row_stride + (region.origin.x + col) * 4;
            let Some(s) = src.get(s_off..s_off + bpp) else { break };
            let Some(d) = dst_pixels.get_mut(d_off..d_off + 4) else { break };
            match bpp {
                1 => {
                    let v = s[0];
                    if pf == 1 {
                        d.copy_from_slice(&[255, 255, 255, v]);
                    } else {
                        d.copy_from_slice(&[v, v, v, 255]);
                    }
                }
                2 => d.copy_from_slice(&[s[0], s[1], 0, 255]),
                8 => {
                    for c in 0..4 {
                        let f = f16_to_f32(u16::from_le_bytes([s[c * 2], s[c * 2 + 1]]));
                        d[c] = (f.clamp(0.0, 1.0) * 255.0).round() as u8;
                    }
                }
                16 => {
                    for c in 0..4 {
                        let o = c * 4;
                        let f = f32::from_le_bytes([s[o], s[o + 1], s[o + 2], s[o + 3]]);
                        d[c] = (f.clamp(0.0, 1.0) * 255.0).round() as u8;
                    }
                }
                _ => d.copy_from_slice(&[s[0], s[1], s[2], s[3]]),
            }
        }
    }
}

extern "C" fn tex_replace_region(
    this: Id,
    _cmd: Sel,
    region: MTLRegion,
    level: usize,
    bytes: *const u8,
    bytes_per_row: usize,
) {
    if this.is_null() || bytes.is_null() || region.size.width == 0 || region.size.height == 0 {
        return;
    }
    if level != 0 {
        return;
    }
    let pf = textures()
        .lock()
        .unwrap()
        .get(&(this as usize))
        .map(|t| t.pixel_format)
        .unwrap_or(0);
    let src_len = if let Some((bw, bh)) = crate::astc::astc_block_dims(pf) {
        let bx = region.size.width.div_ceil(bw);
        let by = region.size.height.div_ceil(bh);
        let bpr = if bytes_per_row >= bx * 16 { bytes_per_row } else { bx * 16 };
        by.saturating_mul(bpr)
    } else {
        let bpp = tex_src_bpp(pf);
        let bpr = if bytes_per_row >= region.size.width * bpp {
            bytes_per_row
        } else {
            region.size.width * bpp
        };
        region.size.height.saturating_mul(bpr)
    };
    if src_len == 0 {
        return;
    }
    let src = unsafe { std::slice::from_raw_parts(bytes, src_len) };
    upload_texture_region_bytes(this as usize, region, src, bytes_per_row);
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
    let mut color_tex = 0usize;
    if !pass.is_null() {
        unsafe {
            let atts: Id = send0(pass, "colorAttachments");
            if !atts.is_null() {
                let att0: Id = send1(atts, "objectAtIndexedSubscript:", 0usize);
                if !att0.is_null() {
                    let load_action: usize = send0(att0, "loadAction");
                    let tex: Id = send0(att0, "texture");
                    color_tex = tex as usize;
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
                                let dst_px = Arc::make_mut(&mut t.pixels);
                                for chunk in dst_px.chunks_exact_mut(4) {
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
    render_encoders().lock().unwrap().insert(
        enc as usize,
        RenderEncoderState {
            color_tex,
            ..Default::default()
        },
    );
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
            let copy_len = expected.min(t.pixels.len());
            let (rgba, bgra) = if is_bgra {
                let mut bgra = t.pixels[..copy_len].to_vec();
                bgra.resize(expected, 0);
                let mut rgba = vec![0u8; expected];
                for (dst, src) in rgba.chunks_exact_mut(4).zip(bgra.chunks_exact(4)) {
                    dst.copy_from_slice(&[src[2], src[1], src[0], src[3]]);
                }
                (rgba, bgra)
            } else {
                let mut rgba = t.pixels[..copy_len].to_vec();
                rgba.resize(expected, 0);
                let mut bgra = vec![0u8; expected];
                for (dst, src) in bgra.chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
                    dst.copy_from_slice(&[src[2], src[1], src[0], src[3]]);
                }
                (rgba, bgra)
            };
            (t.width, t.height, rgba, bgra)
        })
    };
    let Some((w, h, rgba, bgra)) = snapshot else {
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
                        let _ = ctx.present_xlib_pixels(
                            display,
                            window,
                            w as u32,
                            h as u32,
                            &bgra,
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

fn read_f32_le(buf: &[u8], off: usize) -> Option<f32> {
    let b = buf.get(off..off + 4)?;
    Some(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn fmt_byte_size(fmt: usize) -> usize {
    match fmt {
        28 => 4,       // Float
        29 => 8,       // Float2
        30 => 12,      // Float3
        31 => 16,      // Float4
        41 => 4,       // Half2
        42 => 6,       // Half3
        43 => 8,       // Half4
        45 => 4,       // UChar4Normalized
        _ => 0,
    }
}

fn read_uv_pair(fmt: usize, buf: &[u8], off: usize) -> (f32, f32) {
    if fmt == 41 || fmt == 43 {
        let Some(s) = buf.get(off..off + 4) else {
            return (0.0, 0.0);
        };
        let u = f16_to_f32(u16::from_le_bytes([s[0], s[1]]));
        let v = f16_to_f32(u16::from_le_bytes([s[2], s[3]]));
        (u, v)
    } else {
        (
            read_f32_le(buf, off).unwrap_or(0.0),
            read_f32_le(buf, off + 4).unwrap_or(0.0),
        )
    }
}

fn sample_tex_rgba(tex: &(usize, usize, usize, Arc<Vec<u8>>), u: f32, v: f32) -> [f32; 4] {
    let (w, h, pf, pixels) = tex;
    if *w == 0 || *h == 0 || pixels.is_empty() {
        return [1.0, 1.0, 1.0, 1.0];
    }
    let uf = if u.is_finite() {
        if (0.0..1.0).contains(&u) { u } else { u - u.floor() }
    } else {
        0.0
    };
    let vf = if v.is_finite() {
        if (0.0..1.0).contains(&v) { v } else { v - v.floor() }
    } else {
        0.0
    };
    let tx = ((uf * (*w as f32)) as usize).min(*w - 1);
    let ty = ((vf * (*h as f32)) as usize).min(*h - 1);
    let is_bgra = *pf == 80 || *pf == 81;
    let is_r8 = *pf == 10 || *pf == 11 || *pf == 12;
    if is_r8 {
        let idx = ty * *w + tx;
        if let Some(&r) = pixels.get(idx) {
            return [r as f32 / 255.0, r as f32 / 255.0, r as f32 / 255.0, 1.0];
        }
        return [1.0, 1.0, 1.0, 1.0];
    }
    let idx = (ty * *w + tx) * 4;
    let Some(c) = pixels.get(idx..idx + 4) else {
        return [1.0, 1.0, 1.0, 1.0];
    };
    if is_bgra {
        [
            c[2] as f32 / 255.0,
            c[1] as f32 / 255.0,
            c[0] as f32 / 255.0,
            c[3] as f32 / 255.0,
        ]
    } else {
        [
            c[0] as f32 / 255.0,
            c[1] as f32 / 255.0,
            c[2] as f32 / 255.0,
            c[3] as f32 / 255.0,
        ]
    }
}

fn read_mat4(buf: &[u8], off: usize) -> Option<[f32; 16]> {
    let mut m = [0.0f32; 16];
    for (idx, dst) in m.iter_mut().enumerate() {
        let v = read_f32_le(buf, off + idx * 4)?;
        if !v.is_finite() {
            return None;
        }
        *dst = v;
    }
    if !m[..12].iter().any(|&v| v.abs() > 1e-6) || m[15].abs() > 1e4 {
        return None;
    }
    Some(m)
}

#[inline]
fn mul_mat4(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut out = [0.0f32; 16];
    for col in 0..4 {
        for row in 0..4 {
            out[col * 4 + row] = a[0 * 4 + row] * b[col * 4 + 0]
                + a[1 * 4 + row] * b[col * 4 + 1]
                + a[2 * 4 + row] * b[col * 4 + 2]
                + a[3 * 4 + row] * b[col * 4 + 3];
        }
    }
    out
}

#[inline]
fn apply_mvp_to_ndc(m: &[f32; 16], x: f32, y: f32, z: f32) -> Option<(f32, f32)> {
    let cx = m[0] * x + m[4] * y + m[8] * z + m[12];
    let cy = m[1] * x + m[5] * y + m[9] * z + m[13];
    let cw = m[3] * x + m[7] * y + m[11] * z + m[15];
    if !cx.is_finite() || !cy.is_finite() || !cw.is_finite() || cw.abs() <= 1e-5 {
        return None;
    }
    Some((cx / cw, cy / cw))
}

fn is_good_ndc_transform(m: &[f32; 16], verts: &[(f32, f32, f32)]) -> bool {
    if verts.is_empty() {
        return false;
    }
    let mut min_x = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    for &(vx, vy, vz) in verts {
        let Some((nx, ny)) = apply_mvp_to_ndc(m, vx, vy, vz) else {
            return false;
        };
        if nx.abs() > 2.5 || ny.abs() > 2.5 {
            return false;
        }
        min_x = min_x.min(nx);
        max_x = max_x.max(nx);
        min_y = min_y.min(ny);
        max_y = max_y.max(ny);
    }
    (max_x - min_x) > 1e-3 && (max_y - min_y) > 1e-3
}

fn resolve_draw_transform(
    vb_data: &HashMap<usize, &[u8]>,
    pos_buf_idx: usize,
    sample_verts: &[(f32, f32, f32)],
    tw: usize,
    th: usize,
) -> Option<[f32; 16]> {
    if sample_verts.is_empty() {
        return None;
    }
    let raw_in_ndc = sample_verts
        .iter()
        .all(|&(x, y, _)| x.abs() <= 1.5 && y.abs() <= 1.5);
    let is_identity = |m: &[f32; 16]| {
        (m[0] - 1.0).abs() < 1e-4
            && (m[5] - 1.0).abs() < 1e-4
            && (m[10] - 1.0).abs() < 1e-4
            && m[12].abs() < 1e-4
            && m[13].abs() < 1e-4
    };

    if pos_buf_idx != 0 {
        if let Some(buf0) = vb_data.get(&0) {
            if buf0.len() >= 64 {
                let max_off = buf0.len().saturating_sub(64).min(256);
                let mut fallback_id = None;
                for off in (0..=max_off).step_by(16) {
                    if let Some(m) = read_mat4(buf0, off) {
                        if is_good_ndc_transform(&m, sample_verts) {
                            if !is_identity(&m) || raw_in_ndc {
                                return Some(m);
                            }
                            if fallback_id.is_none() {
                                fallback_id = Some(m);
                            }
                        }
                    }
                }
                let max_pair = buf0.len().saturating_sub(128).min(192);
                for off in (0..=max_pair).step_by(16) {
                    if let (Some(ma), Some(mb)) = (read_mat4(buf0, off), read_mat4(buf0, off + 64)) {
                        for cand in [mul_mat4(&ma, &mb), mul_mat4(&mb, &ma)] {
                            if is_good_ndc_transform(&cand, sample_verts) {
                                return Some(cand);
                            }
                        }
                    }
                }
                if let Some(m) = fallback_id {
                    return Some(m);
                }
            }
        }
    }

    if tw > 0 && th > 0 {
        let tw_f = tw as f32;
        let th_f = th as f32;
        let has_large_xy = sample_verts.iter().any(|&(x, y, _)| x.abs() > 2.0 || y.abs() > 2.0);
        let in_pixel_bounds = sample_verts
            .iter()
            .all(|&(x, y, _)| x >= -tw_f && x <= 2.0 * tw_f && y >= -th_f && y <= 2.0 * th_f);
        if has_large_xy && in_pixel_bounds {
            return Some([
                2.0 / tw_f, 0.0, 0.0, 0.0,
                0.0, 2.0 / th_f, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
                -1.0, -1.0, 0.0, 1.0,
            ]);
        }
    }
    None
}

fn rasterize_draw(enc_id: usize, ptype: usize, indices: &[u32]) {
    if indices.len() < 3 || (ptype != 3 && ptype != 4) {
        return;
    }
    let Some(st) = render_encoders().lock().unwrap().get(&enc_id).cloned() else {
        return;
    };
    let pso = pipelines()
        .lock()
        .unwrap()
        .get(&st.pipeline)
        .cloned()
        .unwrap_or_default();

    if st.color_tex == 0 {
        return;
    }

    let frag_tex_id = st.fragment_textures.get(&0).copied().unwrap_or(0);
    let (tw_hint, th_hint, frag_snap) = {
        let tg = textures().lock().unwrap();
        let (twh, thh) = tg
            .get(&st.color_tex)
            .map(|t| (t.width, t.height))
            .unwrap_or((0, 0));
        let fs = if frag_tex_id != 0 {
            tg.get(&frag_tex_id)
                .map(|t| (t.width, t.height, t.pixel_format, Arc::clone(&t.pixels)))
        } else {
            None
        };
        (twh, thh, fs)
    };
    if tw_hint == 0 || th_hint == 0 {
        return;
    }
    let vp_early = st.viewport.unwrap_or(MTLViewport {
        origin_x: 0.0,
        origin_y: 0.0,
        width: tw_hint as f64,
        height: th_hint as f64,
        znear: 0.0,
        zfar: 1.0,
    });

    #[derive(Copy, Clone)]
    struct Vtx {
        sx: f32,
        sy: f32,
        u: f32,
        v: f32,
        col: [f32; 4],
    }

    let pos_attr = pso
        .attrs
        .iter()
        .find(|a| matches!(a.format, 29 | 30 | 31))
        .cloned()
        .unwrap_or(VertexAttrInfo {
            buffer_idx: 0,
            offset: 0,
            format: 30,
        });
    let cand_col_attr = pso
        .attrs
        .iter()
        .find(|a| {
            (a.format == 45 || a.format == 31)
                && !(a.buffer_idx == pos_attr.buffer_idx && a.offset == pos_attr.offset)
        })
        .cloned();

    let stride_for = |buf_idx: usize| -> usize {
        if let Some(&s) = pso.strides.get(&buf_idx) {
            if s > 0 {
                return s;
            }
        }
        let mut max_end = 0usize;
        for a in &pso.attrs {
            if a.buffer_idx == buf_idx {
                max_end = max_end.max(a.offset + fmt_byte_size(a.format));
            }
        }
        max_end.max(12)
    };

    let tris: Vec<[Vtx; 3]> = {
        let bg = buffers().lock().unwrap();
        let mut vb_data: HashMap<usize, &[u8]> = HashMap::with_capacity(
            st.vertex_bytes.len() + st.vertex_buffers.len(),
        );
        for (&idx, bytes) in &st.vertex_bytes {
            vb_data.insert(idx, bytes.as_slice());
        }
        for (&idx, &(buf_id, off)) in &st.vertex_buffers {
            if !vb_data.contains_key(&idx) {
                if let Some(b) = bg.get(&buf_id) {
                    if let Some(slice) = b.bytes.get(off..) {
                        vb_data.insert(idx, slice);
                    }
                }
            }
        }
        if vb_data.is_empty() {
            return;
        }

        // Guard Float4 tangent/normal attributes against being mistaken for vertex colors.
        let col_attr = cand_col_attr.filter(|ca| {
            if ca.format == 45 {
                return true;
            }
            let cs = stride_for(ca.buffer_idx);
            let Some(cb) = vb_data.get(&ca.buffer_idx).copied() else {
                return false;
            };
            let Some(coff) = (indices[0] as usize).checked_mul(cs).and_then(|o| o.checked_add(ca.offset)) else {
                return false;
            };
            let (Some(r), Some(g), Some(b), Some(a)) = (
                read_f32_le(cb, coff),
                read_f32_le(cb, coff + 4),
                read_f32_le(cb, coff + 8),
                read_f32_le(cb, coff + 12),
            ) else {
                return false;
            };
            (0.0..=1.0).contains(&r)
                && (0.0..=1.0).contains(&g)
                && (0.0..=1.0).contains(&b)
                && a > 0.0
                && a <= 1.0
        });

        let same_slot = |a: &VertexAttrInfo, b: &VertexAttrInfo| {
            a.buffer_idx == b.buffer_idx && a.offset == b.offset
        };
        let uv_attr = pso
            .attrs
            .iter()
            .find(|a| matches!(a.format, 29 | 41) && !same_slot(a, &pos_attr))
            .or_else(|| {
                pso.attrs.iter().find(|a| {
                    matches!(a.format, 30 | 31 | 43)
                        && !same_slot(a, &pos_attr)
                        && col_attr.as_ref().map_or(true, |c| !same_slot(a, c))
                })
            })
            .cloned();

        let mut sample_verts: Vec<(f32, f32, f32)> = Vec::new();
        let p_stride = stride_for(pos_attr.buffer_idx);
        if let Some(&p_buf) = vb_data.get(&pos_attr.buffer_idx) {
            let mut seen_idx = Vec::new();
            for &vi in indices {
                if !seen_idx.contains(&vi) {
                    seen_idx.push(vi);
                    if let Some(p_off) = (vi as usize).checked_mul(p_stride).and_then(|o| o.checked_add(pos_attr.offset)) {
                        if let (Some(x), Some(y)) = (read_f32_le(p_buf, p_off), read_f32_le(p_buf, p_off + 4)) {
                            let z = if pos_attr.format >= 30 {
                                read_f32_le(p_buf, p_off + 8).unwrap_or(0.0)
                            } else {
                                0.0
                            };
                            sample_verts.push((x, y, z));
                            if sample_verts.len() >= 4 {
                                break;
                            }
                        }
                    }
                }
            }
        }

        let active_mvp = resolve_draw_transform(&vb_data, pos_attr.buffer_idx, &sample_verts, tw_hint, th_hint);

        if metal_trace_enabled() && MTL_TRACE_COUNT.fetch_add(1, Ordering::Relaxed) < 12 {
            let (fw, fh, fpf, fnz, tex_mid, tex_a_nz) = if let Some(ref ftex) = frag_snap {
                let (w, h, pf, ref p) = *ftex;
                let nz = p.chunks_exact(4).take(4096).filter(|c| c != &[0, 0, 0, 0]).count();
                let mid = sample_tex_rgba(ftex, 0.5, 0.5);
                let mut a_nz = 0usize;
                for gy in 0..16 {
                    for gx in 0..16 {
                        let s = sample_tex_rgba(ftex, (gx as f32 + 0.5) / 16.0, (gy as f32 + 0.5) / 16.0);
                        if s[3] > 0.01 {
                            a_nz += 1;
                        }
                    }
                }
                (w, h, pf, nz, mid, a_nz)
            } else {
                (0, 0, 0, 0, [0.0; 4], 0)
            };
            let i0 = indices[0] as usize;
            let v0_uv = if let Some(ref ua) = uv_attr {
                let us = stride_for(ua.buffer_idx);
                let ub = vb_data.get(&ua.buffer_idx).copied();
                let uoff = i0.checked_mul(us).and_then(|o| o.checked_add(ua.offset));
                match (ub, uoff) {
                    (Some(b), Some(o)) => read_uv_pair(ua.format, b, o),
                    _ => (0.0, 0.0),
                }
            } else {
                (0.0, 0.0)
            };
            let v0_col = if let Some(ref ca) = col_attr {
                let cs = stride_for(ca.buffer_idx);
                let cb = vb_data.get(&ca.buffer_idx).copied();
                let coff = i0.checked_mul(cs).and_then(|o| o.checked_add(ca.offset));
                match (cb, coff) {
                    (Some(b), Some(o)) if ca.format == 45 => b
                        .get(o..o + 4)
                        .map(|s| [s[0] as f32 / 255.0, s[1] as f32 / 255.0, s[2] as f32 / 255.0, s[3] as f32 / 255.0])
                        .unwrap_or([1.0; 4]),
                    (Some(b), Some(o)) if ca.format == 31 => [
                        read_f32_le(b, o).unwrap_or(1.0),
                        read_f32_le(b, o + 4).unwrap_or(1.0),
                        read_f32_le(b, o + 8).unwrap_or(1.0),
                        read_f32_le(b, o + 12).unwrap_or(1.0),
                    ],
                    _ => [1.0; 4],
                }
            } else {
                [1.0; 4]
            };
            let mut s_min_x = f32::INFINITY;
            let mut s_max_x = f32::NEG_INFINITY;
            let mut s_min_y = f32::INFINITY;
            let mut s_max_y = f32::NEG_INFINITY;
            for &(vx, vy, vz) in &sample_verts {
                let (nx, ny) = active_mvp
                    .and_then(|m| apply_mvp_to_ndc(&m, vx, vy, vz))
                    .unwrap_or((vx, vy));
                let sx = (vp_early.origin_x + (nx as f64 * 0.5 + 0.5) * vp_early.width) as f32;
                let sy = (vp_early.origin_y + (0.5 - ny as f64 * 0.5) * vp_early.height) as f32;
                s_min_x = s_min_x.min(sx);
                s_max_x = s_max_x.max(sx);
                s_min_y = s_min_y.min(sy);
                s_max_y = s_max_y.max(sy);
            }
            let mut vb_lens: Vec<(usize, usize)> =
                vb_data.iter().map(|(&k, v)| (k, v.len())).collect();
            vb_lens.sort_by_key(|&(k, _)| k);
            let fmt_attr = |a: Option<&VertexAttrInfo>| -> String {
                match a {
                    Some(x) => format!("({},{},{})", x.format, x.offset, x.buffer_idx),
                    None => "none".to_string(),
                }
            };
            let all_attrs: Vec<(usize, usize, usize)> = pso
                .attrs
                .iter()
                .map(|a| (a.format, a.offset, a.buffer_idx))
                .collect();
            eprintln!(
                "[PearRuntime][metal][draw] tex={:#x} ptype={} indices={} frag=({},{},{},{}) tex_mid=({:.2},{:.2},{:.2},{:.2}) tex_a_nz={}/256 blend={} mvp={} vb_lens={:?} bbox=({:.1},{:.1})..({:.1},{:.1}) v0_uv=({:.3},{:.3}) v0_col=({:.2},{:.2},{:.2},{:.2}) attrs=[pos={},uv={},col={}] all_attrs={:?}",
                st.color_tex, ptype, indices.len(), fw, fh, fpf, fnz,
                tex_mid[0], tex_mid[1], tex_mid[2], tex_mid[3], tex_a_nz,
                pso.blend_enabled, active_mvp.is_some(), vb_lens,
                s_min_x, s_min_y, s_max_x, s_max_y,
                v0_uv.0, v0_uv.1, v0_col[0], v0_col[1], v0_col[2], v0_col[3],
                fmt_attr(Some(&pos_attr)), fmt_attr(uv_attr.as_ref()), fmt_attr(col_attr.as_ref()),
                all_attrs,
            );
        }

        let load_vtx = |vi: u32| -> Option<Vtx> {
            let p_buf = vb_data.get(&pos_attr.buffer_idx).copied()?;
            let p_off = (vi as usize).checked_mul(p_stride)?.checked_add(pos_attr.offset)?;
            let x = read_f32_le(p_buf, p_off)?;
            let y = read_f32_le(p_buf, p_off + 4)?;
            let (nx_f, ny_f) = if let Some(ref m) = active_mvp {
                let z = if pos_attr.format >= 30 {
                    read_f32_le(p_buf, p_off + 8).unwrap_or(0.0)
                } else {
                    0.0
                };
                apply_mvp_to_ndc(m, x, y, z).unwrap_or((x, y))
            } else {
                let w = if pos_attr.format == 31 {
                    read_f32_le(p_buf, p_off + 12).unwrap_or(1.0)
                } else {
                    1.0
                };
                let inv_w = if w.abs() > 1e-6 { 1.0 / w } else { 1.0 };
                (x * inv_w, y * inv_w)
            };
            let nx = nx_f as f64;
            let ny = ny_f as f64;
            let sx = (vp_early.origin_x + (nx * 0.5 + 0.5) * vp_early.width) as f32;
            let sy = (vp_early.origin_y + (0.5 - ny * 0.5) * vp_early.height) as f32;

            let (u, v) = if let Some(ref ua) = uv_attr {
                let us = stride_for(ua.buffer_idx);
                let ub = vb_data.get(&ua.buffer_idx).copied();
                let uoff = (vi as usize).checked_mul(us).and_then(|o| o.checked_add(ua.offset));
                match (ub, uoff) {
                    (Some(b), Some(o)) => read_uv_pair(ua.format, b, o),
                    _ => (0.0, 0.0),
                }
            } else {
                (0.0, 0.0)
            };

            let col = if let Some(ref ca) = col_attr {
                let cs = stride_for(ca.buffer_idx);
                let cb = vb_data.get(&ca.buffer_idx).copied();
                let coff = (vi as usize).checked_mul(cs).and_then(|o| o.checked_add(ca.offset));
                match (cb, coff) {
                    (Some(b), Some(o)) if ca.format == 45 => b
                        .get(o..o + 4)
                        .map(|s| [s[0] as f32 / 255.0, s[1] as f32 / 255.0, s[2] as f32 / 255.0, s[3] as f32 / 255.0])
                        .unwrap_or([1.0, 1.0, 1.0, 1.0]),
                    (Some(b), Some(o)) if ca.format == 31 => [
                        read_f32_le(b, o).unwrap_or(1.0),
                        read_f32_le(b, o + 4).unwrap_or(1.0),
                        read_f32_le(b, o + 8).unwrap_or(1.0),
                        read_f32_le(b, o + 12).unwrap_or(1.0),
                    ],
                    _ => [1.0, 1.0, 1.0, 1.0],
                }
            } else {
                [1.0, 1.0, 1.0, 1.0]
            };

            Some(Vtx { sx, sy, u, v, col })
        };

        let tri_count = if ptype == 3 {
            (indices.len() / 3).min(4096)
        } else {
            indices.len().saturating_sub(2).min(4096)
        };
        let mut out_tris = Vec::with_capacity(tri_count);
        for t_idx in 0..tri_count {
            let (i0, i1, i2) = if ptype == 3 {
                (indices[t_idx * 3], indices[t_idx * 3 + 1], indices[t_idx * 3 + 2])
            } else if t_idx % 2 == 0 {
                (indices[t_idx], indices[t_idx + 1], indices[t_idx + 2])
            } else {
                (indices[t_idx], indices[t_idx + 2], indices[t_idx + 1])
            };
            if let (Some(v0), Some(v1), Some(v2)) = (load_vtx(i0), load_vtx(i1), load_vtx(i2)) {
                let area = (v1.sx - v0.sx) * (v2.sy - v0.sy) - (v1.sy - v0.sy) * (v2.sx - v0.sx);
                if area.abs() >= 1e-4 {
                    out_tris.push([v0, v1, v2]);
                }
            }
        }
        out_tris
    };
    if tris.is_empty() {
        return;
    }

    let tex_uniform: Option<[f32; 4]> = match frag_snap {
        None => Some([1.0, 1.0, 1.0, 1.0]),
        Some(ref ftex) if ftex.0 > 0 && ftex.1 > 0 && ftex.0 <= 4 && ftex.1 <= 4 => {
            let first = sample_tex_rgba(ftex, 0.0, 0.0);
            let px_count = ftex.0 * ftex.1;
            let is_r8 = ftex.2 == 10 || ftex.2 == 11 || ftex.2 == 12;
            let all_eq = if is_r8 {
                ftex.3.get(..px_count).map_or(false, |s| s.iter().all(|&b| b == s[0]))
            } else {
                ftex.3
                    .get(..px_count * 4)
                    .map_or(false, |s| s.chunks_exact(4).all(|c| c == &s[..4]))
            };
            if all_eq { Some(first) } else { None }
        }
        _ => None,
    };

    let mut tg = textures().lock().unwrap();
    let Some(target) = tg.get_mut(&st.color_tex) else {
        return;
    };
    let tw = target.width;
    let th = target.height;
    if tw == 0 || th == 0 || target.pixels.len() < tw * th * 4 {
        return;
    }
    let target_is_bgra = target.pixel_format == 80 || target.pixel_format == 81;
    let dst_pixels = Arc::make_mut(&mut target.pixels);

    for [v0, v1, v2] in tris {
        let area = (v1.sx - v0.sx) * (v2.sy - v0.sy) - (v1.sy - v0.sy) * (v2.sx - v0.sx);
        let inv_area = 1.0 / area;
        let min_x = v0.sx.min(v1.sx).min(v2.sx).floor().max(0.0) as isize;
        let max_x = v0.sx.max(v1.sx).max(v2.sx).ceil().min((tw - 1) as f32) as isize;
        let min_y = v0.sy.min(v1.sy).min(v2.sy).floor().max(0.0) as isize;
        let max_y = v0.sy.max(v1.sy).max(v2.sy).ceil().min((th - 1) as f32) as isize;
        if min_x > max_x || min_y > max_y {
            continue;
        }

        let dw0_dx = -(v2.sy - v1.sy) * inv_area;
        let dw0_dy = (v2.sx - v1.sx) * inv_area;
        let dw1_dx = -(v0.sy - v2.sy) * inv_area;
        let dw1_dy = (v0.sx - v2.sx) * inv_area;
        let start_cx = min_x as f32 + 0.5;
        let start_cy = min_y as f32 + 0.5;
        let mut w0_row =
            ((v2.sx - v1.sx) * (start_cy - v1.sy) - (v2.sy - v1.sy) * (start_cx - v1.sx)) * inv_area;
        let mut w1_row =
            ((v0.sx - v2.sx) * (start_cy - v2.sy) - (v0.sy - v2.sy) * (start_cx - v2.sx)) * inv_area;

        let uniform_col = v0.col == v1.col && v1.col == v2.col;
        if uniform_col {
            if let Some(tc) = tex_uniform {
                let sr = ((tc[0] * v0.col[0]).clamp(0.0, 1.0) * 255.0).round() as u32;
                let sg = ((tc[1] * v0.col[1]).clamp(0.0, 1.0) * 255.0).round() as u32;
                let sb = ((tc[2] * v0.col[2]).clamp(0.0, 1.0) * 255.0).round() as u32;
                let sa = ((tc[3] * v0.col[3]).clamp(0.0, 1.0) * 255.0).round() as u32;
                if pso.blend_enabled && sa == 0 {
                    continue;
                }
                let solid_px = if target_is_bgra {
                    [sb as u8, sg as u8, sr as u8, sa as u8]
                } else {
                    [sr as u8, sg as u8, sb as u8, sa as u8]
                };
                let do_blend = pso.blend_enabled && sa < 255;
                let inv_a = 255 - sa;
                for py in min_y..=max_y {
                    let mut w0 = w0_row;
                    let mut w1 = w1_row;
                    let row_off = (py as usize) * tw;
                    for px in min_x..=max_x {
                        let w2 = 1.0 - w0 - w1;
                        if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                            let off = (row_off + (px as usize)) * 4;
                            let dst = &mut dst_pixels[off..off + 4];
                            if !do_blend {
                                dst.copy_from_slice(&solid_px);
                            } else {
                                let (dr, dg, db) = if target_is_bgra {
                                    (dst[2] as u32, dst[1] as u32, dst[0] as u32)
                                } else {
                                    (dst[0] as u32, dst[1] as u32, dst[2] as u32)
                                };
                                let r = ((sr * sa + dr * inv_a + 127) / 255) as u8;
                                let g = ((sg * sa + dg * inv_a + 127) / 255) as u8;
                                let b = ((sb * sa + db * inv_a + 127) / 255) as u8;
                                let oa = ((sa * 255 + (dst[3] as u32) * inv_a + 127) / 255).min(255) as u8;
                                if target_is_bgra {
                                    dst.copy_from_slice(&[b, g, r, oa]);
                                } else {
                                    dst.copy_from_slice(&[r, g, b, oa]);
                                }
                            }
                        }
                        w0 += dw0_dx;
                        w1 += dw1_dx;
                    }
                    w0_row += dw0_dy;
                    w1_row += dw1_dy;
                }
                continue;
            }
        }

        for py in min_y..=max_y {
            let mut w0 = w0_row;
            let mut w1 = w1_row;
            let row_off = (py as usize) * tw;
            for px in min_x..=max_x {
                let w2 = 1.0 - w0 - w1;
                if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                    let tu = v0.u * w0 + v1.u * w1 + v2.u * w2;
                    let tv = v0.v * w0 + v1.v * w1 + v2.v * w2;
                    let mut src = if let Some(ref ftex) = frag_snap {
                        sample_tex_rgba(ftex, tu, tv)
                    } else {
                        [1.0, 1.0, 1.0, 1.0]
                    };
                    if uniform_col {
                        for c in 0..4 {
                            src[c] *= v0.col[c];
                        }
                    } else {
                        for c in 0..4 {
                            src[c] *= (v0.col[c] * w0 + v1.col[c] * w1 + v2.col[c] * w2).clamp(0.0, 1.0);
                        }
                    }
                    let sa = (src[3].clamp(0.0, 1.0) * 255.0).round() as u32;
                    if !(pso.blend_enabled && sa == 0) {
                        let sr = (src[0].clamp(0.0, 1.0) * 255.0).round() as u32;
                        let sg = (src[1].clamp(0.0, 1.0) * 255.0).round() as u32;
                        let sb = (src[2].clamp(0.0, 1.0) * 255.0).round() as u32;
                        let off = (row_off + (px as usize)) * 4;
                        let dst = &mut dst_pixels[off..off + 4];
                        if pso.blend_enabled && sa < 255 {
                            let inv_a = 255 - sa;
                            let (dr, dg, db) = if target_is_bgra {
                                (dst[2] as u32, dst[1] as u32, dst[0] as u32)
                            } else {
                                (dst[0] as u32, dst[1] as u32, dst[2] as u32)
                            };
                            let r = ((sr * sa + dr * inv_a + 127) / 255) as u8;
                            let g = ((sg * sa + dg * inv_a + 127) / 255) as u8;
                            let b = ((sb * sa + db * inv_a + 127) / 255) as u8;
                            let oa = ((sa * 255 + (dst[3] as u32) * inv_a + 127) / 255).min(255) as u8;
                            if target_is_bgra {
                                dst.copy_from_slice(&[b, g, r, oa]);
                            } else {
                                dst.copy_from_slice(&[r, g, b, oa]);
                            }
                        } else if target_is_bgra {
                            dst.copy_from_slice(&[sb as u8, sg as u8, sr as u8, sa as u8]);
                        } else {
                            dst.copy_from_slice(&[sr as u8, sg as u8, sb as u8, sa as u8]);
                        }
                    }
                }
                w0 += dw0_dx;
                w1 += dw1_dx;
            }
            w0_row += dw0_dy;
            w1_row += dw1_dy;
        }
    }
}

extern "C" fn enc_set_render_pipeline(this: Id, _cmd: Sel, pso: Id) {
    if let Some(st) = render_encoders().lock().unwrap().get_mut(&(this as usize)) {
        st.pipeline = pso as usize;
    }
}
extern "C" fn enc_set_viewport(this: Id, _cmd: Sel, vp: MTLViewport) {
    if let Some(st) = render_encoders().lock().unwrap().get_mut(&(this as usize)) {
        st.viewport = Some(vp);
    }
}
extern "C" fn enc_set_viewports(this: Id, _cmd: Sel, vps: *const MTLViewport, count: usize) {
    if !vps.is_null() && count > 0 {
        if let Some(st) = render_encoders().lock().unwrap().get_mut(&(this as usize)) {
            st.viewport = Some(unsafe { *vps });
        }
    }
}
extern "C" fn enc_set_vertex_buffer(this: Id, _cmd: Sel, buf: Id, offset: usize, idx: usize) {
    if let Some(st) = render_encoders().lock().unwrap().get_mut(&(this as usize)) {
        st.vertex_buffers.insert(idx, (buf as usize, offset));
        st.vertex_bytes.remove(&idx);
    }
}
extern "C" fn enc_set_vertex_buffer_offset(this: Id, _cmd: Sel, offset: usize, idx: usize) {
    if let Some(st) = render_encoders().lock().unwrap().get_mut(&(this as usize)) {
        if let Some(entry) = st.vertex_buffers.get_mut(&idx) {
            entry.1 = offset;
        }
    }
}
extern "C" fn enc_set_vertex_bytes(this: Id, _cmd: Sel, ptr: *const u8, len: usize, idx: usize) {
    if ptr.is_null() || len == 0 {
        return;
    }
    let slice = unsafe { std::slice::from_raw_parts(ptr, len.min(1 << 20)) };
    if let Some(st) = render_encoders().lock().unwrap().get_mut(&(this as usize)) {
        st.vertex_bytes.insert(idx, slice.to_vec());
    }
}
extern "C" fn enc_set_fragment_texture(this: Id, _cmd: Sel, tex: Id, idx: usize) {
    if let Some(st) = render_encoders().lock().unwrap().get_mut(&(this as usize)) {
        st.fragment_textures.insert(idx, tex as usize);
    }
}
extern "C" fn enc_end_encoding(this: Id, _cmd: Sel) {
    render_encoders().lock().unwrap().remove(&(this as usize));
    encoder_to_cb().lock().unwrap().remove(&(this as usize));
}

extern "C" fn enc_draw_primitives(this: Id, _cmd: Sel, ptype: usize, start: usize, count: usize) {
    TOTAL_DRAW_CALLS.fetch_add(1, Ordering::Relaxed);
    if let Some(&cb) = encoder_to_cb().lock().unwrap().get(&(this as usize)) {
        if let Some(st) = cmd_buffers().lock().unwrap().get_mut(&cb) {
            st.draw_calls += 1;
        }
    }
    let capped = count.min(4096 * 3);
    let indices: Vec<u32> = (0..capped).map(|i| (start + i) as u32).collect();
    rasterize_draw(this as usize, ptype, &indices);
}
extern "C" fn enc_draw_indexed(
    this: Id,
    _cmd: Sel,
    ptype: usize,
    icount: usize,
    itype: usize,
    ibuf: Id,
    ioff: usize,
) {
    TOTAL_DRAW_CALLS.fetch_add(1, Ordering::Relaxed);
    if let Some(&cb) = encoder_to_cb().lock().unwrap().get(&(this as usize)) {
        if let Some(st) = cmd_buffers().lock().unwrap().get_mut(&cb) {
            st.draw_calls += 1;
        }
    }
    if ibuf.is_null() || icount == 0 {
        return;
    }
    let capped = icount.min(4096 * 3);
    let mut indices = Vec::with_capacity(capped);
    {
        let bg = buffers().lock().unwrap();
        if let Some(b) = bg.get(&(ibuf as usize)) {
            if itype == 0 {
                for i in 0..capped {
                    let Some(off) = ioff.checked_add(i * 2) else { break };
                    let Some(s) = b.bytes.get(off..off + 2) else { break };
                    indices.push(u16::from_le_bytes([s[0], s[1]]) as u32);
                }
            } else {
                for i in 0..capped {
                    let Some(off) = ioff.checked_add(i * 4) else { break };
                    let Some(s) = b.bytes.get(off..off + 4) else { break };
                    indices.push(u32::from_le_bytes([s[0], s[1], s[2], s[3]]));
                }
            }
        }
    }
    rasterize_draw(this as usize, ptype, &indices);
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

extern "C" fn blit_copy_texture(
    this: Id,
    _cmd: Sel,
    src_tex: Id,
    _src_slice: usize,
    _src_level: usize,
    src_origin: MTLOrigin,
    src_size: MTLSize,
    dst_tex: Id,
    _dst_slice: usize,
    _dst_level: usize,
    dst_origin: MTLOrigin,
) {
    if let Some(&cb) = encoder_to_cb().lock().unwrap().get(&(this as usize)) {
        if let Some(st) = cmd_buffers().lock().unwrap().get_mut(&cb) {
            st.blit_ops += 1;
        }
    }
    if src_tex.is_null() || dst_tex.is_null() || src_size.width == 0 || src_size.height == 0 {
        return;
    }
    let src_snap = {
        let g = textures().lock().unwrap();
        g.get(&(src_tex as usize))
            .map(|t| (t.width, t.height, Arc::clone(&t.pixels)))
    };
    let Some((sw, sh, spixels)) = src_snap else {
        return;
    };
    let mut g = textures().lock().unwrap();
    let Some(dst) = g.get_mut(&(dst_tex as usize)) else {
        return;
    };
    let cols = src_size
        .width
        .min(sw.saturating_sub(src_origin.x))
        .min(dst.width.saturating_sub(dst_origin.x));
    let rows = src_size
        .height
        .min(sh.saturating_sub(src_origin.y))
        .min(dst.height.saturating_sub(dst_origin.y));
    let copy_bytes = cols * 4;
    let dst_w = dst.width;
    let dst_pixels = Arc::make_mut(&mut dst.pixels);
    for r in 0..rows {
        let s_off = ((src_origin.y + r) * sw + src_origin.x) * 4;
        let d_off = ((dst_origin.y + r) * dst_w + dst_origin.x) * 4;
        if let (Some(src_row), Some(dst_row)) = (
            spixels.get(s_off..s_off + copy_bytes),
            dst_pixels.get_mut(d_off..d_off + copy_bytes),
        ) {
            dst_row.copy_from_slice(src_row);
        }
    }
}

extern "C" fn blit_copy_buffer_to_texture(
    this: Id,
    _cmd: Sel,
    src_buf: Id,
    src_off: usize,
    src_bpr: usize,
    _src_bpi: usize,
    src_size: MTLSize,
    dst_tex: Id,
    _dst_slice: usize,
    dst_level: usize,
    dst_origin: MTLOrigin,
) {
    if let Some(&cb) = encoder_to_cb().lock().unwrap().get(&(this as usize)) {
        if let Some(st) = cmd_buffers().lock().unwrap().get_mut(&cb) {
            st.blit_ops += 1;
        }
    }
    if src_buf.is_null() || dst_tex.is_null() || src_size.width == 0 || src_size.height == 0 {
        return;
    }
    if dst_level != 0 {
        return;
    }
    let src_bytes = {
        let g = buffers().lock().unwrap();
        let Some(src) = g.get(&(src_buf as usize)) else { return };
        let Some(slice) = src.bytes.get(src_off..) else { return };
        slice.to_vec()
    };
    upload_texture_region_bytes(
        dst_tex as usize,
        MTLRegion {
            origin: dst_origin,
            size: src_size,
        },
        &src_bytes,
        src_bpr,
    );
}

extern "C" fn blit_copy_texture_to_buffer(
    this: Id,
    _cmd: Sel,
    src_tex: Id,
    _src_slice: usize,
    _src_level: usize,
    src_origin: MTLOrigin,
    src_size: MTLSize,
    dst_buf: Id,
    dst_off: usize,
    dst_bpr: usize,
    _dst_bpi: usize,
) {
    if let Some(&cb) = encoder_to_cb().lock().unwrap().get(&(this as usize)) {
        if let Some(st) = cmd_buffers().lock().unwrap().get_mut(&cb) {
            st.blit_ops += 1;
        }
    }
    if src_tex.is_null() || dst_buf.is_null() || src_size.width == 0 || src_size.height == 0 {
        return;
    }
    let src_snap = {
        let g = textures().lock().unwrap();
        g.get(&(src_tex as usize))
            .map(|t| (t.width, t.height, Arc::clone(&t.pixels)))
    };
    let Some((sw, sh, spixels)) = src_snap else {
        return;
    };
    let cols = src_size.width.min(sw.saturating_sub(src_origin.x));
    let rows = src_size.height.min(sh.saturating_sub(src_origin.y));
    let copy_bytes = cols * 4;
    if copy_bytes == 0 {
        return;
    }
    let dst_stride = if dst_bpr >= copy_bytes { dst_bpr } else { copy_bytes };
    let mut g = buffers().lock().unwrap();
    let Some(dst) = g.get_mut(&(dst_buf as usize)) else {
        return;
    };
    for r in 0..rows {
        let s_off = ((src_origin.y + r) * sw + src_origin.x) * 4;
        let d_off = dst_off.saturating_add(r * dst_stride);
        if let (Some(sr), Some(dr)) = (
            spixels.get(s_off..s_off + copy_bytes),
            dst.bytes.get_mut(d_off..d_off + copy_bytes),
        ) {
            dr.copy_from_slice(sr);
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
fn clone_prop_map<T: Clone>(m: &Mutex<HashMap<(usize, usize), T>>, src: usize, dst: usize) {
    let mut g = m.lock().unwrap();
    let cloned: Vec<(usize, T)> = g
        .iter()
        .filter(|&(&(obj, _), _)| obj == src)
        .map(|(&(_, k), v)| (k, v.clone()))
        .collect();
    for (k, v) in cloned {
        g.insert((dst, k), v);
    }
}
extern "C" fn desc_copy_with_zone(this: Id, _cmd: Sel, _zone: *mut c_void) -> Id {
    if this.is_null() {
        return std::ptr::null_mut();
    }
    let src = this as usize;
    let dst_id = unsafe {
        let cls = *(this as *const *const ClassT);
        alloc(cls)
    };
    let dst = dst_id as usize;
    clone_prop_map(props_usize(), src, dst);
    clone_prop_map(props_f64(), src, dst);
    clone_prop_map(props_color(), src, dst);
    {
        let mut g = props_id().lock().unwrap();
        let cloned: Vec<(usize, usize)> = g
            .iter()
            .filter(|&(&(obj, _), _)| obj == src)
            .map(|(&(_, k), &v)| (k, v))
            .collect();
        for (k, v) in cloned {
            if v != 0 {
                pear_libobjc::object::retain(v as Id);
            }
            g.insert((dst, k), v);
        }
    }
    dst_id
}
extern "C" fn desc_copy(this: Id, cmd: Sel) -> Id {
    desc_copy_with_zone(this, cmd, std::ptr::null_mut())
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
                ("newLibraryWithSource:options:completionHandler:", i(dev_new_library_with_source_async as *const ())),
                ("newDefaultLibrary", i(dev_new_default_library as *const ())),
                ("newDefaultLibraryWithBundle:error:", i(dev_new_default_library as *const ())),
                ("newRenderPipelineStateWithDescriptor:error:", i(dev_new_render_pipeline as *const ())),
                ("newRenderPipelineStateWithDescriptor:options:reflection:error:", i(dev_new_render_pipeline_opts_sync as *const ())),
                ("newRenderPipelineStateWithDescriptor:completionHandler:", i(dev_new_render_pipeline_async as *const ())),
                ("newRenderPipelineStateWithDescriptor:options:completionHandler:", i(dev_new_render_pipeline_opts_async as *const ())),
                ("newComputePipelineStateWithFunction:error:", i(dev_new_compute_pipeline as *const ())),
                ("newComputePipelineStateWithDescriptor:options:reflection:error:", i(dev_new_compute_pipeline_desc as *const ())),
                ("newComputePipelineStateWithFunction:completionHandler:", i(dev_new_compute_pipeline_async as *const ())),
                ("newComputePipelineStateWithFunction:options:completionHandler:", i(dev_new_compute_pipeline_opts_async as *const ())),
                ("newComputePipelineStateWithDescriptor:options:completionHandler:", i(dev_new_compute_pipeline_desc_async as *const ())),
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
                ("setRenderPipelineState:", i(enc_set_render_pipeline as *const ())),
                ("renderCommandEncoder", i(mtl_ret_self as *const ())),
                ("setDepthStencilState:", i(mtl_noop1 as *const ())),
                ("setStencilReferenceValue:", i(mtl_noop1 as *const ())),
                ("setCullMode:", i(mtl_noop1 as *const ())),
                ("setFrontFacingWinding:", i(mtl_noop1 as *const ())),
                ("setTriangleFillMode:", i(mtl_noop1 as *const ())),
                ("setDepthClipMode:", i(mtl_noop1 as *const ())),
                ("setDepthBias:slopeScale:clamp:", i(mtl_noop3 as *const ())),
                ("setViewport:", i(enc_set_viewport as *const ())),
                ("setViewports:count:", i(enc_set_viewports as *const ())),
                ("setScissorRect:", i(mtl_noop1 as *const ())),
                ("setScissorRects:count:", i(mtl_noop2 as *const ())),
                ("setBlendColorRed:green:blue:alpha:", i(mtl_noop4 as *const ())),
                ("setVertexBuffer:offset:atIndex:", i(enc_set_vertex_buffer as *const ())),
                ("setVertexBufferOffset:atIndex:", i(enc_set_vertex_buffer_offset as *const ())),
                ("setVertexBytes:length:atIndex:", i(enc_set_vertex_bytes as *const ())),
                ("setVertexTexture:atIndex:", i(mtl_noop2 as *const ())),
                ("setVertexSamplerState:atIndex:", i(mtl_noop2 as *const ())),
                ("setFragmentBuffer:offset:atIndex:", i(mtl_noop3 as *const ())),
                ("setFragmentBufferOffset:atIndex:", i(mtl_noop2 as *const ())),
                ("setFragmentBytes:length:atIndex:", i(mtl_noop3 as *const ())),
                ("setFragmentTexture:atIndex:", i(enc_set_fragment_texture as *const ())),
                ("setFragmentSamplerState:atIndex:", i(mtl_noop2 as *const ())),
                ("useResource:usage:", i(mtl_noop2 as *const ())),
                ("useHeap:", i(mtl_noop1 as *const ())),
                ("drawPrimitives:vertexStart:vertexCount:", i(enc_draw_primitives as *const ())),
                ("drawPrimitives:vertexStart:vertexCount:instanceCount:", i(enc_draw_primitives as *const ())),
                ("drawIndexedPrimitives:indexCount:indexType:indexBuffer:indexBufferOffset:", i(enc_draw_indexed as *const ())),
                ("drawIndexedPrimitives:indexCount:indexType:indexBuffer:indexBufferOffset:instanceCount:", i(enc_draw_indexed as *const ())),
                ("endEncoding", i(enc_end_encoding as *const ())),
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
                ("copyFromBuffer:sourceOffset:sourceBytesPerRow:sourceBytesPerImage:sourceSize:toTexture:destinationSlice:destinationLevel:destinationOrigin:", i(blit_copy_buffer_to_texture as *const ())),
                ("copyFromTexture:sourceSlice:sourceLevel:sourceOrigin:sourceSize:toTexture:destinationSlice:destinationLevel:destinationOrigin:", i(blit_copy_texture as *const ())),
                ("copyFromTexture:sourceSlice:sourceLevel:sourceOrigin:sourceSize:toBuffer:destinationOffset:destinationBytesPerRow:destinationBytesPerImage:", i(blit_copy_texture_to_buffer as *const ())),
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
                ("resourceOptions", i(prop_get_usize as *const ())),
                ("setResourceOptions:", i(prop_set_usize as *const ())),
                ("cpuCacheMode", i(prop_get_usize as *const ())),
                ("setCpuCacheMode:", i(prop_set_usize as *const ())),
                ("hazardTrackingMode", i(prop_get_usize as *const ())),
                ("setHazardTrackingMode:", i(prop_set_usize as *const ())),
                ("allowGPUOptimizedContents", i(prop_get_usize as *const ())),
                ("setAllowGPUOptimizedContents:", i(prop_set_usize as *const ())),
                ("copy", i(desc_copy as *const ())),
                ("copyWithZone:", i(desc_copy_with_zone as *const ())),
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
                ("isAlphaToCoverageEnabled", i(prop_get_bool_is as *const ())),
                ("setAlphaToCoverageEnabled:", i(prop_set_usize as *const ())),
                ("reset", i(mtl_noop0 as *const ())),
                ("copy", i(desc_copy as *const ())),
                ("copyWithZone:", i(desc_copy_with_zone as *const ())),
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
                ("copy", i(desc_copy as *const ())),
                ("copyWithZone:", i(desc_copy_with_zone as *const ())),
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
                ("isBlendingEnabled", i(prop_get_bool_is as *const ())),
                ("blendingEnabled", i(prop_get_usize as *const ())),
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
                ("copy", i(desc_copy as *const ())),
                ("copyWithZone:", i(desc_copy_with_zone as *const ())),
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
                ("isDepthWriteEnabled", i(prop_get_bool_is as *const ())),
                ("depthWriteEnabled", i(prop_get_usize as *const ())),
                ("setDepthWriteEnabled:", i(prop_set_usize as *const ())),
                ("frontFaceStencil", i(ds_front_face_stencil as *const ())),
                ("setFrontFaceStencil:", i(prop_set_id as *const ())),
                ("backFaceStencil", i(ds_back_face_stencil as *const ())),
                ("setBackFaceStencil:", i(prop_set_id as *const ())),
                ("label", i(prop_get_id as *const ())),
                ("setLabel:", i(prop_set_id as *const ())),
                ("copy", i(desc_copy as *const ())),
                ("copyWithZone:", i(desc_copy_with_zone as *const ())),
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
                ("copy", i(desc_copy as *const ())),
                ("copyWithZone:", i(desc_copy_with_zone as *const ())),
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
