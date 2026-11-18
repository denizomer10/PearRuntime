//! Skips a consent-management SDK in Unity apps that ship it.
//!
//! The SDK's Swift consent UI is built for the iOS Swift runtime; under the
//! iOS-on-Mac runtime its metadata accessor returns an invalid pointer and the
//! app dies with SIGBUS while configuring the banner. The app only needs a
//! consent decision, so the C# entry points that create/show the banner become
//! no-ops and `Initialize` reports "no consent required" to the app's callback.
//! Classes and methods are discovered at run time through the IL2CPP API (the
//! names are the SDK's public metadata; nothing app-specific is hard-coded).
//! Opt out with PEAR_NO_CONSENT_SHIM=1; PEAR_DEBUG_CONSENT=1 logs the walk.

use crate::rt;
use pear_objc::{cls, msg0, msg1, msg2, nsstring, Id};
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

type P = *mut c_void;

/// The IL2CPP exports the shim uses (resolved once UnityFramework is loaded).
#[derive(Clone, Copy)]
struct Api {
    get_corlib: unsafe extern "C" fn() -> P,
    domain_get: unsafe extern "C" fn() -> P,
    thread_attach: unsafe extern "C" fn(P) -> P,
    class_for_each: unsafe extern "C" fn(unsafe extern "C" fn(P, P), P),
    class_get_image: unsafe extern "C" fn(P) -> P,
    image_get_name: Option<unsafe extern "C" fn(P) -> *const c_char>,
    class_is_generic: unsafe extern "C" fn(P) -> bool,
    class_is_inflated: unsafe extern "C" fn(P) -> bool,
    image_get_class_count: unsafe extern "C" fn(P) -> usize,
    image_get_class: unsafe extern "C" fn(P, usize) -> P,
    class_get_name: unsafe extern "C" fn(P) -> *const c_char,
    class_get_namespace: unsafe extern "C" fn(P) -> *const c_char,
    class_get_methods: unsafe extern "C" fn(P, *mut P) -> P,
    class_get_fields: unsafe extern "C" fn(P, *mut P) -> P,
    method_get_name: unsafe extern "C" fn(P) -> *const c_char,
    method_get_param_count: unsafe extern "C" fn(P) -> u32,
    method_is_instance: Option<unsafe extern "C" fn(P) -> bool>,
    method_get_return_type: unsafe extern "C" fn(P) -> P,
    type_get_type: unsafe extern "C" fn(P) -> c_int,
    field_get_name: unsafe extern "C" fn(P) -> *const c_char,
    field_get_type: Option<unsafe extern "C" fn(P) -> P>,
    field_get_value: unsafe extern "C" fn(P, P, P),
    class_from_type: unsafe extern "C" fn(P) -> P,
    object_new: unsafe extern "C" fn(P) -> P,
    field_set_value: unsafe extern "C" fn(P, P, P),
    runtime_invoke: unsafe extern "C" fn(P, P, *mut P, *mut P) -> P,
    string_new_wrapper: unsafe extern "C" fn(*const c_char) -> P,
}

/// Metadata found by the scan, used by the Initialize replacement.
struct Meta {
    status_class: P,
    status_ctor: P,
    on_initialized_invoke: P,
    should_collect: P,
    banner_required: P,
    active_settings: P,
    country: P,
    region: P,
    service_consents: P,
    privacy_on_initialized: P,
}
unsafe impl Send for Meta {}

static API: OnceLock<Api> = OnceLock::new();
static META: Mutex<Meta> = Mutex::new(Meta {
    status_class: std::ptr::null_mut(),
    status_ctor: std::ptr::null_mut(),
    on_initialized_invoke: std::ptr::null_mut(),
    should_collect: std::ptr::null_mut(),
    banner_required: std::ptr::null_mut(),
    active_settings: std::ptr::null_mut(),
    country: std::ptr::null_mut(),
    region: std::ptr::null_mut(),
    service_consents: std::ptr::null_mut(),
    privacy_on_initialized: std::ptr::null_mut(),
});
static INSTALLED: AtomicBool = AtomicBool::new(false);
static DEBUG: AtomicBool = AtomicBool::new(false);
static LAST_SCAN: AtomicU64 = AtomicU64::new(0); // f64 bits
static LOGGED_RESOLVE_FAILURE: AtomicBool = AtomicBool::new(false);
static LOGGED_MISSING_EXPORTS: AtomicBool = AtomicBool::new(false);
static LOGGED_DOMAIN: AtomicBool = AtomicBool::new(false);
static LOGGED_ASSEMBLY: AtomicBool = AtomicBool::new(false);
static LOGGED_NO_MATCH: AtomicBool = AtomicBool::new(false);
static LOGGED_IMAGES: AtomicBool = AtomicBool::new(false);
static LOGGED_CORLIB: AtomicBool = AtomicBool::new(false);
static UNITY: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

