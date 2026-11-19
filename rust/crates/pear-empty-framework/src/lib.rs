//! The single export of PearHost's empty framework images (iAd, ...): an
//! iOS-only framework an app links but does not call. If an app later
//! imports a symbol from one of them, dyld names it and the census picks it
//! up; then it gets a real implementation.

#[no_mangle]
pub extern "C" fn pear_empty_framework() {}
