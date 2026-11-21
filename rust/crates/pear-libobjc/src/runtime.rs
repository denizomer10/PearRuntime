//! Selectors, the class registry, realization, lookup, +initialize and
//! image registration.
//!
//! Classes keep the layout clang emitted (compiled code reads `isa`,
//! `superclass` and ivar offset variables directly); what the runtime knows
//! beyond that lives in a side table keyed by class address.

use crate::abi::*;
use std::collections::HashMap;
use std::ffi::{c_char, CStr, CString};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

// ---- selectors ---------------------------------------------------------

fn selectors() -> &'static Mutex<HashMap<Vec<u8>, usize>> {
    static S: OnceLock<Mutex<HashMap<Vec<u8>, usize>>> = OnceLock::new();
    S.get_or_init(Default::default)
}

/// The unique selector for `name` (a SEL is a pointer to its name).
pub fn sel(name: &str) -> Sel {
    intern(name.as_bytes())
}

pub fn intern(name: &[u8]) -> Sel {
    let mut table = selectors().lock().unwrap();
    if let Some(&p) = table.get(name) {
        return p as Sel;
    }
    let owned = CString::new(name).unwrap_or_default().into_raw() as usize;
    table.insert(name.to_vec(), owned);
    owned as Sel
}

pub unsafe fn intern_c(name: *const c_char) -> Sel {
    intern(CStr::from_ptr(name).to_bytes())
}

pub fn sel_name(s: Sel) -> String {
    if s.is_null() {
        return "(null)".into();
    }
    unsafe { CStr::from_ptr(s) }.to_string_lossy().into_owned()
}

// ---- classes -------------------------------------------------------------

const INIT_NONE: u8 = 0;
const INIT_RUNNING: u8 = 1;
const INIT_DONE: u8 = 2;

/// A method as `Method` handles see it: stable for the life of the process,
/// its IMP replaceable (method_setImplementation, swizzling).
pub struct MethodEntry {
    pub name: Sel,
    pub types: *const c_char,
    pub imp: std::sync::atomic::AtomicUsize,
}
unsafe impl Send for MethodEntry {}
unsafe impl Sync for MethodEntry {}

impl MethodEntry {
    pub fn new(name: Sel, types: *const c_char, imp: Imp) -> &'static MethodEntry {
        Box::leak(Box::new(MethodEntry { name, types, imp: std::sync::atomic::AtomicUsize::new(imp as usize) }))
    }
    pub fn imp(&self) -> Imp {
        self.imp.load(Ordering::Acquire) as Imp
    }
}

/// An instance variable as `Ivar` handles see it.
pub struct IvarEntry {
    pub name: *const c_char,
    pub types: *const c_char,
    /// The offset variable compiled code reads (image ivars), or None for an
    /// ivar added at run time (offset stored here).
    pub offset_var: *mut i32,
    pub offset: i32,
    pub size: u32,
}
unsafe impl Send for IvarEntry {}
unsafe impl Sync for IvarEntry {}

impl IvarEntry {
    pub fn offset(&self) -> isize {
        if self.offset_var.is_null() { self.offset as isize } else { unsafe { *self.offset_var as isize } }
    }
}

pub struct Class {
    pub ptr: *const ClassT,
    pub name: String,
    pub meta: bool,
    pub instance_size: std::sync::atomic::AtomicUsize,
    pub methods: RwLock<HashMap<usize, &'static MethodEntry>>,
    /// Methods in definition order, categories' after the class's own.
    pub method_list: RwLock<Vec<&'static MethodEntry>>,
    pub protocols: RwLock<Vec<*const ProtocolT>>,
    pub ivars: RwLock<Vec<&'static IvarEntry>>,
    pub properties: RwLock<Vec<(*const c_char, *const c_char)>>,
    /// The install name of the image that defined it (class_getImageName).
    pub image: Option<CString>,
    /// Created by objc_allocateClassPair and not yet registered.
    pub building: std::sync::atomic::AtomicBool,
    init: AtomicU8,
    init_thread: Mutex<Option<std::thread::ThreadId>>,
}

