//! A small re-export shim for the C++ runtime (libcxxcompat.dylib).
//!
//! Some Catalyst binaries contain a weak placeholder for
//! `std::__1::piecewise_construct` and expect the C++ runtime to provide the
//! real object. The host runtime does not export that data symbol, so the
//! placeholder is never coalesced and Swift metadata resolution fails. The
//! dylib re-exports the system C++ runtime and supplies the empty tag object;
//! one byte is enough for an empty struct.

#[export_name = "_ZNSt3__119piecewise_constructE"]
pub static PIECEWISE_CONSTRUCT: u8 = 0;
