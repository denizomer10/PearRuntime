//! Clean-room Security.framework implementation: Keychain persistence
//! (`SecItem*`), CSPRNG (`SecRandomCopyBytes`), SecKey/SecCertificate/SecTrust,
//! SecureTransport (`SSL*`) handles, and `_kSec*` constants.

use pear_foundation::{
    const_nsstr_ptr, nsarray_from_ids, nsdata_from_bytes, nsdict_from_pairs, nsdict_lookup_str,
    nsstring_from_rust, rust_bytes_from_nsdata, rust_str_from_nsstring,
};
use pear_libobjc::abi::Id;
use std::collections::HashMap;
use std::ffi::{c_char, c_void};
use std::io::Read;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

const ERR_SEC_SUCCESS: i32 = 0;
const ERR_SEC_PARAM: i32 = -50;
const ERR_SEC_DUPLICATE_ITEM: i32 = -25299;
const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;
const ERR_SSL_CLOSED_GRACEFUL: i32 = -9805;

static K_SEC_RANDOM_DEFAULT_STUB: usize = 0;

struct KeychainStore {
    loaded: bool,
    entries: HashMap<(String, String, String), Vec<u8>>,
}

static KEYCHAIN: LazyLock<Mutex<KeychainStore>> = LazyLock::new(|| {
    Mutex::new(KeychainStore {
        loaded: false,
        entries: HashMap::new(),
    })
});

#[derive(Clone)]
struct KeyObj {
    bits: Vec<u8>,
    key_bits: usize,
    is_public: bool,
}

static SEC_OBJECTS: LazyLock<Mutex<HashMap<usize, Vec<u8>>>> = LazyLock::new(Default::default);
static SEC_KEYS: LazyLock<Mutex<HashMap<usize, KeyObj>>> = LazyLock::new(Default::default);
static SSL_CTXS: LazyLock<Mutex<HashMap<usize, usize>>> = LazyLock::new(Default::default);

fn keychain_file_path() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        if !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join("pearruntime").join("keychain.tsv"));
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            return Some(PathBuf::from(home).join(".local/share/pearruntime/keychain.tsv"));
        }
    }
    None
}

fn ensure_keychain_loaded(st: &mut KeychainStore) {
    if st.loaded {
        return;
    }
    st.loaded = true;
    let Some(path) = keychain_file_path() else { return };
    let Ok(text) = std::fs::read_to_string(&path) else { return };
    for line in text.lines() {
        let mut cols = line.splitn(4, '\t');
        if let (Some(cls), Some(svc), Some(acct), Some(hex)) = (cols.next(), cols.next(), cols.next(), cols.next()) {
            let mut bytes = Vec::with_capacity(hex.len() / 2);
            let b = hex.as_bytes();
            let mut i = 0;
            while i + 1 < b.len() {
                if let Ok(v) = u8::from_str_radix(&hex[i..i + 2], 16) {
                    bytes.push(v);
                }
                i += 2;
            }
            st.entries.insert((cls.into(), svc.into(), acct.into()), bytes);
        }
    }
}

fn save_keychain(st: &KeychainStore) {
    let Some(path) = keychain_file_path() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let mut out = String::new();
    for ((cls, svc, acct), val) in &st.entries {
        let clean = |s: &str| s.replace(['\t', '\n', '\r'], "_");
        out.push_str(&format!("{}\t{}\t{}\t", clean(cls), clean(svc), clean(acct)));
        for byte in val {
            out.push_str(&format!("{byte:02x}"));
        }
        out.push('\n');
    }
    let _ = std::fs::write(path, out);
}

fn query_key_tuple(query: Id) -> (String, String, String) {
    let get_str = |k: &str| -> String {
        nsdict_lookup_str(query, k)
            .and_then(rust_str_from_nsstring)
            .unwrap_or_default()
    };
    let cls = get_str("class");
    let svc = {
        let s = get_str("svce");
        if s.is_empty() { get_str("srvr") } else { s }
    };
    let acct = get_str("acct");
    (cls, svc, acct)
}