impl Class {
    fn new(ptr: *const ClassT, name: &str, meta: bool, size: usize, image: Option<CString>) -> Class {
        Class {
            ptr,
            name: name.to_string(),
            meta,
            instance_size: std::sync::atomic::AtomicUsize::new(size),
            methods: RwLock::new(HashMap::new()),
            method_list: RwLock::new(Vec::new()),
            protocols: RwLock::new(Vec::new()),
            ivars: RwLock::new(Vec::new()),
            properties: RwLock::new(Vec::new()),
            image,
            building: std::sync::atomic::AtomicBool::new(false),
            init: AtomicU8::new(INIT_NONE),
            init_thread: Mutex::new(None),
        }
    }

    pub fn size(&self) -> usize {
        self.instance_size.load(Ordering::Acquire)
    }

    /// Adds methods; a later method for the same selector wins.
    pub fn add(&self, list: impl IntoIterator<Item = &'static MethodEntry>) {
        let mut map = self.methods.write().unwrap();
        let mut order = self.method_list.write().unwrap();
        for m in list {
            map.insert(m.name as usize, m);
            order.push(m);
        }
    }
}

unsafe impl Send for Class {}
unsafe impl Sync for Class {}

#[derive(Default)]
struct Registry {
    classes: HashMap<usize, Arc<Class>>,
    by_name: HashMap<String, usize>,
    /// Classes listed in registered images' `__objc_classlist`: the only
    /// pointers realize() follows besides registered classes (a superclass
    /// field may hold anything when its import is unresolved).
    listed: std::collections::HashSet<usize>,
}

fn registry() -> &'static RwLock<Registry> {
    static R: OnceLock<RwLock<Registry>> = OnceLock::new();
    R.get_or_init(Default::default)
}

fn cache() -> &'static RwLock<HashMap<(usize, usize), usize>> {
    static C: OnceLock<RwLock<HashMap<(usize, usize), usize>>> = OnceLock::new();
    C.get_or_init(Default::default)
}

pub fn flush_cache() {
    cache().write().unwrap().clear();
}

pub fn class(ptr: *const ClassT) -> Option<Arc<Class>> {
    registry().read().unwrap().classes.get(&(ptr as usize)).cloned()
}

pub fn class_named(name: &str) -> Option<*const ClassT> {
    registry().read().unwrap().by_name.get(name).map(|&p| p as *const ClassT)
}

/// Registers a class and its metaclass with methods already known (the
/// runtime's own root classes).
pub fn register_builtin(
    cls: *const ClassT,
    meta: *const ClassT,
    name: &str,
    instance_size: usize,
    instance_methods: &[(&str, Imp)],
    class_methods: &[(&str, Imp)],
) {
    let make = |ptr, meta, size, list: &[(&str, Imp)]| {
        let c = Class::new(ptr, name, meta, size, None);
        // Builtin methods: object-returning by default, as most are.
        c.add(list.iter().map(|(n, imp)| MethodEntry::new(sel(n), c"@16@0:8".as_ptr(), *imp)));
        Arc::new(c)
    };
    let mut r = registry().write().unwrap();
    r.classes.insert(cls as usize, make(cls, false, instance_size, instance_methods));
    r.classes.insert(meta as usize, make(meta, true, std::mem::size_of::<ClassT>(), class_methods));
    r.by_name.insert(name.to_string(), cls as usize);
}

pub unsafe fn ro(cls: *const ClassT) -> *const ClassRo {
    ((*cls).bits & FAST_DATA_MASK) as *const ClassRo
}

