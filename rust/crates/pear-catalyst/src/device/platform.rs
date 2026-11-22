//! Local answers for iOS platform services that cannot work in a sideloaded
//! Catalyst process: StoreKit, LocalAuthentication, CoreTelephony, PassKit,
//! NFC, DeviceCheck/App Attest and HealthKit report their documented
//! "unavailable" state so apps take their fallback path instead of waiting
//! for a callback that never comes. Nothing is simulated: no purchase,
//! biometric success, attestation or health data. PEAR_NO_PLATFORM=1 (or
//! PEAR_NO_SERVICES=1) disables it; PEAR_DEBUG_SERVICES=1 logs each surface.

use crate::rt;
use pear_objc::{cls, msg0, Class, Id, Imp, Sel, NIL};
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::Once;

fn disabled() -> bool {
    rt::env_flag("PEAR_NO_PLATFORM") || rt::env_flag("PEAR_NO_SERVICES")
}

fn log_once(flag: &AtomicBool, message: &str) {
    if rt::env_flag("PEAR_DEBUG_SERVICES") && !flag.swap(true, Ordering::AcqRel) {
        rt::log(message);
    }
}

unsafe extern "C" fn ret_no(_s: Id, _c: Sel) -> bool { false }
unsafe extern "C" fn ret_no_1(_s: Id, _c: Sel, _a: Id) -> bool { false }
unsafe extern "C" fn ret_no_2(_s: Id, _c: Sel, _a: Id, _b: usize) -> bool { false }
unsafe extern "C" fn ret_nil(_s: Id, _c: Sel) -> Id { NIL }
unsafe extern "C" fn ret_zero(_s: Id, _c: Sel) -> isize { 0 }
unsafe extern "C" fn ret_empty_dict(_s: Id, _c: Sel) -> Id {
    msg0(cls("NSDictionary"), "dictionary")
}
unsafe extern "C" fn ret_empty_array(_s: Id, _c: Sel) -> Id {
    msg0(cls("NSArray"), "array")
}

// ---- StoreKit: a product query answers with an empty catalog (no product or
// price is fabricated), so IAP initialization does not retry forever.

static EMPTY_RESPONSE: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(std::ptr::null_mut());

unsafe fn empty_products_response_class() -> Class {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let base = cls("SKProductsResponse");
        if base.is_null() {
            return;
        }
        if let Some(b) = rt::ClassBuilder::new::<()>("PearEmptySKProductsResponse", base) {
            b.method("products", ret_empty_array as Imp, "@@:").method("invalidProductIdentifiers", ret_empty_array as Imp, "@@:");
            EMPTY_RESPONSE.store(b.register() as *mut _, Ordering::Release);
        }
    });
    EMPTY_RESPONSE.load(Ordering::Acquire) as Class
}

unsafe extern "C" fn products_start(s: Id, _c: Sel) {
    let request = pear_objc::objc_retain(s) as usize;
    rt::async_main(move || unsafe {
        let request = request as Id;
        let delegate: Id = msg0(request, "delegate");
        if pear_objc::responds_to(delegate, "productsRequest:didReceiveResponse:") {
            let response_class = empty_products_response_class();
            if !response_class.is_null() {
                let response: Id = msg0(response_class, "new");
                pear_objc::msg2::<Id, Id, ()>(delegate, "productsRequest:didReceiveResponse:", request, response);
                pear_objc::objc_release(response);
                pear_objc::objc_release(request);
                return;
            }
        }
        if pear_objc::responds_to(delegate, "request:didFailWithError:") {
            let error = rt::error("PearSiliconStore", 1, "The App Store is unavailable in this environment.");
            pear_objc::msg2::<Id, Id, ()>(delegate, "request:didFailWithError:", request, error);
        }
        pear_objc::objc_release(request);
    });
}

unsafe fn install_store_requests() {
    let request = cls("SKProductsRequest");
    if request.is_null() {
        return;
    }
    rt::override_instance(request, "start", products_start as Imp);
    static L: AtomicBool = AtomicBool::new(false);
    log_once(&L, "[PearRuntime][platform] StoreKit product requests report an empty catalog");
}

