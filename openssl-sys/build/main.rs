#[cfg(feature = "bindgen")]
extern crate bindgen;
extern crate cc;
#[cfg(feature = "vendored")]
extern crate openssl_src;
extern crate pkg_config;
extern crate vcpkg;

use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
mod cfgs;

mod run_bindgen;

#[derive(PartialEq)]
enum Version {
    Openssl3xx,
    Openssl11x,
    Openssl10x,
    Libressl,
    Boringssl,
}

fn env_inner(name: &str) -> Option<OsString> {
    let var = env::var_os(name);
    println!("cargo:rerun-if-env-changed={}", name);

    match var {
        Some(ref v) => println!("{} = {}", name, v.to_string_lossy()),
        None => println!("{} unset", name),
    }

    var
}

fn env(name: &str) -> Option<OsString> {
    let prefix = env::var("TARGET").unwrap().to_uppercase().replace('-', "_");
    let prefixed = format!("{}_{}", prefix, name);
    env_inner(&prefixed).or_else(|| env_inner(name))
}

fn main() {
    let openssl_dir = env("OPENSSL_DIR").map(PathBuf::from);
    if openssl_dir.is_none() {
        panic!("OpenSSL directory not found: env. variable OPENSSL_DIR probably not set.",)
    }

    let lib_dirs = vec![openssl_dir.clone().unwrap().join("lib64")];
    let include_dir = openssl_dir.clone().unwrap().join("include");

    if !lib_dirs.iter().all(|p| Path::new(p).exists()) {
        panic!("OpenSSL library directory does not exist: {:?}", lib_dirs);
    }
    if !Path::new(&include_dir).exists() {
        panic!(
            "OpenSSL include directory does not exist: {}",
            include_dir.to_string_lossy()
        );
    }

    for lib_dir in lib_dirs.iter() {
        println!(
            "cargo:rustc-link-search=native={}",
            lib_dir.to_string_lossy()
        );
    }
    println!("cargo:include={}", include_dir.to_string_lossy());

    postprocess(&[include_dir]);

    let libs = vec!["ssl", "crypto"];

    let kind = "static";
    for lib in libs.into_iter() {
        println!("cargo:rustc-link-lib={}={}", kind, lib);
    }
}

fn postprocess(include_dirs: &[PathBuf]) -> Version {
    let version = validate_headers(include_dirs);

    // Never run bindgen for BoringSSL, if it was needed we already ran it.
    if version != Version::Boringssl {
        #[cfg(feature = "bindgen")]
        run_bindgen::run(&include_dirs);
    }

    version
}

