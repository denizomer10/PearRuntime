//! The GLES ABI PearHost's OpenGLES.framework exports through the GLES
//! forwarders (`src/forwarders.rs`, `gles_prototypes.rs`).

include!("gles_prototypes.rs");

fn main() {
    println!("cargo:rerun-if-changed=gles_prototypes.rs");
    println!("cargo:rerun-if-changed=src/forwarders.rs");
    write_gles_forwarders();
}
