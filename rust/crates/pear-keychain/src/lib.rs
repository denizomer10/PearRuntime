//! The iOS keychain API (SecItemAdd / SecItemCopyMatching / SecItemUpdate /
//! SecItemDelete, SecKeyCreateRandomKey / SecKeyGeneratePair) on a plain
//! file store, interposed into the process with dyld interposing.
//!
//! A locally signed iOS app on macOS cannot use the system keychain without
//! restricted entitlements (keychain-access-groups): SecItemAdd fails with
//! -34018. This gives apps the same API surface backed by local storage. It
//! is a compatibility layer for a generic storage API and defeats no
//! protection mechanism. Item identities and file names are those of the
//! earlier Objective-C store, so existing items keep working.

use pear_objc::{cls, msg0, msg1, msg2, nsarray_count, nsarray_object_at, nsstring, objc_retain, sel, Id, Sel, NIL};
use std::ffi::{c_char, c_void, CString};
use std::sync::atomic::{AtomicPtr, Ordering};
use std::sync::OnceLock;

type OSStatus = i32;
type CFTypeRef = *const c_void;
const ERR_SUCCESS: OSStatus = 0;
const ERR_ITEM_NOT_FOUND: OSStatus = -25300;
const BINARY_PLIST: usize = 200;

#[link(name = "Security", kind = "framework")]
extern "C" {
    fn SecItemAdd(attributes: Id, result: *mut CFTypeRef) -> OSStatus;
    fn SecItemCopyMatching(query: Id, result: *mut CFTypeRef) -> OSStatus;
    fn SecItemUpdate(query: Id, attributes: Id) -> OSStatus;
    fn SecItemDelete(query: Id) -> OSStatus;
    fn SecKeyCreateRandomKey(parameters: Id, error: *mut Id) -> Id;
    fn SecKeyGeneratePair(parameters: Id, public_key: *mut Id, private_key: *mut Id) -> OSStatus;
    fn SecKeyCopyAttributes(key: Id) -> Id;
    fn SecKeyCopyExternalRepresentation(key: Id, error: *mut Id) -> Id;
    fn SecKeyCreateWithData(data: Id, attributes: Id, error: *mut Id) -> Id;
    fn SecKeyGetTypeID() -> usize;

    static kSecValueData: Id;
    static kSecValueRef: Id;
    static kSecValuePersistentRef: Id;
    static kSecReturnData: Id;
    static kSecReturnAttributes: Id;
    static kSecReturnRef: Id;
    static kSecReturnPersistentRef: Id;
    static kSecMatchLimit: Id;
    static kSecMatchLimitAll: Id;
    static kSecUseDataProtectionKeychain: Id;
    static kSecUseAuthenticationUI: Id;
    static kSecUseAuthenticationContext: Id;
    static kSecAttrAccessGroup: Id;
    static kSecAttrSynchronizable: Id;
    static kSecAttrSynchronizableAny: Id;
    static kSecClass: Id;
    static kSecClassKey: Id;
    static kSecAttrKeyClass: Id;
    static kSecAttrKeyClassPublic: Id;
    static kSecAttrKeyClassPrivate: Id;
    static kSecAttrKeyType: Id;
    static kSecAttrKeySizeInBits: Id;
    static kSecAttrIsPermanent: Id;
    static kSecPrivateKeyAttrs: Id;
    static kSecPublicKeyAttrs: Id;
    static kSecAttrAccessControl: Id;
    static kSecAttrTokenID: Id;
    static kSecAttrApplicationTag: Id;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFGetTypeID(cf: Id) -> usize;
    fn CFRetain(cf: Id) -> Id;
    fn CFRelease(cf: Id);
}

// ---------------------------------------------------------------------------
// Logging: the host image installs its logger (PearLog / PearHostLogMessage).

static LOGGER: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

/// Messages carry their own "[PearRuntime][keychain]" prefix.
pub fn set_logger(f: unsafe extern "C" fn(*const c_char)) {
    LOGGER.store(f as *mut c_void, Ordering::Release);
}