extern "C" {
    fn dlopen(path: *const c_char, mode: c_int) -> P;
    fn dlsym(handle: P, symbol: *const c_char) -> P;
    fn CFAbsoluteTimeGetCurrent() -> f64;
}
const RTLD_LAZY: c_int = 1;
const RTLD_NOLOAD: c_int = 0x10;
const RTLD_DEFAULT: P = -2isize as P;

fn debug() -> bool {
    DEBUG.load(Ordering::Relaxed)
}

/// UnityFramework is not always in the global symbol scope; RTLD_NOLOAD reuses
/// the copy the app loaded instead of loading a second one.
unsafe fn unity_handle() -> P {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let frameworks: Id = msg0(msg0::<Id>(cls("NSBundle"), "mainBundle"), "privateFrameworksPath");
        if frameworks.is_null() {
            return;
        }
        let path = rt::path_join(&rt::ns_string(frameworks), "UnityFramework.framework/UnityFramework");
        let Ok(c) = CString::new(path) else { return };
        let mut h = dlopen(c.as_ptr(), RTLD_LAZY | RTLD_NOLOAD);
        if h.is_null() {
            h = dlopen(c.as_ptr(), RTLD_LAZY);
        }
        UNITY.store(h, Ordering::Release);
    });
    UNITY.load(Ordering::Acquire)
}

unsafe fn symbol(name: &CStr) -> P {
    let h = unity_handle();
    let s = if h.is_null() { std::ptr::null_mut() } else { dlsym(h, name.as_ptr()) };
    if s.is_null() { dlsym(RTLD_DEFAULT, name.as_ptr()) } else { s }
}

