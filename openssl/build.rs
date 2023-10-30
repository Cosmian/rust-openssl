#![allow(
    clippy::inconsistent_digit_grouping,
    clippy::uninlined_format_args,
    clippy::unusual_byte_groupings
)]

use std::env;

fn main() {
    if let Ok(version) = env::var("DEP_OPENSSL_VERSION_NUMBER") {
        let version = u64::from_str_radix(&version, 16).unwrap();

        if version >= 0x1_00_01_00_0 {
            println!("cargo:rustc-cfg=ossl101");
        }
        if version >= 0x1_00_02_00_0 {
            println!("cargo:rustc-cfg=ossl102");
        }
        if version >= 0x1_01_00_00_0 {
            println!("cargo:rustc-cfg=ossl110");
        }
        if version >= 0x1_01_00_07_0 {
            println!("cargo:rustc-cfg=ossl110g");
        }
        if version >= 0x1_01_00_08_0 {
            println!("cargo:rustc-cfg=ossl110h");
        }
        if version >= 0x1_01_01_00_0 {
            println!("cargo:rustc-cfg=ossl111");
        }
        if version >= 0x3_00_00_00_0 {
            println!("cargo:rustc-cfg=ossl300");
        }
        if version >= 0x3_01_00_00_0 {
            println!("cargo:rustc-cfg=ossl310");
        }
        if version >= 0x3_02_00_00_0 {
            println!("cargo:rustc-cfg=ossl320");
        }
    }
}