fn log(message: &str) {
    let f = LOGGER.load(Ordering::Acquire);
    if f.is_null() {
        return;
    }
    if let Ok(c) = CString::new(message.replace('\0', "")) {
        let f: unsafe extern "C" fn(*const c_char) = unsafe { std::mem::transmute(f) };
        unsafe { f(c.as_ptr()) };
    }
}

// ---------------------------------------------------------------------------
// Foundation helpers (formatting and ordering exactly as NSString does, since
// item identities derive from them).

unsafe fn format2(format: &str, a: Id, b: Id) -> Id {
    let f: unsafe extern "C" fn(Id, Sel, Id, ...) -> Id = std::mem::transmute(pear_objc::objc_msgSend as *const c_void);
    f(cls("NSString"), sel("stringWithFormat:"), nsstring(format), a, b)
}

/// `%@` of an object (nil prints "(null)").
unsafe fn describe(obj: Id) -> String {
    let f: unsafe extern "C" fn(Id, Sel, Id, ...) -> Id = std::mem::transmute(pear_objc::objc_msgSend as *const c_void);
    pear_objc::nsstring_to_string(f(cls("NSString"), sel("stringWithFormat:"), nsstring("%@"), obj)).unwrap_or_default()
}

unsafe fn get(d: Id, key: Id) -> Id {
    if d.is_null() { NIL } else { msg1(d, "objectForKey:", key) }
}

unsafe fn set(d: Id, key: Id, value: Id) {
    if value.is_null() {
        msg1::<Id, ()>(d, "removeObjectForKey:", key);
    } else {
        msg2::<Id, Id, ()>(d, "setObject:forKey:", value, key);
    }
}

unsafe fn eq(a: Id, b: Id) -> bool {
    !a.is_null() && msg1::<Id, bool>(a, "isEqual:", b)
}

unsafe fn bool_value(v: Id) -> bool {
    !v.is_null() && pear_objc::responds_to(v, "boolValue") && msg0::<bool>(v, "boolValue")
}

unsafe fn keys(d: Id) -> Vec<Id> {
    let k: Id = if d.is_null() { NIL } else { msg0(d, "allKeys") };
    (0..nsarray_count(k)).map(|i| nsarray_object_at(k, i)).collect()
}

unsafe fn mutable_dict() -> Id {
    msg0(cls("NSMutableDictionary"), "dictionary")
}

unsafe fn mutable_copy(d: Id) -> Id {
    if d.is_null() { mutable_dict() } else { pear_objc::autoreleased(msg0(d, "mutableCopy")) }
}

unsafe fn s(key: &str) -> Id {
    nsstring(key)
}

// ---------------------------------------------------------------------------
// Store

fn store_dir() -> String {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    if let Some(dir) = env("PEAR_KEYCHAIN_DIR") {
        return dir;
    }
    if let Some(root) = env("PEAR_SUPPORT_ROOT") {
        return format!("{}/Keychain", root.trim_end_matches('/'));
    }
    // The real home: PathShim redirects NSHomeDirectory.
    let home = env("HOME").unwrap_or_else(|| unsafe {
        extern "C" {
            fn NSHomeDirectory() -> Id;
        }
        pear_objc::nsstring_to_string(NSHomeDirectory()).unwrap_or_default()
    });
    format!("{}/Library/Application Support/PearSilicon/Keychain", home.trim_end_matches('/'))
}

/// The access group iOS attaches automatically; SDKs split it on '.' for the
/// team identifier, so it must be present and stable.
fn default_access_group() -> Id {
    static G: OnceLock<usize> = OnceLock::new();
    *G.get_or_init(|| unsafe {
        let id: Id = msg0(msg0::<Id>(cls("NSBundle"), "mainBundle"), "bundleIdentifier");
        let id = pear_objc::nsstring_to_string(id).unwrap_or_else(|| "unknown".into());
        objc_retain(nsstring(&format!("PEARSILICON.{id}"))) as usize
    }) as Id
}

