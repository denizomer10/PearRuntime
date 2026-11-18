//! Offline cloud compatibility for apps that gate startup on an account.
//!
//! A locally prepared build has no restricted cloud entitlements, so the real
//! cloud stack can report "no account" and keep an app on a sign-in screen
//! forever. This module provides a **local-only** account surface:
//!
//!   - the key-value store is backed by a plist in the app's own container
//!     (or by the PearHost key-value service when PEAR_HOST_SOCKET is set);
//!   - the ubiquity container and identity token resolve to that container;
//!   - GameKit's local player reports authenticated, with stable local IDs;
//!   - CloudKit's account status reports available and the container/database
//!     objects answer requests with an offline result instead of blocking.
//!
//! Nothing here contacts a remote service, creates an account, or reuses real
//! credentials. Opt out with PEAR_NO_CLOUD_SHIM=1. Diagnostics: PEAR_DEBUG_CLOUD=1.

use crate::core::host;
use crate::rt;
use pear_objc::{cls, msg0, msg1, msg2, nsstring, objc_release, objc_retain, sel, Class, Id, Imp, Sel};
use std::ffi::{c_char, c_void, CString};
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::{Mutex, OnceLock};

const NIL: Id = std::ptr::null_mut();

fn debug() -> bool {
    rt::env_set("PEAR_DEBUG_CLOUD")
}

/// A retained object kept for the life of the process.
#[derive(Clone, Copy)]
struct Keep(usize);
unsafe impl Send for Keep {}
unsafe impl Sync for Keep {}
impl Keep {
    unsafe fn new(obj: Id) -> Keep {
        Keep(objc_retain(obj) as usize)
    }
    fn id(self) -> Id {
        self.0 as Id
    }
}

unsafe fn bundle_id() -> String {
    let b: Id = msg0(msg0::<Id>(cls("NSBundle"), "mainBundle"), "bundleIdentifier");
    if b.is_null() { "unknown".into() } else { rt::ns_string(b) }
}

unsafe fn create_directory(path: &str) {
    let _: bool = pear_objc::msg4(rt::file_manager(), "createDirectoryAtPath:withIntermediateDirectories:attributes:error:", nsstring(path), true, NIL, NIL);
}

/// The same per-app root the path module uses, including its override.
fn cloud_root() -> &'static str {
    static ROOT: OnceLock<String> = OnceLock::new();
    ROOT.get_or_init(|| unsafe {
        let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let base = match env("PEAR_HOME_DIR") {
            Some(h) => h,
            None => {
                let id = bundle_id();
                match env("PEAR_SUPPORT_ROOT") {
                    Some(root) => rt::path_join(&rt::path_join(&root, "Games"), &id),
                    None => {
                        let home = env("HOME").unwrap_or_else(|| "/tmp".into());
                        rt::path_join(&rt::path_join(&home, "Library/Application Support/PearSilicon/Games"), &id)
                    }
                }
            }
        };
        let root = rt::path_join(&base, "Library/Cloud");
        create_directory(&root);
        root
    })
}

fn kv_path() -> String {
    rt::path_join(cloud_root(), "key-values.plist")
}

unsafe fn ubiquity_path() -> String {
    let p = rt::path_join(cloud_root(), "Mobile Documents");
    create_directory(&p);
    p
}

// ---------------------------------------------------------------------------
// Local key-value store

struct KvState {
    kv: Id, // retained NSMutableDictionary, nil until loaded
    path: Option<String>,
    unconfirmed: Id, // retained NSMutableSet of keys, nil until needed
}
unsafe impl Send for KvState {}

/// Legacy (plist) mode lock; host mode uses the semaphore gate instead and
/// takes this mutex only for the short in-memory bookkeeping it owns.
static KV: Mutex<KvState> = Mutex::new(KvState { kv: NIL, path: None, unconfirmed: NIL });

fn host_gate() -> Id {
    static GATE: OnceLock<Keep> = OnceLock::new();
    extern "C" {
        fn dispatch_semaphore_create(value: isize) -> Id;
    }
    GATE.get_or_init(|| unsafe { Keep(dispatch_semaphore_create(1) as usize) }).id()
}

extern "C" {
    fn dispatch_semaphore_signal(sema: Id) -> isize;
}

/// A setter that cannot acquire the gate still records failure without
/// waiting for a lock; only the gate owner consumes this list.
struct Rejected {
    key: Id,
    next: *mut Rejected,
}
static REJECTED: AtomicPtr<Rejected> = AtomicPtr::new(std::ptr::null_mut());

unsafe fn reject_host_write(key: Id) {
    let w = Box::into_raw(Box::new(Rejected { key: msg0(key, "copy"), next: std::ptr::null_mut() }));
    let mut head = REJECTED.load(Ordering::Relaxed);
    loop {
        (*w).next = head;
        match REJECTED.compare_exchange_weak(head, w, Ordering::Release, Ordering::Relaxed) {
            Ok(_) => break,
            Err(h) => head = h,
        }
    }
}

unsafe fn collect_rejected(s: &mut KvState) {
    let mut w = REJECTED.swap(std::ptr::null_mut(), Ordering::Acquire);
    if !w.is_null() && s.unconfirmed.is_null() {
        s.unconfirmed = objc_retain(msg0(cls("NSMutableSet"), "set"));
    }
    while !w.is_null() {
        let b = Box::from_raw(w);
        msg1::<Id, ()>(s.unconfirmed, "addObject:", b.key);
        objc_release(b.key);
        w = b.next;
    }
}

unsafe fn load_kv(s: &mut KvState) {
    if !s.kv.is_null() {
        return;
    }
    let empty = || objc_retain(msg0(cls("NSMutableDictionary"), "dictionary"));
    if host::is_configured() {
        // Only a cache of confirmed host values: the host is the exclusive writer.
        s.kv = empty();
        return;
    }
    let path = kv_path();
    s.path = Some(path.clone());
    let data: Id = msg1(cls("NSData"), "dataWithContentsOfFile:", nsstring(&path));
    if data.is_null() {
        // A first launch has no file; the plist parser raises on nil data.
        s.kv = empty();
        return;
    }
    // An unreadable store starts empty (the parser reports, never raises, here).
    let stored: Id = pear_objc::msg4(cls("NSPropertyListSerialization"), "propertyListWithData:options:format:error:", data, 2usize, NIL, NIL);
    s.kv = if !stored.is_null() && pear_objc::is_kind_of(stored, "NSMutableDictionary") { objc_retain(stored) } else { empty() };
}

