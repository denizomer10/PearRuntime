//! Maps and fixes up each Mach-O named on stdin without running it (imports
//! unresolved, as `allow_missing`), and reports per file: ok with counts, or
//! why not. A loader stress test over real apps on any arm64 host.
use std::io::BufRead;

fn main() {
    let none = |_: Option<&str>, _: &str| None;
    let (mut ok, mut failed) = (0, 0);
    for path in std::io::stdin().lock().lines().map_while(Result::ok) {
        let Ok(data) = std::fs::read(&path) else { continue };
        let start = std::time::Instant::now();
        match pear_dyld::load(&data, &none, pear_dyld::Options { allow_missing: true }) {
            Ok(image) => {
                ok += 1;
                println!(
                    "ok\t{:.0} ms\t{} unresolved imports\t{path}",
                    start.elapsed().as_secs_f64() * 1000.0,
                    image.missing.len()
                );
            }
            Err(e) => {
                failed += 1;
                println!("error\t{e}\t{path}");
            }
        }
    }
    eprintln!("{ok} loaded, {failed} not loaded");
}
