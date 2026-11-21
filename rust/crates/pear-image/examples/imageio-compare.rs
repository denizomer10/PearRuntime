//! macOS only: decodes each file named on stdin with pear-image and with
//! ImageIO, draws ImageIO's image into an sRGB premultiplied RGBA8 bitmap and
//! compares the pixels with pear-image's (premultiplied) output.
//!
//!   find Game.app -name '*.png' | cargo run --release --example imageio-compare
//!
//! One line per file: status, max and mean channel difference, and the
//! number of differing pixels; a summary at the end.

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("imageio-compare needs macOS (ImageIO)");
}

#[cfg(target_os = "macos")]
fn main() {
    use std::ffi::c_void;
    use std::io::BufRead;

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFDataCreate(alloc: *const c_void, bytes: *const u8, len: isize) -> *const c_void;
        fn CFRelease(cf: *const c_void);
    }
    #[link(name = "ImageIO", kind = "framework")]
    extern "C" {
        fn CGImageSourceCreateWithData(data: *const c_void, opts: *const c_void) -> *const c_void;
        fn CGImageSourceCreateImageAtIndex(src: *const c_void, i: usize, opts: *const c_void) -> *const c_void;
    }
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGImageGetWidth(image: *const c_void) -> usize;
        fn CGImageGetHeight(image: *const c_void) -> usize;
        fn CGColorSpaceCreateWithName(name: *const c_void) -> *const c_void;
        static kCGColorSpaceSRGB: *const c_void;
        fn CGBitmapContextCreate(
            data: *mut u8, w: usize, h: usize, bpc: usize, bpr: usize, cs: *const c_void, info: u32,
        ) -> *const c_void;
        fn CGContextDrawImage(ctx: *const c_void, rect: Rect, image: *const c_void);
        fn CGContextRelease(ctx: *const c_void);
        fn CGColorSpaceRelease(cs: *const c_void);
        fn CGImageRelease(image: *const c_void);
        fn CGImageGetDataProvider(image: *const c_void) -> *const c_void;
        fn CGDataProviderCopyData(provider: *const c_void) -> *const c_void;
    }
    const PREMULTIPLIED_LAST_BIG: u32 = 1 | (4 << 12);
    #[repr(C)]
    struct Rect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    }
    // PEAR_COMPARE_DUMP=dir: also write <n>.ours.rgba / <n>.imageio.rgba.
    let dump = std::env::var_os("PEAR_COMPARE_DUMP");

    let (mut files, mut same, mut close, mut differ, mut ours_failed, mut both_failed, mut theirs_failed) =
        (0, 0, 0, 0, 0, 0, 0);
    let mut worst: Vec<(u32, String)> = Vec::new();
    let (mut ours_time, mut imageio_time) = (std::time::Duration::ZERO, std::time::Duration::ZERO);
    for line in std::io::stdin().lock().lines() {
        let path = line.unwrap();
        let Ok(bytes) = std::fs::read(&path) else { continue };
        if pear_image::sniff(&bytes).is_none() {
            continue;
        }
        files += 1;
        let t0 = std::time::Instant::now();
        let ours = pear_image::decode(&bytes);
        ours_time += t0.elapsed();
        let t1 = std::time::Instant::now();
        let theirs = unsafe {
            let data = CFDataCreate(std::ptr::null(), bytes.as_ptr(), bytes.len() as isize);
            let src = CGImageSourceCreateWithData(data, std::ptr::null());
            let image = if src.is_null() { std::ptr::null() } else { CGImageSourceCreateImageAtIndex(src, 0, std::ptr::null()) };
            let decoded = std::time::Instant::now();
            // Make ImageIO decode now (images are decoded lazily on first use).
            if !image.is_null() {
                let provider = CGImageGetDataProvider(image);
                let copy = CGDataProviderCopyData(provider);
                if !copy.is_null() {
                    CFRelease(copy);
                }
            }
            imageio_time += t1.elapsed();
            let _ = decoded;
            let out = if image.is_null() {
                None
            } else {
                let (w, h) = (CGImageGetWidth(image), CGImageGetHeight(image));
                let mut px = vec![0u8; w * h * 4];
                let cs = CGColorSpaceCreateWithName(kCGColorSpaceSRGB);
                let ctx = CGBitmapContextCreate(px.as_mut_ptr(), w, h, 8, w * 4, cs, PREMULTIPLIED_LAST_BIG);
                CGContextDrawImage(ctx, Rect { x: 0.0, y: 0.0, w: w as f64, h: h as f64 }, image);
                CGContextRelease(ctx);
                CGColorSpaceRelease(cs);
                CGImageRelease(image);
                Some((w as u32, h as u32, px))
            };
            if !src.is_null() {
                CFRelease(src);
            }
            CFRelease(data);
            out
        };
        match (ours, theirs) {
            (Err(e), None) => {
                both_failed += 1;
                println!("both-fail\t{path}\t{e}");
            }
            (Err(e), Some(_)) => {
                ours_failed += 1;
                println!("OURS-FAIL\t{path}\t{e}");
            }
            (Ok(_), None) => {
                theirs_failed += 1;
                println!("imageio-fail\t{path}");
            }
            (Ok(mut img), Some((w, h, px))) => {
                if (img.width, img.height) != (w, h) {
                    differ += 1;
                    println!("SIZE\t{path}\tours {}x{} imageio {w}x{h}", img.width, img.height);
                    continue;
                }
                img.premultiply();
                if let Some(dir) = &dump {
                    let base = std::path::Path::new(dir).join(format!("{files}"));
                    std::fs::write(base.with_extension("ours.rgba"), &img.pixels).unwrap();
                    std::fs::write(base.with_extension("imageio.rgba"), &px).unwrap();
                }
                let mut max = 0u32;
                let mut sum = 0u64;
                let mut pixels = 0usize;
                for (a, b) in img.pixels.chunks_exact(4).zip(px.chunks_exact(4)) {
                    let d = a.iter().zip(b).map(|(x, y)| (*x as i32 - *y as i32).unsigned_abs()).max().unwrap();
                    if d > 0 {
                        pixels += 1;
                    }
                    max = max.max(d);
                    sum += d as u64;
                }
                let mean = sum as f64 / (w as f64 * h as f64);
                let status = if max == 0 {
                    same += 1;
                    "same"
                } else if max <= 3 {
                    close += 1;
                    "close"
                } else {
                    differ += 1;
                    "DIFF"
                };
                println!("{status}\t{path}\tn={files} max={max} mean={mean:.3} pixels={pixels} {w}x{h} premul={}", bytes.windows(4).take(64).any(|w| w == b"CgBI"));
                worst.push((max, path));
            }
        }
    }
    eprintln!("decode time: pear-image {:.2}s, ImageIO {:.2}s", ours_time.as_secs_f64(), imageio_time.as_secs_f64());
    eprintln!(
        "{files} files: {same} identical, {close} within 3 levels, {differ} differ, {ours_failed} only ours failed, \
         {theirs_failed} only ImageIO failed, {both_failed} both failed"
    );
}