unsafe fn resolve() -> Option<Api> {
    if let Some(api) = API.get() {
        return Some(*api);
    }
    macro_rules! s {
        ($n:literal) => {
            symbol($n)
        };
    }
    let names: [(&CStr, P); 27] = [
        (c"il2cpp_get_corlib", s!(c"il2cpp_get_corlib")),
        (c"il2cpp_domain_get", s!(c"il2cpp_domain_get")),
        (c"il2cpp_thread_attach", s!(c"il2cpp_thread_attach")),
        // Some builds lack il2cpp_domain_get_assemblies: classes are walked instead.
        (c"il2cpp_class_for_each", s!(c"il2cpp_class_for_each")),
        (c"il2cpp_class_get_image", s!(c"il2cpp_class_get_image")),
        (c"il2cpp_image_get_name", s!(c"il2cpp_image_get_name")),
        (c"il2cpp_class_is_generic", s!(c"il2cpp_class_is_generic")),
        (c"il2cpp_class_is_inflated", s!(c"il2cpp_class_is_inflated")),
        (c"il2cpp_image_get_class_count", s!(c"il2cpp_image_get_class_count")),
        (c"il2cpp_image_get_class", s!(c"il2cpp_image_get_class")),
        (c"il2cpp_class_get_name", s!(c"il2cpp_class_get_name")),
        (c"il2cpp_class_get_namespace", s!(c"il2cpp_class_get_namespace")),
        (c"il2cpp_class_get_methods", s!(c"il2cpp_class_get_methods")),
        (c"il2cpp_class_get_fields", s!(c"il2cpp_class_get_fields")),
        (c"il2cpp_method_get_name", s!(c"il2cpp_method_get_name")),
        (c"il2cpp_method_get_param_count", s!(c"il2cpp_method_get_param_count")),
        (c"il2cpp_method_is_instance", s!(c"il2cpp_method_is_instance")),
        (c"il2cpp_method_get_return_type", s!(c"il2cpp_method_get_return_type")),
        (c"il2cpp_type_get_type", s!(c"il2cpp_type_get_type")),
        (c"il2cpp_field_get_name", s!(c"il2cpp_field_get_name")),
        (c"il2cpp_field_get_type", s!(c"il2cpp_field_get_type")),
        (c"il2cpp_field_get_value", s!(c"il2cpp_field_get_value")),
        (c"il2cpp_class_from_type", s!(c"il2cpp_class_from_type")),
        (c"il2cpp_object_new", s!(c"il2cpp_object_new")),
        (c"il2cpp_field_set_value", s!(c"il2cpp_field_set_value")),
        (c"il2cpp_runtime_invoke", s!(c"il2cpp_runtime_invoke")),
        (c"il2cpp_string_new_wrapper", s!(c"il2cpp_string_new_wrapper")),
    ];
    let get = |n: &str| names.iter().find(|(k, _)| k.to_bytes() == n.as_bytes()).map(|(_, v)| *v).unwrap_or(std::ptr::null_mut());
    let class_from_name = s!(c"il2cpp_class_from_name");
    let class_get_type = s!(c"il2cpp_class_get_type");
    // Optional exports (diagnostics, calling convention) never gate install.
    let optional = ["il2cpp_image_get_name", "il2cpp_method_is_instance", "il2cpp_field_get_type"];
    let complete = names.iter().all(|(k, v)| !v.is_null() || optional.contains(&k.to_str().unwrap_or("")));
    if !complete {
        if !get("il2cpp_get_corlib").is_null() && !LOGGED_MISSING_EXPORTS.swap(true, Ordering::AcqRel) {
            let f = |n: &str| (!get(n).is_null()) as i32;
            rt::log(&format!(
                "[PearRuntime][consent] missing IL2CPP exports: domain={} attach={} assemblies={} image={} imageName={} generic={} inflated={} classCount={} class={} className={} ns={} methods={} fields={} methodName={} paramCount={} returnType={} typeGetType={} fieldName={} fieldType={} classFromName={} classGetType={} objectNew={} fieldSet={} invoke={} string={}",
                f("il2cpp_domain_get"), f("il2cpp_thread_attach"), f("il2cpp_class_for_each"), f("il2cpp_class_get_image"),
                f("il2cpp_image_get_name"), f("il2cpp_class_is_generic"), f("il2cpp_class_is_inflated"),
                f("il2cpp_image_get_class_count"), f("il2cpp_image_get_class"), f("il2cpp_class_get_name"),
                f("il2cpp_class_get_namespace"), f("il2cpp_class_get_methods"), f("il2cpp_class_get_fields"),
                f("il2cpp_method_get_name"), f("il2cpp_method_get_param_count"), f("il2cpp_method_get_return_type"),
                f("il2cpp_type_get_type"), f("il2cpp_field_get_name"), f("il2cpp_field_get_type"),
                (!class_from_name.is_null()) as i32, (!class_get_type.is_null()) as i32, f("il2cpp_object_new"),
                f("il2cpp_field_set_value"), f("il2cpp_runtime_invoke"), f("il2cpp_string_new_wrapper")
            ));
        }
        return None;
    }
    macro_rules! t {
        ($n:literal) => {
            std::mem::transmute(get($n))
        };
    }
    let opt = |n: &str| { let v = get(n); (!v.is_null()).then_some(v) };
    let api = Api {
        get_corlib: t!("il2cpp_get_corlib"),
        domain_get: t!("il2cpp_domain_get"),
        thread_attach: t!("il2cpp_thread_attach"),
        class_for_each: t!("il2cpp_class_for_each"),
        class_get_image: t!("il2cpp_class_get_image"),
        image_get_name: opt("il2cpp_image_get_name").map(|v| std::mem::transmute(v)),
        class_is_generic: t!("il2cpp_class_is_generic"),
        class_is_inflated: t!("il2cpp_class_is_inflated"),
        image_get_class_count: t!("il2cpp_image_get_class_count"),
        image_get_class: t!("il2cpp_image_get_class"),
        class_get_name: t!("il2cpp_class_get_name"),
        class_get_namespace: t!("il2cpp_class_get_namespace"),
        class_get_methods: t!("il2cpp_class_get_methods"),
        class_get_fields: t!("il2cpp_class_get_fields"),
        method_get_name: t!("il2cpp_method_get_name"),
        method_get_param_count: t!("il2cpp_method_get_param_count"),
        method_is_instance: opt("il2cpp_method_is_instance").map(|v| std::mem::transmute(v)),
        method_get_return_type: t!("il2cpp_method_get_return_type"),
        type_get_type: t!("il2cpp_type_get_type"),
        field_get_name: t!("il2cpp_field_get_name"),
        field_get_type: opt("il2cpp_field_get_type").map(|v| std::mem::transmute(v)),
        field_get_value: t!("il2cpp_field_get_value"),
        class_from_type: t!("il2cpp_class_from_type"),
        object_new: t!("il2cpp_object_new"),
        field_set_value: t!("il2cpp_field_set_value"),
        runtime_invoke: t!("il2cpp_runtime_invoke"),
        string_new_wrapper: t!("il2cpp_string_new_wrapper"),
    };
    let _ = API.set(api);
    Some(api)
}

unsafe fn s<'a>(p: *const c_char) -> &'a str {
    if p.is_null() { "" } else { std::str::from_utf8_unchecked(CStr::from_ptr(p).to_bytes()) }
}

/// IL2CPP_TYPE_VOID (0x01), read from the type enum (no allocation).
unsafe fn returns_void(api: &Api, method: P) -> bool {
    let t = (api.method_get_return_type)(method);
    !t.is_null() && (api.type_get_type)(t) == 1
}

fn is_consent_namespace(ns: &str) -> bool {
    ns.contains("Usercentrics")
}

/// The Swift UI entry points (Awake and the init helpers included: letting
/// them run reaches the crashing Swift metadata).
fn is_ui_entry_point(name: &str) -> bool {
    matches!(name, "Awake" | "CreateUsercentricsObject" | "ShowFirstLayer" | "ShowSecondLayer" | "PromptForAppTrackingTransparency" | ".cctor")
        || name.contains("Init")
}

