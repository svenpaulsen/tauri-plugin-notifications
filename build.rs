#[cfg(target_os = "macos")]
use std::{path::PathBuf, process::Command};

const COMMANDS: &[&str] = &[
    "register_listener",
    "remove_listener",
    "notify",
    "request_permission",
    "is_permission_granted",
    "register_for_push_notifications",
    "unregister_for_push_notifications",
    "register_action_types",
    "cancel",
    "cancel_all",
    "get_pending",
    "remove_active",
    "remove_all",
    "get_active",
    "check_permissions",
    "show",
    "batch",
    "list_channels",
    "delete_channel",
    "create_channel",
    "permission_state",
    "set_click_listener_active",
];

fn main() {
    // Native push support is always compiled in; whether push is actually
    // available is decided solely by the Rust-side `push-notifications`
    // feature gates. Communicating the feature state to the native build
    // systems via generated files proved unreliable: the files live in the
    // shared cargo checkout, so builds for different targets clobbered each
    // other, and Xcode/Gradle evaluate their manifests before this script
    // runs on a fresh checkout.
    let result = tauri_plugin::Builder::new(COMMANDS)
        .android_path("android")
        .ios_path("ios")
        .try_build();

    // when building documentation for Android the plugin build result is always Err() and is irrelevant to the crate documentation build
    if !(cfg!(docsrs)
        && std::env::var("TARGET")
            .expect("Failed to get TARGET environment variable")
            .contains("android"))
    {
        result.expect("Failed to build Tauri plugin");
    }

    #[cfg(target_os = "macos")]
    {
        // Only run macOS-specific build steps when building for macOS
        if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() == "macos" {
            // Rebuild when target architecture or deployment target changes
            println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_ARCH");
            println!("cargo:rerun-if-env-changed=MACOSX_DEPLOYMENT_TARGET");

            let bridges = vec!["src/macos.rs"];
            for path in &bridges {
                println!("cargo:rerun-if-changed={path}");
            }

            println!("cargo:rerun-if-changed=macos/Sources/NotificationPlugin.swift");

            swift_bridge_build::parse_bridges(bridges)
                .write_all_concatenated(swift_bridge_out_dir(), env!("CARGO_PKG_NAME"));
            publicize_cdecl_exports(&swift_bridge_out_dir());

            compile_swift();

            println!("cargo:rustc-link-lib=static=tauri-plugin-notifications");
            println!(
                "cargo:rustc-link-search={}",
                swift_library_static_lib_dir()
                    .to_str()
                    .expect("Swift library path must be valid UTF-8")
            );
            if let Some(dir) = swift_toolchain_runtime_dir() {
                println!("cargo:rustc-link-search=native={}", dir.display());
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn compile_swift() {
    let target_triple = swift_target_triple();

    let exit_status = swift_build_command()
        .spawn()
        .expect("Failed to spawn swift build command")
        .wait_with_output()
        .expect("Failed to wait for swift build output");

    assert!(
        exit_status.status.success(),
        r"
Swift build failed for target: {}
Stderr: {}
Stdout: {}
",
        target_triple,
        String::from_utf8(exit_status.stderr).expect("Stderr must be valid UTF-8"),
        String::from_utf8(exit_status.stdout).expect("Stdout must be valid UTF-8"),
    );
}

#[cfg(target_os = "macos")]
fn swift_build_command() -> Command {
    let swift_package_dir = manifest_dir().join("macos");

    let mut cmd = Command::new("swift");

    cmd.current_dir(&swift_package_dir)
        .arg("build")
        // Build into OUT_DIR (under target/) instead of the default `.build`
        // inside the crate source. Source-tree writes don't survive a clean
        // registry re-extraction / cache restore, which leaves cargo's
        // fingerprint saying "built" while the linked artifact is gone.
        .args([
            "--scratch-path",
            swift_build_dir()
                .to_str()
                .expect("Swift build path must be valid UTF-8"),
        ])
        .args(["--triple", &swift_target_triple()])
        .args([
            "-Xswiftc",
            "-import-objc-header",
            "-Xswiftc",
            swift_source_dir()
                .join("bridging-header.h")
                .to_str()
                .expect("Bridging header path must be valid UTF-8"),
        ]);

    if is_release_build() {
        cmd.args(["-c", "release"]);
    }

    cmd
}

/// swift-bridge emits its `@_cdecl` entry points as internal `func`s. Up to
/// Xcode 26 the compiler still exported them; Xcode 27's Swift internalizes
/// internal `@_cdecl` symbols in release builds (nm shows local `t`), so the
/// Rust side fails to link with "undefined symbols". Declaring them `public`
/// is the supported way to keep them exported, so patch the generated code.
#[cfg(target_os = "macos")]
fn publicize_cdecl_exports(dir: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            publicize_cdecl_exports(&path);
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("swift") {
            continue;
        }
        let Ok(source) = std::fs::read_to_string(&path) else {
            continue;
        };
        let mut out = String::with_capacity(source.len());
        let mut after_cdecl = false;
        let mut changed = false;
        for line in source.split_inclusive('\n') {
            if after_cdecl && line.starts_with("func ") {
                out.push_str("public ");
                changed = true;
            }
            after_cdecl = line.trim_start().starts_with("@_cdecl(");
            out.push_str(line);
        }
        if changed {
            std::fs::write(&path, out).expect("Failed to rewrite generated Swift bridge code");
        }
    }
}

/// The toolchain's static Swift runtime directory
/// (`…/XcodeDefault.xctoolchain/usr/lib/swift/macosx`). With a deployment
/// target below macOS 13 the compiled module auto-links the
/// `swiftCompatibility56` / `swiftCompatibilityPacks` shims that live only
/// there; Rust's final link knows `/usr/lib/swift` but not this directory,
/// so without it the app fails on `__swift_FORCE_LOAD_$_swiftCompatibility56`.
/// Same directory swift-rs adds for its packages.
#[cfg(target_os = "macos")]
fn swift_toolchain_runtime_dir() -> Option<PathBuf> {
    let output = Command::new("xcrun")
        .args(["--find", "swift"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let swift = PathBuf::from(String::from_utf8(output.stdout).ok()?.trim());
    // <toolchain>/usr/bin/swift → <toolchain>/usr/lib/swift/macosx
    let dir = swift.parent()?.parent()?.join("lib/swift/macosx");
    dir.is_dir().then_some(dir)
}

#[cfg(target_os = "macos")]
fn swift_bridge_out_dir() -> PathBuf {
    generated_code_dir()
}

#[cfg(target_os = "macos")]
fn manifest_dir() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR must be set");
    PathBuf::from(manifest_dir)
}

#[cfg(target_os = "macos")]
fn out_dir() -> PathBuf {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR must be set");
    PathBuf::from(out_dir)
}

/// `SwiftPM` scratch (build) directory, under `OUT_DIR` so it lives in `target/`
/// and is covered by cargo's fingerprint and any build cache.
#[cfg(target_os = "macos")]
fn swift_build_dir() -> PathBuf {
    out_dir().join("swift-build")
}

#[cfg(target_os = "macos")]
fn is_release_build() -> bool {
    std::env::var("PROFILE").expect("PROFILE must be set") == "release"
}

#[cfg(target_os = "macos")]
fn swift_source_dir() -> PathBuf {
    manifest_dir().join("macos/Sources")
}

#[cfg(target_os = "macos")]
fn generated_code_dir() -> PathBuf {
    swift_source_dir().join("generated")
}

#[cfg(target_os = "macos")]
fn target_arch() -> String {
    std::env::var("CARGO_CFG_TARGET_ARCH").expect("CARGO_CFG_TARGET_ARCH must be set")
}

#[cfg(target_os = "macos")]
fn swift_arch() -> &'static str {
    match target_arch().as_str() {
        "aarch64" => "arm64",
        "x86_64" => "x86_64",
        arch => panic!("Unsupported architecture for macOS: {arch}"),
    }
}

/// Lowest macOS the Swift package builds for; matches `macos/Package.swift`.
#[cfg(target_os = "macos")]
const MACOS_MIN_DEPLOYMENT_TARGET: (u32, u32) = (12, 0);

/// `MACOSX_DEPLOYMENT_TARGET` as the Tauri CLI exports it from the app's
/// `bundle.macOS.minimumSystemVersion`, raised to the package floor: Tauri
/// defaults that setting to 10.13, and Xcode 27's Swift toolchain refuses
/// any target below 12.0 outright instead of warning.
#[cfg(target_os = "macos")]
fn macos_deployment_target() -> String {
    let (min_major, min_minor) = MACOS_MIN_DEPLOYMENT_TARGET;
    let floor = format!("{min_major}.{min_minor}");
    let Ok(requested) = std::env::var("MACOSX_DEPLOYMENT_TARGET") else {
        return floor;
    };
    let mut parts = requested.trim().split('.').map(|p| p.parse::<u32>().ok());
    let (Some(Some(major)), minor) = (parts.next(), parts.next().flatten().unwrap_or(0)) else {
        println!(
            "cargo:warning=MACOSX_DEPLOYMENT_TARGET={requested:?} is not a version; using {floor}"
        );
        return floor;
    };
    if (major, minor) < (min_major, min_minor) {
        println!(
            "cargo:warning=MACOSX_DEPLOYMENT_TARGET={requested} is below the {floor} this plugin's Swift package supports; building for {floor}"
        );
        return floor;
    }
    requested.trim().to_string()
}

#[cfg(target_os = "macos")]
fn swift_target_triple() -> String {
    format!("{}-apple-macosx{}", swift_arch(), macos_deployment_target())
}

#[cfg(target_os = "macos")]
fn swift_library_static_lib_dir() -> PathBuf {
    let output = swift_build_command()
        .arg("--show-bin-path")
        .output()
        .expect("Failed to run swift build --show-bin-path");

    assert!(
        output.status.success(),
        r"
swift build --show-bin-path failed for target: {}
Stderr: {}
Stdout: {}
",
        swift_target_triple(),
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout),
    );

    let bin_path = String::from_utf8(output.stdout)
        .expect("swift build --show-bin-path output must be valid UTF-8");
    let bin_path = bin_path.trim();
    assert!(
        !bin_path.is_empty(),
        "swift build --show-bin-path printed an empty path"
    );

    PathBuf::from(bin_path)
}