/// Validates the header files found in `include_dir` and then returns the
/// version string of OpenSSL.
#[allow(clippy::unusual_byte_groupings)]
fn validate_headers(include_dirs: &[PathBuf]) -> Version {
    // This `*-sys` crate only works with OpenSSL 1.0.1, 1.0.2, 1.1.0, 1.1.1 and 3.0.0.
    // To correctly expose the right API from this crate, take a look at
    // `opensslv.h` to see what version OpenSSL claims to be.
    //
    // OpenSSL has a number of build-time configuration options which affect
    // various structs and such. Since OpenSSL 1.1.0 this isn't really a problem
    // as the library is much more FFI-friendly, but 1.0.{1,2} suffer this problem.
    //
    // To handle all this conditional compilation we slurp up the configuration
    // file of OpenSSL, `opensslconf.h`, and then dump out everything it defines
    // as our own #[cfg] directives. That way the `ossl10x.rs` bindings can
    // account for compile differences and such.
    println!("cargo:rerun-if-changed=build/expando.c");
    let mut gcc = cc::Build::new();
    gcc.includes(include_dirs);
    let expanded = match gcc.file("build/expando.c").try_expand() {
        Ok(expanded) => expanded,
        Err(e) => {
            panic!(
                "
Header expansion error:
{:?}

Failed to find OpenSSL development headers.

You can try fixing this setting the `OPENSSL_DIR` environment variable
pointing to your OpenSSL installation or installing OpenSSL headers package
specific to your distribution:

    # On Ubuntu
    sudo apt-get install libssl-dev
    # On Arch Linux
    sudo pacman -S openssl
    # On Fedora
    sudo dnf install openssl-devel
    # On Alpine Linux
    apk add openssl-dev

See rust-openssl documentation for more information:

    https://docs.rs/openssl
",
                e
            );
        }
    };
    let expanded = String::from_utf8(expanded).unwrap();

    let mut enabled = vec![];
    let mut openssl_version = None;
    let mut libressl_version = None;
    let mut is_boringssl = false;
    for line in expanded.lines() {
        let line = line.trim();

        let openssl_prefix = "RUST_VERSION_OPENSSL_";
        let new_openssl_prefix = "RUST_VERSION_NEW_OPENSSL_";
        let libressl_prefix = "RUST_VERSION_LIBRESSL_";
        let boringsl_prefix = "RUST_OPENSSL_IS_BORINGSSL";
        let conf_prefix = "RUST_CONF_";
        if let Some(version) = line.strip_prefix(openssl_prefix) {
            openssl_version = Some(parse_version(version));
        } else if let Some(version) = line.strip_prefix(new_openssl_prefix) {
            openssl_version = Some(parse_new_version(version));
        } else if let Some(version) = line.strip_prefix(libressl_prefix) {
            libressl_version = Some(parse_version(version));
        } else if let Some(conf) = line.strip_prefix(conf_prefix) {
            enabled.push(conf);
        } else if line.starts_with(boringsl_prefix) {
            is_boringssl = true;
        }
    }

    for enabled in &enabled {
        println!("cargo:rustc-cfg=osslconf=\"{}\"", enabled);
    }
    println!("cargo:conf={}", enabled.join(","));

    if is_boringssl {
        println!("cargo:rustc-cfg=boringssl");
        println!("cargo:boringssl=true");
        run_bindgen::run_boringssl(include_dirs);
        return Version::Boringssl;
    }

    // We set this for any non-BoringSSL lib.
    println!("cargo:rustc-cfg=openssl");

    for cfg in cfgs::get(openssl_version, libressl_version) {
        println!("cargo:rustc-cfg={}", cfg);
    }

    if let Some(libressl_version) = libressl_version {
        println!("cargo:libressl_version_number={:x}", libressl_version);

        let major = (libressl_version >> 28) as u8;
        let minor = (libressl_version >> 20) as u8;
        let fix = (libressl_version >> 12) as u8;
        let (major, minor, fix) = match (major, minor, fix) {
            (2, 5, 0) => ('2', '5', '0'),
            (2, 5, 1) => ('2', '5', '1'),
            (2, 5, 2) => ('2', '5', '2'),
            (2, 5, _) => ('2', '5', 'x'),
            (2, 6, 0) => ('2', '6', '0'),
            (2, 6, 1) => ('2', '6', '1'),
            (2, 6, 2) => ('2', '6', '2'),
            (2, 6, _) => ('2', '6', 'x'),
            (2, 7, _) => ('2', '7', 'x'),
            (2, 8, 0) => ('2', '8', '0'),
            (2, 8, 1) => ('2', '8', '1'),
            (2, 8, _) => ('2', '8', 'x'),
            (2, 9, 0) => ('2', '9', '0'),
            (2, 9, _) => ('2', '9', 'x'),
            (3, 0, 0) => ('3', '0', '0'),
            (3, 0, 1) => ('3', '0', '1'),
            (3, 0, _) => ('3', '0', 'x'),
            (3, 1, 0) => ('3', '1', '0'),
            (3, 1, _) => ('3', '1', 'x'),
            (3, 2, 0) => ('3', '2', '0'),
            (3, 2, 1) => ('3', '2', '1'),
            (3, 2, _) => ('3', '2', 'x'),
            (3, 3, 0) => ('3', '3', '0'),
            (3, 3, 1) => ('3', '3', '1'),
            (3, 3, _) => ('3', '3', 'x'),
            (3, 4, 0) => ('3', '4', '0'),
            (3, 4, _) => ('3', '4', 'x'),
            (3, 5, _) => ('3', '5', 'x'),
            (3, 6, 0) => ('3', '6', '0'),
            (3, 6, _) => ('3', '6', 'x'),
            (3, 7, 0) => ('3', '7', '0'),
            (3, 7, 1) => ('3', '7', '1'),
            (3, 7, _) => ('3', '7', 'x'),
            (3, 8, 0) => ('3', '8', '0'),
            (3, 8, 1) => ('3', '8', '1'),
            _ => version_error(),
        };

        println!("cargo:libressl=true");
        println!("cargo:libressl_version={}{}{}", major, minor, fix);
        println!("cargo:version=101");
        Version::Libressl
    } else {
        let openssl_version = openssl_version.unwrap();
        println!("cargo:version_number={:x}", openssl_version);

        if openssl_version >= 0x4_00_00_00_0 {
            version_error()
        } else if openssl_version >= 0x3_00_00_00_0 {
            Version::Openssl3xx
        } else if openssl_version >= 0x1_01_01_00_0 {
            println!("cargo:version=111");
            Version::Openssl11x
        } else if openssl_version >= 0x1_01_00_06_0 {
            println!("cargo:version=110");
            println!("cargo:patch=f");
            Version::Openssl11x
        } else if openssl_version >= 0x1_01_00_00_0 {
            println!("cargo:version=110");
            Version::Openssl11x
        } else if openssl_version >= 0x1_00_02_00_0 {
            println!("cargo:version=102");
            Version::Openssl10x
        } else if openssl_version >= 0x1_00_01_00_0 {
            println!("cargo:version=101");
            Version::Openssl10x
        } else {
            version_error()
        }
    }
}

fn version_error() -> ! {
    panic!(
        "

This crate is only compatible with OpenSSL (version 1.0.1 through 1.1.1, or 3), or LibreSSL 2.5
through 3.8.1, but a different version of OpenSSL was found. The build is now aborting
due to this version mismatch.

"
    );
}

// parses a string that looks like "0x100020cfL"
fn parse_version(version: &str) -> u64 {
    // cut off the 0x prefix
    assert!(version.starts_with("0x"));
    let version = &version[2..];

    // and the type specifier suffix
    let version = version.trim_end_matches(|c: char| !c.is_ascii_hexdigit());

    u64::from_str_radix(version, 16).unwrap()
}

// parses a string that looks like 3_0_0
fn parse_new_version(version: &str) -> u64 {
    println!("version: {}", version);
    let mut it = version.split('_');
    let major = it.next().unwrap().parse::<u64>().unwrap();
    let minor = it.next().unwrap().parse::<u64>().unwrap();
    let patch = it.next().unwrap().parse::<u64>().unwrap();

    (major << 28) | (minor << 20) | (patch << 4)
}
