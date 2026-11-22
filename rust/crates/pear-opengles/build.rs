//! The GLES ABI the bridge exports (see gles_prototypes.rs).

include!("gles_prototypes.rs");

fn main() {
    write_gles_forwarders();
}