/// Stable, filesystem-safe file name for an identity (FNV-1a over UTF-8).
fn item_path(identifier: &str) -> String {
    let mut h: u64 = 1469598103934665603;
    for &b in identifier.as_bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    format!("{}/{h:016x}.plist", store_dir())
}

/// Keys that select or return data rather than describe the stored item.
unsafe fn is_control_key(key: Id) -> bool {
    [
        kSecValueData,
        kSecValueRef,
        kSecValuePersistentRef,
        kSecReturnData,
        kSecReturnAttributes,
        kSecReturnRef,
        kSecReturnPersistentRef,
        kSecMatchLimit,
        kSecUseDataProtectionKeychain,
        kSecUseAuthenticationUI,
        kSecUseAuthenticationContext,
    ]
    .iter()
    .any(|k| eq(*k, key))
}

unsafe fn load_all() -> Vec<Id> {
    let dir = store_dir();
    let fm: Id = msg0(cls("NSFileManager"), "defaultManager");
    let files: Id = msg2(fm, "contentsOfDirectoryAtPath:error:", nsstring(&dir), std::ptr::null_mut::<Id>());
    (0..nsarray_count(files))
        .filter_map(|i| {
            let file = pear_objc::nsstring_to_string(nsarray_object_at(files, i))?;
            let d: Id = msg1(cls("NSDictionary"), "dictionaryWithContentsOfFile:", nsstring(&format!("{dir}/{file}")));
            (!d.is_null()).then_some(d)
        })
        .collect()
}

unsafe fn save_item(item: Id) -> bool {
    let fm: Id = msg0(cls("NSFileManager"), "defaultManager");
    pear_objc::msg4::<Id, bool, Id, *mut Id, bool>(fm, "createDirectoryAtPath:withIntermediateDirectories:attributes:error:", nsstring(&store_dir()), true, NIL, std::ptr::null_mut());
    let id = pear_objc::nsstring_to_string(get(item, s("__id"))).unwrap_or_default();
    msg2::<Id, bool, bool>(item, "writeToFile:atomically:", nsstring(&item_path(&id)), true)
}

unsafe fn delete_item(identifier: Id) {
    let id = pear_objc::nsstring_to_string(identifier).unwrap_or_default();
    let fm: Id = msg0(cls("NSFileManager"), "defaultManager");
    msg2::<Id, *mut Id, bool>(fm, "removeItemAtPath:error:", nsstring(&item_path(&id)), std::ptr::null_mut());
}

/// Stable identity from the query's descriptive attributes. The synthetic
/// default group means "no group", so it does not change the identity; a real
/// group does, as on iOS.
unsafe fn identifier_for_query(query: Id) -> Id {
    let pairs: Id = msg0(cls("NSMutableArray"), "array");
    for key in keys(query) {
        if is_control_key(key) {
            continue;
        }
        let value = get(query, key);
        if eq(key, kSecAttrAccessGroup) && eq(value, default_access_group()) {
            continue;
        }
        msg1::<Id, ()>(pairs, "addObject:", format2("%@=%@", key, value));
    }
    msg1::<Sel, ()>(pairs, "sortUsingSelector:", sel("compare:"));
    msg1(pairs, "componentsJoinedByString:", nsstring("|"))
}

