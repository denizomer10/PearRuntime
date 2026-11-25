//! PearLegacyJIT — dynamic CPU translation & execution planner for PearRuntime.
//!
//! Unifies 32-bit (`PearLegacyEngine` / A32) and 64-bit (`PearRuntime` / A64)
//! execution across macOS and Linux (x86_64 with AVX2/SSE4.2 & native ARM64):
//!
//! - macOS ARM64 + iOS 64-bit: native ARM64 (no JIT), 64-bit API layer.
//! - macOS ARM64 + iOS 32-bit: `PearLegacyJIT` (A32) + 32-bit API layer.
//! - Linux ARM64 + iOS 64-bit: native ARM64 (no JIT), OS-independent 64-bit API layer.
//! - Linux ARM64 + iOS 32-bit: `PearLegacyJIT` (A32) + 32-bit API layer.
//! - Linux/macOS x86_64 + iOS 64-bit: `PearLegacyJIT` (A64 -> x86_64, AVX2/SSE4.2) + 64-bit API layer.
//! - Linux/macOS x86_64 + iOS 32-bit: `PearLegacyJIT` (A32 -> x86_64, AVX2/SSE4.2) + 32-bit API layer.

pub use pear_macho::GuestArch;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum HostOs {
    MacOS,
    Linux,
    Other,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum HostArch {
    Aarch64,
    X86_64,
    Other,
}

/// x86_64 SIMD capability tier detected at runtime for `PearLegacyJIT` codegen.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct X86SimdProfile {
    pub sse42: bool,
    pub avx2: bool,
    pub fma: bool,
    pub bmi2: bool,
}