unsafe fn save_kv(s: &KvState) {
    if host::is_configured() || s.kv.is_null() {
        return;
    }
    let data: Id = pear_objc::msg4(cls("NSPropertyListSerialization"), "dataWithPropertyList:format:options:error:", s.kv, 200usize, 0usize, NIL);
    if let (false, Some(path)) = (data.is_null(), s.path.as_deref()) {
        let _: bool = msg2(data, "writeToFile:atomically:", nsstring(path), true);
    }
}

unsafe fn ok(response: Id) -> bool {
    if response.is_null() {
        return false;
    }
    let v: Id = msg1(response, "objectForKey:", nsstring("ok"));
    !v.is_null() && msg0::<bool>(v, "boolValue")
}

unsafe fn request(pairs: &[(&str, Id)]) -> Id {
    let d: Id = msg0(cls("NSMutableDictionary"), "dictionary");
    for (k, v) in pairs {
        if !v.is_null() {
            msg2::<Id, Id, ()>(d, "setObject:forKey:", *v, nsstring(k));
        }
    }
    d
}

unsafe fn refresh_host_kv(s: &mut KvState, deadline: u64) -> bool {
    let response = host::call_until(request(&[("op", nsstring("kv.all"))]), deadline);
    if !ok(response) {
        return false;
    }
    // Replace, do not merge: absent keys in a confirmed snapshot are deletions.
    let value: Id = msg1(response, "objectForKey:", nsstring("value"));
    let copy: Id = if value.is_null() { NIL } else { msg0(value, "mutableCopy") };
    if !s.kv.is_null() {
        objc_release(s.kv);
    }
    s.kv = copy; // +1 from mutableCopy (nil when the host sent none)
    true
}

/// Runs `f` while owning the host gate; None when the gate was not acquired.
unsafe fn with_gate<R>(deadline: u64, f: impl FnOnce(&mut KvState) -> R) -> Option<R> {
    if !host::acquire_until(host_gate(), deadline) {
        return None;
    }
    let r = {
        let mut s = KV.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut s)
    };
    dispatch_semaphore_signal(host_gate());
    Some(r)
}

unsafe fn kv_get(s: &KvState, key: Id) -> Id {
    if s.kv.is_null() { NIL } else { msg1(s.kv, "objectForKey:", key) }
}

unsafe fn kv_put(s: &KvState, key: Id, value: Id) {
    if s.kv.is_null() {
        return;
    }
    if value.is_null() {
        msg1::<Id, ()>(s.kv, "removeObjectForKey:", key);
    } else {
        msg2::<Id, Id, ()>(s.kv, "setObject:forKey:", value, key);
    }
}

unsafe fn kv_object(key: Id) -> Id {
    let deadline = host::deadline();
    if key.is_null() {
        return NIL;
    }
    if host::is_configured() {
        return with_gate(deadline, |s| {
            collect_rejected(s);
            load_kv(s);
            let response = host::call_until(request(&[("op", nsstring("kv.get")), ("key", key)]), deadline);
            if ok(response) {
                let value: Id = msg1(response, "objectForKey:", nsstring("value"));
                kv_put(s, key, value);
                return value;
            }
            // An outage may serve the confirmed cache only while we own it.
            rt::autoreleased(objc_retain(kv_get(s, key)))
        })
        .unwrap_or(NIL);
    }
    let mut s = KV.lock().unwrap_or_else(|e| e.into_inner());
    load_kv(&mut s);
    rt::autoreleased(objc_retain(kv_get(&s, key)))
}

unsafe fn kv_set(key: Id, value: Id) -> bool {
    extern "C" {
        fn CFPropertyListCreateDeepCopy(allocator: *const c_void, list: Id, options: usize) -> Id;
    }
    let deadline = host::deadline();
    if key.is_null() {
        return false;
    }
    if host::is_configured() {
        let r = with_gate(deadline, |s| {
            // Collect only before this write: failures queued during its IPC
            // must not be cleared by this earlier operation's ACK.
            collect_rejected(s);
            load_kv(s);
            // Cache and send the same immutable snapshot, independent of
            // the caller's mutable containers.
            let mut snapshot: Id = NIL;
            if !value.is_null() && msg2::<Id, usize, bool>(cls("NSPropertyListSerialization"), "propertyList:isValidForFormat:", value, 200) {
                snapshot = rt::autoreleased(CFPropertyListCreateDeepCopy(std::ptr::null(), value, 0));
            }
            // Omission deletes, never NSNull.
            let saved = (value.is_null() || !snapshot.is_null())
                && ok(host::call_until(request(&[("op", nsstring("kv.set")), ("key", key), ("value", snapshot)]), deadline));
            if !saved {
                if s.unconfirmed.is_null() {
                    s.unconfirmed = objc_retain(msg0(cls("NSMutableSet"), "set"));
                }
                msg1::<Id, ()>(s.unconfirmed, "addObject:", key);
                return false;
            }
            if !s.unconfirmed.is_null() {
                msg1::<Id, ()>(s.unconfirmed, "removeObject:", key);
            }
            kv_put(s, key, snapshot);
            true
        });
        return match r {
            Some(v) => v,
            None => {
                reject_host_write(key);
                false
            }
        };
    }
    let mut s = KV.lock().unwrap_or_else(|e| e.into_inner());
    load_kv(&mut s);
    kv_put(&s, key, value);
    save_kv(&s);
    true
}

unsafe fn kv_copy(s: &KvState) -> Id {
    if s.kv.is_null() { NIL } else { rt::autoreleased(msg0(s.kv, "copy")) }
}

unsafe fn kv_representation() -> Id {
    let deadline = host::deadline();
    if host::is_configured() {
        return with_gate(deadline, |s| {
            collect_rejected(s);
            load_kv(s);
            refresh_host_kv(s, deadline);
            kv_copy(s)
        })
        .unwrap_or_else(|| msg0(cls("NSDictionary"), "dictionary"));
    }
    let mut s = KV.lock().unwrap_or_else(|e| e.into_inner());
    load_kv(&mut s);
    kv_copy(&s)
}