/// Every descriptive attribute of `query` matches `item`.
unsafe fn matches(item: Id, query: Id) -> bool {
    // A persistent reference identifies one stored item.
    let want_persistent = get(query, kSecValuePersistentRef);
    if !want_persistent.is_null() && !eq(get(item, s("__persistentRef")), want_persistent) {
        return false;
    }
    let want_sync = get(query, kSecAttrSynchronizable);
    let want_any_sync = !want_sync.is_null() && eq(want_sync, kSecAttrSynchronizableAny);
    for key in keys(query) {
        if is_control_key(key) {
            continue;
        }
        if eq(key, kSecAttrSynchronizable) {
            // `Any` selects local and synchronized items; it is no stored value.
            if want_any_sync {
                continue;
            }
            if bool_value(get(query, key)) != bool_value(get(item, key)) {
                return false;
            }
            continue;
        }
        let want = get(query, key);
        let mut have = get(item, key);
        if have.is_null() {
            // Items stored before groups were recorded belong to the default group.
            if eq(key, kSecAttrAccessGroup) {
                have = default_access_group();
            } else {
                return false;
            }
        }
        if !eq(want, have) {
            return false;
        }
    }
    // An omitted access group means the default group.
    if get(query, kSecAttrAccessGroup).is_null() {
        let mut have = get(item, kSecAttrAccessGroup);
        if have.is_null() {
            have = default_access_group();
        }
        if !eq(have, default_access_group()) {
            return false;
        }
    }
    true
}

// ---------------------------------------------------------------------------
// SecKey persistence: securityd is unavailable, so a key's external
// representation and attributes are stored and rebuilt with SecKeyCreateWithData.

/// Per-key attributes over shared defaults; caller metadata never changes
/// the class of the actual key.
unsafe fn attributes_for_key(params: Id, key_class: Id) -> Id {
    let attributes = mutable_copy(params);
    let nested_key = if eq(key_class, kSecAttrKeyClassPublic) { kSecPublicKeyAttrs } else { kSecPrivateKeyAttrs };
    let nested = get(params, nested_key);
    if !nested.is_null() && pear_objc::is_kind_of(nested, "NSDictionary") {
        msg1::<Id, ()>(attributes, "addEntriesFromDictionary:", nested);
    }
    set(attributes, kSecPrivateKeyAttrs, NIL);
    set(attributes, kSecPublicKeyAttrs, NIL);
    set(attributes, kSecAttrKeyClass, key_class);
    attributes
}

unsafe fn store_sec_key(key: Id, params: Id, key_class: Id) {
    if key.is_null() {
        return;
    }
    let params = attributes_for_key(params, key_class);
    let item = mutable_dict();
    set(item, kSecClass, kSecClassKey);
    set(item, kSecAttrKeyClass, key_class);
    let skipped = [kSecAttrIsPermanent, kSecClass, kSecAttrKeyClass, kSecPrivateKeyAttrs, kSecPublicKeyAttrs, kSecAttrAccessControl, kSecAttrTokenID];
    for k in keys(params) {
        if is_control_key(k) || skipped.iter().any(|x| eq(*x, k)) {
            continue;
        }
        let value = get(params, k);
        if !msg2::<Id, usize, bool>(cls("NSPropertyListSerialization"), "propertyList:isValidForFormat:", value, BINARY_PLIST) {
            continue;
        }
        set(item, k, value);
    }
    let key_attrs = SecKeyCopyAttributes(key);
    if !key_attrs.is_null() {
        for k in [kSecAttrKeyType, kSecAttrKeySizeInBits] {
            let v = get(key_attrs, k);
            if !v.is_null() && get(item, k).is_null() {
                set(item, k, v);
            }
        }
        CFRelease(key_attrs);
    }
    let mut err: Id = NIL;
    let ext = SecKeyCopyExternalRepresentation(key, &mut err);
    if ext.is_null() {
        log(&format!("[PearRuntime][keychain] SecKeyCopyExternalRepresentation failed: {}", describe(err)));
        if !err.is_null() {
            CFRelease(err);
        }
        return;
    }
    set(item, s("__keydata"), ext);
    CFRelease(ext);
    // Regenerating a tagged key replaces that logical key, not a new record
    // whose identity includes freshly generated key bytes.
    let tagged = !get(item, kSecAttrApplicationTag).is_null();
    let identity = if tagged {
        let identity = mutable_dict();
        for k in [kSecClass, kSecAttrKeyClass, kSecAttrKeyType, kSecAttrApplicationTag, kSecAttrAccessGroup] {
            let v = get(item, k);
            if !v.is_null() {
                set(identity, k, v);
            }
        }
        identity
    } else {
        mutable_copy(item)
    };
    let id = identifier_for_query(identity);
    set(item, s("__id"), id);
    if save_item(item) && tagged {
        // Older versions included key bytes in the identifier: retire only
        // matching tagged records, and only after the replacement was saved.
        for previous in load_all() {
            let pid = get(previous, s("__id"));
            if !eq(pid, id) && matches(previous, identity) {
                delete_item(pid);
            }
        }
    }
}