/// The contained caches directory, built directly: interposing does not
/// redirect this dylib's own NSSearchPathForDirectoriesInDomains calls.
fn contained_caches_path() -> String {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    if let Some(home) = env("PEAR_HOME_DIR") {
        return rt::path_join(&home, "Library/Caches");
    }
    let real_home = env("HOME").unwrap_or_else(|| unsafe {
        extern "C" {
            fn NSHomeDirectory() -> Id;
        }
        rt::ns_string(NSHomeDirectory())
    });
    let bundle_id = unsafe {
        let b: Id = msg0(msg0::<Id>(cls("NSBundle"), "mainBundle"), "bundleIdentifier");
        if b.is_null() { "unknown".to_string() } else { rt::ns_string(b) }
    };
    let games = match env("PEAR_SUPPORT_ROOT") {
        Some(root) => rt::path_join(&root, "Games"),
        None => rt::path_join(&real_home, "Library/Application Support/PearSilicon/Games"),
    };
    rt::path_join(&rt::path_join(&games, &bundle_id), "Library/Caches")
}

/// The app's active consent settings id and its region, from the SDK's own
/// ruleset cache (a made-up id makes the consent request 404).
fn active_settings_id() -> &'static (Option<String>, String) {
    static V: OnceLock<(Option<String>, String)> = OnceLock::new();
    V.get_or_init(|| unsafe {
        pear_objc::autorelease_pool(|| {
            let code: Id = msg0(msg0::<Id>(cls("NSLocale"), "currentLocale"), "countryCode");
            let country = if code.is_null() { "US".to_string() } else { rt::ns_string(code) };
            let root = rt::path_join(&contained_caches_path(), "usercentrics");
            let entries = rt::directory_entries(&root);
            if debug() {
                rt::log(&format!("[PearRuntime][consent] settings cache root={root} entries={}", entries.len()));
            }
            let mut settings: Option<String> = None;
            let mut primary: Option<String> = None;
            let classes: Id = msg2(cls("NSSet"), "setWithObjects:", cls("NSString"), cls("NSMutableString"));
            'outer: for entry in entries.iter().filter(|e| e.starts_with("etags-")) {
                if primary.is_none() {
                    primary = Some(entry["etags-".len()..].to_string());
                }
                let rule_dir = rt::path_join(&rt::path_join(&root, entry), "ruleSet");
                for file in rt::directory_entries(&rule_dir) {
                    let data: Id = msg1(cls("NSData"), "dataWithContentsOfFile:", nsstring(&rt::path_join(&rule_dir, &file)));
                    if data.is_null() {
                        continue;
                    }
                    let archived: Id = pear_objc::msg3(cls("NSKeyedUnarchiver"), "unarchivedObjectOfClasses:fromData:error:", classes, data, std::ptr::null_mut::<Id>());
                    if archived.is_null() || !pear_objc::is_kind_of(archived, "NSString") {
                        continue;
                    }
                    let json_data: Id = msg1(archived, "dataUsingEncoding:", 4usize);
                    let json: Id = pear_objc::msg3(cls("NSJSONSerialization"), "JSONObjectWithData:options:error:", json_data, 0usize, std::ptr::null_mut::<Id>());
                    if json.is_null() || !pear_objc::is_kind_of(json, "NSDictionary") {
                        continue;
                    }
                    let rules: Id = msg1(json, "objectForKey:", nsstring("rules"));
                    if !rules.is_null() && pear_objc::is_kind_of(rules, "NSArray") {
                        for i in 0..pear_objc::nsarray_count(rules) {
                            let rule = pear_objc::nsarray_object_at(rules, i);
                            if !pear_objc::is_kind_of(rule, "NSDictionary") {
                                continue;
                            }
                            let locations: Id = msg1(rule, "objectForKey:", nsstring("locations"));
                            if !locations.is_null()
                                && pear_objc::is_kind_of(locations, "NSArray")
                                && msg1::<Id, bool>(locations, "containsObject:", nsstring(&country))
                            {
                                let id: Id = msg1(rule, "objectForKey:", nsstring("settingsId"));
                                settings = (!id.is_null()).then(|| rt::ns_string(id)).filter(|s| !s.is_empty());
                                if settings.is_some() {
                                    break;
                                }
                            }
                        }
                    }
                    if settings.is_none() {
                        let def: Id = msg1(json, "objectForKey:", nsstring("defaultRule"));
                        if !def.is_null() && pear_objc::is_kind_of(def, "NSDictionary") {
                            let id: Id = msg1(def, "objectForKey:", nsstring("settingsId"));
                            settings = (!id.is_null()).then(|| rt::ns_string(id)).filter(|s| !s.is_empty());
                        }
                    }
                    if settings.is_some() {
                        break 'outer;
                    }
                }
            }
            let settings = settings.or(primary);
            rt::log(&format!(
                "[PearRuntime][consent] settings id={} country={country}",
                settings.as_deref().unwrap_or("(none)")
            ));
            (settings, country)
        })
    })
}

unsafe fn find_method(api: &Api, class: P, wanted: &str, params: u32) -> P {
    let mut iter: P = std::ptr::null_mut();
    loop {
        let m = (api.class_get_methods)(class, &mut iter);
        if m.is_null() {
            return std::ptr::null_mut();
        }
        if s((api.method_get_name)(m)) == wanted && (api.method_get_param_count)(m) == params {
            return m;
        }
    }
}