unsafe fn kv_synchronize() -> bool {
    let deadline = host::deadline();
    if host::is_configured() {
        return with_gate(deadline, |s| {
            collect_rejected(s);
            // Every host set persists before its ACK; a healthy read cannot
            // turn a rejected or lost write into success.
            let healthy = refresh_host_kv(s, deadline);
            let pending = if s.unconfirmed.is_null() { 0 } else { msg0::<usize>(s.unconfirmed, "count") };
            healthy && pending == 0 && REJECTED.load(Ordering::Acquire).is_null()
        })
        .unwrap_or(false);
    }
    let s = KV.lock().unwrap_or_else(|e| e.into_inner());
    save_kv(&s);
    true
}

unsafe extern "C" fn kv_object_imp(_: Id, _: Sel, key: Id) -> Id {
    kv_object(key)
}
unsafe extern "C" fn kv_set_imp(_: Id, _: Sel, value: Id, key: Id) {
    kv_set(key, value);
}
unsafe extern "C" fn kv_remove_imp(_: Id, _: Sel, key: Id) {
    kv_set(key, NIL);
}
unsafe extern "C" fn kv_representation_imp(_: Id, _: Sel) -> Id {
    kv_representation()
}
unsafe extern "C" fn kv_synchronize_imp(_: Id, _: Sel) -> bool {
    kv_synchronize()
}

unsafe fn kind_or_nil(value: Id, class: &str) -> Id {
    if !value.is_null() && pear_objc::is_kind_of(value, class) { value } else { NIL }
}
unsafe extern "C" fn kv_string(_: Id, _: Sel, key: Id) -> Id {
    kind_or_nil(kv_object(key), "NSString")
}
unsafe extern "C" fn kv_array(_: Id, _: Sel, key: Id) -> Id {
    kind_or_nil(kv_object(key), "NSArray")
}
unsafe extern "C" fn kv_dictionary(_: Id, _: Sel, key: Id) -> Id {
    kind_or_nil(kv_object(key), "NSDictionary")
}
unsafe extern "C" fn kv_data(_: Id, _: Sel, key: Id) -> Id {
    kind_or_nil(kv_object(key), "NSData")
}
unsafe extern "C" fn kv_long_long(_: Id, _: Sel, key: Id) -> i64 {
    let v = kv_object(key);
    if !v.is_null() && pear_objc::responds_to(v, "longLongValue") { msg0(v, "longLongValue") } else { 0 }
}
unsafe extern "C" fn kv_double(_: Id, _: Sel, key: Id) -> f64 {
    let v = kv_object(key);
    if !v.is_null() && pear_objc::responds_to(v, "doubleValue") { msg0(v, "doubleValue") } else { 0.0 }
}
unsafe extern "C" fn kv_bool(_: Id, _: Sel, key: Id) -> bool {
    let v = kv_object(key);
    !v.is_null() && pear_objc::responds_to(v, "boolValue") && msg0::<bool>(v, "boolValue")
}
unsafe extern "C" fn kv_set_long_long(_: Id, _: Sel, value: i64, key: Id) {
    kv_set(key, pear_objc::nsnumber_i64(value));
}
unsafe extern "C" fn kv_set_double(_: Id, _: Sel, value: f64, key: Id) {
    kv_set(key, pear_objc::nsnumber_f64(value));
}
unsafe extern "C" fn kv_set_bool(_: Id, _: Sel, value: bool, key: Id) {
    kv_set(key, pear_objc::nsnumber_bool(value));
}

unsafe fn set_instance(c: Class, selector: &str, imp: Imp) {
    let m = rt::class_getInstanceMethod(c, sel(selector));
    if !m.is_null() {
        rt::method_setImplementation(m, imp);
    }
}

unsafe fn install_key_value_store() {
    let c = rt::class_named("NSUbiquitousKeyValueStore");
    if c.is_null() {
        return;
    }
    set_instance(c, "objectForKey:", kv_object_imp as Imp);
    set_instance(c, "setObject:forKey:", kv_set_imp as Imp);
    set_instance(c, "removeObjectForKey:", kv_remove_imp as Imp);
    set_instance(c, "dictionaryRepresentation", kv_representation_imp as Imp);
    set_instance(c, "synchronize", kv_synchronize_imp as Imp);
    // The convenience accessors too, so older callers never touch the real store.
    set_instance(c, "stringForKey:", kv_string as Imp);
    set_instance(c, "arrayForKey:", kv_array as Imp);
    set_instance(c, "dictionaryForKey:", kv_dictionary as Imp);
    set_instance(c, "dataForKey:", kv_data as Imp);
    set_instance(c, "longLongForKey:", kv_long_long as Imp);
    set_instance(c, "doubleForKey:", kv_double as Imp);
    set_instance(c, "boolForKey:", kv_bool as Imp);
    set_instance(c, "setString:forKey:", kv_set_imp as Imp);
    set_instance(c, "setArray:forKey:", kv_set_imp as Imp);
    set_instance(c, "setDictionary:forKey:", kv_set_imp as Imp);
    set_instance(c, "setData:forKey:", kv_set_imp as Imp);
    set_instance(c, "setLongLong:forKey:", kv_set_long_long as Imp);
    set_instance(c, "setDouble:forKey:", kv_set_double as Imp);
    set_instance(c, "setBool:forKey:", kv_set_bool as Imp);
    if host::is_configured() {
        rt::log("[PearRuntime][cloud] host-authoritative key-value store (no local fallback)");
    } else {
        let path = KV.lock().unwrap_or_else(|e| e.into_inner()).path.clone().unwrap_or_else(kv_path);
        rt::log(&format!("[PearRuntime][cloud] local key-value store at {path}"));
    }
}

// ---------------------------------------------------------------------------
// Ubiquity container

unsafe extern "C" fn ubiquity_url(_: Id, _: Sel, _identifier: Id) -> Id {
    msg2(cls("NSURL"), "fileURLWithPath:isDirectory:", nsstring(&ubiquity_path()), true)
}