/// A SecKeyRef (+1) from a stored key item (kSecValueRef → __keydata, or
/// raw data with kSecClassKey → __data).
unsafe fn copy_sec_key(item: Id) -> Id {
    let mut data = get(item, s("__keydata"));
    if data.is_null() {
        data = get(item, s("__data"));
    }
    if data.is_null() {
        return NIL;
    }
    let attrs = mutable_dict();
    set(attrs, kSecClass, kSecClassKey);
    for k in [kSecAttrKeyType, kSecAttrKeyClass, kSecAttrKeySizeInBits] {
        let v = get(item, k);
        if !v.is_null() {
            set(attrs, k, v);
        }
    }
    // A key class is required to interpret the data; default to public.
    if get(attrs, kSecAttrKeyClass).is_null() {
        set(attrs, kSecAttrKeyClass, kSecAttrKeyClassPublic);
    }
    let mut err: Id = NIL;
    let key = SecKeyCreateWithData(data, attrs, &mut err);
    if key.is_null() {
        log(&format!("[PearRuntime][keychain] SecKeyCreateWithData failed: {}", describe(err)));
    }
    key
}

// ---------------------------------------------------------------------------
// Interposed implementations

unsafe fn retained(obj: Id) -> CFTypeRef {
    if obj.is_null() { std::ptr::null() } else { objc_retain(obj) as CFTypeRef }
}

unsafe extern "C" fn pear_sec_item_add(attributes: Id, result: *mut CFTypeRef) -> OSStatus {
    pear_objc::autorelease_pool(|| {
        let query = attributes;
        let item = mutable_dict();
        for key in keys(query) {
            if !is_control_key(key) {
                set(item, key, get(query, key));
            }
        }
        let value = get(query, kSecValueData);
        if !value.is_null() {
            set(item, s("__data"), value);
        }
        // iOS fills this in; SDKs that read it back expect it.
        if get(item, kSecAttrAccessGroup).is_null() {
            set(item, kSecAttrAccessGroup, default_access_group());
        }
        let value_ref = get(query, kSecValueRef);
        let all_keys = pear_objc::nsstring_to_string(msg1(msg0::<Id>(query, "allKeys"), "componentsJoinedByString:", nsstring(","))).unwrap_or_default();
        log(&format!(
            "[PearRuntime][keychain] SecItemAdd query keys: {all_keys} (valueData={} valueRef={} retRef={} retData={})",
            (!value.is_null()) as i32,
            (!value_ref.is_null()) as i32,
            bool_value(get(query, kSecReturnRef)) as i32,
            bool_value(get(query, kSecReturnData)) as i32
        ));
        // A SecKeyRef (kSecValueRef) is persisted through its representation.
        if !value_ref.is_null() && CFGetTypeID(value_ref) == SecKeyGetTypeID() {
            let mut err: Id = NIL;
            let ext = SecKeyCopyExternalRepresentation(value_ref, &mut err);
            if !ext.is_null() {
                set(item, s("__keydata"), ext);
                CFRelease(ext);
                let key_attrs = SecKeyCopyAttributes(value_ref);
                set(item, kSecClass, kSecClassKey);
                for k in [kSecAttrKeyClass, kSecAttrKeyType, kSecAttrKeySizeInBits] {
                    let v = get(key_attrs, k);
                    if !v.is_null() {
                        set(item, k, v);
                    }
                }
                if !key_attrs.is_null() {
                    CFRelease(key_attrs);
                }
                if get(item, kSecAttrKeyClass).is_null() {
                    set(item, kSecAttrKeyClass, kSecAttrKeyClassPublic);
                }
            } else if !err.is_null() {
                CFRelease(err);
            }
        }
        let mut identifier = identifier_for_query(query);
        if msg0::<usize>(identifier, "length") == 0 {
            identifier = msg0(msg0::<Id>(cls("NSUUID"), "UUID"), "UUIDString");
        }
        set(item, s("__id"), identifier);
        let persistent: Id = msg1(identifier, "dataUsingEncoding:", 4usize);
        set(item, s("__persistentRef"), persistent);
        // Upsert: same identity overwrites instead of errSecDuplicateItem.
        save_item(item);
        log(&format!("[PearRuntime][keychain] SecItemAdd id={}", describe(identifier)));
        if !result.is_null() {
            if bool_value(get(query, kSecReturnRef)) {
                // A key added with kSecReturnRef must give back a SecKeyRef.
                *result = if !value_ref.is_null() { CFRetain(value_ref) as CFTypeRef } else { retained(identifier) };
            } else if bool_value(get(query, kSecReturnPersistentRef)) {
                *result = retained(persistent);
            } else if bool_value(get(query, kSecReturnData)) {
                let mut data = get(item, s("__data"));
                if data.is_null() {
                    data = get(item, s("__keydata"));
                }
                *result = retained(data);
            }
            // Otherwise *result is left untouched, as the system does.
        }
        ERR_SUCCESS
    })
}

