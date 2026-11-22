//! Prints a Mach-O's fixups one per line, as `dyld_info -fixups` names them:
//! `address type target` (rebase target as an unslid address, bind target as
//! library/symbol). Used to compare pear-macho with dyld_info.
fn main() {
    let path = std::env::args().nth(1).expect("usage: fixups-dump <mach-o>");
    let data = std::fs::read(&path).unwrap();
    let m = match pear_macho::MachO::parse(&data) {
        Ok(m) => m,
        Err(e) => {
            println!("error {e}");
            return;
        }
    };
    let fixups = match m.fixups() {
        Ok(f) => f,
        Err(e) => {
            println!("error {e}");
            return;
        }
    };
    // dyld_info's short library name: the leaf without ".dylib" and without
    // its last dotted component (libSystem.B -> libSystem, libbz2.1.0 -> libbz2.1).
    let leaf = |path: &str| -> String {
        let last = path.rsplit('/').next().unwrap_or(path);
        match last.strip_suffix(".dylib") {
            Some(stem) => stem.rsplit_once('.').map_or(stem, |(head, _)| head).to_string(),
            None => last.to_string(),
        }
    };
    for f in &fixups.fixups {
        match f.kind {
            pear_macho::FixupKind::Rebase { target, high8 } => {
                let t = target | (high8 as u64) << 56;
                println!("0x{:08X} rebase 0x{t:08X}", f.address)
            }
            pear_macho::FixupKind::Bind { import, addend } => {
                let imp = &fixups.imports[import];
                let lib = match imp.ordinal {
                    pear_macho::Ordinal::Dylib(i) => leaf(&m.dylibs[i].path),
                    pear_macho::Ordinal::SelfImage => "this-image".into(),
                    pear_macho::Ordinal::MainExecutable => "main-executable".into(),
                    pear_macho::Ordinal::Flat => "<flat-namespace>".into(),
                    pear_macho::Ordinal::WeakLookup => "<weak-def-coalesce>".into(),
                };
                let kind = if f.lazy { "lazy-bind" } else { "bind" };
                let add = if addend != 0 { format!(" + 0x{addend:X}") } else { String::new() };
                let weak = if imp.weak { " [weak-import]" } else { "" };
                println!("0x{:08X} {kind} {lib}/{}{add}{weak}", f.address, imp.name);
            }
        }
    }
}
