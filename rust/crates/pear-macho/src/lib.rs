//! PearRuntime Mach-O Inspector & macOS-Host Retargeter (Rust).
//!
//! Pure-Rust parser for 64-bit ARM64 Mach-O headers, FairPlay `cryptid`
//! detection, and `LC_BUILD_VERSION` platform checks / macOS retargeting.

use std::ffi::{c_char, c_int, CStr};
use std::fs;

const MH_MAGIC_64: u32 = 0xFEED_FACF;
const CPU_TYPE_ARM64: u32 = 0x0100_000C;

const LC_BUILD_VERSION: u32 = 0x32;
const LC_VERSION_MIN_IPHONEOS: u32 = 0x25;
const LC_ENCRYPTION_INFO_64: u32 = 0x2C;

pub const PLATFORM_MACOS: u32 = 1;
pub const PLATFORM_IOS: u32 = 2;
pub const PLATFORM_MACCATALYST: u32 = 6;

/// Packed version `14.0.0` in Mach-O `(major << 16) | (minor << 8) | patch`.
pub const MACOS_14_0: u32 = 14 << 16;

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct PearMachOInfo {
    pub is_valid_arm64: bool,
    pub platform: u32,
    pub minos: u32,
    pub sdk: u32,
    pub cryptid: u32,
}

fn read_u32_le(data: &[u8], off: usize) -> Option<u32> {
    let bytes: [u8; 4] = data.get(off..off + 4)?.try_into().ok()?;
    Some(u32::from_le_bytes(bytes))
}

fn write_u32_le(data: &mut [u8], off: usize, val: u32) -> bool {
    if let Some(slice) = data.get_mut(off..off + 4) {
        slice.copy_from_slice(&val.to_le_bytes());
        true
    } else {
        false
    }
}

pub fn inspect_slice(data: &[u8]) -> Option<PearMachOInfo> {
    if data.len() < 32 {
        return None;
    }
    let magic = read_u32_le(data, 0)?;
    if magic != MH_MAGIC_64 {
        return None;
    }
    let cputype = read_u32_le(data, 4)?;
    let ncmds = read_u32_le(data, 16)? as usize;
    let sizeofcmds = read_u32_le(data, 20)? as usize;
    if 32 + sizeofcmds > data.len() {
        return None;
    }

    let mut info = PearMachOInfo {
        is_valid_arm64: cputype == CPU_TYPE_ARM64,
        platform: 0,
        minos: 0,
        sdk: 0,
        cryptid: 0,
    };

    let mut off = 32usize;
    let end = 32 + sizeofcmds;
    for _ in 0..ncmds {
        if off + 8 > end {
            break;
        }
        let cmd = read_u32_le(data, off)?;
        let cmdsize = read_u32_le(data, off + 4)? as usize;
        if cmdsize < 8 || off + cmdsize > end {
            break;
        }
        match cmd {
            LC_BUILD_VERSION if cmdsize >= 24 => {
                info.platform = read_u32_le(data, off + 8)?;
                info.minos = read_u32_le(data, off + 12)?;
                info.sdk = read_u32_le(data, off + 16)?;
            }
            LC_VERSION_MIN_IPHONEOS if cmdsize >= 16 && info.platform == 0 => {
                info.platform = PLATFORM_IOS;
                info.minos = read_u32_le(data, off + 8)?;
                info.sdk = read_u32_le(data, off + 12)?;
            }
            LC_ENCRYPTION_INFO_64 if cmdsize >= 24 => {
                info.cryptid = read_u32_le(data, off + 16)?;
            }
            _ => {}
        }
        off += cmdsize;
    }

    Some(info)
}

