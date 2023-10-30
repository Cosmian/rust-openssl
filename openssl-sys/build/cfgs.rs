#[allow(clippy::unusual_byte_groupings)]
pub fn get(openssl_version: Option<u64>) -> Vec<&'static str> {
    let mut cfgs = vec![];
    let openssl_version = openssl_version.unwrap();

    if openssl_version >= 0x3_02_00_00_0 {
        cfgs.push("ossl320");
    }
    if openssl_version >= 0x3_00_00_00_0 {
        cfgs.push("ossl300");
    }
    if openssl_version >= 0x1_00_01_00_0 {
        cfgs.push("ossl101");
    }
    if openssl_version >= 0x1_00_02_00_0 {
        cfgs.push("ossl102");
    }
    if openssl_version >= 0x1_00_02_06_0 {
        cfgs.push("ossl102f");
    }
    if openssl_version >= 0x1_00_02_08_0 {
        cfgs.push("ossl102h");
    }
    if openssl_version >= 0x1_01_00_00_0 {
        cfgs.push("ossl110");
    }
    if openssl_version >= 0x1_01_00_06_0 {
        cfgs.push("ossl110f");
    }
    if openssl_version >= 0x1_01_00_07_0 {
        cfgs.push("ossl110g");
    }
    if openssl_version >= 0x1_01_00_08_0 {
        cfgs.push("ossl110h");
    }
    if openssl_version >= 0x1_01_01_00_0 {
        cfgs.push("ossl111");
    }
    if openssl_version >= 0x1_01_01_02_0 {
        cfgs.push("ossl111b");
    }
    if openssl_version >= 0x1_01_01_03_0 {
        cfgs.push("ossl111c");
    }
    if openssl_version >= 0x1_01_01_04_0 {
        cfgs.push("ossl111d");
    }

    cfgs
}
