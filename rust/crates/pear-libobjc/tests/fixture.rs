//! Runs the iOS Objective-C fixture (fixtures/fixture.m) on PearRuntime's
//! loader and runtime: on macOS and on arm64 Linux alike.
#![cfg(target_arch = "aarch64")]

use pear_dyld::{load, Options};

extern "C" fn unwind_resume() -> ! {
    eprintln!("_Unwind_Resume reached in the fixture");
    std::process::abort()
}

extern "C" {
    fn malloc(size: usize) -> *mut std::ffi::c_void;
    fn free(p: *mut std::ffi::c_void);
    fn strcmp(a: *const std::ffi::c_char, b: *const std::ffi::c_char) -> i32;
}

fn resolver(library: Option<&str>, symbol: &str) -> Option<usize> {
    match library {
        Some("/usr/lib/libobjc.A.dylib") => pear_libobjc::symbol(symbol),
        Some("/usr/lib/libSystem.B.dylib") => match symbol {
            "__Unwind_Resume" => Some(unwind_resume as *const () as usize),
            "_malloc" => Some(malloc as *const () as usize),
            "_free" => Some(free as *const () as usize),
            "_strcmp" => Some(strcmp as *const () as usize),
            _ => pear_libobjc::symbol(symbol),
        },
        _ => None,
    }
}

#[test]
fn objective_c_fixture_runs() {
    let data = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/fixture-objc.dylib")).unwrap();
    let image = load(&data, &resolver, Options::default()).unwrap();
    let report = unsafe { pear_libobjc::register_image(&image) };
    assert_eq!(report.classes, 3, "{report:?}"); // Rect, Square, Tracked
    assert_eq!(report.categories, 1);
    assert!(report.unrealized.is_empty());
    unsafe {
        pear_libobjc::call_load_methods(&image);
        image.run_initializers();
        let run: extern "C" fn() -> i32 = std::mem::transmute(image.symbol("_fixture_objc").unwrap());
        let score = run();
        let names = ["area", "super", "category", "isKindOfClass", "protocol/respondsTo", "weak set",
                     "dealloc+weak zeroed", "+load order"];
        let failed: Vec<_> = names.iter().enumerate().filter(|(i, _)| score & (1 << i) == 0).map(|(_, n)| *n).collect();
        assert!(failed.is_empty(), "score {score}: failed {failed:?}");
    }
}

#[test]
fn objective_c_fixture2_runs() {
    let data = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/fixture2-objc.dylib")).unwrap();
    let image = load(&data, &resolver, Options::default()).unwrap();
    let report = unsafe { pear_libobjc::register_image(&image) };
    assert!(report.unrealized.is_empty(), "{report:?}");
    unsafe {
        pear_libobjc::call_load_methods(&image);
        image.run_initializers();
        let run: extern "C" fn() -> i32 = std::mem::transmute(image.symbol("_fixture2_objc").unwrap());
        let score = run();
        let names = [
            "custom retain/release",
            "properties",
            "synchronized",
            "associated objects",
            "blocks",
            "imp_implementationWithBlock",
            "swizzling",
            "reflection",
            "runtime class",
        ];
        let failed: Vec<_> = names.iter().enumerate().filter(|(i, _)| score & (1 << i) == 0).map(|(_, n)| *n).collect();
        assert!(failed.is_empty(), "score {score}: failed {failed:?}");
    }
}
