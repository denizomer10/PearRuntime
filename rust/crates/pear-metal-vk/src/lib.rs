//! Clean-room Metal → Vulkan + `MTLB`/AIR → SPIR-V compatibility layer
//! built on `pear-libobjc`, `pear-foundation`, `pear-graphics`, and `pear-host`.

pub mod astc;
pub mod mtlb;
pub mod objc_metal;
pub mod spirv;
pub mod vk;

pub use objc_metal::{
    command_buffer_counts, copy_all_devices, create_system_default_device, function_spirv, init,
    last_presented_frame, next_drawable_for_layer, render_stats, MTLClearColor, MTLOrigin,
    MTLRegion, MTLSize, MTLViewport,
};

/// Resolves Mach-O imports from `Metal.framework` (and Metal helper symbols).
pub fn symbol(name: &str) -> Option<usize> {
    init();
    match name {
        "_MTLCreateSystemDefaultDevice" => Some(create_system_default_device as *const () as usize),
        "_MTLCopyAllDevices" => Some(copy_all_devices as *const () as usize),
        "_OBJC_CLASS_$_MTLTextureDescriptor" => Some(objc_metal::MTLTEXTUREDESCRIPTOR_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_MTLRenderPipelineDescriptor" => {
            Some(objc_metal::MTLRENDERPIPELINEDESCRIPTOR_CLASS.ptr() as usize)
        }
        "_OBJC_CLASS_$_MTLRenderPassDescriptor" => Some(objc_metal::MTLRENDERPASSDESCRIPTOR_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_MTLDepthStencilDescriptor" => Some(objc_metal::MTLDEPTHSTENCILDESCRIPTOR_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_MTLSamplerDescriptor" => Some(objc_metal::MTLSAMPLERDESCRIPTOR_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_MTLVertexDescriptor" => Some(objc_metal::MTLVERTEXDESCRIPTOR_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_MTLComputePipelineDescriptor" => {
            Some(objc_metal::MTLCOMPUTEPIPELINEDESC_CLASS.ptr() as usize)
        }
        "_OBJC_CLASS_$_MTLHeapDescriptor" => Some(objc_metal::MTLHEAPDESCRIPTOR_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_MTLCompileOptions" => Some(objc_metal::MTLCOMPILEOPTIONS_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_MTLFunctionConstantValues" => Some(objc_metal::MTLFUNCTIONCONSTANTS_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_MTLCaptureManager" => Some(objc_metal::MTLCAPTUREMANAGER_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_MTLCaptureDescriptor" => Some(objc_metal::MTLCAPTUREDESCRIPTOR_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_MTLStencilDescriptor" => Some(objc_metal::MTLSTENCILDESCRIPTOR_CLASS.ptr() as usize),
        "_OBJC_CLASS_$_MTLRenderPipelineColorAttachmentDescriptor" => {
            Some(objc_metal::MTLPIPELINE_COLOR_ATTACH_CLASS.ptr() as usize)
        }
        "_OBJC_CLASS_$_MTLRenderPassColorAttachmentDescriptor" => {
            Some(objc_metal::MTLRENDERPASSCOLORATTACH_CLASS.ptr() as usize)
        }
        "_OBJC_CLASS_$_MTLRenderPassDepthAttachmentDescriptor" => {
            Some(objc_metal::MTLRENDERPASSDEPTHATTACH_CLASS.ptr() as usize)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pear_libobjc::abi::{Id, Sel};
    use pear_libobjc::message::lookup_for_send;
    use pear_libobjc::runtime as objc_rt;
    use std::ffi::c_void;

    unsafe fn send0<R>(obj: Id, name: &str) -> R {
        let s = objc_rt::sel(name);
        let d = lookup_for_send(obj, s);
        let f: unsafe extern "C" fn(Id, Sel) -> R = std::mem::transmute(d.imp);
        f(d.receiver, s)
    }

    unsafe fn send1<A1, R>(obj: Id, name: &str, a1: A1) -> R {
        let s = objc_rt::sel(name);
        let d = lookup_for_send(obj, s);
        let f: unsafe extern "C" fn(Id, Sel, A1) -> R = std::mem::transmute(d.imp);
        f(d.receiver, s, a1)
    }

    unsafe fn send2<A1, A2, R>(obj: Id, name: &str, a1: A1, a2: A2) -> R {
        let s = objc_rt::sel(name);
        let d = lookup_for_send(obj, s);
        let f: unsafe extern "C" fn(Id, Sel, A1, A2) -> R = std::mem::transmute(d.imp);
        f(d.receiver, s, a1, a2)
    }

    unsafe fn send3<A1, A2, A3, R>(obj: Id, name: &str, a1: A1, a2: A2, a3: A3) -> R {
        let s = objc_rt::sel(name);
        let d = lookup_for_send(obj, s);
        let f: unsafe extern "C" fn(Id, Sel, A1, A2, A3) -> R = std::mem::transmute(d.imp);
        f(d.receiver, s, a1, a2, a3)
    }

    unsafe fn send5<A1, A2, A3, A4, A5, R>(obj: Id, name: &str, a1: A1, a2: A2, a3: A3, a4: A4, a5: A5) -> R {
        let s = objc_rt::sel(name);
        let d = lookup_for_send(obj, s);
        let f: unsafe extern "C" fn(Id, Sel, A1, A2, A3, A4, A5) -> R = std::mem::transmute(d.imp);
        f(d.receiver, s, a1, a2, a3, a4, a5)
    }

    fn build_synthetic_mtlb(name: &str, stage: u8, bitcode: &[u8]) -> Vec<u8> {
        let mut tlv = Vec::new();
        // NAME
        tlv.extend_from_slice(b"NAME");
        tlv.extend_from_slice(&((name.len() + 1) as u16).to_le_bytes());
        tlv.extend_from_slice(name.as_bytes());
        tlv.push(0);
        // TYPE
        tlv.extend_from_slice(b"TYPE");
        tlv.extend_from_slice(&1u16.to_le_bytes());
        tlv.push(stage);
        // HASH
        tlv.extend_from_slice(b"HASH");
        tlv.extend_from_slice(&32u16.to_le_bytes());
        tlv.extend_from_slice(&[0x42u8; 32]);
        // MDSZ
        tlv.extend_from_slice(b"MDSZ");
        tlv.extend_from_slice(&8u16.to_le_bytes());
        tlv.extend_from_slice(&(bitcode.len() as u64).to_le_bytes());
        // OFFT
        tlv.extend_from_slice(b"OFFT");
        tlv.extend_from_slice(&24u16.to_le_bytes());
        tlv.extend_from_slice(&0u64.to_le_bytes());
        tlv.extend_from_slice(&0u64.to_le_bytes());
        tlv.extend_from_slice(&0u64.to_le_bytes());
        // ENDT
        tlv.extend_from_slice(b"ENDT");

        let mut fl = Vec::new();
        fl.extend_from_slice(&1u32.to_le_bytes()); // count = 1
        fl.extend_from_slice(&(tlv.len() as u32).to_le_bytes()); // entry size
        fl.extend_from_slice(&tlv);

        let fl_off = 88u64;
        let bitcode_off = fl_off + fl.len() as u64;
        let total = bitcode_off + bitcode.len() as u64;

        let mut out = vec![0u8; 88];
        out[0..4].copy_from_slice(b"MTLB");
        out[4..8].copy_from_slice(&[0x01, 0x00, 0x02, 0x00]); // v1.2 iOS
        out[11] = 0x82;
        out[16..24].copy_from_slice(&total.to_le_bytes());
        out[24..32].copy_from_slice(&fl_off.to_le_bytes());
        out[32..40].copy_from_slice(&(fl.len() as u64).to_le_bytes());
        out[72..80].copy_from_slice(&bitcode_off.to_le_bytes());
        out[80..88].copy_from_slice(&(bitcode.len() as u64).to_le_bytes());
        out.extend_from_slice(&fl);
        out.extend_from_slice(bitcode);
        out
    }

    #[test]
    fn mtlb_parser_extracts_functions_and_bitcode() {
        let bc = [0x42, 0x43, 0xC0, 0xDE, 1, 2, 3, 4];
        let raw = build_synthetic_mtlb("xlatMtlMain", 0, &bc);
        let parsed = mtlb::parse_mtlb(&raw).unwrap();
        assert_eq!(parsed.functions.len(), 1);
        let f = &parsed.functions[0];
        assert_eq!(f.name, "xlatMtlMain");
        assert_eq!(f.stage, mtlb::ShaderStage::Vertex);
        assert!(mtlb::is_llvm_bitcode(&f.bitcode));
        assert_eq!(f.bitcode, bc);
    }

    #[test]
    fn parses_real_genshin_mtlb_if_present() {
        let p = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../build/host-apps/GenshinImpact.payload/Payload/GenshinImpact.app/Data/resources.assets"
        );
        let Ok(bytes) = std::fs::read(p) else { return };
        let Some(pos) = bytes.windows(4).position(|w| w == b"MTLB") else { return };
        let lib = mtlb::parse_mtlb(&bytes[pos..pos + 0x2000.min(bytes.len() - pos)]).unwrap();
        assert!(!lib.functions.is_empty());
        assert_eq!(lib.functions[0].name, "xlatMtlMain");
        assert!(mtlb::is_llvm_bitcode(&lib.functions[0].bitcode));
    }

    #[test]
    fn spirv_emitter_produces_valid_modules() {
        let v = spirv::emit_fullscreen_vertex_spirv("vtxMain");
        let f = spirv::emit_solid_fragment_spirv("fragMain", [1.0, 0.5, 0.25, 1.0]);
        let c = spirv::emit_noop_compute_spirv("compMain");
        assert!(spirv::validate_spirv_header(&v));
        assert!(spirv::validate_spirv_header(&f));
        assert!(spirv::validate_spirv_header(&c));
    }

    static PRESENT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn objc_metal_end_to_end_pipeline() {
        let _present_guard = PRESENT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        pear_libobjc::object::init_runtime();
        pear_libobjc::blocks::init_blocks();
        pear_foundation::init_foundation();
        init();
        unsafe {
            let dev = create_system_default_device();
            assert!(!dev.is_null());

            let seed = [1u8, 2, 3, 4, 5, 6, 7, 8];
            let src_buf: Id = send3(dev, "newBufferWithBytes:length:options:", seed.as_ptr(), seed.len(), 0usize);
            let dst_buf: Id = send2(dev, "newBufferWithLength:options:", seed.len(), 0usize);
            assert_eq!(send0::<usize>(src_buf, "length"), 8);

            let cq: Id = send0(dev, "newCommandQueue");
            let cb: Id = send0(cq, "commandBuffer");

            let blit: Id = send0(cb, "blitCommandEncoder");
            send5::<Id, usize, Id, usize, usize, ()>(
                blit,
                "copyFromBuffer:sourceOffset:toBuffer:destinationOffset:size:",
                src_buf,
                0,
                dst_buf,
                0,
                8,
            );
            send0::<()>(blit, "endEncoding");

            let bc = [0x42, 0x43, 0xC0, 0xDE, 9, 8, 7, 6];
            let mtlb_bytes = build_synthetic_mtlb("xlatMtlMain", 0, &bc);
            let nsdata_cls = objc_rt::class_named("NSData").unwrap() as Id;
            let nsdata: Id = send2(nsdata_cls, "dataWithBytes:length:", mtlb_bytes.as_ptr(), mtlb_bytes.len());
            let mut err: Id = std::ptr::null_mut();
            let lib: Id = send2(dev, "newLibraryWithData:error:", nsdata, &mut err as *mut Id);
            let fn_name = pear_foundation::nsstring_from_rust("xlatMtlMain");
            let fn_obj: Id = send1(lib, "newFunctionWithName:", fn_name);
            assert!(!fn_obj.is_null());
            assert!(spirv::validate_spirv_header(&function_spirv(fn_obj).unwrap()));

            let pdesc_cls = objc_rt::class_named("MTLRenderPipelineDescriptor").unwrap() as Id;
            let pdesc: Id = send0(send0(pdesc_cls, "alloc"), "init");
            send1::<Id, ()>(pdesc, "setVertexFunction:", fn_obj);
            let pso: Id = send2(dev, "newRenderPipelineStateWithDescriptor:error:", pdesc, &mut err as *mut Id);
            assert!(!pso.is_null());

            let pass_cls = objc_rt::class_named("MTLRenderPassDescriptor").unwrap() as Id;
            let pass: Id = send0(pass_cls, "renderPassDescriptor");
            let r_enc: Id = send1(cb, "renderCommandEncoderWithDescriptor:", pass);
            send1::<Id, ()>(r_enc, "setRenderPipelineState:", pso);
            send3::<usize, usize, usize, ()>(r_enc, "drawPrimitives:vertexStart:vertexCount:", 3, 0, 3);
            send0::<()>(r_enc, "endEncoding");

            send0::<()>(cb, "commit");
            send0::<()>(cb, "waitUntilCompleted");
            assert_eq!(send0::<usize>(cb, "status"), 4);
            assert_eq!(command_buffer_counts(cb), Some((1, 0, 1)));

            let dst_ptr = send0::<*const c_void>(dst_buf, "contents") as *const u8;
            let copied = std::slice::from_raw_parts(dst_ptr, 8);
            assert_eq!(copied, &seed);

            // Render-pass & depth-stencil descriptor separation and accessors
            assert_ne!(send0::<Id>(pass, "depthAttachment"), send0::<Id>(pass, "stencilAttachment"));
            let depth_att: Id = send0(pass, "depthAttachment");
            assert!((send0::<f64>(depth_att, "clearDepth") - 1.0).abs() < 1e-9);
            send1::<f64, ()>(depth_att, "setClearDepth:", 0.25);
            assert!((send0::<f64>(depth_att, "clearDepth") - 0.25).abs() < 1e-9);

            send1::<usize, ()>(pdesc, "setRasterSampleCount:", 4);
            assert_eq!(send0::<usize>(pdesc, "rasterSampleCount"), 4);
            send1::<usize, ()>(pdesc, "setAlphaToCoverageEnabled:", 1);
            assert_eq!(send0::<usize>(pdesc, "isAlphaToCoverageEnabled"), 1);

            let ds_cls = objc_rt::class_named("MTLDepthStencilDescriptor").unwrap() as Id;
            let ds: Id = send0(send0(ds_cls, "alloc"), "init");
            let ff: Id = send0(ds, "frontFaceStencil");
            let bf: Id = send0(ds, "backFaceStencil");
            assert!(!ff.is_null() && !bf.is_null());
            assert_ne!(ff, ds);
            assert_ne!(ff, bf);
            send1::<usize, ()>(ff, "setReadMask:", 0xff);
            assert_eq!(send0::<usize>(ff, "readMask"), 0xff);

            let (libs, fns, draws, computes, presents) = render_stats();
            assert!(libs >= 1 && fns >= 1 && draws >= 1);
            let _ = (computes, presents);

            // ARC retain/release lifecycle on setObject:atIndexedSubscript: and setStencilAttachment:
            let cdesc_cls = objc_rt::class_named("MTLRenderPassColorAttachmentDescriptor").unwrap() as Id;
            let ca: Id = send0(send0(cdesc_cls, "alloc"), "init");
            let tdesc_cls = objc_rt::class_named("MTLTextureDescriptor").unwrap() as Id;
            let tdesc: Id = send0(send0(tdesc_cls, "alloc"), "init");
            let tex: Id = send1(dev, "newTextureWithDescriptor:", tdesc);
            send1::<Id, ()>(ca, "setTexture:", tex);
            let color_atts: Id = send0(pass, "colorAttachments");
            send2::<Id, usize, ()>(color_atts, "setObject:atIndexedSubscript:", ca, 0);
            pear_libobjc::object::release(ca);
            let ca_back: Id = send1(color_atts, "objectAtIndexedSubscript:", 0);
            assert_eq!(ca_back, ca);
            assert_eq!(send0::<Id>(ca_back, "texture"), tex);
            send2::<Id, usize, ()>(color_atts, "setObject:atIndexedSubscript:", std::ptr::null_mut(), 0);
            let ca_fresh: Id = send1(color_atts, "objectAtIndexedSubscript:", 0);
            assert!(!ca_fresh.is_null());

            let sdesc_cls = objc_rt::class_named("MTLRenderPassDepthAttachmentDescriptor").unwrap() as Id;
            let sa: Id = send0(send0(sdesc_cls, "alloc"), "init");
            send1::<Id, ()>(sa, "setTexture:", tex);
            send1::<Id, ()>(pass, "setStencilAttachment:", sa);
            pear_libobjc::object::release(sa);
            let sa_back: Id = send0(pass, "stencilAttachment");
            assert_eq!(sa_back, sa);
            assert_eq!(send0::<Id>(sa_back, "texture"), tex);

            // Phase 5.7: MTLLoadActionClear + presentDrawable: readback + PEAR_DUMP_FRAME PPM
            let layer_dummy = 0x1234usize as Id;
            let drawable = next_drawable_for_layer(layer_dummy, 4, 4, 80); // BGRA8Unorm
            let dtex: Id = send0(drawable, "texture");
            let ca0: Id = send1(color_atts, "objectAtIndexedSubscript:", 0);
            send1::<Id, ()>(ca0, "setTexture:", dtex);
            send1::<usize, ()>(ca0, "setLoadAction:", 2); // MTLLoadActionClear
            let set_cc_sel = objc_rt::sel("setClearColor:");
            let d_cc = lookup_for_send(ca0, set_cc_sel);
            let f_cc: unsafe extern "C" fn(Id, Sel, MTLClearColor) = std::mem::transmute(d_cc.imp);
            f_cc(d_cc.receiver, set_cc_sel, MTLClearColor { red: 1.0, green: 0.5, blue: 0.0, alpha: 1.0 });

            let cb2: Id = send0(cq, "commandBuffer");
            let renc2: Id = send1(cb2, "renderCommandEncoderWithDescriptor:", pass);
            send0::<()>(renc2, "endEncoding");
            let ppm_path = std::env::temp_dir().join(format!("pear_test_frame_{}.ppm", std::process::id()));
            std::env::set_var("PEAR_DUMP_FRAME", &ppm_path);
            send1::<Id, ()>(cb2, "presentDrawable:", drawable);
            send0::<()>(cb2, "commit");
            std::env::remove_var("PEAR_DUMP_FRAME");

            let (fw, fh, rgba) = last_presented_frame().expect("presented frame snapshot missing");
            assert_eq!((fw, fh), (4, 4));
            assert_eq!(&rgba[0..4], &[255, 128, 0, 255]);

            let ppm_bytes = std::fs::read(&ppm_path).expect("PPM file not written");
            let _ = std::fs::remove_file(&ppm_path);
            assert!(ppm_bytes.starts_with(b"P6\n4 4\n255\n"));
            assert_eq!(ppm_bytes.len(), "P6\n4 4\n255\n".len() + 4 * 4 * 3);
        }
    }

    #[test]
    fn phase5_8_rasterizer_and_blit_texture_copy() {
        let _present_guard = PRESENT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        init();
        unsafe {
            let dev = create_system_default_device();
            let cq: Id = send0(dev, "newCommandQueue");

            // 1. 16x16 BGRA8Unorm drawable target cleared to black
            let layer_dummy = 0x5858usize as Id;
            let drawable = next_drawable_for_layer(layer_dummy, 16, 16, 80);
            let dtex: Id = send0(drawable, "texture");

            // 2. Upload 2x2 solid green (RGBA8Unorm = 70) texture via texture2DDescriptorWithPixelFormat:width:height:mipmapped:
            let tdesc_cls = objc_rt::class_named("MTLTextureDescriptor").unwrap() as Id;
            let s_td = objc_rt::sel("texture2DDescriptorWithPixelFormat:width:height:mipmapped:");
            let d_td = lookup_for_send(tdesc_cls, s_td);
            let f_td: unsafe extern "C" fn(Id, Sel, usize, usize, usize, bool) -> Id =
                std::mem::transmute(d_td.imp);
            let tdesc = f_td(d_td.receiver, s_td, 70, 2, 2, false);
            let tex_src: Id = send1(dev, "newTextureWithDescriptor:", tdesc);
            let green_px: [u8; 16] = [
                0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255,
            ];
            let s_rr = objc_rt::sel("replaceRegion:mipmapLevel:withBytes:bytesPerRow:");
            let d_rr = lookup_for_send(tex_src, s_rr);
            let f_rr: unsafe extern "C" fn(Id, Sel, MTLRegion, usize, *const u8, usize) =
                std::mem::transmute(d_rr.imp);
            f_rr(
                d_rr.receiver,
                s_rr,
                MTLRegion {
                    origin: MTLOrigin { x: 0, y: 0, z: 0 },
                    size: MTLSize { width: 2, height: 2, depth: 1 },
                },
                0,
                green_px.as_ptr(),
                8,
            );

            // 3. Configure MTLVertexDescriptor (Float3 pos @ 0, Float2 uv @ 12, stride 20)
            let vdesc_cls = objc_rt::class_named("MTLVertexDescriptor").unwrap() as Id;
            let vdesc: Id = send0(vdesc_cls, "vertexDescriptor");
            let attrs: Id = send0(vdesc, "attributes");
            let a0: Id = send1(attrs, "objectAtIndexedSubscript:", 0usize);
            send1::<usize, ()>(a0, "setFormat:", 30); // Float3
            send1::<usize, ()>(a0, "setOffset:", 0);
            send1::<usize, ()>(a0, "setBufferIndex:", 0);
            let a1: Id = send1(attrs, "objectAtIndexedSubscript:", 1usize);
            send1::<usize, ()>(a1, "setFormat:", 29); // Float2
            send1::<usize, ()>(a1, "setOffset:", 12);
            send1::<usize, ()>(a1, "setBufferIndex:", 0);
            let layouts: Id = send0(vdesc, "layouts");
            let l0: Id = send1(layouts, "objectAtIndexedSubscript:", 0usize);
            send1::<usize, ()>(l0, "setStride:", 20);

            let pdesc_cls = objc_rt::class_named("MTLRenderPipelineDescriptor").unwrap() as Id;
            let pdesc: Id = send0(send0(pdesc_cls, "alloc"), "init");
            send1::<Id, ()>(pdesc, "setVertexDescriptor:", vdesc);
            let mut err: Id = std::ptr::null_mut();
            let pso: Id = send2(dev, "newRenderPipelineStateWithDescriptor:error:", pdesc, &mut err);
            assert!(!pso.is_null());

            // 4. Fullscreen quad (4 vertices * 5 f32s) + u16 indices
            #[rustfmt::skip]
            let verts: [f32; 20] = [
                -1.0, -1.0, 0.0,  0.0, 0.0,
                 1.0, -1.0, 0.0,  1.0, 0.0,
                 1.0,  1.0, 0.0,  1.0, 1.0,
                -1.0,  1.0, 0.0,  0.0, 1.0,
            ];
            let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];
            let ibuf: Id = send3(
                dev,
                "newBufferWithBytes:length:options:",
                indices.as_ptr() as *const c_void,
                indices.len() * 2,
                0usize,
            );

            let pass_cls = objc_rt::class_named("MTLRenderPassDescriptor").unwrap() as Id;
            let pass: Id = send0(pass_cls, "renderPassDescriptor");
            let catts: Id = send0(pass, "colorAttachments");
            let ca0: Id = send1(catts, "objectAtIndexedSubscript:", 0usize);
            send1::<Id, ()>(ca0, "setTexture:", dtex);
            send1::<usize, ()>(ca0, "setLoadAction:", 2); // MTLLoadActionClear
            let s_cc = objc_rt::sel("setClearColor:");
            let d_cc = lookup_for_send(ca0, s_cc);
            let f_cc: unsafe extern "C" fn(Id, Sel, MTLClearColor) = std::mem::transmute(d_cc.imp);
            f_cc(d_cc.receiver, s_cc, MTLClearColor { red: 0.0, green: 0.0, blue: 0.0, alpha: 1.0 });

            let cb: Id = send0(cq, "commandBuffer");
            let renc: Id = send1(cb, "renderCommandEncoderWithDescriptor:", pass);
            send1::<Id, ()>(renc, "setRenderPipelineState:", pso);
            send3::<*const u8, usize, usize, ()>(
                renc,
                "setVertexBytes:length:atIndex:",
                verts.as_ptr() as *const u8,
                std::mem::size_of_val(&verts),
                0usize,
            );
            send2::<Id, usize, ()>(renc, "setFragmentTexture:atIndex:", tex_src, 0usize);
            send5::<usize, usize, usize, Id, usize, ()>(
                renc,
                "drawIndexedPrimitives:indexCount:indexType:indexBuffer:indexBufferOffset:",
                3usize,
                6usize,
                0usize,
                ibuf,
                0usize,
            );
            send0::<()>(renc, "endEncoding");
            send1::<Id, ()>(cb, "presentDrawable:", drawable);
            send0::<()>(cb, "commit");

            // 5. Verify center pixel (8, 8) is green [0, 255, 0, 255]
            let (fw, fh, rgba) = last_presented_frame().expect("frame missing");
            assert_eq!((fw, fh), (16, 16));
            let center = (8 * 16 + 8) * 4;
            assert_eq!(&rgba[center..center + 4], &[0, 255, 0, 255]);

            // 6. Blit copyFromTexture: -> getBytes:
            let tex_dst: Id = send1(dev, "newTextureWithDescriptor:", tdesc);
            let cb_b: Id = send0(cq, "commandBuffer");
            let blit: Id = send0(cb_b, "blitCommandEncoder");
            let s_cft = objc_rt::sel("copyFromTexture:sourceSlice:sourceLevel:sourceOrigin:sourceSize:toTexture:destinationSlice:destinationLevel:destinationOrigin:");
            let d_cft = lookup_for_send(blit, s_cft);
            let f_cft: unsafe extern "C" fn(Id, Sel, Id, usize, usize, MTLOrigin, MTLSize, Id, usize, usize, MTLOrigin) =
                std::mem::transmute(d_cft.imp);
            f_cft(
                d_cft.receiver,
                s_cft,
                tex_src,
                0,
                0,
                MTLOrigin { x: 0, y: 0, z: 0 },
                MTLSize { width: 2, height: 2, depth: 1 },
                tex_dst,
                0,
                0,
                MTLOrigin { x: 0, y: 0, z: 0 },
            );
            send0::<()>(blit, "endEncoding");
            send0::<()>(cb_b, "commit");

            let mut out_px = [0u8; 16];
            let s_gb = objc_rt::sel("getBytes:bytesPerRow:fromRegion:mipmapLevel:");
            let d_gb = lookup_for_send(tex_dst, s_gb);
            let f_gb: unsafe extern "C" fn(Id, Sel, *mut u8, usize, MTLRegion, usize) =
                std::mem::transmute(d_gb.imp);
            f_gb(
                d_gb.receiver,
                s_gb,
                out_px.as_mut_ptr(),
                8,
                MTLRegion {
                    origin: MTLOrigin { x: 0, y: 0, z: 0 },
                    size: MTLSize { width: 2, height: 2, depth: 1 },
                },
                0,
            );
            assert_eq!(out_px, green_px);
        }
    }

    #[test]
    fn phase5_9_astc_blit_mvp_and_is_bool_props() {
        let _present_guard = PRESENT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        init();
        unsafe {
            // 1. isBlendingEnabled / isDepthWriteEnabled / isAlphaToCoverageEnabled round-trip
            let rp_ca_cls =
                objc_rt::class_named("MTLRenderPipelineColorAttachmentDescriptor").unwrap() as Id;
            let rp_ca: Id = send0(send0(rp_ca_cls, "alloc"), "init");
            assert_eq!(send0::<usize>(rp_ca, "isBlendingEnabled"), 0);
            send1::<usize, ()>(rp_ca, "setBlendingEnabled:", 1);
            assert_eq!(send0::<usize>(rp_ca, "isBlendingEnabled"), 1);

            let ds_cls = objc_rt::class_named("MTLDepthStencilDescriptor").unwrap() as Id;
            let ds: Id = send0(send0(ds_cls, "alloc"), "init");
            assert_eq!(send0::<usize>(ds, "isDepthWriteEnabled"), 0);
            send1::<usize, ()>(ds, "setDepthWriteEnabled:", 1);
            assert_eq!(send0::<usize>(ds, "isDepthWriteEnabled"), 1);

            let pd_cls = objc_rt::class_named("MTLRenderPipelineDescriptor").unwrap() as Id;
            let pd: Id = send0(send0(pd_cls, "alloc"), "init");
            assert_eq!(send0::<usize>(pd, "isAlphaToCoverageEnabled"), 0);
            send1::<usize, ()>(pd, "setAlphaToCoverageEnabled:", 1);
            assert_eq!(send0::<usize>(pd, "isAlphaToCoverageEnabled"), 1);

            // 2. Upload 16-byte ASTC 4x4 void-extent red block via copyFromBuffer:...toTexture:
            let dev = create_system_default_device();
            let cq: Id = send0(dev, "newCommandQueue");
            let tdesc_cls = objc_rt::class_named("MTLTextureDescriptor").unwrap() as Id;
            let s_td = objc_rt::sel("texture2DDescriptorWithPixelFormat:width:height:mipmapped:");
            let d_td = lookup_for_send(tdesc_cls, s_td);
            let f_td: unsafe extern "C" fn(Id, Sel, usize, usize, usize, bool) -> Id =
                std::mem::transmute(d_td.imp);
            let tdesc_astc = f_td(d_td.receiver, s_td, 204, 4, 4, false); // ASTC_4x4_LDR
            let tex_astc: Id = send1(dev, "newTextureWithDescriptor:", tdesc_astc);

            let mut astc_blk = [0u8; 16];
            astc_blk[0] = 0xfc;
            astc_blk[1] = 0x01;
            astc_blk[8..10].copy_from_slice(&0xffffu16.to_le_bytes()); // R = 255
            astc_blk[14..16].copy_from_slice(&0xffffu16.to_le_bytes()); // A = 255
            let astc_buf: Id = send3(
                dev,
                "newBufferWithBytes:length:options:",
                astc_blk.as_ptr() as *const c_void,
                astc_blk.len(),
                0usize,
            );

            let cb_up: Id = send0(cq, "commandBuffer");
            let blit_up: Id = send0(cb_up, "blitCommandEncoder");
            let s_b2t = objc_rt::sel("copyFromBuffer:sourceOffset:sourceBytesPerRow:sourceBytesPerImage:sourceSize:toTexture:destinationSlice:destinationLevel:destinationOrigin:");
            let d_b2t = lookup_for_send(blit_up, s_b2t);
            let f_b2t: unsafe extern "C" fn(
                Id,
                Sel,
                Id,
                usize,
                usize,
                usize,
                MTLSize,
                Id,
                usize,
                usize,
                MTLOrigin,
            ) = std::mem::transmute(d_b2t.imp);
            f_b2t(
                d_b2t.receiver,
                s_b2t,
                astc_buf,
                0,
                16,
                16,
                MTLSize { width: 4, height: 4, depth: 1 },
                tex_astc,
                0,
                0,
                MTLOrigin { x: 0, y: 0, z: 0 },
            );
            send0::<()>(blit_up, "endEncoding");
            send0::<()>(cb_up, "commit");

            // 3. Readback ASTC decoded RGBA pixels via copyFromTexture:...toBuffer:
            let rb_buf: Id = send2(dev, "newBufferWithLength:options:", 4 * 4 * 4, 0usize);
            let cb_rb: Id = send0(cq, "commandBuffer");
            let blit_rb: Id = send0(cb_rb, "blitCommandEncoder");
            let s_t2b = objc_rt::sel("copyFromTexture:sourceSlice:sourceLevel:sourceOrigin:sourceSize:toBuffer:destinationOffset:destinationBytesPerRow:destinationBytesPerImage:");
            let d_t2b = lookup_for_send(blit_rb, s_t2b);
            let f_t2b: unsafe extern "C" fn(
                Id,
                Sel,
                Id,
                usize,
                usize,
                MTLOrigin,
                MTLSize,
                Id,
                usize,
                usize,
                usize,
            ) = std::mem::transmute(d_t2b.imp);
            f_t2b(
                d_t2b.receiver,
                s_t2b,
                tex_astc,
                0,
                0,
                MTLOrigin { x: 0, y: 0, z: 0 },
                MTLSize { width: 4, height: 4, depth: 1 },
                rb_buf,
                0,
                16,
                64,
            );
            send0::<()>(blit_rb, "endEncoding");
            send0::<()>(cb_rb, "commit");
            let rb_ptr = send0::<*const u8>(rb_buf, "contents");
            assert_eq!(std::slice::from_raw_parts(rb_ptr, 4), &[255, 0, 0, 255]);

            // 4. Column-major 4x4 MVP in buffer 0 mapping [0..1, 0..1] -> [-1..1, -1..1]
            let mvp: [f32; 16] = [
                2.0, 0.0, 0.0, 0.0, // col 0
                0.0, 2.0, 0.0, 0.0, // col 1
                0.0, 0.0, 1.0, 0.0, // col 2
                -1.0, -1.0, 0.0, 1.0, // col 3
            ];
            #[rustfmt::skip]
            let obj_verts: [f32; 20] = [
                0.0, 0.0, 0.0,  0.0, 0.0,
                1.0, 0.0, 0.0,  1.0, 0.0,
                1.0, 1.0, 0.0,  1.0, 1.0,
                0.0, 1.0, 0.0,  0.0, 1.0,
            ];
            let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];
            let ibuf: Id = send3(
                dev,
                "newBufferWithBytes:length:options:",
                indices.as_ptr() as *const c_void,
                indices.len() * 2,
                0usize,
            );

            let vdesc_cls = objc_rt::class_named("MTLVertexDescriptor").unwrap() as Id;
            let vdesc: Id = send0(vdesc_cls, "vertexDescriptor");
            let attrs: Id = send0(vdesc, "attributes");
            let a0: Id = send1(attrs, "objectAtIndexedSubscript:", 0usize);
            send1::<usize, ()>(a0, "setFormat:", 30); // Float3
            send1::<usize, ()>(a0, "setOffset:", 0);
            send1::<usize, ()>(a0, "setBufferIndex:", 1);
            let a1: Id = send1(attrs, "objectAtIndexedSubscript:", 1usize);
            send1::<usize, ()>(a1, "setFormat:", 29); // Float2
            send1::<usize, ()>(a1, "setOffset:", 12);
            send1::<usize, ()>(a1, "setBufferIndex:", 1);
            let layouts: Id = send0(vdesc, "layouts");
            let l1: Id = send1(layouts, "objectAtIndexedSubscript:", 1usize);
            send1::<usize, ()>(l1, "setStride:", 20);

            let pdesc: Id = send0(send0(pd_cls, "alloc"), "init");
            send1::<Id, ()>(pdesc, "setVertexDescriptor:", vdesc);
            let mut err: Id = std::ptr::null_mut();
            let pso: Id = send2(dev, "newRenderPipelineStateWithDescriptor:error:", pdesc, &mut err);

            let layer_dummy = 0x5959usize as Id;
            let drawable = next_drawable_for_layer(layer_dummy, 16, 16, 80);
            let dtex: Id = send0(drawable, "texture");
            let pass_cls = objc_rt::class_named("MTLRenderPassDescriptor").unwrap() as Id;
            let pass: Id = send0(pass_cls, "renderPassDescriptor");
            let catts: Id = send0(pass, "colorAttachments");
            let ca0: Id = send1(catts, "objectAtIndexedSubscript:", 0usize);
            send1::<Id, ()>(ca0, "setTexture:", dtex);
            send1::<usize, ()>(ca0, "setLoadAction:", 2);

            let cb: Id = send0(cq, "commandBuffer");
            let renc: Id = send1(cb, "renderCommandEncoderWithDescriptor:", pass);
            send1::<Id, ()>(renc, "setRenderPipelineState:", pso);
            send3::<*const u8, usize, usize, ()>(
                renc,
                "setVertexBytes:length:atIndex:",
                mvp.as_ptr() as *const u8,
                std::mem::size_of_val(&mvp),
                0usize,
            );
            send3::<*const u8, usize, usize, ()>(
                renc,
                "setVertexBytes:length:atIndex:",
                obj_verts.as_ptr() as *const u8,
                std::mem::size_of_val(&obj_verts),
                1usize,
            );
            send2::<Id, usize, ()>(renc, "setFragmentTexture:atIndex:", tex_astc, 0usize);
            send5::<usize, usize, usize, Id, usize, ()>(
                renc,
                "drawIndexedPrimitives:indexCount:indexType:indexBuffer:indexBufferOffset:",
                3usize,
                6usize,
                0usize,
                ibuf,
                0usize,
            );
            send0::<()>(renc, "endEncoding");
            send1::<Id, ()>(cb, "presentDrawable:", drawable);
            send0::<()>(cb, "commit");

            let (fw, fh, rgba) = last_presented_frame().expect("frame missing");
            assert_eq!((fw, fh), (16, 16));
            assert_eq!(&rgba[0..4], &[255, 0, 0, 255]);
            let br = (15 * 16 + 15) * 4;
            assert_eq!(&rgba[br..br + 4], &[255, 0, 0, 255]);
        }
    }

    #[test]
    fn phase5_10_offset_mvp_and_pixel_space_quad() {
        let _present_guard = PRESENT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        init();
        unsafe {
            let dev = create_system_default_device();
            assert!(!dev.is_null());
            let cq: Id = send0(dev, "newCommandQueue");

            // Case 1: Offset-64 MVP matrix in buffer 0 mapping (0..10, 0..10) -> (-1..1, -1..1).
            let mut ubuf_bytes = vec![0u8; 64 + 64];
            #[rustfmt::skip]
            let mvp: [f32; 16] = [
                0.2, 0.0, 0.0, 0.0,
                0.0, 0.2, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
                -1.0, -1.0, 0.0, 1.0,
            ];
            for (i, &val) in mvp.iter().enumerate() {
                let off = 64 + i * 4;
                ubuf_bytes[off..off + 4].copy_from_slice(&val.to_le_bytes());
            }

            #[rustfmt::skip]
            let obj_verts: [f32; 28] = [
                0.0,  0.0,  0.0,  1.0, 0.0, 1.0, 1.0,
                10.0, 0.0,  0.0,  1.0, 0.0, 1.0, 1.0,
                10.0, 10.0, 0.0,  1.0, 0.0, 1.0, 1.0,
                0.0,  10.0, 0.0,  1.0, 0.0, 1.0, 1.0,
            ];
            let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];
            let ibuf: Id = send3(
                dev,
                "newBufferWithBytes:length:options:",
                indices.as_ptr() as *const c_void,
                indices.len() * 2,
                0usize,
            );

            let vdesc_cls = objc_rt::class_named("MTLVertexDescriptor").unwrap() as Id;
            let vdesc: Id = send0(vdesc_cls, "vertexDescriptor");
            let attrs: Id = send0(vdesc, "attributes");
            let a0: Id = send1(attrs, "objectAtIndexedSubscript:", 0usize);
            send1::<usize, ()>(a0, "setFormat:", 30); // Float3
            send1::<usize, ()>(a0, "setOffset:", 0);
            send1::<usize, ()>(a0, "setBufferIndex:", 1);
            let a1: Id = send1(attrs, "objectAtIndexedSubscript:", 1usize);
            send1::<usize, ()>(a1, "setFormat:", 31); // Float4 color
            send1::<usize, ()>(a1, "setOffset:", 12);
            send1::<usize, ()>(a1, "setBufferIndex:", 1);
            let layouts: Id = send0(vdesc, "layouts");
            let l1: Id = send1(layouts, "objectAtIndexedSubscript:", 1usize);
            send1::<usize, ()>(l1, "setStride:", 28);

            let pd_cls = objc_rt::class_named("MTLRenderPipelineDescriptor").unwrap() as Id;
            let pdesc: Id = send0(send0(pd_cls, "alloc"), "init");
            send1::<Id, ()>(pdesc, "setVertexDescriptor:", vdesc);
            let mut err: Id = std::ptr::null_mut();
            let pso: Id = send2(dev, "newRenderPipelineStateWithDescriptor:error:", pdesc, &mut err);

            let layer_dummy = 0x5A5Ausize as Id;
            let drawable = next_drawable_for_layer(layer_dummy, 16, 16, 80);
            let dtex: Id = send0(drawable, "texture");
            let pass_cls = objc_rt::class_named("MTLRenderPassDescriptor").unwrap() as Id;
            let pass: Id = send0(pass_cls, "renderPassDescriptor");
            let catts: Id = send0(pass, "colorAttachments");
            let ca0: Id = send1(catts, "objectAtIndexedSubscript:", 0usize);
            send1::<Id, ()>(ca0, "setTexture:", dtex);
            send1::<usize, ()>(ca0, "setLoadAction:", 2);

            let cb: Id = send0(cq, "commandBuffer");
            let renc: Id = send1(cb, "renderCommandEncoderWithDescriptor:", pass);
            send1::<Id, ()>(renc, "setRenderPipelineState:", pso);
            send3::<*const u8, usize, usize, ()>(
                renc,
                "setVertexBytes:length:atIndex:",
                ubuf_bytes.as_ptr(),
                ubuf_bytes.len(),
                0usize,
            );
            send3::<*const u8, usize, usize, ()>(
                renc,
                "setVertexBytes:length:atIndex:",
                obj_verts.as_ptr() as *const u8,
                std::mem::size_of_val(&obj_verts),
                1usize,
            );
            send5::<usize, usize, usize, Id, usize, ()>(
                renc,
                "drawIndexedPrimitives:indexCount:indexType:indexBuffer:indexBufferOffset:",
                3usize,
                6usize,
                0usize,
                ibuf,
                0usize,
            );
            send0::<()>(renc, "endEncoding");
            send1::<Id, ()>(cb, "presentDrawable:", drawable);
            send0::<()>(cb, "commit");

            let mut rgba = [0u8; 16 * 16 * 4];
            let s_gb = objc_rt::sel("getBytes:bytesPerRow:fromRegion:mipmapLevel:");
            let d_gb = lookup_for_send(dtex, s_gb);
            let f_gb: unsafe extern "C" fn(Id, Sel, *mut u8, usize, MTLRegion, usize) =
                std::mem::transmute(d_gb.imp);
            f_gb(
                d_gb.receiver,
                s_gb,
                rgba.as_mut_ptr(),
                16 * 4,
                MTLRegion {
                    origin: MTLOrigin { x: 0, y: 0, z: 0 },
                    size: MTLSize { width: 16, height: 16, depth: 1 },
                },
                0,
            );
            assert_eq!(&rgba[0..4], &[255, 0, 255, 255]);
            let br = (15 * 16 + 15) * 4;
            assert_eq!(&rgba[br..br + 4], &[255, 0, 255, 255]);

            // Case 2: Pixel-space quad (4..12, 4..12) on 16x16 target with no buffer 0 MVP.
            #[rustfmt::skip]
            let px_verts: [f32; 28] = [
                4.0,  4.0,  0.0,  0.0, 1.0, 0.0, 1.0,
                12.0, 4.0,  0.0,  0.0, 1.0, 0.0, 1.0,
                12.0, 12.0, 0.0,  0.0, 1.0, 0.0, 1.0,
                4.0,  12.0, 0.0,  0.0, 1.0, 0.0, 1.0,
            ];
            let drawable2 = next_drawable_for_layer(layer_dummy, 16, 16, 80);
            let dtex2: Id = send0(drawable2, "texture");
            send1::<Id, ()>(ca0, "setTexture:", dtex2);
            send1::<usize, ()>(ca0, "setLoadAction:", 2);

            let cb2: Id = send0(cq, "commandBuffer");
            let renc2: Id = send1(cb2, "renderCommandEncoderWithDescriptor:", pass);
            send1::<Id, ()>(renc2, "setRenderPipelineState:", pso);
            send3::<*const u8, usize, usize, ()>(
                renc2,
                "setVertexBytes:length:atIndex:",
                px_verts.as_ptr() as *const u8,
                std::mem::size_of_val(&px_verts),
                1usize,
            );
            send5::<usize, usize, usize, Id, usize, ()>(
                renc2,
                "drawIndexedPrimitives:indexCount:indexType:indexBuffer:indexBufferOffset:",
                3usize,
                6usize,
                0usize,
                ibuf,
                0usize,
            );
            send0::<()>(renc2, "endEncoding");
            send1::<Id, ()>(cb2, "presentDrawable:", drawable2);
            send0::<()>(cb2, "commit");

            let mut rgba2 = [0u8; 16 * 16 * 4];
            let d_gb2 = lookup_for_send(dtex2, s_gb);
            let f_gb2: unsafe extern "C" fn(Id, Sel, *mut u8, usize, MTLRegion, usize) =
                std::mem::transmute(d_gb2.imp);
            f_gb2(
                d_gb2.receiver,
                s_gb,
                rgba2.as_mut_ptr(),
                16 * 4,
                MTLRegion {
                    origin: MTLOrigin { x: 0, y: 0, z: 0 },
                    size: MTLSize { width: 16, height: 16, depth: 1 },
                },
                0,
            );
            assert_eq!(&rgba2[0..4], &[0, 0, 0, 0]);
            let center = (8 * 16 + 8) * 4;
            assert_eq!(&rgba2[center..center + 4], &[0, 255, 0, 255]);
        }
    }

    #[test]
    fn phase5_13_async_library_completion_and_tex_desc_copy() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        init();
        static GOT_LIB: AtomicUsize = AtomicUsize::new(0);
        static GOT_ERR: AtomicUsize = AtomicUsize::new(usize::MAX);

        #[repr(C)]
        struct TestBlock {
            isa: *const c_void,
            flags: i32,
            reserved: i32,
            invoke: unsafe extern "C" fn(*const c_void, Id, Id),
            descriptor: *const c_void,
        }
        unsafe extern "C" fn on_done(_blk: *const c_void, lib: Id, err: Id) {
            GOT_LIB.store(lib as usize, Ordering::SeqCst);
            GOT_ERR.store(err as usize, Ordering::SeqCst);
        }

        unsafe {
            let dev = create_system_default_device();
            assert!(!dev.is_null());

            let blk = TestBlock {
                isa: std::ptr::null(),
                flags: 0,
                reserved: 0,
                invoke: on_done,
                descriptor: std::ptr::null(),
            };
            let src = pear_foundation::nsstring_from_rust("fragment half4 fragMain() { return half4(1); }");
            send3::<Id, Id, Id, ()>(
                dev,
                "newLibraryWithSource:options:completionHandler:",
                src,
                std::ptr::null_mut(),
                (&raw const blk) as Id,
            );
            assert_ne!(GOT_LIB.load(Ordering::SeqCst), 0);
            assert_eq!(GOT_ERR.load(Ordering::SeqCst), 0);

            let td_cls = objc_rt::class_named("MTLTextureDescriptor").unwrap() as Id;
            let s_td = objc_rt::sel("texture2DDescriptorWithPixelFormat:width:height:mipmapped:");
            let d_td = lookup_for_send(td_cls, s_td);
            let f_td: unsafe extern "C" fn(Id, Sel, usize, usize, usize, bool) -> Id =
                std::mem::transmute(d_td.imp);
            let td = f_td(d_td.receiver, s_td, 70, 64, 32, false);
            let copy_td: Id = send1(td, "copyWithZone:", std::ptr::null_mut::<c_void>());
            assert!(!copy_td.is_null());
            assert_ne!(copy_td, td);
            assert_eq!(send0::<usize>(copy_td, "pixelFormat"), 70);
            assert_eq!(send0::<usize>(copy_td, "width"), 64);
            assert_eq!(send0::<usize>(copy_td, "height"), 32);
            send1::<usize, ()>(copy_td, "setWidth:", 128);
            assert_eq!(send0::<usize>(copy_td, "width"), 128);
            assert_eq!(send0::<usize>(td, "width"), 64);
        }
    }

    #[test]
    fn phase5_14_zero_copy_cow_and_incremental_rasterizer() {
        let _present_guard = PRESENT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        init();
        unsafe {
            let dev = create_system_default_device();
            let cq: Id = send0(dev, "newCommandQueue");

            // Large 64 KB vertex MTLBuffer (drawn at offset 4096) to exercise borrowed slice path.
            let mut big_vb = vec![0u8; 65536];
            #[rustfmt::skip]
            let quad_verts: [f32; 20] = [
                -1.0, -1.0, 0.0,  0.0, 0.0,
                 1.0, -1.0, 0.0,  1.0, 0.0,
                 1.0,  1.0, 0.0,  1.0, 1.0,
                -1.0,  1.0, 0.0,  0.0, 1.0,
            ];
            for (i, &f) in quad_verts.iter().enumerate() {
                let off = 4096 + i * 4;
                big_vb[off..off + 4].copy_from_slice(&f.to_le_bytes());
            }
            let vbuf: Id = send3(
                dev,
                "newBufferWithBytes:length:options:",
                big_vb.as_ptr() as *const c_void,
                big_vb.len(),
                0usize,
            );
            let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];
            let ibuf: Id = send3(
                dev,
                "newBufferWithBytes:length:options:",
                indices.as_ptr() as *const c_void,
                indices.len() * 2,
                0usize,
            );

            // 2x2 uniform texture [200, 100, 50, 128] blended over clear [0, 0, 0, 255]
            let td_cls = objc_rt::class_named("MTLTextureDescriptor").unwrap() as Id;
            let s_td = objc_rt::sel("texture2DDescriptorWithPixelFormat:width:height:mipmapped:");
            let d_td = lookup_for_send(td_cls, s_td);
            let f_td: unsafe extern "C" fn(Id, Sel, usize, usize, usize, bool) -> Id =
                std::mem::transmute(d_td.imp);
            let tdesc = f_td(d_td.receiver, s_td, 70, 2, 2, false);
            let ftex: Id = send1(dev, "newTextureWithDescriptor:", tdesc);
            let tex_px: [u8; 16] = [
                200, 100, 50, 128, 200, 100, 50, 128, 200, 100, 50, 128, 200, 100, 50, 128,
            ];
            let s_rr = objc_rt::sel("replaceRegion:mipmapLevel:withBytes:bytesPerRow:");
            let d_rr = lookup_for_send(ftex, s_rr);
            let f_rr: unsafe extern "C" fn(Id, Sel, MTLRegion, usize, *const u8, usize) =
                std::mem::transmute(d_rr.imp);
            let rgn2 = MTLRegion {
                origin: MTLOrigin { x: 0, y: 0, z: 0 },
                size: MTLSize { width: 2, height: 2, depth: 1 },
            };
            f_rr(d_rr.receiver, s_rr, rgn2, 0, tex_px.as_ptr(), 8);

            let vdesc_cls = objc_rt::class_named("MTLVertexDescriptor").unwrap() as Id;
            let vdesc: Id = send0(vdesc_cls, "vertexDescriptor");
            let attrs: Id = send0(vdesc, "attributes");
            let a0: Id = send1(attrs, "objectAtIndexedSubscript:", 0usize);
            send1::<usize, ()>(a0, "setFormat:", 30);
            send1::<usize, ()>(a0, "setOffset:", 0);
            send1::<usize, ()>(a0, "setBufferIndex:", 1);
            let a1: Id = send1(attrs, "objectAtIndexedSubscript:", 1usize);
            send1::<usize, ()>(a1, "setFormat:", 29);
            send1::<usize, ()>(a1, "setOffset:", 12);
            send1::<usize, ()>(a1, "setBufferIndex:", 1);
            let layouts: Id = send0(vdesc, "layouts");
            let l1: Id = send1(layouts, "objectAtIndexedSubscript:", 1usize);
            send1::<usize, ()>(l1, "setStride:", 20);

            let pd_cls = objc_rt::class_named("MTLRenderPipelineDescriptor").unwrap() as Id;
            let pdesc: Id = send0(send0(pd_cls, "alloc"), "init");
            send1::<Id, ()>(pdesc, "setVertexDescriptor:", vdesc);
            let p_catts: Id = send0(pdesc, "colorAttachments");
            let p_ca0: Id = send1(p_catts, "objectAtIndexedSubscript:", 0usize);
            send1::<usize, ()>(p_ca0, "setBlendingEnabled:", 1);
            let mut err: Id = std::ptr::null_mut();
            let pso: Id = send2(dev, "newRenderPipelineStateWithDescriptor:error:", pdesc, &mut err);

            let layer_dummy = 0x5E14usize as Id;
            let drawable = next_drawable_for_layer(layer_dummy, 8, 8, 70);
            let dtex: Id = send0(drawable, "texture");
            let pass_cls = objc_rt::class_named("MTLRenderPassDescriptor").unwrap() as Id;
            let pass: Id = send0(pass_cls, "renderPassDescriptor");
            let catts: Id = send0(pass, "colorAttachments");
            let ca0: Id = send1(catts, "objectAtIndexedSubscript:", 0usize);
            send1::<Id, ()>(ca0, "setTexture:", dtex);
            send1::<usize, ()>(ca0, "setLoadAction:", 2);
            let s_cc = objc_rt::sel("setClearColor:");
            let d_cc = lookup_for_send(ca0, s_cc);
            let f_cc: unsafe extern "C" fn(Id, Sel, MTLClearColor) = std::mem::transmute(d_cc.imp);
            f_cc(
                d_cc.receiver,
                s_cc,
                MTLClearColor { red: 0.0, green: 0.0, blue: 0.0, alpha: 1.0 },
            );

            let cb: Id = send0(cq, "commandBuffer");
            let renc: Id = send1(cb, "renderCommandEncoderWithDescriptor:", pass);
            send1::<Id, ()>(renc, "setRenderPipelineState:", pso);
            send3::<Id, usize, usize, ()>(renc, "setVertexBuffer:offset:atIndex:", vbuf, 4096usize, 1usize);
            send2::<Id, usize, ()>(renc, "setFragmentTexture:atIndex:", ftex, 0usize);
            send5::<usize, usize, usize, Id, usize, ()>(
                renc,
                "drawIndexedPrimitives:indexCount:indexType:indexBuffer:indexBufferOffset:",
                3usize,
                6usize,
                0usize,
                ibuf,
                0usize,
            );
            send0::<()>(renc, "endEncoding");
            send1::<Id, ()>(cb, "presentDrawable:", drawable);
            send0::<()>(cb, "commit");

            // Check blended RGBA output: (200*128 + 0*127 + 127)/255 = 100, (100*128 + 127)/255 = 50, (50*128 + 127)/255 = 25
            let mut out = [0u8; 8 * 8 * 4];
            let s_gb = objc_rt::sel("getBytes:bytesPerRow:fromRegion:mipmapLevel:");
            let d_gb = lookup_for_send(dtex, s_gb);
            let f_gb: unsafe extern "C" fn(Id, Sel, *mut u8, usize, MTLRegion, usize) =
                std::mem::transmute(d_gb.imp);
            let rgn8 = MTLRegion {
                origin: MTLOrigin { x: 0, y: 0, z: 0 },
                size: MTLSize { width: 8, height: 8, depth: 1 },
            };
            f_gb(d_gb.receiver, s_gb, out.as_mut_ptr(), 32, rgn8, 0);
            assert_eq!(&out[0..4], &[100, 50, 25, 255]);
            let mid = (4 * 8 + 4) * 4;
            assert_eq!(&out[mid..mid + 4], &[100, 50, 25, 255]);

            // Verify Arc COW mutation on ftex works after draw
            let new_px = [10u8; 16];
            f_rr(d_rr.receiver, s_rr, rgn2, 0, new_px.as_ptr(), 8);
            let mut rb_ftex = [0u8; 16];
            let d_gb2 = lookup_for_send(ftex, s_gb);
            let f_gb2: unsafe extern "C" fn(Id, Sel, *mut u8, usize, MTLRegion, usize) =
                std::mem::transmute(d_gb2.imp);
            f_gb2(d_gb2.receiver, s_gb, rb_ftex.as_mut_ptr(), 8, rgn2, 0);
            assert_eq!(rb_ftex, new_px);
        }
    }

    #[test]
    fn phase5_15_texture_view_arc_ownership_and_sharing() {
        #[repr(C)]
        #[derive(Copy, Clone)]
        struct TestRange {
            location: usize,
            length: usize,
        }

        let _g = PRESENT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        objc_metal::init();
        unsafe {
            let dev = objc_metal::create_system_default_device();
            let td_cls = objc_rt::class_named("MTLTextureDescriptor").unwrap() as Id;
            let s_td = objc_rt::sel("texture2DDescriptorWithPixelFormat:width:height:mipmapped:");
            let d_td = lookup_for_send(td_cls, s_td);
            let f_td: unsafe extern "C" fn(Id, Sel, usize, usize, usize, bool) -> Id =
                std::mem::transmute(d_td.imp);
            let tdesc = f_td(d_td.receiver, s_td, 70, 4, 4, false);
            let parent: Id = send1(dev, "newTextureWithDescriptor:", tdesc);
            assert!(!parent.is_null());

            let mut src_px = [0u8; 4 * 4 * 4];
            for chunk in src_px.chunks_exact_mut(4) {
                chunk.copy_from_slice(&[10, 20, 30, 40]);
            }
            let rgn4 = MTLRegion {
                origin: MTLOrigin { x: 0, y: 0, z: 0 },
                size: MTLSize { width: 4, height: 4, depth: 1 },
            };
            let s_rr = objc_rt::sel("replaceRegion:mipmapLevel:withBytes:bytesPerRow:");
            let d_rr = lookup_for_send(parent, s_rr);
            let f_rr: unsafe extern "C" fn(Id, Sel, MTLRegion, usize, *const u8, usize) =
                std::mem::transmute(d_rr.imp);
            f_rr(d_rr.receiver, s_rr, rgn4, 0, src_px.as_ptr(), 16);

            let view: Id = send1(parent, "newTextureViewWithPixelFormat:", 71usize);
            assert!(!view.is_null());
            assert_ne!(view, parent);
            assert_eq!(send0::<usize>(view, "pixelFormat"), 71);
            assert_eq!(send0::<usize>(parent, "pixelFormat"), 70);
            assert_eq!(send0::<Id>(view, "parentTexture"), parent);

            let s_gb = objc_rt::sel("getBytes:bytesPerRow:fromRegion:mipmapLevel:");
            let mut rb1 = [0u8; 4 * 4 * 4];
            let d_gb1 = lookup_for_send(view, s_gb);
            let f_gb1: unsafe extern "C" fn(Id, Sel, *mut u8, usize, MTLRegion, usize) =
                std::mem::transmute(d_gb1.imp);
            f_gb1(d_gb1.receiver, s_gb, rb1.as_mut_ptr(), 16, rgn4, 0);
            assert_eq!(&rb1[0..4], &[10, 20, 30, 40]);
            assert_eq!(rb1, src_px);

            pear_libobjc::object::release(view);

            let view2: Id = send1(parent, "newTextureViewWithPixelFormat:", 71usize);
            assert!(!view2.is_null());
            assert_ne!(view2, parent);
            let mut rb2 = [0u8; 4 * 4 * 4];
            let d_gb2 = lookup_for_send(view2, s_gb);
            let f_gb2: unsafe extern "C" fn(Id, Sel, *mut u8, usize, MTLRegion, usize) =
                std::mem::transmute(d_gb2.imp);
            f_gb2(d_gb2.receiver, s_gb, rb2.as_mut_ptr(), 16, rgn4, 0);
            assert_eq!(rb2, src_px);
            pear_libobjc::object::release(view2);

            let s_nvf = objc_rt::sel("newTextureViewWithPixelFormat:textureType:levels:slices:");
            let d_nvf = lookup_for_send(parent, s_nvf);
            let f_nvf: unsafe extern "C" fn(Id, Sel, usize, usize, TestRange, TestRange) -> Id =
                std::mem::transmute(d_nvf.imp);
            let rng = TestRange { location: 0, length: 1 };
            let view3 = f_nvf(d_nvf.receiver, s_nvf, 72, 3, rng, rng);
            assert!(!view3.is_null());
            assert_ne!(view3, parent);
            assert_eq!(send0::<usize>(view3, "pixelFormat"), 72);
            assert_eq!(send0::<usize>(view3, "textureType"), 3);
            assert_eq!(send0::<Id>(view3, "parentTexture"), parent);

            pear_libobjc::object::release(view3);
            pear_libobjc::object::release(parent);
            pear_libobjc::object::release(dev);
        }
    }

    #[test]
    fn phase5_16_texture_view_live_aliasing_and_scanline_rasterizer() {
        let _g = PRESENT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        objc_metal::init();
        unsafe {
            let dev = objc_metal::create_system_default_device();
            let cq: Id = send0(dev, "newCommandQueue");
            let td_cls = objc_rt::class_named("MTLTextureDescriptor").unwrap() as Id;
            let s_td = objc_rt::sel("texture2DDescriptorWithPixelFormat:width:height:mipmapped:");
            let d_td = lookup_for_send(td_cls, s_td);
            let f_td: unsafe extern "C" fn(Id, Sel, usize, usize, usize, bool) -> Id =
                std::mem::transmute(d_td.imp);

            // 1. Live BGRA8Unorm <-> sRGB view aliasing
            let parent_desc = f_td(d_td.receiver, s_td, 80, 8, 8, false);
            let parent: Id = send1(dev, "newTextureWithDescriptor:", parent_desc);
            let view: Id = send1(parent, "newTextureViewWithPixelFormat:", 81usize);
            assert_ne!(view, parent);

            // 4x4 non-uniform RGBA fragment texture -> exercises fixed-point scanline loop
            let fdesc = f_td(d_td.receiver, s_td, 70, 4, 4, false);
            let ftex: Id = send1(dev, "newTextureWithDescriptor:", fdesc);
            let mut fpx = [0u8; 4 * 4 * 4];
            for y in 0..4usize {
                for x in 0..4usize {
                    let off = (y * 4 + x) * 4;
                    if x < 2 {
                        fpx[off..off + 4].copy_from_slice(&[200, 100, 50, 128]);
                    } else {
                        fpx[off..off + 4].copy_from_slice(&[40, 220, 120, 255]);
                    }
                }
            }
            let s_rr = objc_rt::sel("replaceRegion:mipmapLevel:withBytes:bytesPerRow:");
            let d_rr = lookup_for_send(ftex, s_rr);
            let f_rr: unsafe extern "C" fn(Id, Sel, MTLRegion, usize, *const u8, usize) =
                std::mem::transmute(d_rr.imp);
            let rgn4 = MTLRegion {
                origin: MTLOrigin { x: 0, y: 0, z: 0 },
                size: MTLSize { width: 4, height: 4, depth: 1 },
            };
            f_rr(d_rr.receiver, s_rr, rgn4, 0, fpx.as_ptr(), 16);

            let verts: [f32; 20] = [
                -1.0, -1.0, 0.0, 0.0, 1.0,
                 1.0, -1.0, 0.0, 1.0, 1.0,
                 1.0,  1.0, 0.0, 1.0, 0.0,
                -1.0,  1.0, 0.0, 0.0, 0.0,
            ];
            let mut vbytes = vec![0u8; verts.len() * 4];
            for (i, &v) in verts.iter().enumerate() {
                vbytes[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
            }
            let vbuf: Id = send3(
                dev,
                "newBufferWithBytes:length:options:",
                vbytes.as_ptr() as *const c_void,
                vbytes.len(),
                0usize,
            );
            let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];
            let ibuf: Id = send3(
                dev,
                "newBufferWithBytes:length:options:",
                indices.as_ptr() as *const c_void,
                indices.len() * 2,
                0usize,
            );

            let vdesc_cls = objc_rt::class_named("MTLVertexDescriptor").unwrap() as Id;
            let vdesc: Id = send0(vdesc_cls, "vertexDescriptor");
            let attrs: Id = send0(vdesc, "attributes");
            let a0: Id = send1(attrs, "objectAtIndexedSubscript:", 0usize);
            send1::<usize, ()>(a0, "setFormat:", 30);
            send1::<usize, ()>(a0, "setOffset:", 0);
            send1::<usize, ()>(a0, "setBufferIndex:", 1);
            let a1: Id = send1(attrs, "objectAtIndexedSubscript:", 1usize);
            send1::<usize, ()>(a1, "setFormat:", 29);
            send1::<usize, ()>(a1, "setOffset:", 12);
            send1::<usize, ()>(a1, "setBufferIndex:", 1);
            let layouts: Id = send0(vdesc, "layouts");
            let l1: Id = send1(layouts, "objectAtIndexedSubscript:", 1usize);
            send1::<usize, ()>(l1, "setStride:", 20);

            let pd_cls = objc_rt::class_named("MTLRenderPipelineDescriptor").unwrap() as Id;
            let pdesc: Id = send0(send0(pd_cls, "alloc"), "init");
            send1::<Id, ()>(pdesc, "setVertexDescriptor:", vdesc);
            let p_catts: Id = send0(pdesc, "colorAttachments");
            let p_ca0: Id = send1(p_catts, "objectAtIndexedSubscript:", 0usize);
            send1::<usize, ()>(p_ca0, "setBlendingEnabled:", 1);
            let mut err: Id = std::ptr::null_mut();
            let pso: Id = send2(dev, "newRenderPipelineStateWithDescriptor:error:", pdesc, &mut err);

            let pass_cls = objc_rt::class_named("MTLRenderPassDescriptor").unwrap() as Id;
            let pass: Id = send0(pass_cls, "renderPassDescriptor");
            let catts: Id = send0(pass, "colorAttachments");
            let ca0: Id = send1(catts, "objectAtIndexedSubscript:", 0usize);
            send1::<Id, ()>(ca0, "setTexture:", view);
            send1::<usize, ()>(ca0, "setLoadAction:", 2);
            let s_cc = objc_rt::sel("setClearColor:");
            let d_cc = lookup_for_send(ca0, s_cc);
            let f_cc: unsafe extern "C" fn(Id, Sel, MTLClearColor) = std::mem::transmute(d_cc.imp);
            f_cc(
                d_cc.receiver,
                s_cc,
                MTLClearColor { red: 0.0, green: 0.0, blue: 0.4, alpha: 1.0 },
            );

            let cb: Id = send0(cq, "commandBuffer");
            let renc: Id = send1(cb, "renderCommandEncoderWithDescriptor:", pass);
            send1::<Id, ()>(renc, "setRenderPipelineState:", pso);
            send3::<Id, usize, usize, ()>(renc, "setVertexBuffer:offset:atIndex:", vbuf, 0usize, 1usize);
            send2::<Id, usize, ()>(renc, "setFragmentTexture:atIndex:", ftex, 0usize);
            send5::<usize, usize, usize, Id, usize, ()>(
                renc,
                "drawIndexedPrimitives:indexCount:indexType:indexBuffer:indexBufferOffset:",
                3usize,
                6usize,
                0usize,
                ibuf,
                0usize,
            );
            send0::<()>(renc, "endEncoding");
            send0::<()>(cb, "commit");

            // Read back directly from parent (BGRA8Unorm -> byte order [B, G, R, A])
            let s_gb = objc_rt::sel("getBytes:bytesPerRow:fromRegion:mipmapLevel:");
            let rgn8 = MTLRegion {
                origin: MTLOrigin { x: 0, y: 0, z: 0 },
                size: MTLSize { width: 8, height: 8, depth: 1 },
            };
            let mut parent_out = [0u8; 8 * 8 * 4];
            let d_gb = lookup_for_send(parent, s_gb);
            let f_gb: unsafe extern "C" fn(Id, Sel, *mut u8, usize, MTLRegion, usize) =
                std::mem::transmute(d_gb.imp);
            f_gb(d_gb.receiver, s_gb, parent_out.as_mut_ptr(), 32, rgn8, 0);
            // Left half samples [200,100,50,128] blended over clear [0,0,102,255] -> R=100, G=50, B=(50*128+102*127+127)/255=76 -> BGRA [76,50,100,255]
            assert_eq!(&parent_out[0..4], &[76, 50, 100, 255]);
            // Right half samples [40,220,120,255] opaque -> BGRA [120,220,40,255]
            assert_eq!(&parent_out[6 * 4..7 * 4], &[120, 220, 40, 255]);

            // Upload via replaceRegion on view and read back from parent
            let view_patch: [u8; 4] = [9, 8, 7, 255];
            let rgn1 = MTLRegion {
                origin: MTLOrigin { x: 1, y: 1, z: 0 },
                size: MTLSize { width: 1, height: 1, depth: 1 },
            };
            let d_rrv = lookup_for_send(view, s_rr);
            let f_rrv: unsafe extern "C" fn(Id, Sel, MTLRegion, usize, *const u8, usize) =
                std::mem::transmute(d_rrv.imp);
            f_rrv(d_rrv.receiver, s_rr, rgn1, 0, view_patch.as_ptr(), 4);
            let mut patch_rb = [0u8; 4];
            f_gb(d_gb.receiver, s_gb, patch_rb.as_mut_ptr(), 4, rgn1, 0);
            assert_eq!(patch_rb, view_patch);

            // 2. Unprojected 3D & behind-camera rejection
            let t4_desc = f_td(d_td.receiver, s_td, 70, 4, 4, false);
            let t4: Id = send1(dev, "newTextureWithDescriptor:", t4_desc);
            let sentinel = [11u8, 22, 33, 255];
            let mut t4_init = [0u8; 4 * 4 * 4];
            for ch in t4_init.chunks_exact_mut(4) {
                ch.copy_from_slice(&sentinel);
            }
            let d_rr4 = lookup_for_send(t4, s_rr);
            let f_rr4: unsafe extern "C" fn(Id, Sel, MTLRegion, usize, *const u8, usize) =
                std::mem::transmute(d_rr4.imp);
            f_rr4(d_rr4.receiver, s_rr, rgn4, 0, t4_init.as_ptr(), 16);

            let huge_verts: [f32; 20] = [
                -500.0, -500.0, 0.0, 0.0, 1.0,
                 500.0, -500.0, 0.0, 1.0, 1.0,
                 500.0,  500.0, 0.0, 1.0, 0.0,
                -500.0,  500.0, 0.0, 0.0, 0.0,
            ];
            let mut hbytes = vec![0u8; huge_verts.len() * 4];
            for (i, &v) in huge_verts.iter().enumerate() {
                hbytes[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
            }
            let hbuf: Id = send3(
                dev,
                "newBufferWithBytes:length:options:",
                hbytes.as_ptr() as *const c_void,
                hbytes.len(),
                0usize,
            );
            let pass4: Id = send0(pass_cls, "renderPassDescriptor");
            let catts4: Id = send0(pass4, "colorAttachments");
            let ca4: Id = send1(catts4, "objectAtIndexedSubscript:", 0usize);
            send1::<Id, ()>(ca4, "setTexture:", t4);
            send1::<usize, ()>(ca4, "setLoadAction:", 1); // Load

            let cb2: Id = send0(cq, "commandBuffer");
            let renc2: Id = send1(cb2, "renderCommandEncoderWithDescriptor:", pass4);
            send1::<Id, ()>(renc2, "setRenderPipelineState:", pso);
            send3::<Id, usize, usize, ()>(renc2, "setVertexBuffer:offset:atIndex:", hbuf, 0usize, 1usize);
            send5::<usize, usize, usize, Id, usize, ()>(
                renc2,
                "drawIndexedPrimitives:indexCount:indexType:indexBuffer:indexBufferOffset:",
                3usize,
                6usize,
                0usize,
                ibuf,
                0usize,
            );
            send0::<()>(renc2, "endEncoding");
            send0::<()>(cb2, "commit");

            // Behind camera matrix: w_out = -1.0 (row 3 / col 3 -> m[15] = -1.0)
            let neg_w_mvp: [f32; 16] = [
                1.0, 0.0, 0.0, 0.0,
                0.0, 1.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
                0.0, 0.0, 0.0, -1.0,
            ];
            let mut mbytes = vec![0u8; 64];
            for (i, &v) in neg_w_mvp.iter().enumerate() {
                mbytes[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
            }
            let cb3: Id = send0(cq, "commandBuffer");
            let renc3: Id = send1(cb3, "renderCommandEncoderWithDescriptor:", pass4);
            send1::<Id, ()>(renc3, "setRenderPipelineState:", pso);
            let s_vb = objc_rt::sel("setVertexBytes:length:atIndex:");
            let d_vb = lookup_for_send(renc3, s_vb);
            let f_vb: unsafe extern "C" fn(Id, Sel, *const u8, usize, usize) =
                std::mem::transmute(d_vb.imp);
            f_vb(d_vb.receiver, s_vb, mbytes.as_ptr(), mbytes.len(), 0);
            send3::<Id, usize, usize, ()>(renc3, "setVertexBuffer:offset:atIndex:", vbuf, 0usize, 1usize);
            send5::<usize, usize, usize, Id, usize, ()>(
                renc3,
                "drawIndexedPrimitives:indexCount:indexType:indexBuffer:indexBufferOffset:",
                3usize,
                6usize,
                0usize,
                ibuf,
                0usize,
            );
            send0::<()>(renc3, "endEncoding");
            send0::<()>(cb3, "commit");

            let mut t4_rb = [0u8; 4 * 4 * 4];
            let d_gb4 = lookup_for_send(t4, s_gb);
            let f_gb4: unsafe extern "C" fn(Id, Sel, *mut u8, usize, MTLRegion, usize) =
                std::mem::transmute(d_gb4.imp);
            f_gb4(d_gb4.receiver, s_gb, t4_rb.as_mut_ptr(), 16, rgn4, 0);
            assert_eq!(t4_rb, t4_init);

            // 3. Self-sampling draw safety (same texture bound as target and fragmentTexture:0)
            let cb4: Id = send0(cq, "commandBuffer");
            let renc4: Id = send1(cb4, "renderCommandEncoderWithDescriptor:", pass4);
            send1::<Id, ()>(renc4, "setRenderPipelineState:", pso);
            send3::<Id, usize, usize, ()>(renc4, "setVertexBuffer:offset:atIndex:", vbuf, 0usize, 1usize);
            send2::<Id, usize, ()>(renc4, "setFragmentTexture:atIndex:", t4, 0usize);
            send5::<usize, usize, usize, Id, usize, ()>(
                renc4,
                "drawIndexedPrimitives:indexCount:indexType:indexBuffer:indexBufferOffset:",
                3usize,
                6usize,
                0usize,
                ibuf,
                0usize,
            );
            send0::<()>(renc4, "endEncoding");
            send0::<()>(cb4, "commit");

            pear_libobjc::object::release(t4);
            pear_libobjc::object::release(ftex);
            pear_libobjc::object::release(view);
            pear_libobjc::object::release(parent);
            pear_libobjc::object::release(dev);
        }
    }

    #[test]
    fn phase5_20_rasterizer_constant_row_and_1to1_blit_parity() {
        fn ref_span(
            len_px: usize,
            frag_bytes: &[u8],
            fw: usize,
            fh: usize,
            pot: bool,
            mut u_fp: i64,
            mut v_fp: i64,
            du_fp: i64,
            dv_fp: i64,
        ) -> Vec<u8> {
            let mut out = vec![0u8; len_px * 4];
            let fw_mask = fw.saturating_sub(1);
            let fh_mask = fh.saturating_sub(1);
            let fw_fp = (fw as i64) << 16;
            let fh_fp = (fh as i64) << 16;
            if !pot {
                if u_fp < 0 {
                    u_fp += fw_fp;
                } else if u_fp >= fw_fp {
                    u_fp -= fw_fp;
                }
                if v_fp < 0 {
                    v_fp += fh_fp;
                } else if v_fp >= fh_fp {
                    v_fp -= fh_fp;
                }
            }
            for dst in out.chunks_exact_mut(4) {
                let (tx, ty) = if pot {
                    let tx = ((u_fp >> 16) as usize) & fw_mask;
                    let ty = ((v_fp >> 16) as usize) & fh_mask;
                    u_fp = u_fp.wrapping_add(du_fp);
                    v_fp = v_fp.wrapping_add(dv_fp);
                    (tx, ty)
                } else {
                    let tx = ((u_fp >> 16).max(0) as usize).min(fw - 1);
                    let ty = ((v_fp >> 16).max(0) as usize).min(fh - 1);
                    u_fp += du_fp;
                    if u_fp >= fw_fp {
                        u_fp -= fw_fp;
                    } else if u_fp < 0 {
                        u_fp += fw_fp;
                    }
                    v_fp += dv_fp;
                    if v_fp >= fh_fp {
                        v_fp -= fh_fp;
                    } else if v_fp < 0 {
                        v_fp += fh_fp;
                    }
                    (tx, ty)
                };
                let off_s = (ty * fw + tx) * 4;
                dst.copy_from_slice(&frag_bytes[off_s..off_s + 4]);
            }
            out
        }

        for &(pot, fw, fh) in &[(true, 8usize, 4usize), (false, 6usize, 5usize)] {
            let mut tex = vec![0u8; fw * fh * 4];
            for y in 0..fh {
                for x in 0..fw {
                    let idx = (y * fw + x) * 4;
                    tex[idx] = (x * 31 + 10) as u8;
                    tex[idx + 1] = (y * 47 + 20) as u8;
                    tex[idx + 2] = ((x ^ y) * 19 + 5) as u8;
                    tex[idx + 3] = 255;
                }
            }
            for &(len_px, u_fp, v_fp, du_fp, dv_fp) in &[
                (17usize, 3i64 << 16, 2i64 << 16, 65536i64, 0i64), // 1:1 blit wrapping multiple times
                (11, 2 << 16, 1 << 16, 0, 0),                        // constant-texel fill
                (15, (4 << 16) + 1234, (3 << 16) + 500, 32768, 0),   // 2x horizontal scale
                (12, 1 << 16, 2 << 16, -24000, 0),                   // negative du wrap
            ] {
                let mut got = vec![0u8; len_px * 4];
                if pot {
                    objc_metal::rasterize_textured_span::<true, false, false>(
                        &mut got, &tex, fw, fh, 0, 2, 0, 2, u_fp, v_fp, du_fp, dv_fp, 255, 255,
                        255, 255,
                    );
                } else {
                    objc_metal::rasterize_textured_span::<false, false, false>(
                        &mut got, &tex, fw, fh, 0, 2, 0, 2, u_fp, v_fp, du_fp, dv_fp, 255, 255,
                        255, 255,
                    );
                }
                let expected = ref_span(len_px, &tex, fw, fh, pot, u_fp, v_fp, du_fp, dv_fp);
                assert_eq!(
                    got, expected,
                    "mismatch for pot={pot} fw={fw} fh={fh} u={u_fp} v={v_fp} du={du_fp}"
                );
            }
        }
    }
}
