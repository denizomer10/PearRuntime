//! Data symbols needed by Swift overlays that were compiled against the UIKit
//! module. The force-load marker is intentionally empty; the protocol
//! conformance, witness table, and generic operators are emitted by the Swift
//! compiler from compat/swift/PearUIKitNumeric.swift.
//!
//! No vendor code; the symbol names are interface facts.

#[export_name = "_swift_FORCE_LOAD_$_swiftUIKit"]
pub static SWIFT_FORCE_LOAD_UIKIT: u8 = 0;
