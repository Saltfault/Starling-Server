//! Locate or download the target-specific static Opus library and bindings.

use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const OPUS_VERSION: &str = "2026.1.0";

fn main() {
    println!("cargo:rerun-if-env-changed=LIBOPUS_LIB_DIR");
    println!("cargo:rerun-if-env-changed=LIBOPUS_BINDINGS_PATH");

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR not set"));
    let lib_dir = out_dir.join("opus_lib");
    let bindings_path = out_dir.join("opus_bindings.rs");

    if let Some(system_lib_dir) = env::var_os("LIBOPUS_LIB_DIR") {
        let supplied_bindings = env::var_os("LIBOPUS_BINDINGS_PATH")
            .expect("LIBOPUS_BINDINGS_PATH must be set when LIBOPUS_LIB_DIR is used");
        fs::copy(supplied_bindings, &bindings_path).expect("failed to copy LIBOPUS_BINDINGS_PATH");
        link_opus(Path::new(&system_lib_dir));
        return;
    }

    if lib_dir.exists() && bindings_path.exists() {
        link_opus(&lib_dir);
        return;
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS").expect("target OS not set by Cargo");
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").expect("target arch not set by Cargo");
    let asset_target = match (target_os.as_str(), target_arch.as_str()) {
        ("windows", "x86_64") => "windows_x86_64",
        ("linux", "x86_64") => "ubuntu-24.04_x86_64",
        ("linux", "aarch64") => "ubuntu-24.04_arm64",
        ("macos", "aarch64") => "macos_arm64",
        _ => panic!(
            "unsupported Opus target {target_arch}-{target_os}; set LIBOPUS_LIB_DIR and LIBOPUS_BINDINGS_PATH"
        ),
    };
    let lib_file = if target_os == "windows" {
        "opus.lib"
    } else {
        "libopus.a"
    };
    let url = format!(
        "https://github.com/shiguredo/opus-rs/releases/download/{OPUS_VERSION}/libopus-{asset_target}.tar.gz"
    );

    let archive_path = out_dir.join("libopus.tar.gz");
    let partial_path = out_dir.join("libopus.tar.gz.part");
    eprintln!("Downloading pre-built Opus: {url}");
    let status = Command::new("curl")
        .args([
            "--fail",
            "--show-error",
            "--location",
            "--proto",
            "=https",
            "--tlsv1.2",
            "--retry",
            "3",
            "--output",
        ])
        .arg(&partial_path)
        .arg(&url)
        .status()
        .expect(
            "failed to run curl; install curl or set LIBOPUS_LIB_DIR and LIBOPUS_BINDINGS_PATH",
        );
    if !status.success() {
        let _ = fs::remove_file(&partial_path);
        panic!("failed to download pre-built Opus from {url}");
    }
    fs::rename(&partial_path, &archive_path).expect("failed to finalize Opus download");

    validate_archive(&archive_path);
    let extract_dir = out_dir.join("opus_extract");
    let _ = fs::remove_dir_all(&extract_dir);
    fs::create_dir_all(&extract_dir).expect("failed to create Opus extraction directory");
    let status = Command::new("tar")
        .arg("xzf")
        .arg(&archive_path)
        .arg("-C")
        .arg(&extract_dir)
        .status()
        .expect("failed to run tar; install tar or set LIBOPUS_LIB_DIR and LIBOPUS_BINDINGS_PATH");
    if !status.success() {
        panic!("failed to extract Opus archive");
    }

    fs::create_dir_all(&lib_dir).expect("failed to create Opus library directory");
    let source_library = extract_dir.join("lib").join(lib_file);
    fs::copy(&source_library, lib_dir.join(lib_file)).unwrap_or_else(|error| {
        panic!(
            "failed to copy {} from the Opus archive: {error}",
            source_library.display()
        )
    });
    fs::copy(extract_dir.join("bindings.rs"), &bindings_path)
        .expect("failed to copy Opus bindings from archive");

    link_opus(&lib_dir);
}

fn validate_archive(archive_path: &Path) {
    let output = Command::new("tar")
        .arg("tzf")
        .arg(archive_path)
        .output()
        .expect("failed to inspect Opus archive");
    if !output.status.success() {
        panic!("downloaded Opus archive is invalid");
    }
    let listing = String::from_utf8(output.stdout).expect("Opus archive contains invalid paths");
    for entry in listing.lines() {
        let path = Path::new(entry);
        if path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
        {
            panic!("unsafe path in Opus archive: {entry}");
        }
    }
}

fn link_opus(lib_dir: &Path) {
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=static=opus");
}
