//! Build-time platform selection for the CLI crate.
//!
//! Mirrors [`rubyc-core/build.rs`](../../rubyc-core/build.rs): emits the
//! same `target_os_<name>` check-cfg declarations plus the selected host
//! flag, so `tests/rubyc-cli/native_target_os_<name>.rs` suites gate correctly
//! (`#if LINUX_X86_64`-style) instead of compiling empty.

use std::env;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let names: &[&str] = &[
        "linux_x86_64",
        "linux_arm64",
        "macos_arm64",
        "macos_x64",
        "windows_x64",
        "windows_arm64",
        "android_arm64",
        "ios_arm64",
    ];
    for name in names {
        println!("cargo:rustc-check-cfg=cfg(target_os_{name})");
    }

    let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let selected = match (os.as_str(), arch.as_str()) {
        ("linux", "x86_64") => Some("linux_x86_64"),
        ("linux", "aarch64") => Some("linux_arm64"),
        ("macos", "aarch64") => Some("macos_arm64"),
        ("macos", "x86_64") => Some("macos_x64"),
        ("windows", "x86_64") => Some("windows_x64"),
        ("windows", "aarch64") => Some("windows_arm64"),
        ("android", _) => Some("android_arm64"),
        ("ios", _) => Some("ios_arm64"),
        _ => None,
    };
    if let Some(name) = selected {
        println!("cargo:rustc-cfg=target_os_{name}");
    }

    // Install the plugin `.so`s into the per-OS plugin directory. This crate is
    // built after every plugin (it depends on them), so their artifacts already
    // exist in the target dir when this runs. Failures are non-fatal: a missing
    // artifact just means the plugin was not rebuilt this pass.
    install_plugins();
}

/// Per-OS plugin install root, mirroring `rubyc_core::extensions::install_root()`.
/// Precedence: `RUBYC_BASE` > `RUBYC_HOME` > platform default.
fn install_root() -> std::path::PathBuf {
    if let Some(path) = std::env::var_os("RUBYC_BASE") {
        return path.into();
    }
    if let Some(path) = std::env::var_os("RUBYC_HOME") {
        return path.into();
    }
    #[cfg(target_os = "windows")]
    if let Some(path) = std::env::var_os("LOCALAPPDATA") {
        return std::path::PathBuf::from(path).join("rubyc");
    }
    #[cfg(target_os = "macos")]
    if let Some(path) = std::env::var_os("HOME") {
        return std::path::PathBuf::from(path).join("Library/Application Support/rubyc");
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
            let xdg = std::path::PathBuf::from(xdg);
            return if xdg.is_absolute() {
                xdg.join("rubyc")
            } else if let Some(home) = std::env::var_os("HOME") {
                std::path::PathBuf::from(home).join(xdg).join("rubyc")
            } else {
                xdg.join("rubyc")
            };
        }
        if let Some(path) = std::env::var_os("HOME") {
            return std::path::PathBuf::from(path).join(".local/share/rubyc");
        }
    }
    std::path::PathBuf::from(".local/share/rubyc")
}

/// Copy each built target plugin `.so` into `<install_root>/targets/<name>/`
/// and write a discovery manifest next to it.
fn install_plugins() {
    let target_dir = env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| {
        // CARGO_MANIFEST_DIR is the workspace root here (the cli crate).
        let manifest = env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
        format!("{manifest}/target")
    });
    let profile = env::var("CARGO_PROFILE_NAME").unwrap_or_else(|_| "debug".into());
    let profile_dir = std::path::PathBuf::from(&target_dir).join(&profile);

    let so_ext = if cfg!(target_os = "windows") {
        "dll"
    } else if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    };
    let artifact = |name: &str| profile_dir.join(format!("lib{name}.{so_ext}"));

    let plugins: &[(&str, &str, &str, &str, &str)] = &[
        // (crate_name, plugin_name, triple, display_name, description)
        (
            "rubyc_target_linux_x86_64",
            "linux_x86_64",
            "x86_64-unknown-linux-gnu",
            "Linux x86-64",
            "Static ELF and in-process JIT backend",
        ),
        (
            "rubyc_target_windows_x86_64",
            "windows_x86_64",
            "x86_64-pc-windows-msvc",
            "Windows x86-64",
            "Not implemented",
        ),
    ];

    let root = install_root();
    for (crate_name, plugin_name, triple, display_name, description) in plugins {
        let (crate_name, plugin_name, triple, display_name, description) =
            (*crate_name, *plugin_name, *triple, *display_name, *description);
        let src = artifact(crate_name);
        if !src.is_file() {
            // Not built this pass (e.g. `cargo build -p rubyc-cli` only). Skip.
            continue;
        }
        let dest_dir = root.join("targets").join(plugin_name);
        if std::fs::create_dir_all(&dest_dir).is_err() {
            continue;
        }
        let dest = dest_dir.join(format!("lib{crate_name}.{so_ext}"));
        // Replace any previous artifact (including a stale symlink) so the
        // copy never fails on an existing destination.
        let _ = std::fs::remove_file(&dest);
        if std::fs::copy(&src, &dest).is_err() {
            continue;
        }

        let version = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.1.0".into());
        let manifest = format!(
            "[target]\nname = \"{plugin_name}\"\nversion = \"{version}\"\ntriple = \"{triple}\"\nlibrary = \"lib{crate_name}.{so_ext}\"\ndisplay-name = \"{plugin_name}\"\nauthor = \"RubyC contributors\"\ndescription = \"{description}\"\nlicense = \"MIT\"\n"
        );
        let _ = std::fs::write(dest_dir.join(format!("{plugin_name}.toml")), manifest);
    }
}
