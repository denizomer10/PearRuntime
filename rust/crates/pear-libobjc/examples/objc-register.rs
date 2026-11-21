//! Loads each Mach-O named on stdin (imports from libobjc resolved by
//! pear-libobjc, the rest left unresolved) and registers its Objective-C
//! metadata without running any code: selectors uniqued, classes realized
//! (or not, when their superclass lives in a framework not loaded), categories
//! attached. One line per image, a total at the end.
use std::io::BufRead;

fn main() {
    let resolver = |library: Option<&str>, symbol: &str| match library {
        Some("/usr/lib/libobjc.A.dylib") => pear_libobjc::symbol(symbol),
        _ => None,
    };
    let (mut images, mut selectors, mut classes, mut unrealized, mut categories) = (0, 0, 0, 0, 0);
    let mut images_kept = Vec::new();
    for path in std::io::stdin().lock().lines().map_while(Result::ok) {
        let Ok(data) = std::fs::read(&path) else { continue };
        let image = match pear_dyld::load(&data, &resolver, pear_dyld::Options { allow_missing: true }) {
            Ok(image) => image,
            Err(e) => {
                println!("load-error\t{e}\t{path}");
                continue;
            }
        };
        let r = unsafe { pear_libobjc::register_image(&image) };
        println!("ok\t{} selectors\t{} classes\t{} unrealized\t{} categories\t{path}", r.selectors, r.classes, r.unrealized.len(), r.categories);
        images += 1;
        selectors += r.selectors;
        classes += r.classes;
        unrealized += r.unrealized.len();
        categories += r.categories;
        images_kept.push(image); // classes point into image memory
    }
    eprintln!("{images} images: {selectors} selector refs, {classes} classes realized, {unrealized} need frameworks not loaded, {categories} categories");
}
