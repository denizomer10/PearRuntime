//! Clean-room: standalone `pear-run` CLI runner for loading and booting
//! 64-bit arm64 Mach-O iOS binaries on `pear-runtime` (macOS & Linux).

use std::env;
use std::fs;
use std::process;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        eprintln!("Usage: pear-run <macho-binary> [--inspect-only]");
        process::exit(if args.is_empty() { 2 } else { 0 });
    }
    let path = &args[0];
    let inspect_only = args.iter().any(|a| a == "--inspect-only");

    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("pear-run: failed to read {path}: {e}");
            process::exit(1);
        }
    };

    if inspect_only {
        match pear_macho::MachO::parse(&bytes) {
            Ok(m) => {
                println!(
                    "Mach-O: segments={} dylibs={} exports={}",
                    m.segments.len(),
                    m.dylibs.len(),
                    m.exports().map(|v| v.len()).unwrap_or(0),
                );
                return;
            }
            Err(e) => {
                eprintln!("pear-run: Mach-O parse failed: {e:?}");
                process::exit(1);
            }
        }
    }

    let opts = pear_dyld::Options {
        allow_missing: true,
    };
    match pear_runtime::load_and_boot(&bytes, opts) {
        Ok((img, report)) => {
            println!(
                "pear-run: booted {path} (classes={} categories={} selectors={})",
                report.classes,
                report.categories,
                report.selectors,
            );
            if let Some(entry) = img.symbol("_main") {
                let main_fn: extern "C" fn(i32, *const *const i8) -> i32 =
                    unsafe { std::mem::transmute(entry) };
                let rc = main_fn(0, std::ptr::null());
                process::exit(rc);
            }
        }
        Err(e) => {
            eprintln!("pear-run: load_and_boot failed: {e:?}");
            process::exit(1);
        }
    }
}
