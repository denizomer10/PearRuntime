//! Clean-room dynamic Vulkan 1.2/1.3 loader and X11 WSI wrapper.
//! Loads `libvulkan` via `dlopen` at runtime; degrades cleanly to `None` when
//! no Vulkan loader/ICD is present on the host.

use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void, CStr};
use std::sync::Mutex;

extern "C" {
    fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> c_int;
}

const VK_SUCCESS: i32 = 0;
const VK_SUBOPTIMAL_KHR: i32 = 1000001003;
const VK_STRUCTURE_TYPE_APPLICATION_INFO: u32 = 0;
const VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO: u32 = 1;
const VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO: u32 = 2;
const VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO: u32 = 3;
const VK_STRUCTURE_TYPE_SUBMIT_INFO: u32 = 4;
const VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO: u32 = 9;
const VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO: u32 = 16;
const VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO: u32 = 39;
const VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO: u32 = 40;
const VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO: u32 = 42;
const VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER: u32 = 45;
const VK_STRUCTURE_TYPE_SWAPCHAIN_CREATE_INFO_KHR: u32 = 1000001000;
const VK_STRUCTURE_TYPE_PRESENT_INFO_KHR: u32 = 1000001001;
const VK_STRUCTURE_TYPE_XLIB_SURFACE_CREATE_INFO_KHR: u32 = 1000004000;
const VK_QUEUE_GRAPHICS_BIT: u32 = 0x1;
const VK_FORMAT_B8G8R8A8_UNORM: u32 = 44;
const VK_IMAGE_USAGE_COLOR_TRANSFER_DST: u32 = 0x12; // COLOR_ATTACHMENT_BIT (0x10) | TRANSFER_DST_BIT (0x02)
const VK_IMAGE_LAYOUT_UNDEFINED: u32 = 0;
const VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL: u32 = 7;
const VK_IMAGE_LAYOUT_PRESENT_SRC_KHR: u32 = 1000001002;
const VK_ACCESS_TRANSFER_WRITE_BIT: u32 = 0x1000;
const VK_ACCESS_MEMORY_READ_BIT: u32 = 0x8000;
const VK_PIPELINE_STAGE_TOP_OF_PIPE_BIT: u32 = 0x1;
const VK_PIPELINE_STAGE_TRANSFER_BIT: u32 = 0x1000;
const VK_PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT: u32 = 0x2000;
const VK_PRESENT_MODE_FIFO_KHR: u32 = 2;
const VK_QUEUE_FAMILY_IGNORED: u32 = u32::MAX;

#[repr(C)]
struct VkApplicationInfo {
    s_type: u32,
    p_next: *const c_void,
    p_application_name: *const c_char,
    application_version: u32,
    p_engine_name: *const c_char,
    engine_version: u32,
    api_version: u32,
}

#[repr(C)]
struct VkInstanceCreateInfo {
    s_type: u32,
    p_next: *const c_void,
    flags: u32,
    p_application_info: *const VkApplicationInfo,
    enabled_layer_count: u32,
    pp_enabled_layer_names: *const *const c_char,
    enabled_extension_count: u32,
    pp_enabled_extension_names: *const *const c_char,
}

#[repr(C)]
struct VkDeviceQueueCreateInfo {
    s_type: u32,
    p_next: *const c_void,
    flags: u32,
    queue_family_index: u32,
    queue_count: u32,
    p_queue_priorities: *const f32,
}

#[repr(C)]
struct VkDeviceCreateInfo {
    s_type: u32,
    p_next: *const c_void,
    flags: u32,
    queue_create_info_count: u32,
    p_queue_create_infos: *const VkDeviceQueueCreateInfo,
    enabled_layer_count: u32,
    pp_enabled_layer_names: *const *const c_char,
    enabled_extension_count: u32,
    pp_enabled_extension_names: *const *const c_char,
    p_enabled_features: *const c_void,
}

#[repr(C)]
struct VkShaderModuleCreateInfo {
    s_type: u32,
    p_next: *const c_void,
    flags: u32,
    code_size: usize,
    p_code: *const u32,
}

#[repr(C)]
struct VkCommandPoolCreateInfo {
    s_type: u32,
    p_next: *const c_void,
    flags: u32,
    queue_family_index: u32,
}