/// Realizes an image class and its metaclass (superclass first). False if
/// its superclass is unknown (a missing weak-linked class, as with dyld).
pub unsafe fn realize(cls: *const ClassT) -> bool {
    if cls.is_null() {
        return false;
    }
    if class(cls).is_some() {
        return true;
    }
    let sup = (*cls).superclass;
    if !sup.is_null() {
        let known = { let r = registry().read().unwrap(); r.classes.contains_key(&(sup as usize)) || r.listed.contains(&(sup as usize)) };
        if !known || !realize(sup) {
            return false;
        }
    }
    let r = ro(cls);
    let name = CStr::from_ptr((*r).name).to_string_lossy().into_owned();
    // Non-fragile ivars: if the superclass grew past where this class's
    // ivars start, slide them (and the offset variables code reads).
    let mut size = (*r).instance_size as usize;
    if !sup.is_null() {
        let sup_size = class(sup).map_or(0, |c| c.size());
        let start = (*r).instance_start as usize;
        if sup_size > start {
            let delta = (sup_size - start + 7) & !7;
            for ivar in ivars((*r).ivars) {
                if !(*ivar).offset.is_null() {
                    *(*ivar).offset += delta as i32;
                }
            }
            size += delta;
        }
    }
    let image_name = CURRENT_IMAGE.with(|i| i.borrow().clone());
    let make = |ptr: *const ClassT, meta: bool, size: usize| -> Arc<Class> {
        let r = ro(ptr);
        let c = Class::new(ptr, &name, meta, size, image_name.clone());
        c.add(methods((*r).base_methods).into_iter().map(|m| MethodEntry::new(m.name, m.types, m.imp)));
        *c.protocols.write().unwrap() = protocols((*r).base_protocols);
        *c.properties.write().unwrap() = properties((*r).base_properties);
        if !meta {
            *c.ivars.write().unwrap() = ivars((*r).ivars)
                .into_iter()
                .map(|iv| {
                    &*Box::leak(Box::new(IvarEntry {
                        name: (*iv).name,
                        types: (*iv).kind,
                        offset_var: (*iv).offset,
                        offset: 0,
                        size: (*iv).size,
                    }))
                })
                .collect();
        }
        Arc::new(c)
    };
    let meta = (*cls).isa;
    let info = make(cls, false, size);
    let meta_info = make(meta, true, std::mem::size_of::<ClassT>());
    let mut reg = registry().write().unwrap();
    reg.classes.insert(cls as usize, info);
    reg.classes.entry(meta as usize).or_insert(meta_info);
    reg.by_name.entry(name).or_insert(cls as usize);
    true
}

/// Adds methods to a class (categories, class_addMethod): later additions
/// win, as category methods override the class's own.
pub fn add_methods(cls: *const ClassT, list: &[MethodDesc]) {
    if let Some(c) = class(cls) {
        c.add(list.iter().map(|m| MethodEntry::new(m.name, m.types, m.imp)));
    }
    flush_cache();
}

/// The method entry for `sel`, searching superclasses.
pub fn lookup_method(cls: *const ClassT, s: Sel) -> Option<&'static MethodEntry> {
    let mut c = cls;
    while !c.is_null() {
        let info = class(c)?;
        if let Some(&m) = info.methods.read().unwrap().get(&(s as usize)) {
            return Some(m);
        }
        c = unsafe { (*c).superclass };
    }
    None
}

/// The class's own method entry (no superclasses).
pub fn own_method_entry(cls: *const ClassT, s: Sel) -> Option<&'static MethodEntry> {
    class(cls)?.methods.read().unwrap().get(&(s as usize)).copied()
}

/// Registers a class built at run time (objc_allocateClassPair).
pub fn register_dynamic(cls: *const ClassT, meta: *const ClassT, name: &str, size: usize) {
    let c = Class::new(cls, name, false, size, None);
    c.building.store(true, Ordering::Release);
    let m = Class::new(meta, name, true, std::mem::size_of::<ClassT>(), None);
    let mut r = registry().write().unwrap();
    r.classes.insert(cls as usize, Arc::new(c));
    r.classes.insert(meta as usize, Arc::new(m));
}

/// Makes a class built at run time findable by name (objc_registerClassPair).
pub fn publish(cls: *const ClassT) {
    if let Some(c) = class(cls) {
        c.building.store(false, Ordering::Release);
        registry().write().unwrap().by_name.entry(c.name.clone()).or_insert(cls as usize);
    }
}

/// Every registered class (not metaclasses), in no particular order.
pub fn all_classes() -> Vec<Arc<Class>> {
    let r = registry().read().unwrap();
    r.by_name.values().filter_map(|p| r.classes.get(p).cloned()).collect()
}

thread_local! {
    /// The image whose classes are being realized (class_getImageName).
    static CURRENT_IMAGE: std::cell::RefCell<Option<CString>> = const { std::cell::RefCell::new(None) };
}