// ---- LocalAuthentication: never success; the policy cannot be evaluated.

const BIOMETRY_UNAVAILABLE: &str = "Biometric authentication is unavailable in this environment.";

unsafe extern "C" fn can_evaluate(_s: Id, _c: Sel, _policy: isize, error: *mut Id) -> bool {
    if !error.is_null() {
        *error = rt::error("com.apple.LocalAuthentication", 1, BIOMETRY_UNAVAILABLE);
    }
    false
}

unsafe extern "C" fn evaluate(_s: Id, _c: Sel, _policy: isize, _reason: Id, reply: Id) {
    rt::call_block2_on_main(reply, 0, rt::error("com.apple.LocalAuthentication", 1, BIOMETRY_UNAVAILABLE));
}

unsafe fn install_local_authentication() {
    let context = cls("LAContext");
    if context.is_null() {
        return;
    }
    rt::override_instance(context, "canEvaluatePolicy:error:", can_evaluate as Imp);
    rt::override_instance(context, "evaluatePolicy:localizedReason:reply:", evaluate as Imp);
    rt::override_instance(context, "biometryType", ret_zero as Imp);
    static L: AtomicBool = AtomicBool::new(false);
    log_once(&L, "[PearRuntime][platform] LocalAuthentication reports unavailable");
}

// ---- CoreTelephony: no cellular carrier (a Wi-Fi-only device's contract).

unsafe fn install_telephony() {
    let info = cls("CTTelephonyNetworkInfo");
    if info.is_null() {
        return;
    }
    rt::override_instance(info, "subscriberCellularProvider", ret_nil as Imp);
    rt::override_instance(info, "serviceSubscriberCellularProviders", ret_empty_dict as Imp);
    rt::override_instance(info, "currentRadioAccessTechnology", ret_nil as Imp);
    rt::override_instance(info, "serviceCurrentRadioAccessTechnology", ret_empty_dict as Imp);
    static L: AtomicBool = AtomicBool::new(false);
    log_once(&L, "[PearRuntime][platform] cellular carrier reports unavailable");
}

// ---- PassKit: Wallet and Apple Pay unavailable; no payment is simulated.

unsafe fn install_passkit() {
    let authorization = cls("PKPaymentAuthorizationViewController");
    if !authorization.is_null() {
        rt::override_class(authorization, "canMakePayments", ret_no as Imp);
        rt::override_class(authorization, "canMakePaymentsUsingNetworks:", ret_no_1 as Imp);
        rt::override_class(authorization, "canMakePaymentsUsingNetworks:capabilities:", ret_no_2 as Imp);
    }
    let controller = cls("PKPaymentAuthorizationController");
    if !controller.is_null() {
        rt::override_class(controller, "canMakePayments", ret_no as Imp);
        rt::override_class(controller, "canMakePaymentsUsingNetworks:", ret_no_1 as Imp);
    }
    let library = cls("PKPassLibrary");
    if !library.is_null() {
        rt::override_class(library, "isPassLibraryAvailable", ret_no as Imp);
    }
    static L: AtomicBool = AtomicBool::new(false);
    log_once(&L, "[PearRuntime][platform] Wallet and Apple Pay report unavailable");
}

unsafe fn install_nfc() {
    let ndef = cls("NFCNDEFReaderSession");
    let tag = cls("NFCTagReaderSession");
    rt::override_class(ndef, "readingAvailable", ret_no as Imp);
    rt::override_class(tag, "readingAvailable", ret_no as Imp);
    if !ndef.is_null() || !tag.is_null() {
        static L: AtomicBool = AtomicBool::new(false);
        log_once(&L, "[PearRuntime][platform] NFC reports unavailable");
    }
}

// ---- DeviceCheck / App Attest: never fabricated; unsupported, with errors.