unsafe fn find_field(api: &Api, class: P, wanted: &str) -> P {
    let mut iter: P = std::ptr::null_mut();
    loop {
        let f = (api.class_get_fields)(class, &mut iter);
        if f.is_null() {
            return std::ptr::null_mut();
        }
        if s((api.field_get_name)(f)) == wanted {
            return f;
        }
    }
}

unsafe fn cstring(t: &str) -> CString {
    CString::new(t).unwrap_or_default()
}

/// Replacement for Initialize(OnInitialized): a "no consent required" status
/// handed to the callback the app's privacy service waits for.
unsafe extern "C" fn consent_initialize(_this: P, callback: P) {
    let Some(api) = API.get() else { return };
    let (status_class, ctor, invoke, should, banner, active, country_f, region, consents) = {
        let m = META.lock().unwrap_or_else(|e| e.into_inner());
        (m.status_class, m.status_ctor, m.on_initialized_invoke, m.should_collect, m.banner_required, m.active_settings, m.country, m.region, m.service_consents)
    };
    if callback.is_null() || status_class.is_null() || ctor.is_null() || invoke.is_null() {
        return;
    }
    let mut exception: P = std::ptr::null_mut();
    let status = (api.object_new)(status_class);
    if status.is_null() {
        return;
    }
    (api.runtime_invoke)(ctor, status, std::ptr::null_mut(), &mut exception);
    if !exception.is_null() {
        rt::log("[PearRuntime][consent] status ctor threw");
        return;
    }
    let mut no = false;
    if !should.is_null() {
        (api.field_set_value)(status, should, &mut no as *mut bool as P);
    }
    if !banner.is_null() {
        (api.field_set_value)(status, banner, &mut no as *mut bool as P);
    }
    // A null ServiceConsents list makes the privacy service throw: give it an
    // empty List<ServiceConsent> when the parameterless ctor left it null.
    if !consents.is_null() {
        let mut list: P = std::ptr::null_mut();
        (api.field_get_value)(status, consents, &mut list as *mut P as P);
        if list.is_null() {
            if let Some(field_get_type) = api.field_get_type {
                let t = field_get_type(consents);
                let class = if t.is_null() { std::ptr::null_mut() } else { (api.class_from_type)(t) };
                let mut new_list = if class.is_null() { std::ptr::null_mut() } else { (api.object_new)(class) };
                let list_ctor = if new_list.is_null() { std::ptr::null_mut() } else { find_method(api, class, ".ctor", 0) };
                if !new_list.is_null() && !list_ctor.is_null() {
                    let mut e: P = std::ptr::null_mut();
                    (api.runtime_invoke)(list_ctor, new_list, std::ptr::null_mut(), &mut e);
                    if e.is_null() {
                        (api.field_set_value)(status, consents, &mut new_list as *mut P as P);
                        rt::log("[PearRuntime][consent] initialised ServiceConsents");
                    }
                }
            }
        } else {
            rt::log("[PearRuntime][consent] ServiceConsents already set");
        }
    }
    let (settings, country) = active_settings_id();
    let settings_c = cstring(settings.as_deref().filter(|s| !s.is_empty()).unwrap_or("pearsilicon"));
    let country_c = cstring(if country.is_empty() { "US" } else { country });
    let mut settings_id = (api.string_new_wrapper)(settings_c.as_ptr());
    let mut country_s = (api.string_new_wrapper)(country_c.as_ptr());
    if !active.is_null() {
        (api.field_set_value)(status, active, &mut settings_id as *mut P as P);
    }
    if !country_f.is_null() {
        (api.field_set_value)(status, country_f, &mut country_s as *mut P as P);
    }
    if !region.is_null() {
        (api.field_set_value)(status, region, &mut country_s as *mut P as P);
    }
    let error = (api.string_new_wrapper)(c"".as_ptr());
    let mut params: [P; 2] = [status, error];
    exception = std::ptr::null_mut();
    (api.runtime_invoke)(invoke, callback, params.as_mut_ptr(), &mut exception);
    if exception.is_null() {
        rt::log("[PearRuntime][consent] reported no consent required");
    } else {
        rt::log("[PearRuntime][consent] OnInitialized threw");
    }
}

/// Static variant: IL2CPP passes a static method's arguments first.
unsafe extern "C" fn consent_initialize_static(callback: P) {
    consent_initialize(std::ptr::null_mut(), callback);
}

/// The replaced C# methods all return System.Void.
extern "C" fn consent_noop() {}

unsafe fn find_class(api: &Api, image: P, count: usize, namespace: Option<&str>, name: &str) -> P {
    for i in 0..count {
        let c = (api.image_get_class)(image, i);
        if c.is_null() || s((api.class_get_name)(c)) != name {
            continue;
        }
        if let Some(ns) = namespace {
            if s((api.class_get_namespace)(c)) != ns {
                continue;
            }
        }
        return c;
    }
    std::ptr::null_mut()
}

struct Search<'a> {
    api: &'a Api,
    name: &'a str,
    namespace_contains: Option<&'a str>,
    class: P,
    image: P,
}

