//! One PearHost stub or overlay framework, chosen at build time
//! (PEAR_HOST_FRAMEWORK, see build.rs and tools/gen-host-frameworks.py).
#![allow(non_upper_case_globals, non_snake_case, dead_code)]

include!(concat!(env!("OUT_DIR"), "/framework.rs"));