unsafe extern "C" fn device_token(_s: Id, _c: Sel, done: Id) {
    rt::call_block2_on_main(done, 0, rt::error("PearSiliconDeviceCheck", 1, "Device attestation is unavailable in this environment."));
}
unsafe fn attest_error() -> Id {
    rt::error("PearSiliconAppAttest", 1, "App Attest is unavailable in this environment.")
}
unsafe extern "C" fn attest_generate_key(_s: Id, _c: Sel, done: Id) {
    rt::call_block2_on_main(done, 0, attest_error());
}
unsafe extern "C" fn attest_with_hash(_s: Id, _c: Sel, _key: Id, _hash: Id, done: Id) {
    rt::call_block2_on_main(done, 0, attest_error());
}

unsafe fn install_device_check() {
    let device = cls("DCDevice");
    if !device.is_null() {
        rt::override_instance(device, "isSupported", ret_no as Imp);
        rt::override_instance(device, "generateTokenWithCompletionHandler:", device_token as Imp);
    }
    let attest = cls("DCAppAttestService");
    if !attest.is_null() {
        rt::override_instance(attest, "isSupported", ret_no as Imp);
        rt::override_instance(attest, "generateKeyWithCompletionHandler:", attest_generate_key as Imp);
        rt::override_instance(attest, "attestKey:clientDataHash:completionHandler:", attest_with_hash as Imp);
        rt::override_instance(attest, "generateAssertion:clientDataHash:completionHandler:", attest_with_hash as Imp);
    }
    if !device.is_null() || !attest.is_null() {
        static L: AtomicBool = AtomicBool::new(false);
        log_once(&L, "[PearRuntime][platform] DeviceCheck and App Attest report unavailable");
    }
}

unsafe fn install_healthkit() {
    let store = cls("HKHealthStore");
    if store.is_null() {
        return;
    }
    rt::override_class(store, "isHealthDataAvailable", ret_no as Imp);
    static L: AtomicBool = AtomicBool::new(false);
    log_once(&L, "[PearRuntime][platform] HealthKit reports unavailable");
}

/// The installer's MarketplaceKit stub (or iOSSupport's) loaded globally
/// before Swift metadata resolves weak references to its types.
fn load_marketplacekit() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| unsafe {
        extern "C" {
            fn dlerror() -> *const std::ffi::c_char;
        }
        let bundle = rt::ns_string(msg0(msg0::<Id>(cls("NSBundle"), "mainBundle"), "bundlePath"));
        let local = rt::path_join(&bundle, "Frameworks/MarketplaceKit.framework/MarketplaceKit");
        if !rt::dlopen_path(&local, rt::RTLD_LAZY | rt::RTLD_GLOBAL).is_null() {
            rt::log("[PearRuntime][platform] loaded MarketplaceKit from the app bundle");
            return;
        }
        let system = "/System/iOSSupport/System/Library/Frameworks/MarketplaceKit.framework/MarketplaceKit";
        if !rt::dlopen_path(system, rt::RTLD_LAZY | rt::RTLD_GLOBAL).is_null() {
            rt::log("[PearRuntime][platform] loaded MarketplaceKit from iOSSupport");
        } else {
            let e = dlerror();
            rt::log(&format!("[PearRuntime][platform] MarketplaceKit unavailable: {}", if e.is_null() { "unknown" } else { rt::cstr(e) }));
        }
    });
}

fn install_all() {
    load_marketplacekit();
    unsafe {
        pear_objc::autorelease_pool(|| {
            install_store_requests();
            install_local_authentication();
            install_telephony();
            install_passkit();
            install_nfc();
            install_device_check();
            install_healthkit();
        });
    }
}

pub fn init() {
    if !rt::module_enabled("device") || disabled() {
        return;
    }
    install_all();
    // These frameworks often load lazily: re-run so a late StoreKit, Wallet,
    // NFC or HealthKit class still gets its local surface.
    for i in 1..=5 {
        rt::after_main(i as f64 * 2.0, install_all);
    }
}
