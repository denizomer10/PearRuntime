//! One line per file named on stdin: an FNV-1a hash of the decoded pixels
//! (and the decoder's source kind), to compare decoders across hosts.
use std::io::BufRead;

fn main() {
    for path in std::io::stdin().lock().lines().map_while(Result::ok) {
        let Ok(data) = std::fs::read(&path) else { continue };
        match pear_image::decode(&data) {
            Ok(img) => {
                let mut h: u64 = 0xcbf29ce484222325;
                for &b in &img.pixels {
                    h = (h ^ b as u64).wrapping_mul(0x100000001b3);
                }
                let kind = format!("{:?}", img.source);
                let kind = kind.split([' ', '{']).next().unwrap_or("");
                println!("{h:016x} {}x{} {kind}\t{path}", img.width, img.height);
            }
            Err(e) => println!("error {e}\t{path}"),
        }
    }
}