/// Install names of registered images, in registration order.
pub fn image_names() -> &'static Mutex<Vec<CString>> {
    static N: OnceLock<Mutex<Vec<CString>>> = OnceLock::new();
    N.get_or_init(Default::default)
}

/// The IMP for `sel` on instances of `cls`, searching superclasses.
pub fn lookup(cls: *const ClassT, s: Sel) -> Option<Imp> {
    let key = (cls as usize, s as usize);
    if let Some(&imp) = cache().read().unwrap().get(&key) {
        return Some(imp as Imp);
    }
    let imp = lookup_method(cls, s)?.imp();
    cache().write().unwrap().insert(key, imp as usize);
    Some(imp)
}

/// The class's own method (no superclasses).
pub fn own_method(cls: *const ClassT, s: Sel) -> Option<Imp> {
    own_method_entry(cls, s).map(|m| m.imp())
}

pub fn superclass(cls: *const ClassT) -> *const ClassT {
    if cls.is_null() {
        std::ptr::null()
    } else {
        unsafe { (*cls).superclass }
    }
}

pub fn is_meta(cls: *const ClassT) -> bool {
    class(cls).is_some_and(|c| c.meta)
}

pub unsafe fn object_class(obj: Id) -> *const ClassT {
    if obj.is_null() {
        std::ptr::null()
    } else {
        *(obj as *const *const ClassT)
    }
}

/// Sends +initialize to `cls` (a class, not a metaclass) once, superclass
/// first, before its first message, as objc4 does. A class's own thread
/// may message it while +initialize runs.
pub unsafe fn ensure_initialized(cls: *const ClassT) {
    let Some(info) = class(cls) else { return };
    if info.init.load(Ordering::Acquire) == INIT_DONE {
        return;
    }
    let me = std::thread::current().id();
    if info.init.load(Ordering::Acquire) == INIT_RUNNING && *info.init_thread.lock().unwrap() == Some(me) {
        return;
    }
    let sup = superclass(cls);
    if !sup.is_null() {
        ensure_initialized(sup);
    }
    if info
        .init
        .compare_exchange(INIT_NONE, INIT_RUNNING, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        *info.init_thread.lock().unwrap() = Some(me);
        let initialize = sel("initialize");
        if let Some(imp) = lookup((*cls).isa, initialize) {
            let f: extern "C" fn(*const ClassT, Sel) = std::mem::transmute(imp);
            f(cls, initialize);
        }
        info.init.store(INIT_DONE, Ordering::Release);
    } else {
        // Another thread is initializing it: wait.
        while info.init.load(Ordering::Acquire) != INIT_DONE {
            std::thread::yield_now();
        }
    }
}

/// Whether `cls` or a superclass conforms to a protocol named `name`.
pub unsafe fn conforms(cls: *const ClassT, name: &CStr) -> bool {
    unsafe fn proto_matches(p: *const ProtocolT, name: &CStr, depth: usize) -> bool {
        if p.is_null() || depth > 32 {
            return false;
        }
        if CStr::from_ptr((*p).mangled_name) == name {
            return true;
        }
        protocols((*p).protocols).into_iter().any(|q| proto_matches(q, name, depth + 1))
    }
    let mut c = cls;
    while !c.is_null() {
        if let Some(info) = class(c) {
            if info.protocols.read().unwrap().iter().any(|&p| proto_matches(p, name, 0)) {
                return true;
            }
        }
        c = (*c).superclass;
    }
    false
}

// ---- images ----------------------------------------------------------------

/// What registering an image found.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ImageReport {
    pub selectors: usize,
    pub classes: usize,
    pub categories: usize,
    /// Classes whose superclass is not loaded (skipped).
    pub unrealized: Vec<String>,
}

unsafe fn slots(image: &pear_dyld::LoadedImage, name: &str) -> Vec<*mut usize> {
    let mut out = Vec::new();
    for s in image.sections_named(name) {
        let base = image.address(s.addr) as *mut usize;
        for i in 0..(s.size / 8) as usize {
            out.push(base.add(i));
        }
    }
    out
}