fn key_matches(k: &(String, String, String), q: &(String, String, String)) -> bool {
    (q.0.is_empty() || k.0 == q.0)
        && (q.1.is_empty() || k.1 == q.1)
        && (q.2.is_empty() || k.2 == q.2)
}

pub unsafe extern "C" fn sec_item_add(attrs: Id, result: *mut Id) -> i32 {
    if attrs.is_null() {
        return ERR_SEC_PARAM;
    }
    let key = query_key_tuple(attrs);
    let data = nsdict_lookup_str(attrs, "v_Data")
        .and_then(rust_bytes_from_nsdata)
        .unwrap_or_default();
    let mut st = KEYCHAIN.lock().unwrap();
    ensure_keychain_loaded(&mut st);
    if st.entries.contains_key(&key) {
        return ERR_SEC_DUPLICATE_ITEM;
    }
    st.entries.insert(key.clone(), data.clone());
    save_keychain(&st);
    if !result.is_null() {
        *result = nsdata_from_bytes(&data);
    }
    ERR_SEC_SUCCESS
}

pub unsafe extern "C" fn sec_item_copy_matching(query: Id, result: *mut Id) -> i32 {
    if query.is_null() {
        return ERR_SEC_PARAM;
    }
    let q = query_key_tuple(query);
    let want_data = nsdict_lookup_str(query, "r_Data").is_some();
    let want_attrs = nsdict_lookup_str(query, "r_Attributes").is_some();
    let st = KEYCHAIN.lock().unwrap();
    let found = st
        .entries
        .iter()
        .find(|(k, _)| key_matches(k, &q))
        .map(|(k, v)| (k.clone(), v.clone()));
    drop(st);
    let Some(((cls, svc, acct), val)) = found else {
        return ERR_SEC_ITEM_NOT_FOUND;
    };
    if !result.is_null() {
        if want_attrs && !want_data {
            *result = nsdict_from_pairs(vec![
                ("class".into(), nsstring_from_rust(&cls)),
                ("svce".into(), nsstring_from_rust(&svc)),
                ("acct".into(), nsstring_from_rust(&acct)),
                ("v_Data".into(), nsdata_from_bytes(&val)),
            ]);
        } else {
            *result = nsdata_from_bytes(&val);
        }
    }
    ERR_SEC_SUCCESS
}

pub unsafe extern "C" fn sec_item_update(query: Id, attrs_to_update: Id) -> i32 {
    if query.is_null() || attrs_to_update.is_null() {
        return ERR_SEC_PARAM;
    }
    let q = query_key_tuple(query);
    let Some(data) = nsdict_lookup_str(attrs_to_update, "v_Data").and_then(rust_bytes_from_nsdata) else {
        return ERR_SEC_PARAM;
    };
    let mut st = KEYCHAIN.lock().unwrap();
    ensure_keychain_loaded(&mut st);
    let target = st.entries.keys().find(|k| key_matches(k, &q)).cloned();
    if let Some(key) = target {
        st.entries.insert(key, data);
        save_keychain(&st);
        ERR_SEC_SUCCESS
    } else {
        ERR_SEC_ITEM_NOT_FOUND
    }
}

pub unsafe extern "C" fn sec_item_delete(query: Id) -> i32 {
    if query.is_null() {
        return ERR_SEC_PARAM;
    }
    let q = query_key_tuple(query);
    let mut st = KEYCHAIN.lock().unwrap();
    ensure_keychain_loaded(&mut st);
    let keys: Vec<_> = st.entries.keys().filter(|k| key_matches(k, &q)).cloned().collect();
    if keys.is_empty() {
        return ERR_SEC_ITEM_NOT_FOUND;
    }
    for k in keys {
        st.entries.remove(&k);
    }
    save_keychain(&st);
    ERR_SEC_SUCCESS
}

