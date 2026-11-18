//! Keeps an app's data out of the real user folders.
//!
//! An unsandboxed iOS app on macOS resolves NSDocumentDirectory to
//! ~/Documents and so on, scattering saves across the user's own folders.
//! Home-relative directories go to a per-app container instead:
//!
//!   ~/Library/Application Support/PearSilicon/Games/<bundleID>/
//!     Documents/  Library/  Library/Caches/  Library/Group Containers/  tmp/
//!
//! Covered: the NSFileManager directory lookups, NSSearchPathForDirectoriesInDomains,
//! security group containers, the temporary directory and NSHomeDirectory.
//! Disable with PEAR_NO_PATH_SHIM; override the container with PEAR_HOME_DIR.

use crate::rt::{self, Interpose};
use pear_objc::{cls, msg0, msg2, nsarray_from_ids, nsstring, objc_retain, sel, Id, Imp, Sel};
use std::ffi::c_void;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::OnceLock;

const DOCUMENT: usize = 9;
const LIBRARY: usize = 5;
const APPLICATION_SUPPORT: usize = 14;
const CACHES: usize = 13;
const USER: usize = 7;
const USER_DOMAIN: usize = 1;

extern "C" {
    fn NSSearchPathForDirectoriesInDomains(directory: usize, domain: usize, expand_tilde: bool) -> Id;
    fn NSHomeDirectory() -> Id;
}

/// The per-app container root (retained NSString), or None when disabled.
fn root() -> Option<Id> {
    static ROOT: OnceLock<usize> = OnceLock::new();
    let r = *ROOT.get_or_init(|| unsafe {
        if rt::env_set("PEAR_NO_PATH_SHIM") {
            return 0;
        }
        if let Ok(dir) = std::env::var("PEAR_HOME_DIR") {
            if !dir.is_empty() {
                return objc_retain(nsstring(&dir)) as usize;
            }
        }
        let bundle_id: Id = msg0(msg0::<Id>(cls("NSBundle"), "mainBundle"), "bundleIdentifier");
        let bundle_id = if bundle_id.is_null() { "unknown".to_string() } else { rt::ns_string(bundle_id) };
        let path = match std::env::var("PEAR_SUPPORT_ROOT") {
            Ok(r) if !r.is_empty() => format!("{}/Games/{bundle_id}", r.trim_end_matches('/')),
            _ => {
                // The real home, from the environment: NSHomeDirectory is what is redirected.
                let home = std::env::var("HOME").ok().filter(|h| !h.is_empty()).unwrap_or_else(|| "/tmp".into());
                format!("{}/Library/Application Support/PearSilicon/Games/{bundle_id}", home.trim_end_matches('/'))
            }
        };
        objc_retain(nsstring(&path)) as usize
    });
    (r != 0).then_some(r as Id)
}

fn root_string() -> Option<String> {
    root().map(|r| unsafe { rt::ns_string(r) })
}

fn subpath(directory: usize) -> Option<&'static str> {
    match directory {
        DOCUMENT => Some("Documents"),
        LIBRARY => Some("Library"),
        APPLICATION_SUPPORT => Some("Library/Application Support"),
        CACHES => Some("Library/Caches"),
        USER => Some(""),
        _ => None,
    }
}

fn join(base: &str, sub: &str) -> String {
    if sub.is_empty() {
        base.to_string()
    } else {
        format!("{}/{sub}", base.trim_end_matches('/'))
    }
}

fn create_dir(path: &str) {
    let _ = std::fs::create_dir_all(path);
}

/// A per-app group container: a locally prepared app has no signed group
/// entitlement, so the real path cannot be created; the same API surface
/// lives inside the app's own container.
fn group_container_path(identifier: &str, create: bool) -> Option<String> {
    let root = root_string()?;
    if identifier.is_empty() {
        return None;
    }
    // Keep malformed identifiers from escaping the container.
    let safe = identifier.replace('/', "_").replace("..", "_");
    let path = join(&root, &format!("Library/Group Containers/{safe}"));
    if create {
        create_dir(&path);
    }
    Some(path)
}

fn shimmed_path(directory: usize, domain: usize, create: bool) -> Option<String> {
    let root = root_string()?;
    if domain != USER_DOMAIN {
        return None; // only the user domain lives under home
    }
    let path = join(&root, subpath(directory)?);
    if create {
        create_dir(&path);
    }
    Some(path)
}

/// A directory relative to an existing shimmed URL (a group container).
unsafe fn shimmed_path_under(base: Id, directory: usize, create: bool) -> Option<String> {
    let root = root_string()?;
    if base.is_null() || !msg0::<bool>(base, "isFileURL") {
        return None;
    }
    let base_path = rt::ns_string(msg0(base, "path"));
    if base_path != root && !base_path.starts_with(&format!("{root}/")) {
        return None;
    }
    let path = join(&base_path, subpath(directory)?);
    if create {
        create_dir(&path);
    }
    Some(path)
}