#[repr(C)]
struct VkXlibSurfaceCreateInfoKHR {
    s_type: u32,
    p_next: *const c_void,
    flags: u32,
    dpy: *mut c_void,
    window: usize,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct VkQueueFamilyProperties {
    queue_flags: u32,
    queue_count: u32,
    timestamp_valid_bits: u32,
    min_image_transfer_granularity: [u32; 3],
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct VkExtent2D {
    width: u32,
    height: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct VkSurfaceCapabilitiesKHR {
    min_image_count: u32,
    max_image_count: u32,
    current_extent: VkExtent2D,
    min_image_extent: VkExtent2D,
    max_image_extent: VkExtent2D,
    max_image_array_layers: u32,
    supported_transforms: u32,
    current_transform: u32,
    supported_composite_alpha: u32,
    supported_usage_flags: u32,
}

#[repr(C)]
struct VkSwapchainCreateInfoKHR {
    s_type: u32,
    p_next: *const c_void,
    flags: u32,
    surface: u64,
    min_image_count: u32,
    image_format: u32,
    image_color_space: u32,
    image_extent: VkExtent2D,
    image_array_layers: u32,
    image_usage: u32,
    image_sharing_mode: u32,
    queue_family_index_count: u32,
    p_queue_family_indices: *const u32,
    pre_transform: u32,
    composite_alpha: u32,
    present_mode: u32,
    clipped: u32,
    old_swapchain: u64,
}

#[repr(C)]
struct VkSemaphoreCreateInfo {
    s_type: u32,
    p_next: *const c_void,
    flags: u32,
}

#[repr(C)]
struct VkCommandBufferAllocateInfo {
    s_type: u32,
    p_next: *const c_void,
    command_pool: u64,
    level: u32,
    command_buffer_count: u32,
}

#[repr(C)]
struct VkCommandBufferBeginInfo {
    s_type: u32,
    p_next: *const c_void,
    flags: u32,
    p_inheritance_info: *const c_void,
}

#[repr(C)]
#[derive(Copy, Clone)]
struct VkImageSubresourceRange {
    aspect_mask: u32,
    base_mip_level: u32,
    level_count: u32,
    base_array_layer: u32,
    layer_count: u32,
}

#[repr(C)]
struct VkImageMemoryBarrier {
    s_type: u32,
    p_next: *const c_void,
    src_access_mask: u32,
    dst_access_mask: u32,
    old_layout: u32,
    new_layout: u32,
    src_queue_family_index: u32,
    dst_queue_family_index: u32,
    image: u64,
    subresource_range: VkImageSubresourceRange,
}

#[repr(C)]
#[derive(Copy, Clone)]
union VkClearColorValue {
    float32: [f32; 4],
}

#[repr(C)]
struct VkSubmitInfo {
    s_type: u32,
    p_next: *const c_void,
    wait_semaphore_count: u32,
    p_wait_semaphores: *const u64,
    p_wait_dst_stage_mask: *const u32,
    command_buffer_count: u32,
    p_command_buffers: *const *mut c_void,
    signal_semaphore_count: u32,
    p_signal_semaphores: *const u64,
}

#[repr(C)]
struct VkPresentInfoKHR {
    s_type: u32,
    p_next: *const c_void,
    wait_semaphore_count: u32,
    p_wait_semaphores: *const u64,
    swapchain_count: u32,
    p_swapchains: *const u64,
    p_image_indices: *const u32,
    p_results: *mut i32,
}

struct VkWindowSwapchain {
    surface: u64,
    swapchain: u64,
    width: u32,
    height: u32,
    images: Vec<u64>,
    cmd_buf: *mut c_void,
    img_avail_sem: u64,
    render_done_sem: u64,
}

unsafe impl Send for VkWindowSwapchain {}
unsafe impl Sync for VkWindowSwapchain {}

pub struct VkContext {
    lib: *mut c_void,
    instance: *mut c_void,
    physical_device: *mut c_void,
    device: *mut c_void,
    queue: *mut c_void,
    q_idx: u32,
    command_pool: u64,
    device_name: String,
    swapchains: Mutex<HashMap<(usize, u64), VkWindowSwapchain>>,
    vk_destroy_device: Option<unsafe extern "C" fn(*mut c_void, *const c_void)>,
    vk_destroy_instance: Option<unsafe extern "C" fn(*mut c_void, *const c_void)>,
    vk_create_shader_module:
        Option<unsafe extern "C" fn(*mut c_void, *const VkShaderModuleCreateInfo, *const c_void, *mut u64) -> i32>,
    vk_destroy_shader_module: Option<unsafe extern "C" fn(*mut c_void, u64, *const c_void)>,
    vk_destroy_command_pool: Option<unsafe extern "C" fn(*mut c_void, u64, *const c_void)>,
    vk_create_xlib_surface:
        Option<unsafe extern "C" fn(*mut c_void, *const VkXlibSurfaceCreateInfoKHR, *const c_void, *mut u64) -> i32>,
    vk_destroy_surface: Option<unsafe extern "C" fn(*mut c_void, u64, *const c_void)>,
    vk_get_surface_caps:
        Option<unsafe extern "C" fn(*mut c_void, u64, *mut VkSurfaceCapabilitiesKHR) -> i32>,
    vk_create_swapchain:
        Option<unsafe extern "C" fn(*mut c_void, *const VkSwapchainCreateInfoKHR, *const c_void, *mut u64) -> i32>,
    vk_destroy_swapchain: Option<unsafe extern "C" fn(*mut c_void, u64, *const c_void)>,
    vk_get_swapchain_images:
        Option<unsafe extern "C" fn(*mut c_void, u64, *mut u32, *mut u64) -> i32>,
    vk_acquire_next_image:
        Option<unsafe extern "C" fn(*mut c_void, u64, u64, u64, u64, *mut u32) -> i32>,
    vk_queue_present: Option<unsafe extern "C" fn(*mut c_void, *const VkPresentInfoKHR) -> i32>,
    vk_alloc_command_buffers:
        Option<unsafe extern "C" fn(*mut c_void, *const VkCommandBufferAllocateInfo, *mut *mut c_void) -> i32>,
    vk_free_command_buffers: Option<unsafe extern "C" fn(*mut c_void, u64, u32, *const *mut c_void)>,
    vk_begin_command_buffer:
        Option<unsafe extern "C" fn(*mut c_void, *const VkCommandBufferBeginInfo) -> i32>,
    vk_end_command_buffer: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    vk_cmd_clear_color_image: Option<
        unsafe extern "C" fn(
            *mut c_void,
            u64,
            u32,
            *const VkClearColorValue,
            u32,
            *const VkImageSubresourceRange,
        ),
    >,
    vk_cmd_pipeline_barrier: Option<
        unsafe extern "C" fn(
            *mut c_void,
            u32,
            u32,
            u32,
            u32,
            *const c_void,
            u32,
            *const c_void,
            u32,
            *const VkImageMemoryBarrier,
        ),
    >,
    vk_queue_submit:
        Option<unsafe extern "C" fn(*mut c_void, u32, *const VkSubmitInfo, u64) -> i32>,
    vk_queue_wait_idle: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    vk_create_semaphore:
        Option<unsafe extern "C" fn(*mut c_void, *const VkSemaphoreCreateInfo, *const c_void, *mut u64) -> i32>,
    vk_destroy_semaphore: Option<unsafe extern "C" fn(*mut c_void, u64, *const c_void)>,
}

unsafe impl Send for VkContext {}
unsafe impl Sync for VkContext {}

impl VkContext {
    pub fn try_create() -> Option<Self> {
        unsafe {
            let mut lib = std::ptr::null_mut();
            for name in [
                c"libvulkan.so.1",
                c"libvulkan.so",
                c"libvulkan.1.dylib",
                c"libMoltenVK.dylib",
            ] {
                lib = dlopen(name.as_ptr(), 1);
                if !lib.is_null() {
                    break;
                }
            }
            if lib.is_null() {
                return None;
            }

            let get_inst_proc_raw = dlsym(lib, c"vkGetInstanceProcAddr".as_ptr());
            if get_inst_proc_raw.is_null() {
                dlclose(lib);
                return None;
            }
            let vk_get_instance_proc_addr: unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void =
                std::mem::transmute(get_inst_proc_raw);

            let create_inst_raw = vk_get_instance_proc_addr(std::ptr::null_mut(), c"vkCreateInstance".as_ptr());
            if create_inst_raw.is_null() {
                dlclose(lib);
                return None;
            }
            let vk_create_instance: unsafe extern "C" fn(*const VkInstanceCreateInfo, *const c_void, *mut *mut c_void) -> i32 =
                std::mem::transmute(create_inst_raw);

            let app_info = VkApplicationInfo {
                s_type: VK_STRUCTURE_TYPE_APPLICATION_INFO,
                p_next: std::ptr::null(),
                p_application_name: c"PearRuntime".as_ptr(),
                application_version: 1,
                p_engine_name: c"pear-metal-vk".as_ptr(),
                engine_version: 1,
                api_version: (1 << 22) | (2 << 12), // Vulkan 1.2
            };

            let mut instance: *mut c_void = std::ptr::null_mut();
            let exts = [c"VK_KHR_surface".as_ptr(), c"VK_KHR_xlib_surface".as_ptr()];
            let mut ci = VkInstanceCreateInfo {
                s_type: VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO,
                p_next: std::ptr::null(),
                flags: 0,
                p_application_info: &app_info,
                enabled_layer_count: 0,
                pp_enabled_layer_names: std::ptr::null(),
                enabled_extension_count: exts.len() as u32,
                pp_enabled_extension_names: exts.as_ptr(),
            };
            if vk_create_instance(&ci, std::ptr::null(), &mut instance) != VK_SUCCESS {
                ci.enabled_extension_count = 0;
                ci.pp_enabled_extension_names = std::ptr::null();
                if vk_create_instance(&ci, std::ptr::null(), &mut instance) != VK_SUCCESS || instance.is_null() {
                    dlclose(lib);
                    return None;
                }
            }

            let load_inst = |sym: &CStr| -> *mut c_void { vk_get_instance_proc_addr(instance, sym.as_ptr()) };
            let vk_destroy_instance: Option<unsafe extern "C" fn(*mut c_void, *const c_void)> =
                (!load_inst(c"vkDestroyInstance").is_null()).then(|| std::mem::transmute(load_inst(c"vkDestroyInstance")));
            let enum_raw = load_inst(c"vkEnumeratePhysicalDevices");
            let qfp_raw = load_inst(c"vkGetPhysicalDeviceQueueFamilyProperties");
            let props_raw = load_inst(c"vkGetPhysicalDeviceProperties");
            let cdev_raw = load_inst(c"vkCreateDevice");
            let dproc_raw = load_inst(c"vkGetDeviceProcAddr");
            if enum_raw.is_null() || qfp_raw.is_null() || cdev_raw.is_null() || dproc_raw.is_null() {
                if let Some(f) = vk_destroy_instance { f(instance, std::ptr::null()); }
                dlclose(lib);
                return None;
            }
            let vk_enum_devs: unsafe extern "C" fn(*mut c_void, *mut u32, *mut *mut c_void) -> i32 = std::mem::transmute(enum_raw);
            let vk_qfp: unsafe extern "C" fn(*mut c_void, *mut u32, *mut VkQueueFamilyProperties) = std::mem::transmute(qfp_raw);
            let vk_create_device: unsafe extern "C" fn(*mut c_void, *const VkDeviceCreateInfo, *const c_void, *mut *mut c_void) -> i32 = std::mem::transmute(cdev_raw);
            let vk_get_device_proc_addr: unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void = std::mem::transmute(dproc_raw);

            let mut count = 0u32;
            if vk_enum_devs(instance, &mut count, std::ptr::null_mut()) != VK_SUCCESS || count == 0 {
                if let Some(f) = vk_destroy_instance { f(instance, std::ptr::null()); }
                dlclose(lib);
                return None;
            }
            let mut devs = vec![std::ptr::null_mut(); count as usize];
            if vk_enum_devs(instance, &mut count, devs.as_mut_ptr()) != VK_SUCCESS {
                if let Some(f) = vk_destroy_instance { f(instance, std::ptr::null()); }
                dlclose(lib);
                return None;
            }
            let physical_device = devs[0];

            let mut device_name = "Vulkan Device".to_string();
            if !props_raw.is_null() {
                let vk_props: unsafe extern "C" fn(*mut c_void, *mut u8) = std::mem::transmute(props_raw);
                let mut raw_props = vec![0u8; 1024];
                vk_props(physical_device, raw_props.as_mut_ptr());
                // VkPhysicalDeviceProperties.deviceName sits at offset 20 (256 bytes).
                let name_slice = &raw_props[20..276];
                let nul = name_slice.iter().position(|&b| b == 0).unwrap_or(name_slice.len());
                if nul > 0 {
                    device_name = String::from_utf8_lossy(&name_slice[..nul]).into_owned();
                }
            }

            let mut q_count = 0u32;
            vk_qfp(physical_device, &mut q_count, std::ptr::null_mut());
            let mut qfps = vec![VkQueueFamilyProperties::default(); q_count as usize];
            if q_count > 0 {
                vk_qfp(physical_device, &mut q_count, qfps.as_mut_ptr());
            }
            let q_idx = qfps
                .iter()
                .position(|q| (q.queue_flags & VK_QUEUE_GRAPHICS_BIT) != 0)
                .unwrap_or(0) as u32;

            let prio = 1.0f32;
            let qci = VkDeviceQueueCreateInfo {
                s_type: VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO,
                p_next: std::ptr::null(),
                flags: 0,
                queue_family_index: q_idx,
                queue_count: 1,
                p_queue_priorities: &prio,
            };
            let dev_exts = [c"VK_KHR_swapchain".as_ptr()];
            let mut dci = VkDeviceCreateInfo {
                s_type: VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO,
                p_next: std::ptr::null(),
                flags: 0,
                queue_create_info_count: 1,
                p_queue_create_infos: &qci,
                enabled_layer_count: 0,
                pp_enabled_layer_names: std::ptr::null(),
                enabled_extension_count: dev_exts.len() as u32,
                pp_enabled_extension_names: dev_exts.as_ptr(),
                p_enabled_features: std::ptr::null(),
            };
            let mut device: *mut c_void = std::ptr::null_mut();
            if vk_create_device(physical_device, &dci, std::ptr::null(), &mut device) != VK_SUCCESS
                || device.is_null()
            {
                dci.enabled_extension_count = 0;
                dci.pp_enabled_extension_names = std::ptr::null();
                if vk_create_device(physical_device, &dci, std::ptr::null(), &mut device)
                    != VK_SUCCESS
                    || device.is_null()
                {
                    if let Some(f) = vk_destroy_instance {
                        f(instance, std::ptr::null());
                    }
                    dlclose(lib);
                    return None;
                }
            }

            let load_dev = |sym: &CStr| -> *mut c_void { vk_get_device_proc_addr(device, sym.as_ptr()) };
            macro_rules! vk_fn {
                ($loader:expr, $sym:expr) => {{
                    let p = $loader($sym);
                    (!p.is_null()).then(|| std::mem::transmute(p))
                }};
            }
            let vk_destroy_device: Option<unsafe extern "C" fn(*mut c_void, *const c_void)> =
                vk_fn!(load_dev, c"vkDestroyDevice");
            let vk_get_queue_raw = load_dev(c"vkGetDeviceQueue");
            let mut queue: *mut c_void = std::ptr::null_mut();
            if !vk_get_queue_raw.is_null() {
                let vk_get_queue: unsafe extern "C" fn(*mut c_void, u32, u32, *mut *mut c_void) =
                    std::mem::transmute(vk_get_queue_raw);
                vk_get_queue(device, q_idx, 0, &mut queue);
            }

            let mut command_pool = 0u64;
            let ccp_raw = load_dev(c"vkCreateCommandPool");
            if !ccp_raw.is_null() {
                let vk_create_cp: unsafe extern "C" fn(*mut c_void, *const VkCommandPoolCreateInfo, *const c_void, *mut u64) -> i32 =
                    std::mem::transmute(ccp_raw);
                let cpci = VkCommandPoolCreateInfo {
                    s_type: VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO,
                    p_next: std::ptr::null(),
                    flags: 0x2, // RESET_COMMAND_BUFFER_BIT
                    queue_family_index: q_idx,
                };
                let _ = vk_create_cp(device, &cpci, std::ptr::null(), &mut command_pool);
            }

            Some(Self {
                lib,
                instance,
                physical_device,
                device,
                queue,
                q_idx,
                command_pool,
                device_name,
                swapchains: Mutex::new(HashMap::new()),
                vk_destroy_device,
                vk_destroy_instance,
                vk_create_shader_module: vk_fn!(load_dev, c"vkCreateShaderModule"),
                vk_destroy_shader_module: vk_fn!(load_dev, c"vkDestroyShaderModule"),
                vk_destroy_command_pool: vk_fn!(load_dev, c"vkDestroyCommandPool"),
                vk_create_xlib_surface: vk_fn!(load_inst, c"vkCreateXlibSurfaceKHR"),
                vk_destroy_surface: vk_fn!(load_inst, c"vkDestroySurfaceKHR"),
                vk_get_surface_caps: vk_fn!(load_inst, c"vkGetPhysicalDeviceSurfaceCapabilitiesKHR"),
                vk_create_swapchain: vk_fn!(load_dev, c"vkCreateSwapchainKHR"),
                vk_destroy_swapchain: vk_fn!(load_dev, c"vkDestroySwapchainKHR"),
                vk_get_swapchain_images: vk_fn!(load_dev, c"vkGetSwapchainImagesKHR"),
                vk_acquire_next_image: vk_fn!(load_dev, c"vkAcquireNextImageKHR"),
                vk_queue_present: vk_fn!(load_dev, c"vkQueuePresentKHR"),
                vk_alloc_command_buffers: vk_fn!(load_dev, c"vkAllocateCommandBuffers"),
                vk_free_command_buffers: vk_fn!(load_dev, c"vkFreeCommandBuffers"),
                vk_begin_command_buffer: vk_fn!(load_dev, c"vkBeginCommandBuffer"),
                vk_end_command_buffer: vk_fn!(load_dev, c"vkEndCommandBuffer"),
                vk_cmd_clear_color_image: vk_fn!(load_dev, c"vkCmdClearColorImage"),
                vk_cmd_pipeline_barrier: vk_fn!(load_dev, c"vkCmdPipelineBarrier"),
                vk_queue_submit: vk_fn!(load_dev, c"vkQueueSubmit"),
                vk_queue_wait_idle: vk_fn!(load_dev, c"vkQueueWaitIdle"),
                vk_create_semaphore: vk_fn!(load_dev, c"vkCreateSemaphore"),
                vk_destroy_semaphore: vk_fn!(load_dev, c"vkDestroySemaphore"),
            })
        }
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    pub fn has_queue(&self) -> bool {
        !self.queue.is_null() && !self.physical_device.is_null()
    }

    pub fn create_shader_module(&self, spirv: &[u32]) -> Option<u64> {
        let f = self.vk_create_shader_module?;
        let smci = VkShaderModuleCreateInfo {
            s_type: VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO,
            p_next: std::ptr::null(),
            flags: 0,
            code_size: spirv.len() * 4,
            p_code: spirv.as_ptr(),
        };
        let mut handle = 0u64;
        let rc = unsafe { f(self.device, &smci, std::ptr::null(), &mut handle) };
        (rc == VK_SUCCESS && handle != 0).then_some(handle)
    }

    pub fn destroy_shader_module(&self, module: u64) {
        if module != 0 {
            if let Some(f) = self.vk_destroy_shader_module {
                unsafe { f(self.device, module, std::ptr::null()) };
            }
        }
    }

    pub fn create_xlib_surface(&self, dpy: *mut c_void, win: u64) -> Option<u64> {
        let f = self.vk_create_xlib_surface?;
        let sci = VkXlibSurfaceCreateInfoKHR {
            s_type: VK_STRUCTURE_TYPE_XLIB_SURFACE_CREATE_INFO_KHR,
            p_next: std::ptr::null(),
            flags: 0,
            dpy,
            window: win as usize,
        };
        let mut surf = 0u64;
        let rc = unsafe { f(self.instance, &sci, std::ptr::null(), &mut surf) };
        (rc == VK_SUCCESS && surf != 0).then_some(surf)
    }

    unsafe fn destroy_swapchain_entry(&self, entry: VkWindowSwapchain) {
        if let Some(f) = self.vk_queue_wait_idle {
            let _ = f(self.queue);
        }
        if !entry.cmd_buf.is_null() && self.command_pool != 0 {
            if let Some(f) = self.vk_free_command_buffers {
                f(self.device, self.command_pool, 1, &entry.cmd_buf);
            }
        }
        if let Some(f) = self.vk_destroy_semaphore {
            if entry.img_avail_sem != 0 {
                f(self.device, entry.img_avail_sem, std::ptr::null());
            }
            if entry.render_done_sem != 0 {
                f(self.device, entry.render_done_sem, std::ptr::null());
            }
        }
        if entry.swapchain != 0 {
            if let Some(f) = self.vk_destroy_swapchain {
                f(self.device, entry.swapchain, std::ptr::null());
            }
        }
        if entry.surface != 0 {
            if let Some(f) = self.vk_destroy_surface {
                f(self.instance, entry.surface, std::ptr::null());
            }
        }
    }

    unsafe fn create_window_swapchain(
        &self,
        dpy: *mut c_void,
        win: u64,
        req_w: u32,
        req_h: u32,
    ) -> Option<VkWindowSwapchain> {
        if self.queue.is_null() || self.command_pool == 0 {
            return None;
        }
        let get_caps = self.vk_get_surface_caps?;
        let create_sc = self.vk_create_swapchain?;
        let get_imgs = self.vk_get_swapchain_images?;
        let alloc_cb = self.vk_alloc_command_buffers?;
        let create_sem = self.vk_create_semaphore?;

        let surface = self.create_xlib_surface(dpy, win)?;
        let mut caps = VkSurfaceCapabilitiesKHR::default();
        if get_caps(self.physical_device, surface, &mut caps) != VK_SUCCESS {
            if let Some(f) = self.vk_destroy_surface {
                f(self.instance, surface, std::ptr::null());
            }
            return None;
        }
        let extent = if caps.current_extent.width != u32::MAX && caps.current_extent.width > 0 {
            caps.current_extent
        } else {
            let min_w = caps.min_image_extent.width.max(1);
            let min_h = caps.min_image_extent.height.max(1);
            let max_w = caps.max_image_extent.width.max(min_w);
            let max_h = caps.max_image_extent.height.max(min_h);
            VkExtent2D {
                width: req_w.max(1).clamp(min_w, max_w),
                height: req_h.max(1).clamp(min_h, max_h),
            }
        };
        let mut min_imgs = caps.min_image_count.max(2);
        if caps.max_image_count > 0 {
            min_imgs = min_imgs.min(caps.max_image_count);
        }
        let pre_transform = if caps.current_transform != 0 {
            caps.current_transform
        } else {
            1
        };
        let sci = VkSwapchainCreateInfoKHR {
            s_type: VK_STRUCTURE_TYPE_SWAPCHAIN_CREATE_INFO_KHR,
            p_next: std::ptr::null(),
            flags: 0,
            surface,
            min_image_count: min_imgs,
            image_format: VK_FORMAT_B8G8R8A8_UNORM,
            image_color_space: 0, // VK_COLOR_SPACE_SRGB_NONLINEAR_KHR
            image_extent: extent,
            image_array_layers: 1,
            image_usage: VK_IMAGE_USAGE_COLOR_TRANSFER_DST,
            image_sharing_mode: 0, // EXCLUSIVE
            queue_family_index_count: 1,
            p_queue_family_indices: &self.q_idx,
            pre_transform,
            composite_alpha: 1, // OPAQUE_BIT_KHR
            present_mode: VK_PRESENT_MODE_FIFO_KHR,
            clipped: 1,
            old_swapchain: 0,
        };
        let mut swapchain = 0u64;
        let mut ok = create_sc(self.device, &sci, std::ptr::null(), &mut swapchain) == VK_SUCCESS
            && swapchain != 0;
        let mut img_count = 0u32;
        if ok && get_imgs(self.device, swapchain, &mut img_count, std::ptr::null_mut()) == VK_SUCCESS && img_count > 0 {
        } else {
            ok = false;
        }
        let mut images = vec![0u64; img_count as usize];
        if ok && get_imgs(self.device, swapchain, &mut img_count, images.as_mut_ptr()) != VK_SUCCESS {
            ok = false;
        }
        let mut cmd_buf = std::ptr::null_mut();
        if ok {
            let cbai = VkCommandBufferAllocateInfo {
                s_type: VK_STRUCTURE_TYPE_COMMAND_BUFFER_ALLOCATE_INFO,
                p_next: std::ptr::null(),
                command_pool: self.command_pool,
                level: 0,
                command_buffer_count: 1,
            };
            if alloc_cb(self.device, &cbai, &mut cmd_buf) != VK_SUCCESS || cmd_buf.is_null() {
                ok = false;
            }
        }
        let sem_ci = VkSemaphoreCreateInfo {
            s_type: VK_STRUCTURE_TYPE_SEMAPHORE_CREATE_INFO,
            p_next: std::ptr::null(),
            flags: 0,
        };
        let mut img_avail_sem = 0u64;
        let mut render_done_sem = 0u64;
        if ok
            && (create_sem(self.device, &sem_ci, std::ptr::null(), &mut img_avail_sem) != VK_SUCCESS
                || create_sem(self.device, &sem_ci, std::ptr::null(), &mut render_done_sem) != VK_SUCCESS)
        {
            ok = false;
        }
        let entry = VkWindowSwapchain {
            surface,
            swapchain,
            width: extent.width,
            height: extent.height,
            images,
            cmd_buf,
            img_avail_sem,
            render_done_sem,
        };
        if !ok {
            self.destroy_swapchain_entry(entry);
            return None;
        }
        Some(entry)
    }

    pub fn present_xlib_clear(
        &self,
        dpy: *mut c_void,
        win: u64,
        width: u32,
        height: u32,
        rgba: [f32; 4],
    ) -> bool {
        if dpy.is_null() || win == 0 || self.queue.is_null() {
            return false;
        }
        let (Some(acquire), Some(begin_cb), Some(end_cb), Some(clear_img), Some(barrier), Some(submit), Some(present), Some(wait_idle)) = (
            self.vk_acquire_next_image,
            self.vk_begin_command_buffer,
            self.vk_end_command_buffer,
            self.vk_cmd_clear_color_image,
            self.vk_cmd_pipeline_barrier,
            self.vk_queue_submit,
            self.vk_queue_present,
            self.vk_queue_wait_idle,
        ) else {
            return false;
        };

        let key = (dpy as usize, win);
        let mut guard = self.swapchains.lock().unwrap();
        let need_recreate = guard
            .get(&key)
            .is_none_or(|s| s.width != width.max(1) || s.height != height.max(1));
        if need_recreate {
            if let Some(old) = guard.remove(&key) {
                unsafe { self.destroy_swapchain_entry(old) };
            }
            let Some(fresh) = (unsafe { self.create_window_swapchain(dpy, win, width, height) }) else {
                return false;
            };
            guard.insert(key, fresh);
        }
        let Some(sc) = guard.get(&key) else {
            return false;
        };
        unsafe {
            let mut img_idx = 0u32;
            let acq_rc = acquire(
                self.device,
                sc.swapchain,
                u64::MAX,
                sc.img_avail_sem,
                0,
                &mut img_idx,
            );
            if acq_rc != VK_SUCCESS && acq_rc != VK_SUBOPTIMAL_KHR {
                if let Some(old) = guard.remove(&key) {
                    self.destroy_swapchain_entry(old);
                }
                return false;
            }
            let Some(&image) = sc.images.get(img_idx as usize) else {
                return false;
            };
            let bi = VkCommandBufferBeginInfo {
                s_type: VK_STRUCTURE_TYPE_COMMAND_BUFFER_BEGIN_INFO,
                p_next: std::ptr::null(),
                flags: 0x1, // ONE_TIME_SUBMIT_BIT
                p_inheritance_info: std::ptr::null(),
            };
            if begin_cb(sc.cmd_buf, &bi) != VK_SUCCESS {
                return false;
            }
            let range = VkImageSubresourceRange {
                aspect_mask: 0x1, // COLOR_BIT
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            };
            let to_dst = VkImageMemoryBarrier {
                s_type: VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER,
                p_next: std::ptr::null(),
                src_access_mask: 0,
                dst_access_mask: VK_ACCESS_TRANSFER_WRITE_BIT,
                old_layout: VK_IMAGE_LAYOUT_UNDEFINED,
                new_layout: VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,
                src_queue_family_index: VK_QUEUE_FAMILY_IGNORED,
                dst_queue_family_index: VK_QUEUE_FAMILY_IGNORED,
                image,
                subresource_range: range,
            };
            barrier(
                sc.cmd_buf,
                VK_PIPELINE_STAGE_TOP_OF_PIPE_BIT,
                VK_PIPELINE_STAGE_TRANSFER_BIT,
                0,
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                1,
                &to_dst,
            );
            let clear = VkClearColorValue { float32: rgba };
            clear_img(
                sc.cmd_buf,
                image,
                VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,
                &clear,
                1,
                &range,
            );
            let to_present = VkImageMemoryBarrier {
                s_type: VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER,
                p_next: std::ptr::null(),
                src_access_mask: VK_ACCESS_TRANSFER_WRITE_BIT,
                dst_access_mask: VK_ACCESS_MEMORY_READ_BIT,
                old_layout: VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,
                new_layout: VK_IMAGE_LAYOUT_PRESENT_SRC_KHR,
                src_queue_family_index: VK_QUEUE_FAMILY_IGNORED,
                dst_queue_family_index: VK_QUEUE_FAMILY_IGNORED,
                image,
                subresource_range: range,
            };
            barrier(
                sc.cmd_buf,
                VK_PIPELINE_STAGE_TRANSFER_BIT,
                VK_PIPELINE_STAGE_BOTTOM_OF_PIPE_BIT,
                0,
                0,
                std::ptr::null(),
                0,
                std::ptr::null(),
                1,
                &to_present,
            );
            if end_cb(sc.cmd_buf) != VK_SUCCESS {
                return false;
            }
            let wait_stage = VK_PIPELINE_STAGE_TRANSFER_BIT;
            let si = VkSubmitInfo {
                s_type: VK_STRUCTURE_TYPE_SUBMIT_INFO,
                p_next: std::ptr::null(),
                wait_semaphore_count: 1,
                p_wait_semaphores: &sc.img_avail_sem,
                p_wait_dst_stage_mask: &wait_stage,
                command_buffer_count: 1,
                p_command_buffers: &sc.cmd_buf,
                signal_semaphore_count: 1,
                p_signal_semaphores: &sc.render_done_sem,
            };
            if submit(self.queue, 1, &si, 0) != VK_SUCCESS {
                return false;
            }
            let pi = VkPresentInfoKHR {
                s_type: VK_STRUCTURE_TYPE_PRESENT_INFO_KHR,
                p_next: std::ptr::null(),
                wait_semaphore_count: 1,
                p_wait_semaphores: &sc.render_done_sem,
                swapchain_count: 1,
                p_swapchains: &sc.swapchain,
                p_image_indices: &img_idx,
                p_results: std::ptr::null_mut(),
            };
            let pres_rc = present(self.queue, &pi);
            let _ = wait_idle(self.queue);
            if pres_rc != VK_SUCCESS && pres_rc != VK_SUBOPTIMAL_KHR {
                if let Some(old) = guard.remove(&key) {
                    self.destroy_swapchain_entry(old);
                }
                return false;
            }
            true
        }
    }
}

impl Drop for VkContext {
    fn drop(&mut self) {
        unsafe {
            let drained: Vec<VkWindowSwapchain> =
                self.swapchains.lock().unwrap().drain().map(|(_, v)| v).collect();
            for entry in drained {
                self.destroy_swapchain_entry(entry);
            }
            if self.command_pool != 0 {
                if let Some(f) = self.vk_destroy_command_pool {
                    f(self.device, self.command_pool, std::ptr::null());
                }
            }
            if !self.device.is_null() {
                if let Some(f) = self.vk_destroy_device {
                    f(self.device, std::ptr::null());
                }
            }
            if !self.instance.is_null() {
                if let Some(f) = self.vk_destroy_instance {
                    f(self.instance, std::ptr::null());
                }
            }
            if !self.lib.is_null() {
                dlclose(self.lib);
            }
        }
    }
}