/// Stable local identity bytes (iOS SDKs may Base64-encode this opaque token).
unsafe extern "C" fn ubiquity_token(_: Id, _: Sel) -> Id {
    let identity = format!("pearsilicon-local-{}", bundle_id());
    pear_objc::nsdata_from_bytes(identity.as_bytes())
}

/// The container is an ordinary local directory: every item is present.
unsafe extern "C" fn ubiquity_yes(_: Id, _: Sel, _url: Id, error: *mut Id) -> bool {
    if !error.is_null() {
        *error = NIL;
    }
    true
}

unsafe fn install_ubiquity_container() {
    let fm = cls("NSFileManager");
    set_instance(fm, "URLForUbiquityContainerIdentifier:", ubiquity_url as Imp);
    set_instance(fm, "ubiquityIdentityToken", ubiquity_token as Imp);
    set_instance(fm, "isUbiquitousItemAtURL:error:", ubiquity_yes as Imp);
    set_instance(fm, "startDownloadingUbiquitousItemAtURL:error:", ubiquity_yes as Imp);
    set_instance(fm, "evictUbiquitousItemAtURL:error:", ubiquity_yes as Imp);
    rt::log(&format!("[PearRuntime][cloud] ubiquity container at {}", ubiquity_path()));
}

// ---------------------------------------------------------------------------
// CloudKit compatibility objects

unsafe fn ensure_class(name: &str, framework: &str) -> Class {
    let c = rt::class_named(name);
    if !c.is_null() {
        return c;
    }
    for prefix in ["/System/iOSSupport", ""] {
        rt::dlopen_path(&format!("{prefix}/System/Library/Frameworks/{framework}.framework/{framework}"), rt::RTLD_LAZY | rt::RTLD_GLOBAL);
        let c = rt::class_named(name);
        if !c.is_null() {
            return c;
        }
    }
    std::ptr::null_mut()
}

unsafe fn cloudkit(name: &str) -> Class {
    ensure_class(name, "CloudKit")
}

unsafe fn gamekit(name: &str) -> Class {
    ensure_class(name, "GameKit")
}

extern "C" {
    fn method_getTypeEncoding(m: rt::Method) -> *const c_char;
}

/// Adds with the inherited method's encoding when there is one.
/// `[c instancesRespondToSelector:s]`.
unsafe fn class_instancesRespondToSelector(c: Class, s: Sel) -> bool {
    msg1(c, "instancesRespondToSelector:", s)
}

unsafe fn add(c: Class, selector: &str, imp: Imp, fallback: &str) {
    let s = sel(selector);
    let existing = rt::class_getInstanceMethod(c, s);
    let fb = CString::new(fallback).unwrap_or_default();
    let types = if existing.is_null() { fb.as_ptr() } else { method_getTypeEncoding(existing) };
    rt::class_addMethod(c, s, imp, if types.is_null() { fb.as_ptr() } else { types });
}

unsafe fn offline_error() -> Id {
    rt::error("CKErrorDomain", 6, "The local cloud compatibility layer is offline.")
}

/// `valueForKey:` limited to keys the object answers (KVC raises otherwise).
unsafe fn value(object: Id, key: &str) -> Id {
    if object.is_null() || !pear_objc::responds_to(object, key) {
        return NIL;
    }
    msg1(object, "valueForKey:", nsstring(key))
}

unsafe fn array_value(object: Id, key: &str) -> Id {
    let v = value(object, key);
    if !v.is_null() && pear_objc::is_kind_of(v, "NSArray") { v } else { msg0(cls("NSArray"), "array") }
}

unsafe fn each(array: Id, mut f: impl FnMut(Id)) {
    for i in 0..pear_objc::nsarray_count(array) {
        f(pear_objc::nsarray_object_at(array, i));
    }
}

/// Local object store keyed by an identifier's description.
struct Store(Mutex<Keep>);
impl Store {
    unsafe fn dict(&self) -> std::sync::MutexGuard<'_, Keep> {
        let mut g = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if g.0 == 0 {
            *g = Keep::new(msg0(cls("NSMutableDictionary"), "dictionary"));
        }
        g
    }
    unsafe fn put(&self, key: Id, obj: Id) {
        if key.is_null() || obj.is_null() {
            return;
        }
        let d = self.dict();
        msg2::<Id, Id, ()>(d.id(), "setObject:forKey:", obj, msg0::<Id>(key, "description"));
    }
    unsafe fn get(&self, key: Id) -> Id {
        if key.is_null() {
            return NIL;
        }
        let d = self.dict();
        rt::autoreleased(objc_retain(msg1(d.id(), "objectForKey:", msg0::<Id>(key, "description"))))
    }
    unsafe fn remove(&self, key: Id) {
        if key.is_null() {
            return;
        }
        let d = self.dict();
        msg1::<Id, ()>(d.id(), "removeObjectForKey:", msg0::<Id>(key, "description"));
    }
    unsafe fn all(&self) -> Id {
        let d = self.dict();
        msg0(d.id(), "allValues")
    }
    unsafe fn snapshot(&self) -> Id {
        let d = self.dict();
        rt::autoreleased(msg0(d.id(), "copy"))
    }
}

static ZONES: Store = Store(Mutex::new(Keep(0)));
static RECORDS: Store = Store(Mutex::new(Keep(0)));
static SUBSCRIPTIONS: Store = Store(Mutex::new(Keep(0)));

unsafe fn store_zone(zone: Id) {
    ZONES.put(value(zone, "zoneID"), zone);
}
unsafe fn store_record(record: Id) {
    RECORDS.put(value(record, "recordID"), record);
}
unsafe fn store_subscription(subscription: Id) {
    SUBSCRIPTIONS.put(value(subscription, "subscriptionID"), subscription);
}

/// A local record-zone object for an identifier (create, save, fetch back).
unsafe fn zone_with_id(zone_id: Id) -> Id {
    let c = cloudkit("CKRecordZone");
    if c.is_null() || zone_id.is_null() || !class_instancesRespondToSelector(c, sel("initWithZoneID:")) {
        return NIL;
    }
    rt::autoreleased(msg1(msg0::<Id>(c, "alloc"), "initWithZoneID:", zone_id))
}

