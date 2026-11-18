//! Keeps a null or unusable client identity away from CFNetwork's credential
//! constructor, which compares it with CFEqual and traps. The documented
//! factory returns nil for an unusable credential; this restores that for the
//! class factory and the initializer. Certificate validation and trust are
//! untouched. Opt out with PEAR_NO_CREDENTIAL_SHIM=1.

use crate::rt;
use pear_objc::{cls, msg0, objc_release, Id, Imp, Sel, NIL};
use std::ffi::c_void;
use std::sync::atomic::{AtomicPtr, Ordering};

type CFTypeRef = *const c_void;

#[link(name = "Security", kind = "framework")]
extern "C" {
    fn SecIdentityCopyCertificate(identity: CFTypeRef, cert: *mut CFTypeRef) -> i32;
    fn SecIdentityCopyPrivateKey(identity: CFTypeRef, key: *mut CFTypeRef) -> i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: CFTypeRef);
    fn CFGetTypeID(cf: CFTypeRef) -> usize;
    fn CFArrayGetTypeID() -> usize;
    fn CFArrayGetCount(array: CFTypeRef) -> isize;
    fn CFArrayGetValueAtIndex(array: CFTypeRef, index: isize) -> CFTypeRef;
    fn CFArrayCreateMutable(allocator: CFTypeRef, capacity: isize, callbacks: *const c_void) -> CFTypeRef;
    fn CFArrayAppendValue(array: CFTypeRef, value: CFTypeRef);
    static kCFTypeArrayCallBacks: c_void;
}

static ORIGINAL_FACTORY: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static ORIGINAL_INIT: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

type CredentialFn = unsafe extern "C" fn(Id, Sel, CFTypeRef, Id, usize) -> Id;

unsafe fn empty_array() -> Id {
    msg0(cls("NSArray"), "array")
}

/// CFNetwork dereferences the array unconditionally (nil crashes in
/// CFEqual(NULL)), and a bridged CFArray may hold NULL entries: hand it a
/// clean array, empty when there is nothing usable.
unsafe fn sanitized_certificates(certificates: Id) -> Id {
    if certificates.is_null() || CFGetTypeID(certificates) != CFArrayGetTypeID() {
        return empty_array();
    }
    let count = CFArrayGetCount(certificates);
    if count == 0 {
        return empty_array();
    }
    let filtered = CFArrayCreateMutable(std::ptr::null(), count, &kCFTypeArrayCallBacks);
    if filtered.is_null() {
        return empty_array();
    }
    for i in 0..count {
        let item = CFArrayGetValueAtIndex(certificates, i);
        if !item.is_null() {
            CFArrayAppendValue(filtered, item);
        }
    }
    rt::autoreleased(filtered as Id)
}

/// A non-NULL pointer is not necessarily a valid identity: it needs a
/// certificate and its private key, or CFNetwork crashes comparing them.
unsafe fn identity_usable(identity: CFTypeRef) -> bool {
    if identity.is_null() {
        return false;
    }
    let mut cert: CFTypeRef = std::ptr::null();
    let has_cert = SecIdentityCopyCertificate(identity, &mut cert) == 0 && !cert.is_null();
    if !cert.is_null() {
        CFRelease(cert);
    }
    if !has_cert {
        return false;
    }
    let mut key: CFTypeRef = std::ptr::null();
    let has_key = SecIdentityCopyPrivateKey(identity, &mut key) == 0 && !key.is_null();
    if !key.is_null() {
        CFRelease(key);
    }
    has_key
}

unsafe extern "C" fn credential_with_identity(s: Id, c: Sel, identity: CFTypeRef, certs: Id, persistence: usize) -> Id {
    if !identity_usable(identity) {
        rt::log("[PearRuntime][credential] ignoring a credential with no usable identity");
        return NIL;
    }
    let original = ORIGINAL_FACTORY.load(Ordering::Acquire);
    if original.is_null() {
        return NIL;
    }
    let f: CredentialFn = std::mem::transmute(original);
    f(s, c, identity, sanitized_certificates(certs), persistence)
}

unsafe extern "C" fn init_with_identity(s: Id, c: Sel, identity: CFTypeRef, certs: Id, persistence: usize) -> Id {
    let original = ORIGINAL_INIT.load(Ordering::Acquire);
    if !identity_usable(identity) || original.is_null() {
        rt::log("[PearRuntime][credential] refusing to initialize a credential with no usable identity");
        objc_release(s); // an initializer that fails consumes its receiver
        return NIL;
    }
    let f: CredentialFn = std::mem::transmute(original);
    f(s, c, identity, sanitized_certificates(certs), persistence)
}

pub fn init() {
    if !rt::module_enabled("storage") || rt::env_set("PEAR_NO_CREDENTIAL_SHIM") {
        return;
    }
    unsafe {
        let c = cls("NSURLCredential");
        if c.is_null() {
            return;
        }
        let factory = rt::class_getClassMethod(c, pear_objc::sel("credentialWithIdentity:certificates:persistence:"));
        if !factory.is_null() {
            ORIGINAL_FACTORY.store(rt::method_getImplementation(factory) as *mut c_void, Ordering::Release);
            rt::method_setImplementation(factory, credential_with_identity as Imp);
        }
        let init = rt::class_getInstanceMethod(c, pear_objc::sel("initWithIdentity:certificates:persistence:"));
        if !init.is_null() {
            ORIGINAL_INIT.store(rt::method_getImplementation(init) as *mut c_void, Ordering::Release);
            rt::method_setImplementation(init, init_with_identity as Imp);
        }
    }
}
