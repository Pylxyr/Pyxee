//! Compiles the C decode shim against the minimal static `FFmpeg` build and links it.
//!
//! The libav* static libraries come from `cargo xtask ffmpeg` (see docs/engine.md). Point
//! `PYXEE_FFMPEG_DIR` at the install prefix (it holds `include/` and `lib/`); without it the
//! default is `<workspace>/target/ffmpeg/<target triple>`.

// Pedantic lints this project does not follow, allowed once per crate root. (A workspace-level allow loses to the
// `-D clippy::pedantic` that is passed on the command line, so it has to be an attribute.) Each is a trade-off for
// application code: `# Errors` / `# Panics` sections and `#[must_use]` on internal items, long match-heavy functions,
// flag structs, short names in numeric code, and the casts between sample counts, milliseconds and pixels (their
// ranges are bounded by what the device and the files can hold) are not worth the noise.
#![allow(
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::return_self_not_must_use,
    clippy::too_many_lines,
    clippy::struct_excessive_bools,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss
)]

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=PYXEE_FFMPEG_DIR");
    println!("cargo:rerun-if-changed=csrc/pxdec.c");
    println!("cargo:rerun-if-changed=csrc/pxdec.h");

    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets this"));
    let target = std::env::var("TARGET").expect("cargo sets this");
    let prefix = std::env::var_os("PYXEE_FFMPEG_DIR").map_or_else(
        || manifest.join("../../target/ffmpeg").join(&target),
        PathBuf::from,
    );
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