unsafe fn local_record(record_id: Id) -> Id {
    let c = cloudkit("CKRecord");
    if c.is_null() || record_id.is_null() || !class_instancesRespondToSelector(c, sel("initWithRecordType:recordID:")) {
        return NIL;
    }
    rt::autoreleased(msg2(msg0::<Id>(c, "alloc"), "initWithRecordType:recordID:", nsstring("LocalRecord"), record_id))
}

unsafe fn stored_or_new_record(record_id: Id) -> Id {
    let mut record = RECORDS.get(record_id);
    if record.is_null() && !record_id.is_null() {
        record = local_record(record_id);
        if !record.is_null() {
            store_record(record);
        }
    }
    record
}

/// Local stand-ins for the records a fetch asked for (callers index the result).
unsafe fn placeholder_records(operation: Id) -> Id {
    let records: Id = msg0(cls("NSMutableDictionary"), "dictionary");
    each(array_value(operation, "recordIDs"), |record_id| {
        let record = stored_or_new_record(record_id);
        if !record.is_null() && !record_id.is_null() {
            msg2::<Id, Id, ()>(records, "setObject:forKey:", record, record_id);
        }
    });
    records
}

unsafe fn placeholder_zones(operation: Id) -> Id {
    let ids = array_value(operation, "recordZoneIDs");
    let zones: Id = msg0(cls("NSMutableDictionary"), "dictionary");
    if pear_objc::nsarray_count(ids) == 0 {
        each(ZONES.all(), |zone| {
            let id = value(zone, "zoneID");
            if !id.is_null() {
                msg2::<Id, Id, ()>(zones, "setObject:forKey:", zone, id);
            }
        });
        return zones;
    }
    each(ids, |id| {
        let mut zone = ZONES.get(id);
        if zone.is_null() {
            zone = zone_with_id(id);
        }
        if !zone.is_null() && !id.is_null() {
            store_zone(zone);
            msg2::<Id, Id, ()>(zones, "setObject:forKey:", zone, id);
        }
    });
    zones
}

unsafe fn subscription_results(operation: Id) -> Id {
    let ids = array_value(operation, "subscriptionIDs");
    let all = SUBSCRIPTIONS.snapshot();
    let result: Id = msg0(cls("NSMutableDictionary"), "dictionary");
    if pear_objc::nsarray_count(ids) == 0 {
        msg1::<Id, ()>(result, "addEntriesFromDictionary:", all);
    } else {
        each(ids, |id| {
            if id.is_null() {
                return;
            }
            let s: Id = msg1(all, "objectForKey:", msg0::<Id>(id, "description"));
            if !s.is_null() {
                msg2::<Id, Id, ()>(result, "setObject:forKey:", s, id);
            }
        });
    }
    result
}

type B1 = unsafe extern "C" fn(Id, Id);
type B2 = unsafe extern "C" fn(Id, Id, Id);
type B3 = unsafe extern "C" fn(Id, Id, Id, Id);
type BIntErr = unsafe extern "C" fn(Id, isize, Id);
type BIdBoolErr = unsafe extern "C" fn(Id, Id, bool, Id);
type B0 = unsafe extern "C" fn(Id);

unsafe fn call1(block: Id, a: Id) {
    if let Some(f) = rt::block_invoke::<B1>(block) {
        f(block, a)
    }
}
unsafe fn call2(block: Id, a: Id, b: Id) {
    if let Some(f) = rt::block_invoke::<B2>(block) {
        f(block, a, b)
    }
}
unsafe fn call3(block: Id, a: Id, b: Id, c: Id) {
    if let Some(f) = rt::block_invoke::<B3>(block) {
        f(block, a, b, c)
    }
}
unsafe fn call_int(block: Id, v: isize, e: Id) {
    if let Some(f) = rt::block_invoke::<BIntErr>(block) {
        f(block, v, e)
    }
}

/// Answers a cloud operation's result block while the compatibility object
/// holds it; the real operation never starts, so no request leaves the Mac.
unsafe fn finish_operation(operation: Id) {
    if operation.is_null() {
        return;
    }
    let name = pear_objc::class_name(operation);
    if debug() {
        rt::log(&format!("[PearRuntime][cloud] finishing {name}"));
    }
    match name {
        "CKQueryOperation" => {
            let mut b = value(operation, "queryResultBlock");
            if b.is_null() {
                b = value(operation, "queryCompletionBlock");
            }
            call2(b, NIL, NIL);
        }
        "CKModifyRecordsOperation" => {
            let saved = array_value(operation, "recordsToSave");
            let deleted = array_value(operation, "recordIDsToDelete");
            each(saved, |r| store_record(r));
            each(deleted, |r| RECORDS.remove(r));
            call3(value(operation, "modifyRecordsCompletionBlock"), saved, deleted, NIL);
        }
        "CKFetchRecordsOperation" => {
            let b = value(operation, "fetchRecordsCompletionBlock");
            if !b.is_null() {
                call2(b, placeholder_records(operation), NIL);
            }
        }
        "CKFetchRecordZonesOperation" => {
            let b = value(operation, "fetchRecordZonesCompletionBlock");
            if !b.is_null() {
                call2(b, placeholder_zones(operation), NIL);
            }
        }
        "CKModifyRecordZonesOperation" => {
            let saved = array_value(operation, "recordZonesToSave");
            let deleted = array_value(operation, "recordZoneIDsToDelete");
            each(saved, |z| store_zone(z));
            each(deleted, |z| ZONES.remove(z));
            call3(value(operation, "modifyRecordZonesCompletionBlock"), saved, deleted, NIL);
        }
        "CKFetchSubscriptionsOperation" => {
            let b = value(operation, "fetchSubscriptionsCompletionBlock");
            if !b.is_null() {
                call2(b, subscription_results(operation), NIL);
            }
        }
        "CKModifySubscriptionsOperation" => {
            let saved = array_value(operation, "subscriptionsToSave");
            let deleted = array_value(operation, "subscriptionIDsToDelete");
            each(saved, |s| store_subscription(s));
            each(deleted, |s| SUBSCRIPTIONS.remove(s));
            call3(value(operation, "modifySubscriptionsCompletionBlock"), saved, deleted, NIL);
        }
        "CKFetchRecordZoneChangesOperation" => call1(value(operation, "fetchRecordZoneChangesCompletionBlock"), NIL),
        "CKFetchDatabaseChangesOperation" => {
            let b = value(operation, "fetchDatabaseChangesCompletionBlock");
            if let Some(f) = rt::block_invoke::<BIdBoolErr>(b) {
                f(b, NIL, false, NIL);
            }
        }
        _ => {}
    }
    let completion: Id = msg0(operation, "completionBlock");
    if let Some(f) = rt::block_invoke::<B0>(completion) {
        f(completion);
    }
}

