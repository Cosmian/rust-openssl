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

    let kind = "static"; // "dylib" for dynamic linking, "static" else.
    for lib in libs.into_iter() {
        println!("cargo:rustc-link-lib={}={}", kind, lib);
    }
}

fn postprocess(include_dirs: &[PathBuf]) -> Version {
    let version = validate_headers(include_dirs);

    // Never run bindgen for BoringSSL, if it was needed we already ran it.
    if version != Version::Boringssl {
        #[cfg(feature = "bindgen")]
        run_bindgen::run(include_dirs);
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
    for line in expanded.lines() {
        let line = line.trim();

        let openssl_prefix = "RUST_VERSION_OPENSSL_";
        let new_openssl_prefix = "RUST_VERSION_NEW_OPENSSL_";
        let conf_prefix = "RUST_CONF_";
        if let Some(version) = line.strip_prefix(openssl_prefix) {
            openssl_version = Some(parse_version(version));
        } else if let Some(version) = line.strip_prefix(new_openssl_prefix) {
            openssl_version = Some(parse_new_version(version));
        } else if let Some(conf) = line.strip_prefix(conf_prefix) {
            enabled.push(conf);
        }
    }

    for enabled in &enabled {
        println!("cargo:rustc-cfg=osslconf=\"{}\"", enabled);
    }
    println!("cargo:conf={}", enabled.join(","));

    // We set this for any non-BoringSSL lib.
    println!("cargo:rustc-cfg=openssl");

    for cfg in cfgs::get(openssl_version) {
        println!("cargo:rustc-cfg={}", cfg);
    }

    let openssl_version = openssl_version.unwrap();
    println!("cargo:version_number={:x}", openssl_version);

    if openssl_version != 0x3_01_00_00_0 {
        version_error()
    } else {
        Version::Openssl3xx
    }
}

fn version_error() -> ! {
    panic!(
        "

This crate is only compatible with OpenSSL version 3.1.0 but a different version of OpenSSL was found. The build is now aborting
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