unsafe fn strip_internal(attrs: Id, extra: &[&str]) {
    for k in extra {
        set(attrs, s(k), NIL);
    }
    if get(attrs, kSecAttrAccessGroup).is_null() {
        set(attrs, kSecAttrAccessGroup, default_access_group());
    }
}

unsafe extern "C" fn pear_sec_item_copy_matching(query: Id, result: *mut CFTypeRef) -> OSStatus {
    pear_objc::autorelease_pool(|| {
        let q = query;
        let mut found: Vec<Id> = load_all().into_iter().filter(|item| matches(*item, q)).collect();
        let want_all = eq(get(q, kSecMatchLimit), kSecMatchLimitAll);
        if !want_all && found.len() > 1 {
            found.truncate(1);
        }
        if found.is_empty() {
            return ERR_ITEM_NOT_FOUND;
        }
        let want_data = bool_value(get(q, kSecReturnData));
        let want_attrs = bool_value(get(q, kSecReturnAttributes));
        let want_ref = bool_value(get(q, kSecReturnRef));
        let outputs: Id = msg0(cls("NSMutableArray"), "array");
        for item in found {
            let value = get(item, s("__data"));
            // Key items: a key class item yields a key even when stored as data.
            let is_key_class = eq(get(item, kSecClass), kSecClassKey);
            if !get(item, s("__keydata")).is_null() || is_key_class {
                log(&format!(
                    "[PearRuntime][keychain] key item: wantData={} wantAttrs={} wantRef={}",
                    want_data as i32, want_attrs as i32, want_ref as i32
                ));
                if want_ref {
                    let key = copy_sec_key(item);
                    if !key.is_null() {
                        msg1::<Id, ()>(outputs, "addObject:", key);
                        CFRelease(key);
                        continue;
                    }
                }
                let mut key_data = get(item, s("__keydata"));
                if key_data.is_null() {
                    key_data = get(item, s("__data"));
                }
                // Data alone means NSData, not an attribute dictionary.
                if want_data && !want_attrs && !want_ref {
                    if !key_data.is_null() {
                        msg1::<Id, ()>(outputs, "addObject:", key_data);
                    }
                    continue;
                }
                let attrs = mutable_copy(item);
                strip_internal(attrs, &["__id", "__keydata", "__persistentRef"]);
                if want_data && !key_data.is_null() {
                    set(attrs, kSecValueData, key_data);
                }
                msg1::<Id, ()>(outputs, "addObject:", attrs);
                continue;
            }
            // Attributes (or a ref) give a dictionary, with kSecValueData when
            // data is also requested; data alone only without attribute output.
            if want_attrs || want_ref {
                let attrs = mutable_copy(item);
                strip_internal(attrs, &["__id", "__data"]);
                if want_data && !value.is_null() {
                    set(attrs, kSecValueData, value);
                }
                msg1::<Id, ()>(outputs, "addObject:", attrs);
            } else if want_data {
                if !value.is_null() {
                    msg1::<Id, ()>(outputs, "addObject:", value);
                }
            } else {
                let attrs = mutable_copy(item);
                strip_internal(attrs, &["__id"]);
                msg1::<Id, ()>(outputs, "addObject:", attrs);
            }
        }
        let count = nsarray_count(outputs);
        if count == 0 {
            return ERR_ITEM_NOT_FOUND;
        }
        let value: Id = if want_all { outputs } else { nsarray_object_at(outputs, 0) };
        if !result.is_null() {
            *result = retained(value);
        }
        log(&format!("[PearRuntime][keychain] SecItemCopyMatching -> {count} item(s)"));
        ERR_SUCCESS
    })
}

