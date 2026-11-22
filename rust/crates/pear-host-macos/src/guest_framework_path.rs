//! Resolve iOS framework paths used by dynamic imports to the same prepared
//! frameworks as the guest's rewritten Mach-O load commands.

use std::ffi::{c_char, c_int, c_void, CStr, CString, OsStr};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::sync::OnceLock;

#[repr(C)]
struct DlInfo {
    filename: *const c_char,
    base: *mut c_void,
    symbol: *const c_char,
    address: *mut c_void,
}

extern "C" {
    fn dladdr(address: *const c_void, info: *mut DlInfo) -> c_int;
}

fn bundled_framework(cache: &'static OnceLock<Option<CString>>, relative: &str) -> Option<&'static CStr> {
    cache.get_or_init(|| unsafe {
        let mut info: DlInfo = std::mem::zeroed();
        if dladdr(pear_guest_framework_path as *const c_void, &mut info) == 0 || info.filename.is_null() {
            return None;
        }
        let image = Path::new(OsStr::from_bytes(CStr::from_ptr(info.filename).to_bytes()));
        let framework = image.parent()?.parent()?.join(relative);
        if !framework.is_file() {
            return None;
        }
        CString::new(framework.as_os_str().as_bytes()).ok()
    }).as_deref()
}

#[no_mangle]
unsafe extern "C" fn pear_guest_framework_path(path: *const c_char) -> *const c_char {
    static UIKIT: OnceLock<Option<CString>> = OnceLock::new();
    static GLES: OnceLock<Option<CString>> = OnceLock::new();
    if path.is_null() {
        return path;
    }
    let framework = match CStr::from_ptr(path).to_bytes() {
        b"/System/Library/Frameworks/UIKit.framework/UIKit"
        | b"/System/iOSSupport/System/Library/Frameworks/UIKit.framework/UIKit" => {
            bundled_framework(&UIKIT, "UIKit.framework/UIKit")
        }
        b"/System/Library/Frameworks/OpenGLES.framework/OpenGLES"
        | b"/System/iOSSupport/System/Library/Frameworks/OpenGLES.framework/OpenGLES" => {
            bundled_framework(&GLES, "OpenGLES.framework/OpenGLES")
        }
        _ => None,
    };
    framework.map_or(path, CStr::as_ptr)
}