/// Registers a loaded image's Objective-C metadata: uniques its selector
/// references, realizes its classes and attaches its categories. Run
/// [`call_load_methods`] next, then the image's initializers.
///
/// # Safety
/// `image` must be a loaded, fixed-up image whose memory stays mapped.
pub unsafe fn register_image(image: &pear_dyld::LoadedImage) -> ImageReport {
    crate::object::init_runtime();
    let mut report = ImageReport::default();
    let install = CString::new(image.install_name.clone().unwrap_or_default()).unwrap_or_default();
    image_names().lock().unwrap().push(install.clone());
    CURRENT_IMAGE.with(|i| *i.borrow_mut() = Some(install));
    for slot in slots(image, "__objc_selrefs") {
        *slot = intern_c(*slot as *const c_char) as usize;
        report.selectors += 1;
    }
    {
        let mut r = registry().write().unwrap();
        for slot in slots(image, "__objc_classlist") {
            r.listed.insert(*slot);
        }
    }
    for slot in slots(image, "__objc_classlist").into_iter().chain(slots(image, "__objc_nlclslist")) {
        let cls = *slot as *const ClassT;
        if class(cls).is_some() {
            continue;
        }
        if realize(cls) {
            report.classes += 1;
        } else {
            report.unrealized.push(CStr::from_ptr((*ro(cls)).name).to_string_lossy().into_owned());
        }
    }
    for slot in slots(image, "__objc_catlist").into_iter().chain(slots(image, "__objc_catlist2")) {
        let cat = *slot as *const CategoryT;
        let cls = (*cat).cls;
        if cls.is_null() || class(cls).is_none() {
            continue;
        }
        add_methods(cls, &methods((*cat).instance_methods));
        add_methods((*cls).isa, &methods((*cat).class_methods));
        if let Some(info) = class(cls) {
            info.protocols.write().unwrap().extend(protocols((*cat).protocols));
            info.properties.write().unwrap().extend(properties((*cat).instance_properties));
        }
        report.categories += 1;
    }
    CURRENT_IMAGE.with(|i| *i.borrow_mut() = None);
    flush_cache();
    report
}

/// Calls +load of the image's classes (superclasses first) and then of its
/// categories, each method itself (not inherited), as objc4 does before the
/// image's initializers run.
///
/// # Safety
/// Runs the image's code.
pub unsafe fn call_load_methods(image: &pear_dyld::LoadedImage) {
    let load = sel("load");
    let mut done: Vec<usize> = Vec::new();
    unsafe fn class_load(cls: *const ClassT, load: Sel, done: &mut Vec<usize>) {
        if cls.is_null() || done.contains(&(cls as usize)) || class(cls).is_none() {
            return;
        }
        class_load((*cls).superclass, load, done);
        done.push(cls as usize);
        // The class's own +load as compiled (ro), not one a category added.
        // The runtime's built-in classes have no ro and no +load to call.
        let meta_ro = ro((*cls).isa);
        if meta_ro.is_null() {
            return;
        }
        let own = methods((*meta_ro).base_methods).into_iter().find(|m| m.name == load);
        if let Some(MethodDesc { imp, .. }) = own {
            let f: extern "C" fn(*const ClassT, Sel) = std::mem::transmute(imp);
            f(cls, load);
        }
    }
    for slot in slots(image, "__objc_nlclslist").into_iter().chain(slots(image, "__objc_classlist")) {
        class_load(*slot as *const ClassT, load, &mut done);
    }
    // A category with +load is listed in both __objc_nlcatlist and
    // __objc_catlist: call it once.
    let mut seen: Vec<usize> = Vec::new();
    for slot in slots(image, "__objc_nlcatlist").into_iter().chain(slots(image, "__objc_catlist")) {
        let cat = *slot as *const CategoryT;
        if (*cat).cls.is_null() || seen.contains(&(cat as usize)) {
            continue;
        }
        seen.push(cat as usize);
        if let Some(MethodDesc { imp, .. }) = methods((*cat).class_methods).into_iter().find(|m| m.name == load) {
            let f: extern "C" fn(*const ClassT, Sel) = std::mem::transmute(imp);
            f((*cat).cls, load);
        }
    }
}