unsafe extern "C" fn pear_sec_item_update(query: Id, update: Id) -> OSStatus {
    pear_objc::autorelease_pool(|| {
        let mut updated = false;
        for item in load_all() {
            if !matches(item, query) {
                continue;
            }
            let changed = mutable_copy(item);
            for key in keys(update) {
                if eq(key, kSecValueData) {
                    set(changed, s("__data"), get(update, key));
                } else if !is_control_key(key) {
                    set(changed, key, get(update, key));
                }
            }
            save_item(changed);
            updated = true;
        }
        log(&format!("[PearRuntime][keychain] SecItemUpdate updated={}", updated as i32));
        if updated { ERR_SUCCESS } else { ERR_ITEM_NOT_FOUND }
    })
}

unsafe extern "C" fn pear_sec_item_delete(query: Id) -> OSStatus {
    pear_objc::autorelease_pool(|| {
        let mut deleted = false;
        for item in load_all() {
            if matches(item, query) {
                delete_item(get(item, s("__id")));
                deleted = true;
            }
        }
        log(&format!("[PearRuntime][keychain] SecItemDelete deleted={}", deleted as i32));
        if deleted { ERR_SUCCESS } else { ERR_ITEM_NOT_FOUND }
    })
}

/// Attributes that make the platform store a key in securityd, which needs
/// restricted entitlements: removed so it builds an in-memory key.
unsafe fn strip_store_only_key_attributes(params: Id) {
    let strip = |d: Id| {
        set(d, kSecAttrIsPermanent, pear_objc::nsnumber_bool(false));
        for k in [kSecAttrAccessControl, kSecAttrAccessGroup, kSecAttrTokenID, kSecAttrApplicationTag] {
            set(d, k, NIL);
        }
    };
    strip(params);
    for nested_key in [kSecPrivateKeyAttrs, kSecPublicKeyAttrs] {
        let nested = get(params, nested_key);
        if nested.is_null() || !pear_objc::is_kind_of(nested, "NSDictionary") {
            continue;
        }
        let attrs = mutable_copy(nested);
        strip(attrs);
        set(params, nested_key, attrs);
    }
}

unsafe fn wants_persistent_key(params: Id, key_class: Id, default: bool) -> bool {
    let permanent = get(attributes_for_key(params, key_class), kSecAttrIsPermanent);
    if permanent.is_null() { default } else { bool_value(permanent) }
}

unsafe fn or_question(v: Id) -> String {
    if v.is_null() { "?".into() } else { describe(v) }
}