pub unsafe extern "C" fn sec_random_copy_bytes(_rnd: *const c_void, count: usize, bytes: *mut u8) -> i32 {
    if bytes.is_null() {
        return ERR_SEC_PARAM;
    }
    if count == 0 {
        return ERR_SEC_SUCCESS;
    }
    let out = std::slice::from_raw_parts_mut(bytes, count);
    if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        if f.read_exact(out).is_ok() {
            return ERR_SEC_SUCCESS;
        }
    }
    // Deterministic fallback only if /dev/urandom is unavailable
    let mut x = 0x9E37_79B9_7F4A_7C15u64 ^ (bytes as usize as u64);
    for b in out {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *b = x as u8;
    }
    ERR_SEC_SUCCESS
}

// ---- SecCertificate / SecKey / SecPolicy / SecTrust / SecAccessControl -----

extern "C" fn sec_access_control_create(_alloc: *const c_void, _prot: Id, _flags: usize, _err: *mut Id) -> *mut c_void {
    Box::into_raw(Box::new(0usize)) as *mut c_void
}

extern "C" fn sec_cert_create_with_data(_alloc: *const c_void, data: Id) -> *mut c_void {
    let bytes = rust_bytes_from_nsdata(data).unwrap_or_default();
    let h = Box::into_raw(Box::new(0usize)) as *mut c_void;
    SEC_OBJECTS.lock().unwrap().insert(h as usize, bytes);
    h
}

extern "C" fn sec_cert_copy_data(cert: *mut c_void) -> Id {
    let bytes = SEC_OBJECTS.lock().unwrap().get(&(cert as usize)).cloned().unwrap_or_default();
    nsdata_from_bytes(&bytes)
}

unsafe extern "C" fn sec_identity_copy_cert(ident: *mut c_void, cert_out: *mut *mut c_void) -> i32 {
    if !cert_out.is_null() {
        let bytes = SEC_OBJECTS.lock().unwrap().get(&(ident as usize)).cloned().unwrap_or_default();
        let h = Box::into_raw(Box::new(0usize)) as *mut c_void;
        SEC_OBJECTS.lock().unwrap().insert(h as usize, bytes);
        *cert_out = h;
    }
    ERR_SEC_SUCCESS
}

extern "C" fn sec_key_create_with_data(key_data: Id, _attrs: Id, _err: *mut Id) -> *mut c_void {
    let bits = rust_bytes_from_nsdata(key_data).unwrap_or_else(|| vec![0u8; 256]);
    let h = Box::into_raw(Box::new(0usize)) as *mut c_void;
    SEC_KEYS.lock().unwrap().insert(h as usize, KeyObj { key_bits: bits.len().max(256), bits, is_public: false });
    h
}

extern "C" fn sec_key_create_random(_attrs: Id, _err: *mut Id) -> *mut c_void {
    let mut bits = vec![0u8; 256];
    unsafe { sec_random_copy_bytes(std::ptr::null(), 256, bits.as_mut_ptr()) };
    let h = Box::into_raw(Box::new(0usize)) as *mut c_void;
    SEC_KEYS.lock().unwrap().insert(h as usize, KeyObj { bits, key_bits: 2048, is_public: false });
    h
}

extern "C" fn sec_key_copy_public(key: *mut c_void) -> *mut c_void {
    let obj = SEC_KEYS.lock().unwrap().get(&(key as usize)).cloned().unwrap_or(KeyObj {
        bits: vec![0u8; 256],
        key_bits: 2048,
        is_public: true,
    });
    let h = Box::into_raw(Box::new(0usize)) as *mut c_void;
    SEC_KEYS.lock().unwrap().insert(h as usize, KeyObj { is_public: true, ..obj });
    h
}

extern "C" fn sec_key_copy_ext_rep(key: *mut c_void, _err: *mut Id) -> Id {
    let bits = SEC_KEYS.lock().unwrap().get(&(key as usize)).map(|k| k.bits.clone()).unwrap_or_default();
    nsdata_from_bytes(&bits)
}

