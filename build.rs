// build.rs — C glue + FFmpeg 静态链接
// cc crate 仅在 FFI 模式需要

#[cfg(feature = "ffi-ffmpeg")]
use cc;

#[cfg(feature = "ffi-ffmpeg")]
use std::{env, path::PathBuf, process};

#[cfg(feature = "ffi-ffmpeg")]
fn get_platform_id() -> String {
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    format!("{}-{}", os, arch)
}

fn main() {
    #[cfg(feature = "ffi-ffmpeg")]
    build_ffi();

    #[cfg(feature = "external-ffmpeg")]
    println!("cargo:warning=external-ffmpeg mode — skipping FFmpeg link");
}

#[cfg(feature = "ffi-ffmpeg")]
fn build_ffi() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let platform_id = get_platform_id();
    let platform_dir = manifest_dir.join("ffbuild").join(&platform_id);

    if !platform_dir.exists() {
        eprintln!("ERROR: FFmpeg not found: {}", platform_dir.display());
        process::exit(1);
    }

    let lib_dir = platform_dir.join("lib");
    let include_dir = platform_dir.join("include");
    let ffmpeg_src = manifest_dir.join("ffmpeg");

    // ── 编译 C glue ──
    let mut cc = cc::Build::new();
    cc.file("src/ffmpeg/glue.c");
    if include_dir.exists() { cc.include(&include_dir); }
    if ffmpeg_src.exists() { cc.include(&ffmpeg_src); }
    cc.compile("ffmpeg_glue");

    // ── 链接 FFmpeg 静态库 ──
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static=avformat");
    println!("cargo:rustc-link-lib=static=avcodec");
    println!("cargo:rustc-link-lib=static=avutil");

    // ── 系统库 ──
    match platform_id.as_str() {
        "macos-aarch64" | "macos-x86_64" => {
            for fw in &["CoreFoundation","CoreVideo","CoreMedia","AVFoundation","VideoToolbox","AudioToolbox"] {
                println!("cargo:rustc-link-lib=framework={}", fw);
            }
        }
        "linux-x86_64" | "linux-aarch64" => {}
        "windows-x86_64" => {
            for l in &["ws2_32","strmiids","ole32","vfw32","secur32","bcrypt"] {
                println!("cargo:rustc-link-lib={}", l);
            }
        }
        _ => eprintln!("[WARN] unsupported platform: {}", platform_id),
    }
}