unsafe extern "C" fn search_callback(class: P, data: P) {
    let search = &mut *(data as *mut Search);
    if !search.class.is_null() || s((search.api.class_get_name)(class)) != search.name {
        return;
    }
    if let Some(sub) = search.namespace_contains {
        if !s((search.api.class_get_namespace)(class)).contains(sub) {
            return;
        }
    }
    search.class = class;
    search.image = (search.api.class_get_image)(class);
}

/// A class by name across the whole domain (classes may live in several assemblies).
unsafe fn find_class_anywhere(api: &Api, name: &str, namespace_contains: Option<&str>) -> (P, P) {
    let mut search = Search { api, name, namespace_contains, class: std::ptr::null_mut(), image: std::ptr::null_mut() };
    (api.class_for_each)(search_callback, &mut search as *mut Search as P);
    (search.class, search.image)
}

unsafe fn dump_fields(api: &Api, class: P) {
    if class.is_null() || !debug() {
        return;
    }
    rt::log(&format!(
        "[PearRuntime][consent] fields of {}.{}",
        s((api.class_get_namespace)(class)),
        s((api.class_get_name)(class))
    ));
    let mut iter: P = std::ptr::null_mut();
    loop {
        let f = (api.class_get_fields)(class, &mut iter);
        if f.is_null() {
            break;
        }
        let kind = match api.field_get_type {
            Some(get_type) => {
                let t = get_type(f);
                if t.is_null() { -1 } else { (api.type_get_type)(t) }
            }
            None => -1,
        };
        rt::log(&format!("[PearRuntime][consent]   {} kind={kind}", s((api.field_get_name)(f))));
    }
}

/// The app's privacy OnInitialized(string json, string error) event, with a
/// "no consent required" payload (main thread: Unity API calls).
unsafe fn fire_privacy_initialized() {
    let Some(api) = API.get() else { return };
    let method = META.lock().unwrap_or_else(|e| e.into_inner()).privacy_on_initialized;
    if method.is_null() {
        return;
    }
    let domain = (api.domain_get)();
    if !domain.is_null() {
        (api.thread_attach)(domain);
    }
    let (settings, country) = active_settings_id();
    let id = settings.as_deref().unwrap_or("pearsilicon");
    let json = format!(
        "{{\"shouldCollectConsent\":false,\"consents\":[],\"geolocationRuleset\":{{\"activeSettingsId\":\"{id}\",\"bannerRequiredAtLocation\":false}},\"location\":{{\"countryCode\":\"{country}\",\"regionCode\":\"{country}\"}}}}"
    );
    let json_c = cstring(&json);
    let mut params: [P; 2] = [(api.string_new_wrapper)(json_c.as_ptr()), (api.string_new_wrapper)(c"".as_ptr())];
    let mut exception: P = std::ptr::null_mut();
    (api.runtime_invoke)(method, std::ptr::null_mut(), params.as_mut_ptr(), &mut exception);
    if exception.is_null() {
        rt::log("[PearRuntime][consent] reported privacy initialized (no consent required)");
    } else {
        rt::log("[PearRuntime][consent] privacy OnInitialized threw");
    }
}

struct Patch<'a> {
    api: &'a Api,
    patched: i32,
    can_initialize: bool,
}

unsafe extern "C" fn patch_callback(class: P, data: P) {
    let ctx = &mut *(data as *mut Patch);
    let api = ctx.api;
    let ns = s((api.class_get_namespace)(class));
    if !is_consent_namespace(ns) {
        return;
    }
    // Methods of a generic definition block on the main thread's class
    // initializer: skip generic and inflated classes.
    if (api.class_is_generic)(class) || (api.class_is_inflated)(class) {
        return;
    }
    let name = s((api.class_get_name)(class));
    if debug() {
        rt::log(&format!("[PearRuntime][consent] scanning {ns}.{name}"));
    }
    let mut iter: P = std::ptr::null_mut();
    loop {
        let method = (api.class_get_methods)(class, &mut iter);
        if method.is_null() {
            break;
        }
        let method_name_p = (api.method_get_name)(method);
        if method_name_p.is_null() {
            continue;
        }
        let method_name = s(method_name_p);
        if debug() {
            let ret = (api.method_get_return_type)(method);
            rt::log(&format!(
                "[PearRuntime][consent] method {ns}.{name}::{method_name} kind={}",
                if ret.is_null() { -1 } else { (api.type_get_type)(ret) }
            ));
        }
        if !returns_void(api, method) {
            continue;
        }
        // MethodInfo's first field is the native method pointer.
        let slot = method as *mut *const c_void;
        if method_name == "Initialize" {
            // The replacement takes one argument: other arities would misread theirs.
            if ctx.can_initialize && (api.method_get_param_count)(method) == 1 {
                let is_static = api.method_is_instance.map(|f| !f(method)).unwrap_or(false);
                *slot = if is_static { consent_initialize_static as *const c_void } else { consent_initialize as *const c_void };
                ctx.patched += 1;
                rt::log(&format!("[PearRuntime][consent] replaced {ns}.{name}::{method_name}{}", if is_static { " (static)" } else { "" }));
            } else {
                // A no-op would never call back and the app would wait forever.
                rt::log(&format!("[PearRuntime][consent] left {ns}.{name}::{method_name} alone (metadata incomplete)"));
            }
            continue;
        }
        if is_ui_entry_point(method_name) {
            *slot = consent_noop as *const c_void;
            ctx.patched += 1;
            rt::log(&format!("[PearRuntime][consent] skipped {ns}.{name}::{method_name}"));
        }
    }
}

