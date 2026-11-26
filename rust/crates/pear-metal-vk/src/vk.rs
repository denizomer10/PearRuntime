//! Clean-room dynamic Vulkan 1.2/1.3 loader and X11 WSI wrapper.
//! Loads `libvulkan` via `dlopen` at runtime; degrades cleanly to `None` when
//! no Vulkan loader/ICD is present on the host.

use std::ffi::{c_char, c_int, c_void, CStr};

extern "C" {
    fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> c_int;
}

const VK_SUCCESS: i32 = 0;
const VK_STRUCTURE_TYPE_APPLICATION_INFO: u32 = 0;
const VK_STRUCTURE_TYPE_INSTANCE_CREATE_INFO: u32 = 1;
const VK_STRUCTURE_TYPE_DEVICE_QUEUE_CREATE_INFO: u32 = 2;
const VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO: u32 = 3;
const VK_STRUCTURE_TYPE_SHADER_MODULE_CREATE_INFO: u32 = 16;
const VK_STRUCTURE_TYPE_COMMAND_POOL_CREATE_INFO: u32 = 39;
const VK_STRUCTURE_TYPE_XLIB_SURFACE_CREATE_INFO_KHR: u32 = 1000004000;
const VK_QUEUE_GRAPHICS_BIT: u32 = 0x1;

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

pub struct VkContext {
    lib: *mut c_void,
    instance: *mut c_void,
    physical_device: *mut c_void,
    device: *mut c_void,
    queue: *mut c_void,
    command_pool: u64,
    device_name: String,
    vk_destroy_device: Option<unsafe extern "C" fn(*mut c_void, *const c_void)>,
    vk_destroy_instance: Option<unsafe extern "C" fn(*mut c_void, *const c_void)>,
    vk_create_shader_module:
        Option<unsafe extern "C" fn(*mut c_void, *const VkShaderModuleCreateInfo, *const c_void, *mut u64) -> i32>,
    vk_destroy_shader_module: Option<unsafe extern "C" fn(*mut c_void, u64, *const c_void)>,
    vk_destroy_command_pool: Option<unsafe extern "C" fn(*mut c_void, u64, *const c_void)>,
    vk_create_xlib_surface:
        Option<unsafe extern "C" fn(*mut c_void, *const VkXlibSurfaceCreateInfoKHR, *const c_void, *mut u64) -> i32>,
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
            let dci = VkDeviceCreateInfo {
                s_type: VK_STRUCTURE_TYPE_DEVICE_CREATE_INFO,
                p_next: std::ptr::null(),
                flags: 0,
                queue_create_info_count: 1,
                p_queue_create_infos: &qci,
                enabled_layer_count: 0,
                pp_enabled_layer_names: std::ptr::null(),
                enabled_extension_count: 0,
                pp_enabled_extension_names: std::ptr::null(),
                p_enabled_features: std::ptr::null(),
            };
            let mut device: *mut c_void = std::ptr::null_mut();
            if vk_create_device(physical_device, &dci, std::ptr::null(), &mut device) != VK_SUCCESS || device.is_null() {
                if let Some(f) = vk_destroy_instance { f(instance, std::ptr::null()); }
                dlclose(lib);
                return None;
            }

            let load_dev = |sym: &CStr| -> *mut c_void { vk_get_device_proc_addr(device, sym.as_ptr()) };
            let vk_destroy_device: Option<unsafe extern "C" fn(*mut c_void, *const c_void)> =
                (!load_dev(c"vkDestroyDevice").is_null()).then(|| std::mem::transmute(load_dev(c"vkDestroyDevice")));
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

            let vk_create_shader_module = (!load_dev(c"vkCreateShaderModule").is_null())
                .then(|| std::mem::transmute(load_dev(c"vkCreateShaderModule")));
            let vk_destroy_shader_module = (!load_dev(c"vkDestroyShaderModule").is_null())
                .then(|| std::mem::transmute(load_dev(c"vkDestroyShaderModule")));
            let vk_destroy_command_pool = (!load_dev(c"vkDestroyCommandPool").is_null())
                .then(|| std::mem::transmute(load_dev(c"vkDestroyCommandPool")));
            let xlib_raw = load_inst(c"vkCreateXlibSurfaceKHR");
            let vk_create_xlib_surface = (!xlib_raw.is_null()).then(|| std::mem::transmute(xlib_raw));

            Some(Self {
                lib,
                instance,
                physical_device,
                device,
                queue,
                command_pool,
                device_name,
                vk_destroy_device,
                vk_destroy_instance,
                vk_create_shader_module,
                vk_destroy_shader_module,
                vk_destroy_command_pool,
                vk_create_xlib_surface,
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
}

impl Drop for VkContext {
    fn drop(&mut self) {
        unsafe {
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
