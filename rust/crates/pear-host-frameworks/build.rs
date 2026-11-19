//! Selects the framework this build is for: PEAR_HOST_FRAMEWORK=<name> picks
//! src/generated/<name>.rs (tools/gen-host-frameworks.py writes them).
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=PEAR_HOST_FRAMEWORK");
    println!("cargo:rerun-if-changed=src/generated");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("framework.rs");
    let source = match env::var("PEAR_HOST_FRAMEWORK") {
        Ok(name) if !name.is_empty() => {
            let path = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join(format!("src/generated/{name}.rs"));
            println!("cargo:rerun-if-changed={}", path.display());
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        }
        // A plain workspace build checks the crate without a framework.
        _ => String::new(),
    };
    fs::write(out, source).unwrap();
}