unsafe extern "C" fn log_image_callback(class: P, data: P) {
    let api = &*(data as *const Api);
    let ns = s((api.class_get_namespace)(class));
    if !is_consent_namespace(ns) {
        return;
    }
    let image = (api.class_get_image)(class);
    let image_name = match (api.image_get_name, image.is_null()) {
        (Some(f), false) => s(f(image)).to_string(),
        _ => "?".into(),
    };
    rt::log(&format!("[PearRuntime][consent] found {ns}.{} image={image_name}", s((api.class_get_name)(class))));
}

unsafe fn install() {
    if INSTALLED.load(Ordering::Acquire) {
        return;
    }
    let Some(api) = resolve() else {
        if !LOGGED_RESOLVE_FAILURE.swap(true, Ordering::AcqRel) {
            rt::log("[PearRuntime][consent] IL2CPP exports not available yet");
        }
        return;
    };
    // Before il2cpp_init finishes the API dereferences null globals.
    if (api.get_corlib)().is_null() {
        return;
    }
    if !LOGGED_CORLIB.swap(true, Ordering::AcqRel) {
        rt::log("[PearRuntime][consent] IL2CPP corlib resolved");
    }
    let domain = (api.domain_get)();
    if domain.is_null() {
        if !LOGGED_DOMAIN.swap(true, Ordering::AcqRel) {
            rt::log("[PearRuntime][consent] IL2CPP domain not ready yet");
        }
        return;
    }
    // The calling thread must be attached to the domain.
    (api.thread_attach)(domain);
    // A full class walk is not free: throttle the poll.
    let now = CFAbsoluteTimeGetCurrent();
    if now - f64::from_bits(LAST_SCAN.load(Ordering::Acquire)) < 0.1 {
        return;
    }
    LAST_SCAN.store(now.to_bits(), Ordering::Release);

    let (status_class, image) = find_class_anywhere(&api, "UsercentricsStatus", Some("Usercentrics"));
    if status_class.is_null() || image.is_null() {
        if !LOGGED_ASSEMBLY.swap(true, Ordering::AcqRel) {
            rt::log("[PearRuntime][consent] consent assembly not loaded yet");
        }
        return;
    }
    let count = (api.image_get_class_count)(image);
    let on_initialized = find_class(&api, image, count, Some(""), "OnInitialized");
    {
        let mut m = META.lock().unwrap_or_else(|e| e.into_inner());
        m.status_class = status_class;
        m.status_ctor = find_method(&api, status_class, ".ctor", 0);
        m.should_collect = find_field(&api, status_class, "ShouldCollectConsent");
        m.banner_required = find_field(&api, status_class, "BannerRequiredAtLocation");
        m.active_settings = find_field(&api, status_class, "ActiveSettingsId");
        m.country = find_field(&api, status_class, "CountryCode");
        m.region = find_field(&api, status_class, "RegionCode");
        m.service_consents = find_field(&api, status_class, "ServiceConsents");
        if !on_initialized.is_null() {
            m.on_initialized_invoke = find_method(&api, on_initialized, "Invoke", 2);
        }
    }
    dump_fields(&api, status_class);
    // The app listens to the privacy bridge's OnInitialized(string, string).
    let (privacy_events, privacy_image) = find_class_anywhere(&api, "PrivacyEventsApple", None);
    if !privacy_events.is_null() {
        META.lock().unwrap_or_else(|e| e.into_inner()).privacy_on_initialized = find_method(&api, privacy_events, "OnInitialized", 2);
        dump_fields(&api, privacy_events);
    }
    if !privacy_image.is_null() {
        let n = (api.image_get_class_count)(privacy_image);
        let manager = find_class(&api, privacy_image, n, None, "PrivacyManager");
        let is_ready = if manager.is_null() { std::ptr::null_mut() } else { find_method(&api, manager, "IsReady", 0) };
        dump_fields(&api, manager);
        // Not forced: the main UI follows the normal callback; forcing the
        // flag skips both and runs headless.
        rt::log(&format!("[PearRuntime][consent] privacy image={privacy_image:p} manager={manager:p} isReady={is_ready:p}"));
    }
    if debug() && !LOGGED_IMAGES.swap(true, Ordering::AcqRel) {
        let mut a = api;
        (api.class_for_each)(log_image_callback, &mut a as *mut Api as P);
    }
    let can_initialize = {
        let m = META.lock().unwrap_or_else(|e| e.into_inner());
        !m.status_class.is_null() && !m.status_ctor.is_null() && !m.on_initialized_invoke.is_null()
    };
    let mut patch = Patch { api: &api, patched: 0, can_initialize };
    (api.class_for_each)(patch_callback, &mut patch as *mut Patch as P);
    if patch.patched > 0 {
        INSTALLED.store(true, Ordering::Release);
        rt::log(&format!("[PearRuntime][consent] {} consent entry point(s) disabled", patch.patched));
        // The privacy service subscribes after start-up: fire until one lands.
        for i in 5..=30 {
            rt::after_main(i as f64, || unsafe { pear_objc::autorelease_pool(|| fire_privacy_initialized()) });
        }
    } else if !LOGGED_NO_MATCH.swap(true, Ordering::AcqRel) {
        rt::log("[PearRuntime][consent] assembly loaded but no entry points matched");
    }
}