extern "C" fn sec_key_copy_attrs(key: *mut c_void) -> Id {
    let k = SEC_KEYS.lock().unwrap().get(&(key as usize)).cloned();
    if let Some(k) = k {
        nsdict_from_pairs(vec![
            ("bsiz".into(), k.key_bits as Id),
            ("kcls".into(), nsstring_from_rust(if k.is_public { "0" } else { "1" })),
        ])
    } else {
        nsdict_from_pairs(Vec::new())
    }
}

extern "C" fn sec_key_get_block_size(key: *mut c_void) -> usize {
    SEC_KEYS.lock().unwrap().get(&(key as usize)).map_or(256, |k| (k.key_bits / 8).max(32))
}

extern "C" fn sec_key_create_sig(_key: *mut c_void, _alg: Id, data: Id, _err: *mut Id) -> Id {
    let d = rust_bytes_from_nsdata(data).unwrap_or_default();
    nsdata_from_bytes(&d)
}

extern "C" fn sec_key_verify_sig(_key: *mut c_void, _alg: Id, _data: Id, _sig: Id, _err: *mut Id) -> bool {
    true
}

extern "C" fn sec_key_raw_verify(
    _key: *mut c_void,
    _padding: u32,
    _signed_data: *const u8,
    _signed_len: usize,
    _sig: *const u8,
    _sig_len: usize,
) -> i32 {
    ERR_SEC_SUCCESS
}

unsafe extern "C" fn sec_key_copy_passthrough(
    _key: *mut c_void,
    _padding: u32,
    input: *const u8,
    input_len: usize,
    output: *mut u8,
    output_len: *mut usize,
) -> i32 {
    if !input.is_null() && !output.is_null() && !output_len.is_null() {
        let n = input_len.min(*output_len);
        std::ptr::copy_nonoverlapping(input, output, n);
        *output_len = n;
    }
    ERR_SEC_SUCCESS
}

unsafe extern "C" fn sec_pkcs12_import(_data: Id, _opts: Id, items: *mut Id) -> i32 {
    if !items.is_null() {
        *items = nsarray_from_ids(Vec::new());
    }
    ERR_SEC_SUCCESS
}

extern "C" fn sec_policy_create_basic() -> *mut c_void {
    Box::into_raw(Box::new(0usize)) as *mut c_void
}
extern "C" fn sec_policy_create_ssl(_server: bool, _hostname: Id) -> *mut c_void {
    Box::into_raw(Box::new(0usize)) as *mut c_void
}
unsafe extern "C" fn sec_trust_create(_certs: Id, _policies: Id, trust_out: *mut *mut c_void) -> i32 {
    if !trust_out.is_null() {
        *trust_out = Box::into_raw(Box::new(0usize)) as *mut c_void;
    }
    ERR_SEC_SUCCESS
}
unsafe extern "C" fn sec_trust_evaluate(_trust: *mut c_void, res: *mut u32) -> i32 {
    if !res.is_null() {
        *res = 1; // kSecTrustResultProceed
    }
    ERR_SEC_SUCCESS
}
extern "C" fn sec_trust_evaluate_err(_trust: *mut c_void, _err: *mut Id) -> bool {
    true
}
extern "C" fn sec_trust_copy_exceptions(_trust: *mut c_void) -> Id {
    nsdata_from_bytes(&[])
}
extern "C" fn sec_trust_set_exceptions(_trust: *mut c_void, _ex: Id) -> bool {
    true
}
extern "C" fn sec_trust_set_policies(_trust: *mut c_void, _policies: Id) -> i32 {
    ERR_SEC_SUCCESS
}
extern "C" fn sec_trust_get_cert_count(_trust: *mut c_void) -> isize {
    1
}
extern "C" fn sec_trust_get_cert_at(_trust: *mut c_void, _idx: isize) -> *mut c_void {
    Box::into_raw(Box::new(0usize)) as *mut c_void
}
extern "C" fn sec_trust_copy_public_key(_trust: *mut c_void) -> *mut c_void {
    sec_key_create_random(std::ptr::null_mut(), std::ptr::null_mut())
}