impl X86SimdProfile {
    pub fn detect() -> Self {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            Self {
                sse42: std::is_x86_feature_detected!("sse4.2"),
                avx2: std::is_x86_feature_detected!("avx2"),
                fma: std::is_x86_feature_detected!("fma"),
                bmi2: std::is_x86_feature_detected!("bmi2"),
            }
        }
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        {
            Self {
                sse42: false,
                avx2: false,
                fma: false,
                bmi2: false,
            }
        }
    }

    pub fn label(&self) -> &'static str {
        if self.avx2 && self.fma {
            "x86_64-AVX2+FMA"
        } else if self.avx2 {
            "x86_64-AVX2"
        } else if self.sse42 {
            "x86_64-SSE4.2"
        } else {
            "x86_64-baseline"
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ApiTier {
    /// 32-bit iOS HLE APIs (PearLegacyEngine / touchHLE fork + pear-legacy-host).
    Ios32,
    /// 64-bit iOS APIs (pear-dyld + pear-libobjc + pear-foundation + pear-graphics + pear-host).
    Ios64,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum JitTranslator {
    /// ARMv7 / Thumb-2 (A32) -> Host JIT (via `PearLegacyJIT` / dynarmic A32).
    Arm32ToHost,
    /// ARM64 (A64) -> x86_64 JIT (via `PearLegacyJIT` / dynarmic A64).
    Arm64ToX86_64,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ExecutionMode {
    /// Guest and host are both ARM64: execute directly on CPU, translate OS APIs only.
    NativeDirect,
    /// 64-bit ARM64 guest on x86_64 host: activate A64->x86_64 JIT + 64-bit APIs.
    Arm64ToX86_64Jit,
    /// 32-bit ARM guest on any host: activate A32 JIT + 32-bit APIs (`PearLegacyEngine`).
    Legacy32BitJit,
}

/// Bitmask matching `Dynarmic::OptimizationFlag` in `third_party/legacy-jit`.
pub mod opt_flags {
    pub const BLOCK_LINKING: u32 = 0x0000_0001;
    pub const RETURN_STACK_BUFFER: u32 = 0x0000_0002;
    pub const FAST_DISPATCH: u32 = 0x0000_0004;
    pub const GET_SET_ELIMINATION: u32 = 0x0000_0008;
    pub const CONST_PROP: u32 = 0x0000_0010;
    pub const MISC_IR_OPT: u32 = 0x0000_0020;
    pub const UNSAFE_UNFUSE_FMA: u32 = 0x0001_0000;
    pub const ALL_SAFE: u32 = BLOCK_LINKING
        | RETURN_STACK_BUFFER
        | FAST_DISPATCH
        | GET_SET_ELIMINATION
        | CONST_PROP
        | MISC_IR_OPT;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionPlan {
    pub host_os: HostOs,
    pub host_arch: HostArch,
    pub guest_arch: GuestArch,
    pub api_tier: ApiTier,
    pub mode: ExecutionMode,
    pub jit: Option<JitTranslator>,
    pub x86_simd: Option<X86SimdProfile>,
    pub dynarmic_opt_flags: u32,
}

impl ExecutionPlan {
    pub fn current_host(guest_arch: GuestArch) -> Self {
        let host_os = if cfg!(target_os = "macos") {
            HostOs::MacOS
        } else if cfg!(target_os = "linux") {
            HostOs::Linux
        } else {
            HostOs::Other
        };
        let host_arch = if cfg!(target_arch = "aarch64") {
            HostArch::Aarch64
        } else if cfg!(target_arch = "x86_64") {
            HostArch::X86_64
        } else {
            HostArch::Other
        };
        let simd = if host_arch == HostArch::X86_64 {
            Some(X86SimdProfile::detect())
        } else {
            None
        };
        Self::decide(host_os, host_arch, guest_arch, simd)
    }

    /// Pure decision function: determines whether JIT is needed, which API tier
    /// (32-bit vs 64-bit) to boot, and which x86_64 SIMD optimizations to enable.
    pub fn decide(
        host_os: HostOs,
        host_arch: HostArch,
        guest_arch: GuestArch,
        x86_simd: Option<X86SimdProfile>,
    ) -> Self {
        let api_tier = match guest_arch {
            GuestArch::Arm32 => ApiTier::Ios32,
            GuestArch::Arm64 => ApiTier::Ios64,
        };

        let (mode, jit) = match (host_arch, guest_arch) {
            (HostArch::Aarch64, GuestArch::Arm64) => (ExecutionMode::NativeDirect, None),
            (HostArch::X86_64, GuestArch::Arm64) => {
                (ExecutionMode::Arm64ToX86_64Jit, Some(JitTranslator::Arm64ToX86_64))
            }
            (_, GuestArch::Arm32) => {
                (ExecutionMode::Legacy32BitJit, Some(JitTranslator::Arm32ToHost))
            }
            (_, GuestArch::Arm64) => {
                (ExecutionMode::Arm64ToX86_64Jit, Some(JitTranslator::Arm64ToX86_64))
            }
        };

        let mut dynarmic_opt_flags = if jit.is_some() {
            opt_flags::ALL_SAFE
        } else {
            0
        };
        if let Some(s) = x86_simd {
            if jit.is_some() && !s.fma {
                // Older SSE4.2 / non-FMA x86_64 CPUs benefit from unfusing FMA.
                dynarmic_opt_flags |= opt_flags::UNSAFE_UNFUSE_FMA;
            }
        }

        Self {
            host_os,
            host_arch,
            guest_arch,
            api_tier,
            mode,
            jit,
            x86_simd,
            dynarmic_opt_flags,
        }
    }

    pub fn summary(&self) -> String {
        let simd_str = self
            .x86_simd
            .map(|s| s.label())
            .unwrap_or("native");
        format!(
            "os={:?} host={:?} guest={:?} api={:?} mode={:?} jit={:?} simd={} opt=0x{:x}",
            self.host_os,
            self.host_arch,
            self.guest_arch,
            self.api_tier,
            self.mode,
            self.jit,
            simd_str,
            self.dynarmic_opt_flags
        )
    }
}

/// Guest memory callback interface for `PearLegacyJIT` (A32 & A64).
pub trait GuestMemory {
    fn read_u8(&self, addr: u64) -> u8;
    fn read_u16(&self, addr: u64) -> u16;
    fn read_u32(&self, addr: u64) -> u32;
    fn read_u64(&self, addr: u64) -> u64;
    fn write_u8(&mut self, addr: u64, val: u8);
    fn write_u16(&mut self, addr: u64, val: u16);
    fn write_u32(&mut self, addr: u64, val: u32);
    fn write_u64(&mut self, addr: u64, val: u64);
}

#[derive(Clone, Debug, Default)]
pub struct A64CpuState {
    pub x: [u64; 31],
    pub sp: u64,
    pub pc: u64,
    pub pstate: u32,
}

#[derive(Clone, Debug, Default)]
pub struct A32CpuState {
    pub r: [u32; 16],
    pub cpsr: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_arm64_skips_jit_for_ios64_and_enables_for_ios32() {
        let p64 = ExecutionPlan::decide(HostOs::MacOS, HostArch::Aarch64, GuestArch::Arm64, None);
        assert_eq!(p64.mode, ExecutionMode::NativeDirect);
        assert_eq!(p64.api_tier, ApiTier::Ios64);
        assert_eq!(p64.jit, None);

        let p32 = ExecutionPlan::decide(HostOs::MacOS, HostArch::Aarch64, GuestArch::Arm32, None);
        assert_eq!(p32.mode, ExecutionMode::Legacy32BitJit);
        assert_eq!(p32.api_tier, ApiTier::Ios32);
        assert_eq!(p32.jit, Some(JitTranslator::Arm32ToHost));
    }

    #[test]
    fn linux_x86_64_enables_a64_and_a32_jit_with_simd_tuning() {
        let avx2 = X86SimdProfile {
            sse42: true,
            avx2: true,
            fma: true,
            bmi2: true,
        };
        let p64 = ExecutionPlan::decide(
            HostOs::Linux,
            HostArch::X86_64,
            GuestArch::Arm64,
            Some(avx2),
        );
        assert_eq!(p64.mode, ExecutionMode::Arm64ToX86_64Jit);
        assert_eq!(p64.api_tier, ApiTier::Ios64);
        assert_eq!(p64.jit, Some(JitTranslator::Arm64ToX86_64));
        assert_eq!(p64.dynarmic_opt_flags & opt_flags::BLOCK_LINKING, opt_flags::BLOCK_LINKING);
        assert_eq!(p64.dynarmic_opt_flags & opt_flags::UNSAFE_UNFUSE_FMA, 0);

        let old_sse42 = X86SimdProfile {
            sse42: true,
            avx2: false,
            fma: false,
            bmi2: false,
        };
        let p32 = ExecutionPlan::decide(
            HostOs::Linux,
            HostArch::X86_64,
            GuestArch::Arm32,
            Some(old_sse42),
        );
        assert_eq!(p32.mode, ExecutionMode::Legacy32BitJit);
        assert_eq!(p32.api_tier, ApiTier::Ios32);
        assert_ne!(p32.dynarmic_opt_flags & opt_flags::UNSAFE_UNFUSE_FMA, 0);
    }

    #[test]
    fn linux_arm64_runs_ios64_native_and_ios32_via_jit() {
        let p64 = ExecutionPlan::decide(HostOs::Linux, HostArch::Aarch64, GuestArch::Arm64, None);
        assert_eq!(p64.mode, ExecutionMode::NativeDirect);
        assert_eq!(p64.jit, None);

        let p32 = ExecutionPlan::decide(HostOs::Linux, HostArch::Aarch64, GuestArch::Arm32, None);
        assert_eq!(p32.mode, ExecutionMode::Legacy32BitJit);
        assert_eq!(p32.jit, Some(JitTranslator::Arm32ToHost));
    }
}
