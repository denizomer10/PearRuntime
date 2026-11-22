// Included by `pear-gles/build.rs`: writes OUT_DIR/forwarders.rs, the list of
// GLES prototypes for `src/forwarders.rs`.
//
// The GLES ABI: every `gl*` prototype in ANGLE's pinned Khronos headers (GLES
// 1 + extensions, GLES 2/3.x + extensions), so a game image binds at launch
// even for entry points ANGLE lacks (those resolve through
// PearANGLEGameSymbol: adapters, or a logged abort on call).

use std::collections::BTreeSet;
use std::path::PathBuf;

fn write_gles_forwarders() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let include = manifest.join("../../../third_party/angle/include");
    let _ = &manifest;
    let headers = ["GLES/gl.h", "GLES/glext.h", "GLES2/gl2.h", "GLES2/gl2ext.h", "GLES3/gl3.h", "GLES3/gl31.h", "GLES3/gl32.h"];
    let mut names = BTreeSet::new();
    for header in headers {
        let path = include.join(header);
        println!("cargo:rerun-if-changed={}", path.display());
        let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        // `GL_API[CALL] <type> GL_APIENTRY glName (` prototypes, possibly
        // spanning whitespace; typedefs (PFNGL…) are not prototypes.
        for (index, _) in text.match_indices("GL_APIENTRY") {
            let rest = text[index + "GL_APIENTRY".len()..].trim_start();
            let name: String = rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '_').collect();
            if !name.starts_with("gl") || name.len() < 3 || !name.as_bytes()[2].is_ascii_uppercase() {
                continue;
            }
            let after = rest[name.len()..].trim_start();
            let line_start = text[..index].rfind('\n').map(|i| i + 1).unwrap_or(0);
            let line = &text[line_start..index];
            if after.starts_with('(') && (line.contains("GL_API ") || line.contains("GL_APICALL ")) && !line.contains("typedef") {
                names.insert(name);
            }
        }
    }
    assert!(names.len() > 300, "too few GLES prototypes found ({})", names.len());
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("forwarders.rs");
    let list: Vec<String> = names.iter().cloned().collect();
    std::fs::write(&out, format!("forward!({});\n", list.join(", "))).unwrap();
}