unsafe fn dir_url(path: &str) -> Id {
    msg2(cls("NSURL"), "fileURLWithPath:isDirectory:", nsstring(path), true)
}

static URL_FOR_DIRECTORY: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static GROUP_CONTAINER: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static HOME_FOR_USER: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static TEMPORARY: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static URLS_FOR_DIRECTORY: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

unsafe extern "C" fn url_for_directory(s: Id, c: Sel, directory: usize, domain: usize, url: Id, create: bool, error: *mut Id) -> Id {
    if let Some(p) = shimmed_path_under(url, directory, create).or_else(|| shimmed_path(directory, domain, create)) {
        return dir_url(&p);
    }
    let f: unsafe extern "C" fn(Id, Sel, usize, usize, Id, bool, *mut Id) -> Id = std::mem::transmute(URL_FOR_DIRECTORY.load(Ordering::Acquire));
    f(s, c, directory, domain, url, create, error)
}

unsafe extern "C" fn group_container(s: Id, c: Sel, identifier: Id) -> Id {
    if let Some(p) = group_container_path(&rt::ns_string(identifier), true) {
        return dir_url(&p);
    }
    let f: unsafe extern "C" fn(Id, Sel, Id) -> Id = std::mem::transmute(GROUP_CONTAINER.load(Ordering::Acquire));
    f(s, c, identifier)
}

unsafe extern "C" fn home_for_user(s: Id, c: Sel) -> Id {
    if let Some(r) = root_string() {
        return dir_url(&r);
    }
    let f: unsafe extern "C" fn(Id, Sel) -> Id = std::mem::transmute(HOME_FOR_USER.load(Ordering::Acquire));
    f(s, c)
}

unsafe extern "C" fn temporary_directory(s: Id, c: Sel) -> Id {
    if let Some(r) = root_string() {
        let path = join(&r, "tmp");
        create_dir(&path);
        return dir_url(&path);
    }
    let f: unsafe extern "C" fn(Id, Sel) -> Id = std::mem::transmute(TEMPORARY.load(Ordering::Acquire));
    f(s, c)
}

unsafe extern "C" fn urls_for_directory(s: Id, c: Sel, directory: usize, domain: usize) -> Id {
    if let Some(p) = shimmed_path(directory, domain, false) {
        return nsarray_from_ids(&[dir_url(&p)]);
    }
    let f: unsafe extern "C" fn(Id, Sel, usize, usize) -> Id = std::mem::transmute(URLS_FOR_DIRECTORY.load(Ordering::Acquire));
    f(s, c, directory, domain)
}

// dyld interposing: every image but this one sees the replacements, so the
// originals are called directly below.
unsafe extern "C" fn pear_search_path(directory: usize, domain: usize, expand_tilde: bool) -> Id {
    if let Some(p) = shimmed_path(directory, domain, expand_tilde) {
        return nsarray_from_ids(&[nsstring(&p)]);
    }
    NSSearchPathForDirectoriesInDomains(directory, domain, expand_tilde)
}

unsafe extern "C" fn pear_home_directory() -> Id {
    match root() {
        Some(r) => r,
        None => NSHomeDirectory(),
    }
}

#[used]
#[link_section = "__DATA,__interpose"]
static SEARCH_PATH_INTERPOSE: Interpose = Interpose {
    replacement: pear_search_path as *const c_void,
    replacee: NSSearchPathForDirectoriesInDomains as *const c_void,
};

#[used]
#[link_section = "__DATA,__interpose"]
static HOME_INTERPOSE: Interpose = Interpose {
    replacement: pear_home_directory as *const c_void,
    replacee: NSHomeDirectory as *const c_void,
};

unsafe fn hook(selector: &str, imp: Imp, slot: &AtomicPtr<c_void>) {
    let m = rt::class_getInstanceMethod(cls("NSFileManager"), sel(selector));
    if !m.is_null() {
        slot.store(rt::method_getImplementation(m) as *mut c_void, Ordering::Release);
        rt::method_setImplementation(m, imp);
    }
}

pub fn init() {
    if !rt::module_enabled("storage") {
        return;
    }
    let Some(r) = root_string() else { return };
    unsafe {
        hook("URLForDirectory:inDomain:appropriateForURL:create:error:", url_for_directory as Imp, &URL_FOR_DIRECTORY);
        hook("containerURLForSecurityApplicationGroupIdentifier:", group_container as Imp, &GROUP_CONTAINER);
        hook("homeDirectoryForCurrentUser", home_for_user as Imp, &HOME_FOR_USER);
        hook("temporaryDirectory", temporary_directory as Imp, &TEMPORARY);
        hook("URLsForDirectory:inDomains:", urls_for_directory as Imp, &URLS_FOR_DIRECTORY);
    }
    rt::log(&format!("[PearRuntime][paths] app data contained under {r}"));
}
