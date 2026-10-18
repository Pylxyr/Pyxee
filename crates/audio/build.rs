//! Compiles the C decode shim against the minimal static FFmpeg build and links it.
//!
//! The libav* static libraries come from `cargo xtask ffmpeg` (see docs/engine.md). Point
//! `PYXEE_FFMPEG_DIR` at the install prefix (it holds `include/` and `lib/`); without it the
//! default is `<workspace>/target/ffmpeg/<target triple>`.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=PYXEE_FFMPEG_DIR");
    println!("cargo:rerun-if-changed=csrc/pxdec.c");
    println!("cargo:rerun-if-changed=csrc/pxdec.h");

    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets this"));
    let target = std::env::var("TARGET").expect("cargo sets this");
    let prefix = std::env::var_os("PYXEE_FFMPEG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("../../target/ffmpeg").join(&target));
    let (include, lib) = (prefix.join("include"), prefix.join("lib"));
    assert!(
        include.join("libavformat/avformat.h").is_file(),
        "minimal FFmpeg not found at {}.\nBuild it with `cargo xtask ffmpeg`, or set PYXEE_FFMPEG_DIR \
         to an install prefix that has include/ and lib/.",
        prefix.display()
    );

    cc::Build::new()
        .file("csrc/pxdec.c")
        .include(&include)
        .warnings(true)
        .compile("pxdec");

    println!("cargo:rustc-link-search=native={}", lib.display());
    // Dependency order for static linking: each library comes before the ones it needs.
    for l in ["avformat", "avcodec", "swresample", "avutil"] {
        println!("cargo:rustc-link-lib=static={l}");
    }
    if target.contains("windows") {
        println!("cargo:rustc-link-lib=bcrypt");
    } else {
        println!("cargo:rustc-link-lib=m");
        println!("cargo:rustc-link-lib=pthread");
    }
}
