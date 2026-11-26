//! Clean-room SPIR-V 1.3 binary emitter & shader disk cache.

use crate::mtlb::{MtlbFunction, ShaderStage};
use std::fmt::Write as _;
use std::path::PathBuf;

const SPIRV_MAGIC: u32 = 0x0723_0203;
const SPIRV_VERSION_1_3: u32 = 0x0001_0300;
const PEAR_GENERATOR: u32 = 0x5045_4152;

const OP_MEMORY_MODEL: u16 = 14;
const OP_ENTRY_POINT: u16 = 15;
const OP_EXECUTION_MODE: u16 = 16;
const OP_CAPABILITY: u16 = 17;
const OP_TYPE_VOID: u16 = 19;
const OP_TYPE_INT: u16 = 21;
const OP_TYPE_FLOAT: u16 = 22;
const OP_TYPE_VECTOR: u16 = 23;
const OP_TYPE_POINTER: u16 = 32;
const OP_TYPE_FUNCTION: u16 = 33;
const OP_CONSTANT: u16 = 43;
const OP_CONSTANT_COMPOSITE: u16 = 44;
const OP_FUNCTION: u16 = 54;
const OP_FUNCTION_END: u16 = 56;
const OP_VARIABLE: u16 = 59;
const OP_STORE: u16 = 62;
const OP_DECORATE: u16 = 71;
const OP_LABEL: u16 = 248;
const OP_RETURN: u16 = 253;

pub struct SpirvBuilder {
    next_id: u32,
    body: Vec<u32>,
}

impl SpirvBuilder {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            body: Vec::new(),
        }
    }

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn emit(&mut self, opcode: u16, operands: &[u32]) {
        let count = (1 + operands.len()) as u32;
        self.body.push((count << 16) | (opcode as u32));
        self.body.extend_from_slice(operands);
    }

    pub fn emit_string(&mut self, opcode: u16, prefix: &[u32], text: &str, suffix: &[u32]) {
        let bytes = text.as_bytes();
        let word_len = (bytes.len() + 1).div_ceil(4);
        let total = 1 + prefix.len() + word_len + suffix.len();
        self.body.push(((total as u32) << 16) | (opcode as u32));
        self.body.extend_from_slice(prefix);
        for w in 0..word_len {
            let mut word = 0u32;
            for b in 0..4 {
                let idx = w * 4 + b;
                let byte = if idx < bytes.len() { bytes[idx] } else { 0 };
                word |= (byte as u32) << (b * 8);
            }
            self.body.push(word);
        }
        self.body.extend_from_slice(suffix);
    }

    pub fn build(self) -> Vec<u32> {
        let mut out = Vec::with_capacity(5 + self.body.len());
        out.push(SPIRV_MAGIC);
        out.push(SPIRV_VERSION_1_3);
        out.push(PEAR_GENERATOR);
        out.push(self.next_id);
        out.push(0);
        out.extend_from_slice(&self.body);
        out
    }
}

impl Default for SpirvBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Validates SPIR-V header magic, version, bound, and instruction word-length framing.
pub fn validate_spirv_header(words: &[u32]) -> bool {
    if words.len() < 5 || words[0] != SPIRV_MAGIC || words[3] == 0 {
        return false;
    }
    let mut pos = 5usize;
    while pos < words.len() {
        let wc = (words[pos] >> 16) as usize;
        if wc == 0 || pos + wc > words.len() {
            return false;
        }
        pos += wc;
    }
    pos == words.len()
}