/// Retargets an `LC_BUILD_VERSION` in-place to `PLATFORM_MACOS` (`minos = 14.0`).
/// Returns `Ok(true)` if modified, `Ok(false)` if already macOS, `Err` if invalid or encrypted.
pub fn retarget_macos_in_place(data: &mut [u8]) -> Result<bool, &'static str> {
    let info = inspect_slice(data).ok_or("not a valid 64-bit Mach-O")?;
    if !info.is_valid_arm64 {
        return Err("not an ARM64 Mach-O");
    }
    if info.cryptid != 0 {
        return Err("Mach-O is FairPlay-encrypted (cryptid != 0)");
    }

    let ncmds = read_u32_le(data, 16).ok_or("bad header")? as usize;
    let sizeofcmds = read_u32_le(data, 20).ok_or("bad header")? as usize;
    let end = 32 + sizeofcmds;
    let mut off = 32usize;
    for _ in 0..ncmds {
        if off + 8 > end {
            break;
        }
        let cmd = read_u32_le(data, off).ok_or("bad command")?;
        let cmdsize = read_u32_le(data, off + 4).ok_or("bad command")? as usize;
        if cmdsize < 8 || off + cmdsize > end {
            break;
        }
        if cmd == LC_BUILD_VERSION && cmdsize >= 24 {
            let cur = read_u32_le(data, off + 8).ok_or("bad build version")?;
            if cur == PLATFORM_MACOS {
                return Ok(false);
            }
            write_u32_le(data, off + 8, PLATFORM_MACOS);
            write_u32_le(data, off + 12, MACOS_14_0);
            return Ok(true);
        }
        off += cmdsize;
    }

    Err("no LC_BUILD_VERSION load command found")
}

#[no_mangle]
pub extern "C" fn PearMachOInspectFile(path: *const c_char, out_info: *mut PearMachOInfo) -> c_int {
    if path.is_null() || out_info.is_null() {
        return -1;
    }
    let Ok(path_str) = (unsafe { CStr::from_ptr(path) }).to_str() else {
        return -1;
    };
    let Ok(bytes) = fs::read(path_str) else {
        return -2;
    };
    match inspect_slice(&bytes) {
        Some(info) => {
            unsafe { *out_info = info };
            0
        }
        None => -3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_minimal_arm64_macho(platform: u32, cryptid: u32) -> Vec<u8> {
        let mut buf = vec![0u8; 32 + 24 + 24];
        write_u32_le(&mut buf, 0, MH_MAGIC_64);
        write_u32_le(&mut buf, 4, CPU_TYPE_ARM64);
        write_u32_le(&mut buf, 16, 2); // ncmds
        write_u32_le(&mut buf, 20, 48); // sizeofcmds

        // LC_BUILD_VERSION (24 bytes)
        write_u32_le(&mut buf, 32, LC_BUILD_VERSION);
        write_u32_le(&mut buf, 36, 24);
        write_u32_le(&mut buf, 40, platform);
        write_u32_le(&mut buf, 44, 17 << 16);
        write_u32_le(&mut buf, 48, 17 << 16);

        // LC_ENCRYPTION_INFO_64 (24 bytes)
        write_u32_le(&mut buf, 56, LC_ENCRYPTION_INFO_64);
        write_u32_le(&mut buf, 60, 24);
        write_u32_le(&mut buf, 72, cryptid);

        buf
    }

    #[test]
    fn inspect_and_retarget_ios_to_macos() {
        let mut data = make_minimal_arm64_macho(PLATFORM_IOS, 0);
        let info = inspect_slice(&data).unwrap();
        assert!(info.is_valid_arm64);
        assert_eq!(info.platform, PLATFORM_IOS);
        assert_eq!(info.cryptid, 0);

        assert_eq!(retarget_macos_in_place(&mut data), Ok(true));
        let updated = inspect_slice(&data).unwrap();
        assert_eq!(updated.platform, PLATFORM_MACOS);
        assert_eq!(updated.minos, MACOS_14_0);
        assert_eq!(retarget_macos_in_place(&mut data), Ok(false));
    }

    #[test]
    fn rejects_encrypted_binary() {
        let mut data = make_minimal_arm64_macho(PLATFORM_IOS, 1);
        assert!(retarget_macos_in_place(&mut data).is_err());
    }
}