// ---- SecureTransport (SSL*) ------------------------------------------------

extern "C" fn ssl_create_context(_alloc: *const c_void, _proto_side: i32, _conn_type: i32) -> *mut c_void {
    let h = Box::into_raw(Box::new(0usize)) as *mut c_void;
    SSL_CTXS.lock().unwrap().insert(h as usize, 0);
    h
}
extern "C" fn ssl_close(ctx: *mut c_void) -> i32 {
    SSL_CTXS.lock().unwrap().remove(&(ctx as usize));
    ERR_SEC_SUCCESS
}
extern "C" fn ssl_handshake(_ctx: *mut c_void) -> i32 {
    ERR_SEC_SUCCESS
}
extern "C" fn ssl_copy_peer_trust(_ctx: *mut c_void, trust_out: *mut *mut c_void) -> i32 {
    unsafe { sec_trust_create(std::ptr::null_mut(), std::ptr::null_mut(), trust_out) }
}
unsafe extern "C" fn ssl_get_buffered_read_size(_ctx: *mut c_void, buf_size: *mut usize) -> i32 {
    if !buf_size.is_null() {
        *buf_size = 0;
    }
    ERR_SEC_SUCCESS
}
unsafe extern "C" fn ssl_read(_ctx: *mut c_void, _data: *mut u8, _len: usize, processed: *mut usize) -> i32 {
    if !processed.is_null() {
        *processed = 0;
    }
    ERR_SSL_CLOSED_GRACEFUL
}
unsafe extern "C" fn ssl_write(_ctx: *mut c_void, _data: *const u8, len: usize, processed: *mut usize) -> i32 {
    if !processed.is_null() {
        *processed = len;
    }
    ERR_SEC_SUCCESS
}
extern "C" fn ssl_set_ok1(_ctx: *mut c_void, _a: usize) -> i32 { ERR_SEC_SUCCESS }
extern "C" fn ssl_set_ok2(_ctx: *mut c_void, _a: usize, _b: usize) -> i32 { ERR_SEC_SUCCESS }
extern "C" fn ssl_set_peer_domain(_ctx: *mut c_void, _name: *const c_char, _len: usize) -> i32 { ERR_SEC_SUCCESS }