extern "C" {
    fn dispatch_get_global_queue(identifier: isize, flags: usize) -> P;
    fn dispatch_async_f(queue: P, context: P, work: extern "C" fn(P));
    fn dispatch_source_create(kind: *const c_void, handle: usize, mask: usize, queue: P) -> P;
    fn dispatch_source_set_timer(source: P, start: u64, interval: u64, leeway: u64);
    fn dispatch_source_set_event_handler_f(source: P, handler: extern "C" fn(P));
    fn dispatch_set_context(object: P, context: P);
    fn dispatch_resume(object: P);
    fn dispatch_source_cancel(source: P);
    fn dispatch_time(when: u64, delta: i64) -> u64;
    static _dispatch_source_type_timer: c_void;
}

static POLL_SOURCE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static POLL_ATTEMPTS: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

extern "C" fn poll(_: P) {
    unsafe {
        pear_objc::autorelease_pool(|| {
            // 2400 × 250 ms = 10 minutes: long enough for a cold launch.
            if INSTALLED.load(Ordering::Acquire) || POLL_ATTEMPTS.fetch_add(1, Ordering::AcqRel) >= 2400 {
                dispatch_source_cancel(POLL_SOURCE.load(Ordering::Acquire));
                return;
            }
            install();
        })
    }
}

unsafe fn schedule() {
    if rt::env_set("PEAR_NO_CONSENT_SHIM") {
        return;
    }
    DEBUG.store(rt::env_set("PEAR_DEBUG_CONSENT"), Ordering::Relaxed);
    rt::log("[PearRuntime][consent] shim scheduled");
    // IL2CPP may already be up when the runtime loads late.
    install();
    // The assembly loads seconds (or, cold, minutes) after IL2CPP starts; poll
    // on a background queue (the main thread is busy loading).
    let queue = rt::serial_queue(c"com.pearsilicon.consent");
    let source = dispatch_source_create(&_dispatch_source_type_timer, 0, 0, queue);
    POLL_SOURCE.store(source, Ordering::Release);
    dispatch_source_set_timer(source, dispatch_time(0, 250_000_000), 250_000_000, 50_000_000);
    dispatch_set_context(source, std::ptr::null_mut());
    dispatch_source_set_event_handler_f(source, poll);
    dispatch_resume(source);
}

/// Walking the IL2CPP class table is only safe for an app that ships the
/// consent module: look for its marker in the app's own binaries first.
extern "C" fn detect(_: P) {
    unsafe {
        pear_objc::autorelease_pool(|| {
            let bundle: Id = msg0(cls("NSBundle"), "mainBundle");
            let executable: Id = msg0(bundle, "executablePath");
            let root = rt::ns_string(msg0(bundle, "bundlePath"));
            let paths = [
                if executable.is_null() { String::new() } else { rt::ns_string(executable) },
                rt::path_join(&root, "Frameworks/UnityFramework.framework/UnityFramework"),
                rt::path_join(&root, "Data/Managed/Metadata/global-metadata.dat"),
            ];
            let marker: Id = msg1(nsstring("CreateUsercentricsObject"), "dataUsingEncoding:", 4usize);
            let found = paths.iter().filter(|p| !p.is_empty()).any(|p| {
                let data: Id = pear_objc::msg3(cls("NSData"), "dataWithContentsOfFile:options:error:", nsstring(p), 1usize, std::ptr::null_mut::<Id>());
                if data.is_null() {
                    return false;
                }
                let len: usize = msg0(data, "length");
                #[repr(C)]
                struct Range {
                    location: usize,
                    length: usize,
                }
                let r: Range = pear_objc::msg3(data, "rangeOfData:options:range:", marker, 0usize, Range { location: 0, length: len });
                r.location < len // NSNotFound otherwise
            });
            if !found {
                rt::log("[PearRuntime][consent] module not present; shim skipped");
                return;
            }
            schedule();
        })
    }
}

pub fn init() {
    if !rt::module_enabled("services") {
        return;
    }
    unsafe {
        let q = dispatch_get_global_queue(0x11, 0); // QOS_CLASS_UTILITY
        dispatch_async_f(q, std::ptr::null_mut(), detect);
    }
}
