//! Clean-room Metal → Vulkan + `MTLB`/AIR → SPIR-V compatibility layer
//! built on `pear-libobjc`, `pear-foundation`, `pear-graphics`, and `pear-host`.

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

    #[test]
    fn objc_metal_end_to_end_pipeline() {
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
}