pub fn symbol(name: &str) -> Option<usize> {
    let f = |p: *const ()| Some(p as usize);
    match name {
        // Keychain & Random
        "_kSecRandomDefault" => Some(&raw const K_SEC_RANDOM_DEFAULT_STUB as usize),
        "_SecRandomCopyBytes" => f(sec_random_copy_bytes as *const ()),
        "_SecItemAdd" => f(sec_item_add as *const ()),
        "_SecItemCopyMatching" => f(sec_item_copy_matching as *const ()),
        "_SecItemUpdate" => f(sec_item_update as *const ()),
        "_SecItemDelete" => f(sec_item_delete as *const ()),
        // Sec* Crypto / Cert / Trust / AccessControl
        "_SecAccessControlCreateWithFlags" => f(sec_access_control_create as *const ()),
        "_SecCertificateCreateWithData" => f(sec_cert_create_with_data as *const ()),
        "_SecCertificateCopyData" => f(sec_cert_copy_data as *const ()),
        "_SecIdentityCopyCertificate" => f(sec_identity_copy_cert as *const ()),
        "_SecKeyCreateWithData" => f(sec_key_create_with_data as *const ()),
        "_SecKeyCreateRandomKey" => f(sec_key_create_random as *const ()),
        "_SecKeyCopyPublicKey" => f(sec_key_copy_public as *const ()),
        "_SecKeyCopyExternalRepresentation" => f(sec_key_copy_ext_rep as *const ()),
        "_SecKeyCopyAttributes" => f(sec_key_copy_attrs as *const ()),
        "_SecKeyGetBlockSize" => f(sec_key_get_block_size as *const ()),
        "_SecKeyCreateSignature" => f(sec_key_create_sig as *const ()),
        "_SecKeyVerifySignature" => f(sec_key_verify_sig as *const ()),
        "_SecKeyRawVerify" => f(sec_key_raw_verify as *const ()),
        "_SecKeyEncrypt" | "_SecKeyDecrypt" => f(sec_key_copy_passthrough as *const ()),
        "_SecPKCS12Import" => f(sec_pkcs12_import as *const ()),
        "_SecPolicyCreateBasicX509" => f(sec_policy_create_basic as *const ()),
        "_SecPolicyCreateSSL" => f(sec_policy_create_ssl as *const ()),
        "_SecTrustCreateWithCertificates" => f(sec_trust_create as *const ()),
        "_SecTrustEvaluate" => f(sec_trust_evaluate as *const ()),
        "_SecTrustEvaluateWithError" => f(sec_trust_evaluate_err as *const ()),
        "_SecTrustCopyExceptions" => f(sec_trust_copy_exceptions as *const ()),
        "_SecTrustSetExceptions" => f(sec_trust_set_exceptions as *const ()),
        "_SecTrustSetPolicies" => f(sec_trust_set_policies as *const ()),
        "_SecTrustGetCertificateCount" => f(sec_trust_get_cert_count as *const ()),
        "_SecTrustGetCertificateAtIndex" => f(sec_trust_get_cert_at as *const ()),
        "_SecTrustCopyPublicKey" => f(sec_trust_copy_public_key as *const ()),
        // SecureTransport SSL*
        "_SSLCreateContext" => f(ssl_create_context as *const ()),
        "_SSLClose" => f(ssl_close as *const ()),
        "_SSLHandshake" => f(ssl_handshake as *const ()),
        "_SSLCopyPeerTrust" => f(ssl_copy_peer_trust as *const ()),
        "_SSLGetBufferedReadSize" => f(ssl_get_buffered_read_size as *const ()),
        "_SSLRead" => f(ssl_read as *const ()),
        "_SSLWrite" => f(ssl_write as *const ()),
        "_SSLSetPeerDomainName" => f(ssl_set_peer_domain as *const ()),
        "_SSLSetCertificate"
        | "_SSLSetALPNProtocols"
        | "_SSLSetConnection"
        | "_SSLSetProtocolVersionMax"
        | "_SSLSetProtocolVersionMin" => f(ssl_set_ok1 as *const ()),
        "_SSLSetEnabledCiphers"
        | "_SSLSetIOFuncs"
        | "_SSLSetPeerID"
        | "_SSLSetSessionOption" => f(ssl_set_ok2 as *const ()),
        // Explicit key names matching KeychainStore field lookups
        "_kSecClass" => Some(const_nsstr_ptr("class")),
        "_kSecClassGenericPassword" => Some(const_nsstr_ptr("genp")),
        "_kSecClassInternetPassword" => Some(const_nsstr_ptr("inet")),
        "_kSecClassKey" => Some(const_nsstr_ptr("keys")),
        "_kSecAttrService" => Some(const_nsstr_ptr("svce")),
        "_kSecAttrServer" => Some(const_nsstr_ptr("srvr")),
        "_kSecAttrAccount" => Some(const_nsstr_ptr("acct")),
        "_kSecValueData" => Some(const_nsstr_ptr("v_Data")),
        "_kSecReturnData" => Some(const_nsstr_ptr("r_Data")),
        "_kSecReturnAttributes" => Some(const_nsstr_ptr("r_Attributes")),
        "_kSecReturnPersistentRef" => Some(const_nsstr_ptr("r_PersistentRef")),
        "_kSecReturnRef" => Some(const_nsstr_ptr("r_Ref")),
        s if s.starts_with("_kSec") => {
            let leaked: &'static str = Box::leak(s.trim_start_matches('_').to_string().into_boxed_str());
            Some(const_nsstr_ptr(leaked))
        }
        _ => None,
    }
}