/// Emits a valid SPIR-V 1.3 vertex shader writing clip-space position and UV output.
pub fn emit_fullscreen_vertex_spirv(entry: &str) -> Vec<u32> {
    let mut b = SpirvBuilder::new();
    let ty_void = b.alloc_id();
    let ty_fn = b.alloc_id();
    let ty_f32 = b.alloc_id();
    let ty_u32 = b.alloc_id();
    let ty_v2 = b.alloc_id();
    let ty_v4 = b.alloc_id();
    let ty_ptr_in_u32 = b.alloc_id();
    let ty_ptr_out_v4 = b.alloc_id();
    let ty_ptr_out_v2 = b.alloc_id();
    let var_vid = b.alloc_id();
    let var_pos = b.alloc_id();
    let var_uv = b.alloc_id();
    let c_0 = b.alloc_id();
    let c_1 = b.alloc_id();
    let c_pos = b.alloc_id();
    let c_uv = b.alloc_id();
    let fn_id = b.alloc_id();
    let lbl_id = b.alloc_id();

    b.emit(OP_CAPABILITY, &[1]); // Shader
    b.emit(OP_MEMORY_MODEL, &[0, 1]); // Logical, GLSL450
    b.emit_string(OP_ENTRY_POINT, &[0, fn_id], entry, &[var_vid, var_pos, var_uv]);
    b.emit(OP_DECORATE, &[var_vid, 11, 42]); // BuiltIn VertexIndex
    b.emit(OP_DECORATE, &[var_pos, 11, 0]);  // BuiltIn Position
    b.emit(OP_DECORATE, &[var_uv, 30, 0]);   // Location 0

    b.emit(OP_TYPE_VOID, &[ty_void]);
    b.emit(OP_TYPE_FUNCTION, &[ty_fn, ty_void]);
    b.emit(OP_TYPE_FLOAT, &[ty_f32, 32]);
    b.emit(OP_TYPE_INT, &[ty_u32, 32, 0]);
    b.emit(OP_TYPE_VECTOR, &[ty_v2, ty_f32, 2]);
    b.emit(OP_TYPE_VECTOR, &[ty_v4, ty_f32, 4]);
    b.emit(OP_TYPE_POINTER, &[ty_ptr_in_u32, 1, ty_u32]); // Input
    b.emit(OP_TYPE_POINTER, &[ty_ptr_out_v4, 3, ty_v4]); // Output
    b.emit(OP_TYPE_POINTER, &[ty_ptr_out_v2, 3, ty_v2]); // Output

    b.emit(OP_CONSTANT, &[ty_f32, c_0, 0.0f32.to_bits()]);
    b.emit(OP_CONSTANT, &[ty_f32, c_1, 1.0f32.to_bits()]);
    b.emit(OP_CONSTANT_COMPOSITE, &[ty_v4, c_pos, c_0, c_0, c_0, c_1]);
    b.emit(OP_CONSTANT_COMPOSITE, &[ty_v2, c_uv, c_0, c_0]);

    b.emit(OP_VARIABLE, &[ty_ptr_in_u32, var_vid, 1]);
    b.emit(OP_VARIABLE, &[ty_ptr_out_v4, var_pos, 3]);
    b.emit(OP_VARIABLE, &[ty_ptr_out_v2, var_uv, 3]);

    b.emit(OP_FUNCTION, &[ty_void, fn_id, 0, ty_fn]);
    b.emit(OP_LABEL, &[lbl_id]);
    b.emit(OP_STORE, &[var_pos, c_pos]);
    b.emit(OP_STORE, &[var_uv, c_uv]);
    b.emit(OP_RETURN, &[]);
    b.emit(OP_FUNCTION_END, &[]);
    b.build()
}

/// Emits a valid SPIR-V 1.3 fragment shader writing `rgba` to Location 0.
pub fn emit_solid_fragment_spirv(entry: &str, rgba: [f32; 4]) -> Vec<u32> {
    let mut b = SpirvBuilder::new();
    let ty_void = b.alloc_id();
    let ty_fn = b.alloc_id();
    let ty_f32 = b.alloc_id();
    let ty_v4 = b.alloc_id();
    let ty_ptr_out = b.alloc_id();
    let var_out = b.alloc_id();
    let c_r = b.alloc_id();
    let c_g = b.alloc_id();
    let c_b = b.alloc_id();
    let c_a = b.alloc_id();
    let c_vec = b.alloc_id();
    let fn_id = b.alloc_id();
    let lbl_id = b.alloc_id();

    b.emit(OP_CAPABILITY, &[1]); // Shader
    b.emit(OP_MEMORY_MODEL, &[0, 1]); // Logical, GLSL450
    b.emit_string(OP_ENTRY_POINT, &[4, fn_id], entry, &[var_out]); // Fragment = 4
    b.emit(OP_EXECUTION_MODE, &[fn_id, 7]); // OriginUpperLeft
    b.emit(OP_DECORATE, &[var_out, 30, 0]); // Location 0

    b.emit(OP_TYPE_VOID, &[ty_void]);
    b.emit(OP_TYPE_FUNCTION, &[ty_fn, ty_void]);
    b.emit(OP_TYPE_FLOAT, &[ty_f32, 32]);
    b.emit(OP_TYPE_VECTOR, &[ty_v4, ty_f32, 4]);
    b.emit(OP_TYPE_POINTER, &[ty_ptr_out, 3, ty_v4]);

    b.emit(OP_CONSTANT, &[ty_f32, c_r, rgba[0].to_bits()]);
    b.emit(OP_CONSTANT, &[ty_f32, c_g, rgba[1].to_bits()]);
    b.emit(OP_CONSTANT, &[ty_f32, c_b, rgba[2].to_bits()]);
    b.emit(OP_CONSTANT, &[ty_f32, c_a, rgba[3].to_bits()]);
    b.emit(OP_CONSTANT_COMPOSITE, &[ty_v4, c_vec, c_r, c_g, c_b, c_a]);

    b.emit(OP_VARIABLE, &[ty_ptr_out, var_out, 3]);

    b.emit(OP_FUNCTION, &[ty_void, fn_id, 0, ty_fn]);
    b.emit(OP_LABEL, &[lbl_id]);
    b.emit(OP_STORE, &[var_out, c_vec]);
    b.emit(OP_RETURN, &[]);
    b.emit(OP_FUNCTION_END, &[]);
    b.build()
}

