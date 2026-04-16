use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/provider/apple_intelligence_bridge.swift");

    if env::var_os("CARGO_FEATURE_APPLE_INTELLIGENCE").is_none() {
        return;
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    if target_os != "macos" || target_arch != "aarch64" {
        panic!(
            "the `apple-intelligence` feature requires the aarch64-apple-darwin target \
             (got target_os={target_os}, target_arch={target_arch})",
        );
    }

    // Locate the macOS SDK via xcrun. If xcrun is missing, the user needs Xcode CLT.
    let sdk_path_out = Command::new("xcrun")
        .args(["-sdk", "macosx", "--show-sdk-path"])
        .output()
        .expect(
            "failed to invoke `xcrun`; install the Xcode Command Line Tools \
             (`xcode-select --install`) to build the apple-intelligence feature",
        );
    if !sdk_path_out.status.success() {
        panic!(
            "`xcrun -sdk macosx --show-sdk-path` failed: {}",
            String::from_utf8_lossy(&sdk_path_out.stderr)
        );
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let lib_path = out_dir.join("libapple_intelligence_bridge.a");
    let src_path = "src/provider/apple_intelligence_bridge.swift";

    let status = Command::new("xcrun")
        .args([
            "-sdk",
            "macosx",
            "swiftc",
            "-emit-library",
            "-static",
            "-parse-as-library",
            "-O",
            "-target",
            "arm64-apple-macos26.0",
            "-module-name",
            "AppleIntelligenceBridge",
            "-o",
        ])
        .arg(&lib_path)
        .arg(src_path)
        .status()
        .expect("failed to invoke `xcrun swiftc`");

    if !status.success() {
        panic!("swiftc failed to build the Apple Intelligence bridge");
    }

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=apple_intelligence_bridge");
    println!("cargo:rustc-link-lib=framework=FoundationModels");
    println!("cargo:rustc-link-lib=framework=Foundation");

    // Pull in the system Swift runtime (ABI-stable since macOS 10.14.4).
    println!("cargo:rustc-link-search=/usr/lib/swift");
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
}