unsafe extern "C" fn add_operation(_: Id, _: Sel, operation: Id) {
    if operation.is_null() {
        return;
    }
    let op = Keep::new(operation);
    rt::async_main(move || unsafe {
        pear_objc::autorelease_pool(|| finish_operation(op.id()));
        objc_release(op.id());
    });
}

unsafe extern "C" fn return_self(this: Id, _: Sel) -> Id {
    this
}
unsafe extern "C" fn database_scope(_: Id, _: Sel) -> isize {
    2 // private
}
unsafe extern "C" fn db_save_record(_: Id, _: Sel, record: Id, done: Id) {
    store_record(record);
    call2(done, record, NIL);
}
unsafe extern "C" fn db_fetch_record(_: Id, _: Sel, record_id: Id, done: Id) {
    let record = stored_or_new_record(record_id);
    call2(done, record, NIL);
}
unsafe extern "C" fn db_delete_record(_: Id, _: Sel, record_id: Id, done: Id) {
    RECORDS.remove(record_id);
    call2(done, NIL, NIL);
}
unsafe extern "C" fn db_fetch_zone(_: Id, _: Sel, zone_id: Id, done: Id) {
    let mut zone = ZONES.get(zone_id);
    if zone.is_null() {
        zone = zone_with_id(zone_id);
    }
    if !zone.is_null() {
        store_zone(zone);
    }
    call2(done, zone, NIL);
}
unsafe extern "C" fn db_save_zone(_: Id, _: Sel, zone: Id, done: Id) {
    store_zone(zone);
    call2(done, zone, NIL);
}
unsafe extern "C" fn db_delete_zone(_: Id, _: Sel, zone_id: Id, done: Id) {
    ZONES.remove(zone_id);
    call2(done, zone_id, NIL);
}
unsafe extern "C" fn db_all_zones(_: Id, _: Sel, done: Id) {
    call2(done, ZONES.all(), NIL);
}
unsafe extern "C" fn db_all_subscriptions(_: Id, _: Sel, done: Id) {
    call2(done, SUBSCRIPTIONS.all(), NIL);
}
/// The store does not evaluate predicates: callers get the local records.
unsafe extern "C" fn db_perform_query(_: Id, _: Sel, _query: Id, _zone: Id, done: Id) {
    call2(done, RECORDS.all(), NIL);
}
unsafe extern "C" fn db_fetch_subscription(_: Id, _: Sel, id: Id, done: Id) {
    call2(done, SUBSCRIPTIONS.get(id), NIL);
}
unsafe extern "C" fn db_save_subscription(_: Id, _: Sel, subscription: Id, done: Id) {
    store_subscription(subscription);
    call2(done, subscription, NIL);
}
unsafe extern "C" fn db_delete_subscription(_: Id, _: Sel, id: Id, done: Id) {
    SUBSCRIPTIONS.remove(id);
    call2(done, id, NIL);
}

unsafe fn database_class() -> Class {
    let base = cloudkit("CKDatabase");
    if base.is_null() {
        return std::ptr::null_mut();
    }
    let existing = rt::class_named("PearCloudDatabase");
    if !existing.is_null() {
        return existing;
    }
    let Some(b) = rt::ClassBuilder::new::<()>("PearCloudDatabase", base) else { return std::ptr::null_mut() };
    let c = b.class();
    add(c, "init", return_self as Imp, "@@:");
    add(c, "databaseScope", database_scope as Imp, "q@:");
    add(c, "saveRecord:completionHandler:", db_save_record as Imp, "v@:@@?");
    add(c, "fetchRecordWithID:completionHandler:", db_fetch_record as Imp, "v@:@@?");
    add(c, "deleteRecordWithID:completionHandler:", db_delete_record as Imp, "v@:@@?");
    add(c, "fetchRecordZoneWithID:completionHandler:", db_fetch_zone as Imp, "v@:@@?");
    add(c, "saveRecordZone:completionHandler:", db_save_zone as Imp, "v@:@@?");
    add(c, "deleteRecordZoneWithID:completionHandler:", db_delete_zone as Imp, "v@:@@?");
    add(c, "fetchAllRecordZonesWithCompletionHandler:", db_all_zones as Imp, "v@:@?");
    add(c, "fetchAllSubscriptionsWithCompletionHandler:", db_all_subscriptions as Imp, "v@:@?");
    add(c, "performQuery:inZoneWithID:completionHandler:", db_perform_query as Imp, "v@:@@@?");
    add(c, "fetchSubscriptionWithID:completionHandler:", db_fetch_subscription as Imp, "v@:@@?");
    add(c, "saveSubscription:completionHandler:", db_save_subscription as Imp, "v@:@@?");
    add(c, "deleteSubscriptionWithID:completionHandler:", db_delete_subscription as Imp, "v@:@@?");
    add(c, "addOperation:", add_operation as Imp, "v@:@");
    b.register()
}

unsafe fn database() -> Id {
    static DB: OnceLock<Keep> = OnceLock::new();
    DB.get_or_init(|| unsafe {
        let c = database_class();
        Keep(if c.is_null() { 0 } else { msg0::<Id>(msg0::<Id>(c, "alloc"), "init") as usize })
    })
    .id()
}

static CONTAINER_ID_KEY: u8 = 0;