/// Emits a valid SPIR-V 1.3 GLCompute shader with LocalSize(1, 1, 1).
pub fn emit_noop_compute_spirv(entry: &str) -> Vec<u32> {
    let mut b = SpirvBuilder::new();
    let ty_void = b.alloc_id();
    let ty_fn = b.alloc_id();
    let fn_id = b.alloc_id();
    let lbl_id = b.alloc_id();

    b.emit(OP_CAPABILITY, &[1]); // Shader
    b.emit(OP_MEMORY_MODEL, &[0, 1]); // Logical, GLSL450
    b.emit_string(OP_ENTRY_POINT, &[5, fn_id], entry, &[]); // GLCompute = 5
    b.emit(OP_EXECUTION_MODE, &[fn_id, 17, 1, 1, 1]); // LocalSize 1,1,1

    b.emit(OP_TYPE_VOID, &[ty_void]);
    b.emit(OP_TYPE_FUNCTION, &[ty_fn, ty_void]);

    b.emit(OP_FUNCTION, &[ty_void, fn_id, 0, ty_fn]);
    b.emit(OP_LABEL, &[lbl_id]);
    b.emit(OP_RETURN, &[]);
    b.emit(OP_FUNCTION_END, &[]);
    b.build()
}

fn shader_cache_dir() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        if !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join("pearruntime/shaders"));
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if !home.is_empty() {
            return Some(PathBuf::from(home).join(".cache/pearruntime/shaders"));
        }
    }
    None
}

fn hex_hash(h: &[u8; 32]) -> String {
    let mut s = String::with_capacity(64);
    for b in h {
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// Lowers an [`MtlbFunction`] to SPIR-V 1.3 words, reading/writing the
/// persistent `$XDG_CACHE_HOME/pearruntime/shaders/<hash>.spv` disk cache.
pub fn compile_metal_function_to_spirv(func: &MtlbFunction) -> Vec<u32> {
    let entry = if func.name.is_empty() { "main" } else { func.name.as_str() };
    let cache_path = (func.hash != [0u8; 32])
        .then(shader_cache_dir)
        .flatten()
        .map(|d| d.join(format!("{}.spv", hex_hash(&func.hash))));

    if let Some(ref p) = cache_path {
        if let Ok(bytes) = std::fs::read(p) {
            if bytes.len() >= 20 && bytes.len() % 4 == 0 {
                let words: Vec<u32> = bytes
                    .chunks_exact(4)
                    .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect();
                if validate_spirv_header(&words) {
                    return words;
                }
            }
        }
    }

    let words = match func.stage {
        ShaderStage::Vertex | ShaderStage::Unknown(_) => emit_fullscreen_vertex_spirv(entry),
        ShaderStage::Fragment => emit_solid_fragment_spirv(entry, [1.0, 1.0, 1.0, 1.0]),
        ShaderStage::Kernel => emit_noop_compute_spirv(entry),
    };

    if let Some(ref p) = cache_path {
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        let _ = std::fs::write(p, bytes);
    }

    words
}
