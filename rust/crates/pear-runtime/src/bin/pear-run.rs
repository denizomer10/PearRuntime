//! Clean-room: standalone `pearruntime` / `pear-run` CLI runner for loading
//! and booting 32-bit & 64-bit iOS Mach-O binaries or `.app` bundles on
//! `pear-runtime` (macOS & Linux, ARM64 native or x86_64 via `PearLegacyJIT`).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process;

fn resolve_executable(input: &str) -> PathBuf {
    let p = Path::new(input);
    if p.is_dir() && input.ends_with(".app") {
        // Try matching bundle stem (e.g., MyGame.app -> MyGame).
        if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
            let candidate = p.join(stem);
            if candidate.is_file() {
                return candidate;
            }
        }
        // Fallback: first non-hidden Mach-O file in `.app` root.
        if let Ok(entries) = fs::read_dir(p) {
            for ent in entries.flatten() {
                let ep = ent.path();
                let name = ent.file_name();
                if name.to_string_lossy().starts_with('.') {
                    continue;
                }
                if ep.is_file() && ep.extension().is_none() {
                    if let Ok(head) = fs::read(&ep).map(|b| b.into_iter().take(4).collect::<Vec<u8>>()) {
                        if head.len() == 4 && pear_macho::detect_guest_arch(&[head[0], head[1], head[2], head[3], 0, 0, 0, 0]).is_some()
                            || head == [0xCF, 0xFA, 0xED, 0xFE]
                            || head == [0xCE, 0xFA, 0xED, 0xFE]
                            || head == [0xCA, 0xFE, 0xBA, 0xBE]
                        {
                            return ep;
                        }
                    }
                }
            }
        }
    }
    p.to_path_buf()
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        eprintln!("Usage: pearruntime <macho-binary-or-.app> [--plan] [--inspect-only] [--check-load] [--list-missing] [--window-test]");
        process::exit(if args.is_empty() { 2 } else { 0 });
    }
    if args.iter().any(|a| a == "--window-test") {
        pear_runtime::init();
        let host = pear_runtime::pear_host::host();
        host.initialize();
        let win = host.create_window(
            pear_runtime::pear_host::GuestObject(std::ptr::null_mut()),
            pear_runtime::pear_host::Rect { x: 80.0, y: 60.0, width: 960.0, height: 600.0 },
        );
        let layer = pear_runtime::pear_host::LayerRef(0x1234 as *mut std::ffi::c_void);
        host.set_root_layer(win, layer);
        host.show_window(win, true);
        let surf_info = host.surface(layer);
        println!("pearruntime: opened host window (DISPLAY={:?})", env::var("DISPLAY").ok());
        host.run(Box::new(move || {
            if let Some(info) = surf_info {
                if let pear_runtime::pear_host::NativeSurface::Xlib { display, window } = info.surface {
                    let w = info.pixels.width as i32;
                    let h = info.pixels.height as i32;
                    let dpy = display as usize;
                    std::thread::spawn(move || {
                        if let Some(egl) = pear_runtime::egl_bridge::EglSurfaceCtx::create_from_x11(dpy as *mut std::ffi::c_void, window) {
                            let start = std::time::Instant::now();
                            loop {
                                let t = start.elapsed().as_secs_f32();
                                egl.render_demo_frame(w, h, t);
                                std::thread::sleep(std::time::Duration::from_millis(16));
                            }
                        }
                    });
                }
            }
        }));
        return;
    }
    let exe_path = resolve_executable(&args[0]);
    let raw_p = Path::new(&args[0]);
    if raw_p.is_dir() && args[0].ends_with(".app") {
        env::set_var("PEAR_BUNDLE_PATH", raw_p);
    }
    let plan_only = args.iter().any(|a| a == "--plan");
    let inspect_only = args.iter().any(|a| a == "--inspect-only");
    let check_load = args.iter().any(|a| a == "--check-load");
    let list_missing = args.iter().any(|a| a == "--list-missing");

    let bytes = match fs::read(&exe_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("pearruntime: failed to read {}: {e}", exe_path.display());
            process::exit(1);
        }
    };

    let plan = pear_runtime::plan_execution(&bytes);
    if plan_only {
        match plan {
            Some(p) => {
                println!("ExecutionPlan: {}", p.summary());
                return;
            }
            None => {
                eprintln!("pearruntime: unsupported binary format in {}", exe_path.display());
                process::exit(1);
            }
        }
    }

    if inspect_only {
        if let Some(ref p) = plan {
            println!("ExecutionPlan: {}", p.summary());
        }
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
                eprintln!("pearruntime: Mach-O parse failed: {e:?}");
                process::exit(1);
            }
        }
    }

    if let Some(ref p) = plan {
        if p.api_tier == pear_runtime::pear_jit::ApiTier::Ios32 {
            pear_runtime::api32::init(p);
            println!(
                "pearruntime: 32-bit iOS mode active ({}) -> handoff to PearLegacyEngine + PearLegacyJIT",
                p.summary()
            );
            if args.iter().any(|a| a == "--check-load" || a == "--list-missing") {
                return;
            }
            let legacy_bin = env::var("PEAR_LEGACY_ENGINE_BIN")
                .ok()
                .map(PathBuf::from)
                .filter(|b| b.is_file())
                .or_else(|| {
                    let exe_dir = env::current_exe().ok().and_then(|e| e.parent().map(|d| d.to_path_buf()));
                    let mut candidates: Vec<PathBuf> = Vec::new();
                    if let Some(dir) = exe_dir {
                        candidates.push(dir.join("touchHLE"));
                        candidates.push(dir.join("pear-legacy-engine"));
                    }
                    candidates.extend([
                        PathBuf::from("/tmp/pear-legacy-target/release/touchHLE"),
                        PathBuf::from("/tmp/pear-legacy-target/debug/touchHLE"),
                        PathBuf::from("/work/third_party/legacy-engine/target/release/touchHLE"),
                        PathBuf::from("/work/third_party/legacy-engine/target/debug/touchHLE"),
                        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../third_party/legacy-engine/target/release/touchHLE")),
                        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../third_party/legacy-engine/target/debug/touchHLE")),
                    ]);
                    candidates.into_iter().find(|c| c.is_file())
                });
            let Some(bin) = legacy_bin else {
                eprintln!(
                    "pearruntime: PearLegacyEngine binary not found. Set PEAR_LEGACY_ENGINE_BIN or build third_party/legacy-engine."
                );
                process::exit(1);
            };
            let mut cmd = process::Command::new(&bin);
            #[cfg(target_os = "linux")]
            if env::var_os("ALSOFT_DRIVERS").is_none() && !Path::new("/dev/snd").exists() {
                cmd.env("ALSOFT_DRIVERS", "null");
            }
            if env::var_os("PEAR_LEGACY_RESOURCES_DIR").is_none() {
                for res_dir in [
                    PathBuf::from("/work/third_party/legacy-engine"),
                    PathBuf::from(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../../third_party/legacy-engine"
                    )),
                ] {
                    if res_dir.join("touchHLE_fonts").is_dir() {
                        cmd.env("PEAR_LEGACY_RESOURCES_DIR", &res_dir);
                        break;
                    }
                }
            }
            let target_arg = fs::canonicalize(&args[0]).unwrap_or_else(|_| PathBuf::from(&args[0]));
            cmd.arg(target_arg);
            for extra in args.iter().skip(1) {
                if !matches!(extra.as_str(), "--plan" | "--inspect-only" | "--check-load" | "--list-missing" | "--window-test") {
                    cmd.arg(extra);
                }
            }
            match cmd.status() {
                Ok(st) => process::exit(st.code().unwrap_or(1)),
                Err(e) => {
                    eprintln!("pearruntime: failed to launch PearLegacyEngine ({}): {e}", bin.display());
                    process::exit(1);
                }
            }
        }
    }

    let opts = pear_dyld::Options {
        allow_missing: true,
    };
    if check_load || list_missing {
        pear_runtime::init();
        let missing = std::sync::Mutex::new(std::collections::BTreeSet::<String>::new());
        let resolver = |lib: Option<&str>, sym: &str| {
            let r = pear_runtime::resolve_symbol(lib, sym);
            if r.is_none() && list_missing {
                missing.lock().unwrap().insert(format!("{} :: {}", lib.unwrap_or("<self>"), sym));
            }
            r
        };
        match pear_dyld::load(&bytes, &resolver, opts) {
            Ok(img) => {
                let report = unsafe { pear_libobjc::register_image(&img) };
                println!(
                    "pearruntime --check-load: {} classes={} unrealized={} categories={} selectors={}",
                    exe_path.display(),
                    report.classes,
                    report.unrealized.len(),
                    report.categories,
                    report.selectors,
                );
                if !report.unrealized.is_empty() {
                    println!("unrealized: {:?}", report.unrealized);
                }
                if list_missing {
                    let m = missing.lock().unwrap();
                    println!("missing symbols ({}):", m.len());
                    for entry in m.iter().take(60) {
                        println!("  {entry}");
                    }
                }
                return;
            }
            Err(e) => {
                eprintln!("pearruntime: load failed: {e:?}");
                process::exit(1);
            }
        }
    }

    match pear_runtime::load_and_boot(&bytes, opts) {
        Ok((img, report)) => {
            if let Some(ref p) = plan {
                println!("pearruntime: {}", p.summary());
            }
            println!(
                "pearruntime: booted {} (classes={} categories={} selectors={})",
                exe_path.display(),
                report.classes,
                report.categories,
                report.selectors,
            );
            if let Some(entry) = img.symbol("_main").or_else(|| img.entry()) {
                let main_fn: extern "C" fn(i32, *const *const std::os::raw::c_char) -> i32 =
                    unsafe { std::mem::transmute(entry) };
                let arg0 = std::ffi::CString::new(exe_path.to_string_lossy().as_bytes())
                    .unwrap_or_default();
                let argv = [arg0.as_ptr(), std::ptr::null()];
                let rc = main_fn(1, argv.as_ptr());
                process::exit(rc);
            }
        }
        Err(e) => {
            eprintln!("pearruntime: load_and_boot failed: {e:?}");
            process::exit(1);
        }
    }
}