// dyld does not apply interposing to the image that defines it, so the
// functions called below are the system originals.
unsafe extern "C" fn pear_sec_key_create_random_key(parameters: Id, error: *mut Id) -> Id {
    pear_objc::autorelease_pool(|| {
        let requested = if parameters.is_null() { msg0(cls("NSDictionary"), "dictionary") } else { parameters };
        let persistent = wants_persistent_key(requested, kSecAttrKeyClassPrivate, false);
        let params = mutable_copy(requested);
        strip_store_only_key_attributes(params);
        log(&format!(
            "[PearRuntime][keychain] SecKeyCreateRandomKey type={} size={} persistent={}",
            or_question(get(params, kSecAttrKeyType)),
            or_question(get(params, kSecAttrKeySizeInBits)),
            persistent as i32
        ));
        let key = SecKeyCreateRandomKey(params, error);
        if key.is_null() {
            let e = if error.is_null() || (*error).is_null() { "unknown".to_string() } else { describe(*error) };
            log(&format!("[PearRuntime][keychain] SecKeyCreateRandomKey failed: {e}"));
            return NIL;
        }
        if persistent {
            store_sec_key(key, requested, kSecAttrKeyClassPrivate);
        }
        key
    })
}

unsafe extern "C" fn pear_sec_key_generate_pair(parameters: Id, public_key: *mut Id, private_key: *mut Id) -> OSStatus {
    pear_objc::autorelease_pool(|| {
        let requested = if parameters.is_null() { msg0(cls("NSDictionary"), "dictionary") } else { parameters };
        let params = mutable_copy(requested);
        strip_store_only_key_attributes(params);
        log(&format!(
            "[PearRuntime][keychain] SecKeyGeneratePair type={} size={}",
            or_question(get(params, kSecAttrKeyType)),
            or_question(get(params, kSecAttrKeySizeInBits))
        ));
        // Pairs persist by default (legacy behaviour); explicit transient
        // requests are honoured per key.
        let persist_public = wants_persistent_key(requested, kSecAttrKeyClassPublic, true);
        let persist_private = wants_persistent_key(requested, kSecAttrKeyClassPrivate, true);
        let (mut tmp_public, mut tmp_private): (Id, Id) = (NIL, NIL);
        let public_result: *mut Id = if !public_key.is_null() { public_key } else if persist_public { &mut tmp_public } else { std::ptr::null_mut() };
        let private_result: *mut Id = if !private_key.is_null() { private_key } else if persist_private { &mut tmp_private } else { std::ptr::null_mut() };
        let status = SecKeyGeneratePair(params, public_result, private_result);
        if status == ERR_SUCCESS {
            if persist_public && !public_result.is_null() {
                store_sec_key(*public_result, requested, kSecAttrKeyClassPublic);
            }
            if persist_private && !private_result.is_null() {
                store_sec_key(*private_result, requested, kSecAttrKeyClassPrivate);
            }
        } else {
            log(&format!("[PearRuntime][keychain] SecKeyGeneratePair failed: {status}"));
        }
        if !tmp_public.is_null() {
            CFRelease(tmp_public);
        }
        if !tmp_private.is_null() {
            CFRelease(tmp_private);
        }
        status
    })
}

#[repr(C)]
struct Interpose {
    replacement: *const c_void,
    replacee: *const c_void,
}
unsafe impl Sync for Interpose {}

#[used]
#[link_section = "__DATA,__interpose"]
static INTERPOSE: [Interpose; 6] = [
    Interpose { replacement: pear_sec_item_add as *const c_void, replacee: SecItemAdd as *const c_void },
    Interpose { replacement: pear_sec_item_copy_matching as *const c_void, replacee: SecItemCopyMatching as *const c_void },
    Interpose { replacement: pear_sec_item_update as *const c_void, replacee: SecItemUpdate as *const c_void },
    Interpose { replacement: pear_sec_item_delete as *const c_void, replacee: SecItemDelete as *const c_void },
    Interpose { replacement: pear_sec_key_create_random_key as *const c_void, replacee: SecKeyCreateRandomKey as *const c_void },
    Interpose { replacement: pear_sec_key_generate_pair as *const c_void, replacee: SecKeyGeneratePair as *const c_void },
];
