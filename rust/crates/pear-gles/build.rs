//! The GLES ABI PearHost's OpenGLES.framework exports through the shared
//! forwarders (pear-opengles/src/forwarders.rs, gles_prototypes.rs).

include!("../pear-opengles/gles_prototypes.rs");

fn main() {
    println!("cargo:rerun-if-changed=../pear-opengles/gles_prototypes.rs");
    println!("cargo:rerun-if-changed=../pear-opengles/src/forwarders.rs");
    write_gles_forwarders();
}