unsafe extern "C" fn container_identifier(this: Id, _: Sel) -> Id {
    let id = pear_objc::assoc_get(this, &CONTAINER_ID_KEY);
    if !id.is_null() && msg0::<usize>(id, "length") > 0 {
        return id;
    }
    nsstring(&format!("iCloud.{}", bundle_id()))
}
unsafe extern "C" fn account_status(_: Id, _: Sel, done: Id) {
    rt::call_block2_on_main(done, 1, NIL); // available
}
unsafe extern "C" fn application_permission(_: Id, _: Sel, _permission: isize, done: Id) {
    call_int(done, 1, NIL); // granted
}
unsafe extern "C" fn empty_array_done(_: Id, _: Sel, done: Id) {
    call2(done, msg0(cls("NSArray"), "array"), NIL);
}
unsafe extern "C" fn nil_done_with_arg(_: Id, _: Sel, _arg: Id, done: Id) {
    call2(done, NIL, NIL);
}
unsafe extern "C" fn offline_done_with_arg(_: Id, _: Sel, _arg: Id, done: Id) {
    call2(done, NIL, offline_error());
}
unsafe extern "C" fn fetch_user_record_id(_: Id, _: Sel, done: Id) {
    let c = cloudkit("CKRecordID");
    let mut record_id: Id = NIL;
    if !c.is_null() && class_instancesRespondToSelector(c, sel("initWithRecordName:")) {
        record_id = msg1(msg0::<Id>(c, "alloc"), "initWithRecordName:", nsstring("pearsilicon-local")); // +1
    }
    if done.is_null() {
        if !record_id.is_null() {
            objc_release(record_id);
        }
        return;
    }
    let (block, record) = (Keep(rt::copy_block(done) as usize), Keep(record_id as usize));
    rt::async_main(move || unsafe {
        call2(block.id(), record.id(), NIL);
        if !record.id().is_null() {
            objc_release(record.id());
        }
        rt::release_block(block.id());
    });
}
unsafe extern "C" fn container_database(_: Id, _: Sel) -> Id {
    database()
}

unsafe fn container_class() -> Class {
    let base = cloudkit("CKContainer");
    if base.is_null() {
        return std::ptr::null_mut();
    }
    let existing = rt::class_named("PearCloudContainer");
    if !existing.is_null() {
        return existing;
    }
    let Some(b) = rt::ClassBuilder::new::<()>("PearCloudContainer", base) else { return std::ptr::null_mut() };
    let c = b.class();
    add(c, "init", return_self as Imp, "@@:");
    add(c, "containerIdentifier", container_identifier as Imp, "@@:");
    add(c, "accountStatusWithCompletionHandler:", account_status as Imp, "v@:@?");
    add(c, "statusForApplicationPermission:completionHandler:", application_permission as Imp, "v@:q@?");
    add(c, "requestApplicationPermission:completionHandler:", application_permission as Imp, "v@:q@?");
    add(c, "fetchAllLongLivedOperationIDsWithCompletionHandler:", empty_array_done as Imp, "v@:@?");
    add(c, "fetchLongLivedOperationWithID:completionHandler:", nil_done_with_arg as Imp, "v@:@@?");
    add(c, "discoverAllContactUserInfosWithCompletionHandler:", empty_array_done as Imp, "v@:@?");
    add(c, "discoverUserIdentityWithEmailAddress:completionHandler:", nil_done_with_arg as Imp, "v@:@@?");
    add(c, "discoverUserIdentityWithPhoneNumber:completionHandler:", nil_done_with_arg as Imp, "v@:@@?");
    add(c, "fetchAllShareParticipantsWithCompletionHandler:", empty_array_done as Imp, "v@:@?");
    add(c, "fetchShareMetadataWithURL:completionHandler:", offline_done_with_arg as Imp, "v@:@@?");
    add(c, "acceptShareMetadata:completionHandler:", offline_done_with_arg as Imp, "v@:@@?");
    add(c, "fetchUserRecordIDWithCompletionHandler:", fetch_user_record_id as Imp, "v@:@?");
    add(c, "privateCloudDatabase", container_database as Imp, "@@:");
    add(c, "publicCloudDatabase", container_database as Imp, "@@:");
    add(c, "sharedCloudDatabase", container_database as Imp, "@@:");
    add(c, "addOperation:", add_operation as Imp, "v@:@");
    b.register()
}

unsafe fn container(identifier: Id) -> Id {
    let c = container_class();
    if c.is_null() {
        return NIL;
    }
    let container = rt::autoreleased(msg0(msg0::<Id>(c, "alloc"), "init"));
    if !identifier.is_null() && msg0::<usize>(identifier, "length") > 0 {
        pear_objc::assoc_set_copy(container, &CONTAINER_ID_KEY, identifier);
    }
    container
}

unsafe extern "C" fn container_with_identifier(_: Id, _: Sel, identifier: Id) -> Id {
    container(identifier)
}
unsafe extern "C" fn default_container(_: Id, _: Sel) -> Id {
    container(NIL)
}

unsafe fn set_class_method(c: Class, selector: &str, imp: Imp) {
    let m = rt::class_getClassMethod(c, sel(selector));
    if !m.is_null() {
        rt::method_setImplementation(m, imp);
    }
}

unsafe fn install_cloudkit() {
    let c = cloudkit("CKContainer");
    if c.is_null() {
        return;
    }
    set_class_method(c, "containerWithIdentifier:", container_with_identifier as Imp);
    set_class_method(c, "defaultContainer", default_container as Imp);
    if debug() {
        rt::log("[PearRuntime][cloud] CloudKit compatibility container installed");
    }
}

// ---------------------------------------------------------------------------
// GameKit local player

static AUTH_HANDLER: Mutex<Keep> = Mutex::new(Keep(0));

unsafe extern "C" fn yes(_: Id, _: Sel) -> bool {
    true
}
unsafe extern "C" fn no(_: Id, _: Sel) -> bool {
    false
}
unsafe extern "C" fn player_id(_: Id, _: Sel) -> Id {
    nsstring("G:pearsilicon-local")
}
unsafe extern "C" fn player_name(_: Id, _: Sel) -> Id {
    nsstring("Player")
}
unsafe extern "C" fn authenticate_handler(_: Id, _: Sel) -> Id {
    let h = AUTH_HANDLER.lock().unwrap_or_else(|e| e.into_inner()).id();
    rt::autoreleased(objc_retain(h))
}
unsafe extern "C" fn set_authenticate_handler(_: Id, _: Sel, handler: Id) {
    {
        let mut g = AUTH_HANDLER.lock().unwrap_or_else(|e| e.into_inner());
        let old = g.id();
        *g = Keep(rt::copy_block(handler) as usize);
        rt::release_block(old);
    }
    if !handler.is_null() {
        rt::call_block2_on_main(handler, 0, NIL);
    }
}
unsafe extern "C" fn done_nil(_: Id, _: Sel, done: Id) {
    call1(done, NIL);
}
unsafe extern "C" fn done_nil_with_arg(_: Id, _: Sel, _arg: Id, done: Id) {
    call1(done, NIL);
}
unsafe extern "C" fn friends_authorization(_: Id, _: Sel, done: Id) {
    call_int(done, 1, NIL); // authorized
}
unsafe extern "C" fn nil_nil_with_two_args(_: Id, _: Sel, _a: Id, _b: Id, done: Id) {
    call2(done, NIL, NIL);
}

