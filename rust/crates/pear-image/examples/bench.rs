//! Decode time per stage: files named on stdin, PNG inflate alone vs the
//! whole decode, and JPEG.
use std::io::BufRead;
use std::time::{Duration, Instant};

fn main() {
    let files: Vec<Vec<u8>> = std::io::stdin().lock().lines().filter_map(|l| std::fs::read(l.ok()?).ok()).collect();
    let (mut inflate, mut png, mut jpeg) = (Duration::ZERO, Duration::ZERO, Duration::ZERO);
    let mut megapixels = 0f64;
    for data in &files {
        match pear_image::sniff(data) {
            Some(pear_image::Format::Png) => {
                // IDAT payload, then a zlib or raw inflate of it.
                let (mut pos, mut idat, mut cgbi) = (8, Vec::new(), false);
                while pos + 8 <= data.len() {
                    let len = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
                    let kind = &data[pos + 4..pos + 8];
                    if kind == b"IDAT" {
                        idat.extend_from_slice(&data[pos + 8..(pos + 8 + len).min(data.len())]);
                    }
                    cgbi |= kind == b"CgBI";
                    pos += 12 + len;
                }
                let t = Instant::now();
                let _ = if cgbi { pear_image::inflate::inflate(&idat, 0) } else { pear_image::inflate::zlib_decompress(&idat, 0) };
                inflate += t.elapsed();
                let t = Instant::now();
                if let Ok(img) = pear_image::decode(data) {
                    megapixels += img.width as f64 * img.height as f64 / 1e6;
                }
                png += t.elapsed();
            }
            Some(pear_image::Format::Jpeg) => {
                let t = Instant::now();
                let _ = pear_image::decode(data);
                jpeg += t.elapsed();
            }
            None => {}
        }
    }
    println!(
        "PNG {:.2}s ({:.2}s inflate) for {megapixels:.0} MP; JPEG {:.2}s",
        png.as_secs_f64(),
        inflate.as_secs_f64(),
        jpeg.as_secs_f64()
    );
}
