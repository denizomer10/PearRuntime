//! Loads the iOS fixture dylibs (fixtures/, both fixup formats) and runs
//! their code: on macOS and on arm64 Linux alike.
#![cfg(target_arch = "aarch64")]

use pear_dyld::{load, Error, Options};
use std::ffi::{c_char, CStr};

extern "C" fn host_add(a: i32, b: i32) -> i32 {
    a + b
}

#[repr(transparent)]
struct Ptr(*const c_char);
unsafe impl Sync for Ptr {}
static HOST_NAME: Ptr = Ptr(c"PearRuntime".as_ptr());

fn resolver(library: Option<&str>, symbol: &str) -> Option<usize> {
    match (library, symbol) {
        (Some("@rpath/libpearhost.dylib"), "_host_add") => Some(host_add as *const () as usize),
        (Some("@rpath/libpearhost.dylib"), "_host_name") => Some(&HOST_NAME as *const Ptr as usize),
        _ => None,
    }
}

fn run(path: &str) {
    let data = std::fs::read(path).unwrap();
    let image = load(&data, &resolver, Options::default()).unwrap();
    assert_eq!(image.install_name.as_deref(), Some("@rpath/fixture.dylib"));
    unsafe {
        let value: extern "C" fn() -> i32 = std::mem::transmute(image.symbol("_fixture_value").unwrap());
        assert_eq!(value(), 1, "before initializers the counter is 0");
        image.run_initializers();
        assert_eq!(value(), 42, "the constructor set the counter");

        let call: extern "C" fn(i32, i32) -> i32 = std::mem::transmute(image.symbol("_fixture_call").unwrap());
        assert_eq!(call(2, 3), (2 + 3) * 2 + 2, "bound function pointer and stub call");

        let greeting: extern "C" fn() -> *const c_char = std::mem::transmute(image.symbol("_fixture_greeting").unwrap());
        assert_eq!(CStr::from_ptr(greeting()).to_str().unwrap(), "hello from an iOS image");

        let name: extern "C" fn() -> *const c_char = std::mem::transmute(image.symbol("_fixture_host_name").unwrap());
        assert_eq!(CStr::from_ptr(name()).to_str().unwrap(), "PearRuntime", "imported data symbol");

        let table: extern "C" fn() -> i32 = std::mem::transmute(image.symbol("_fixture_table").unwrap());
        assert_eq!(table(), 3, "rebased pointer into the image's own data");

        let missing: extern "C" fn() -> i32 = std::mem::transmute(image.symbol("_fixture_has_missing").unwrap());
        assert_eq!(missing(), 0, "a missing weak import is NULL");
    }
}

#[test]
fn dyld_info_opcodes_fixture() {
    run(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/fixture-dyldinfo.dylib"));
}

#[test]
fn chained_fixups_fixture() {
    run(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/fixture-chained.dylib"));
}

#[test]
fn missing_strong_import_fails_unless_allowed() {
    let data = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/fixture-chained.dylib")).unwrap();
    let only_add = |_: Option<&str>, s: &str| (s == "_host_add").then_some(host_add as *const () as usize);
    match load(&data, &only_add, Options::default()) {
        Err(Error::MissingSymbols(names)) => assert_eq!(names, ["_host_name"]),
        other => panic!("expected missing symbols, got {:?}", other.err()),
    }
    let image = load(&data, &only_add, Options { allow_missing: true }).unwrap();
    assert_eq!(image.missing, ["_host_name"]);
}

fn thread_locals(path: &str) {
    let data = std::fs::read(path).unwrap();
    let image = load(&data, &resolver, Options::default()).unwrap();
    unsafe { image.run_initializers() };
    let bump: extern "C" fn() -> i32 = unsafe { std::mem::transmute(image.symbol("_fixture_tls_bump").unwrap()) };
    let text: extern "C" fn() -> *const c_char = unsafe { std::mem::transmute(image.symbol("_fixture_tls_text").unwrap()) };
    // tls_counter starts at 5 and tls_zero at 0 in every thread.
    assert_eq!(bump(), 6 + 10);
    assert_eq!(bump(), 7 + 20);
    let text_here = unsafe { CStr::from_ptr(text()) }.to_str().unwrap().to_string();
    assert_eq!(text_here, "per-thread", "a slid pointer in __thread_data");
    let bump_addr = bump as usize;
    let other = std::thread::spawn(move || {
        let bump: extern "C" fn() -> i32 = unsafe { std::mem::transmute(bump_addr) };
        (bump(), bump())
    })
    .join()
    .unwrap();
    assert_eq!(other, (16, 27), "a new thread starts from the template");
    assert_eq!(bump(), 8 + 30, "this thread's values were not touched");
}

#[test]
fn thread_locals_dyld_info() {
    thread_locals(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/fixture-dyldinfo.dylib"));
}

#[test]
fn thread_locals_chained() {
    thread_locals(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/fixture-chained.dylib"));
}

#[test]
fn reexported_symbol_resolves_through_resolver() {
    let data = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/fixture-chained.dylib")).unwrap();
    let mut image = load(&data, &resolver, Options::default()).unwrap();
    let expected = image.dylibs.first().cloned().unwrap_or_default();
    image.inject_reexport_for_test("_reexported_puts", 0, "_puts");
    let res = |lib: Option<&str>, sym: &str| -> Option<usize> {
        if lib == Some(expected.as_str()) && sym == "_puts" {
            Some(0xDEAD_BEEF)
        } else {
            None
        }
    };
    let ptr = image.symbol_with_resolver("_reexported_puts", Some(&res));
    assert_eq!(ptr.map(|p| p as usize), Some(0xDEAD_BEEF));
}

#[test]
fn resolve_dylib_path_expands_rpath_executable_and_loader_tokens() {
    let dir = std::env::temp_dir().join(format!("pear-dyld-rpath-test-{}", std::process::id()));
    let fw_dir = dir.join("Frameworks/Demo.framework");
    let helper_dir = dir.join("PlugIns");
    std::fs::create_dir_all(&fw_dir).unwrap();
    std::fs::create_dir_all(&helper_dir).unwrap();
    let fw_bin = fw_dir.join("Demo");
    let helper_bin = helper_dir.join("Helper.dylib");
    std::fs::write(&fw_bin, b"demo").unwrap();
    std::fs::write(&helper_bin, b"helper").unwrap();

    let rpaths = vec!["@executable_path/Frameworks".to_string()];
    assert_eq!(
        pear_dyld::resolve_dylib_path("@rpath/Demo.framework/Demo", &dir, &dir, &rpaths),
        Some(fw_bin)
    );
    assert_eq!(
        pear_dyld::resolve_dylib_path("@loader_path/PlugIns/Helper.dylib", &dir, &dir, &rpaths),
        Some(helper_bin)
    );
    assert_eq!(
        pear_dyld::resolve_dylib_path("/System/Library/Frameworks/UIKit.framework/UIKit", &dir, &dir, &rpaths),
        None
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_bundle_loads_main_executable_fixture() {
    let fixture = std::path::PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../pear-runtime/fixtures/fixture-ios-app"
    ));
    if fixture.is_file() {
        let empty = |_: Option<&str>, _: &str| None;
        let set = pear_dyld::load_bundle(&fixture, &empty, Options { allow_missing: true }).unwrap();
        assert_eq!(set.image_count(), 1);
        assert_eq!(set.init_order, vec![0]);
        assert!(set.main_image().path.is_some());
    }
}