unsafe fn local_player_class() -> Class {
    let base = gamekit("GKLocalPlayer");
    if base.is_null() {
        return std::ptr::null_mut();
    }
    static CLS: OnceLock<usize> = OnceLock::new();
    *CLS.get_or_init(|| unsafe {
        let Some(b) = rt::ClassBuilder::new::<()>("PearLocalPlayer", base) else { return 0 };
        let c = b.class();
        add(c, "init", return_self as Imp, "@@:");
        add(c, "isAuthenticated", yes as Imp, "c@:");
        add(c, "authenticated", yes as Imp, "c@:");
        add(c, "scopedIDsArePersistent", yes as Imp, "c@:");
        add(c, "isMultiplayerGamingRestricted", no as Imp, "c@:");
        add(c, "isPersonalizedCommunicationRestricted", no as Imp, "c@:");
        add(c, "playerID", player_id as Imp, "@@:");
        add(c, "gamePlayerID", player_id as Imp, "@@:");
        add(c, "teamPlayerID", player_id as Imp, "@@:");
        add(c, "displayName", player_name as Imp, "@@:");
        add(c, "alias", player_name as Imp, "@@:");
        add(c, "isUnderage", no as Imp, "c@:");
        add(c, "authenticateHandler", authenticate_handler as Imp, "@@:");
        add(c, "setAuthenticateHandler:", set_authenticate_handler as Imp, "v@:@?");
        add(c, "authenticateWithCompletionHandler:", done_nil as Imp, "v@:@?");
        add(c, "fetchSavedGamesWithCompletionHandler:", empty_array_done as Imp, "v@:@?");
        add(c, "loadFriendsWithCompletionHandler:", empty_array_done as Imp, "v@:@?");
        add(c, "loadFriendsAuthorizationStatus:", friends_authorization as Imp, "v@:@?");
        add(c, "loadRecentPlayersWithCompletionHandler:", empty_array_done as Imp, "v@:@?");
        add(c, "saveGameData:withName:completionHandler:", nil_nil_with_two_args as Imp, "v@:@@@?");
        add(c, "deleteSavedGamesWithName:completionHandler:", done_nil_with_arg as Imp, "v@:@@?");
        add(c, "resolveConflictingSavedGames:withData:completionHandler:", nil_nil_with_two_args as Imp, "v@:@@@?");
        b.register() as usize
    }) as Class
}

/// Replaces the class method, or adds it to the metaclass when missing.
unsafe fn replace_class_method(c: Class, selector: &str, imp: Imp, fallback: &str) {
    if c.is_null() {
        return;
    }
    rt::replace_class(c, selector, imp, fallback);
}

unsafe extern "C" fn class_empty_array_with_arg(_: Id, _: Sel, _arg: Id, done: Id) {
    call2(done, msg0(cls("NSArray"), "array"), NIL);
}

/// Local answers for the GameKit surfaces apps touch around startup.
unsafe fn install_gamekit_extras() {
    replace_class_method(gamekit("GKPlayer"), "loadPlayersForIdentifiers:completionHandler:", class_empty_array_with_arg as Imp, "v@:@?");
    let achievement = gamekit("GKAchievement");
    replace_class_method(achievement, "loadAchievementsWithCompletionHandler:", empty_array_done as Imp, "v@:@?");
    replace_class_method(achievement, "reportAchievements:withCompletionHandler:", done_nil_with_arg as Imp, "v@:@@?");
    replace_class_method(achievement, "resetAchievementsWithCompletionHandler:", done_nil as Imp, "v@:@?");
    let leaderboard = gamekit("GKLeaderboard");
    replace_class_method(leaderboard, "loadLeaderboardsWithCompletionHandler:", empty_array_done as Imp, "v@:@?");
    replace_class_method(leaderboard, "loadCategoriesWithCompletionHandler:", empty_array_done as Imp, "v@:@?");
    replace_class_method(gamekit("GKScore"), "reportScores:withCompletionHandler:", done_nil_with_arg as Imp, "v@:@@?");
}

static LOCAL_PLAYER: OnceLock<Keep> = OnceLock::new();

unsafe extern "C" fn local_player(_: Id, _: Sel) -> Id {
    LOCAL_PLAYER.get().map(|k| k.id()).unwrap_or(NIL)
}

unsafe fn install_gamekit() {
    let c = gamekit("GKLocalPlayer");
    if c.is_null() {
        return;
    }
    let fake = local_player_class();
    let m = rt::class_getClassMethod(c, sel("localPlayer"));
    if !m.is_null() && !fake.is_null() {
        LOCAL_PLAYER.get_or_init(|| unsafe { Keep(msg0::<Id>(msg0::<Id>(fake, "alloc"), "init") as usize) });
        rt::method_setImplementation(m, local_player as Imp);
    }
    if debug() {
        rt::log("[PearRuntime][cloud] GameKit local player compatibility installed");
    }
    install_gamekit_extras();
}

pub fn init() {
    if !rt::module_enabled("storage") || rt::env_set("PEAR_NO_CLOUD_SHIM") {
        return;
    }
    unsafe {
        pear_objc::autorelease_pool(|| {
            install_key_value_store();
            install_ubiquity_container();
            install_cloudkit();
            install_gamekit();
        });
    }
    // Frameworks loaded after the first scene still get the surface.
    for i in 1..=4 {
        rt::after_main(i as f64 * 2.0, || unsafe {
            pear_objc::autorelease_pool(|| {
                install_cloudkit();
                install_gamekit();
            })
        });
    }
}
