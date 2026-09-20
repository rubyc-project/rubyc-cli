//! End-to-end test: a RubyC program compiles through the target plugin ABI
//! (target `.so`), not through statically-linked backends.
//!
//! The language frontend is in-process (built into `rubyc-core`); only the
//! target is a dynamic plugin. The test stages the real target cdylib into a
//! `RUBYC_BASE` tree, points the `rubyc` binary at it, and verifies the
//! program builds and runs. This exercises the dynamic path: AST (in-process)
//! -> IR -> target plugin -> machine code -> executable image.

use std::path::{Path, PathBuf};

/// Locate the target plugin `.so` artifact produced by the workspace build.
///
/// Falls back from the release profile to the debug profile, then to the
/// workspace root `target/` directory.
fn plugin_so(crate_name: &str) -> Option<PathBuf> {
    let profile = |dir: &Path| dir.join(format!("lib{crate_name}.so"));
    // The test harness lives under target/<profile>/deps; the `.so` siblings
    // sit one level up in target/<profile>/. Return an absolute path.
    let exe = std::env::current_exe().ok()?;
    if let Some(profile_dir) = exe.ancestors().find(|d| d.join("deps").is_dir()) {
        let p = profile_dir.canonicalize().ok()?;
        let p = profile(&p);
        if p.is_file() {
            return Some(p);
        }
    }
    // Walk up to the workspace root and check both profiles.
    if let Some(root) = exe.ancestors().find(|d| d.join("Cargo.toml").is_file()) {
        let root = root.canonicalize().ok()?;
        for prof in ["release", "debug"] {
            let p = root.join("target").join(prof).join(format!("lib{crate_name}.so"));
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// Stage the target plugin into a fresh `RUBYC_BASE` tree.
/// The `tag` parameter disambiguates concurrent tests (same PID, different tests).
fn stage_target(tag: &str) -> Option<PathBuf> {
    let tgt = plugin_so("rubyc_target_linux_x86_64")?;

    let base = std::env::temp_dir().join(format!("rubyc-plugin-e2e-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let tgt_dir = base.join("targets").join("linux_x86_64");
    std::fs::create_dir_all(&tgt_dir).ok()?;

    // Symlink the `.so` file so rebuilds are picked up automatically.
    let tgt_link = tgt_dir.join("librubyc_target_linux_x86_64.so");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&tgt, &tgt_link).ok()?;
    }
    #[cfg(not(unix))]
    {
        std::fs::copy(&tgt, &tgt_link).ok()?;
    }

    // Minimal manifest so the loader can discover the plugin.
    std::fs::write(
        tgt_dir.join("linux_x86_64.toml"),
        "name = \"linux_x86_64\"\nversion = \"0.1.0\"\ntriple = \"x86_64-unknown-linux-gnu\"\nlibrary = \"librubyc_target_linux_x86_64.so\"\n",
    )
    .ok()?;

    Some(base)
}

#[test]
fn e2e_compile_via_target_plugin() {
    let base = stage_target("compile")
        .expect("target plugin .so artifact not found — run `cargo build --release` first");
    let out_dir = std::env::temp_dir().join(format!("rubyc-plugin-e2e-out-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).unwrap();
    let out = out_dir.join("hello");

    let src_dir = std::env::temp_dir().join(format!("rubyc-plugin-e2e-src-{}", std::process::id()));
    std::fs::create_dir_all(&src_dir).unwrap();
    let src = src_dir.join("main.ruby");
    std::fs::write(
        &src,
        "namespace test\n{\n    class Main\n    {\n        int main()\n        {\n            return 42;\n        }\n    }\n}\n",
    )
    .unwrap();

    let mut cmd = crate::tests::common::rubyc();
    cmd.arg("build")
        .arg("--target")
        .arg("linux_x86_64")
        .arg(&src)
        .arg("-o")
        .arg(&out)
        .env("RUBYC_BASE", &base);
    let result = crate::tests::common::exec(&mut cmd);

    assert!(
        result.code == 0,
        "target plugin build failed (exit {}): stderr = {}",
        result.code,
        result.stderr_str()
    );

    // The produced image must be a real ELF executable.
    let image = std::fs::read(&out).unwrap_or_else(|_| {
        panic!("output image not written: {:?}", out);
    });
    assert!(
        image.len() >= 4 && &image[0..4] == &[0x7f, b'E', b'L', b'F'],
        "output is not an ELF image ({} bytes)",
        image.len()
    );

    // Clean up after the assertions have read the output.
    let _ = std::fs::remove_dir_all(&base);
    let _ = std::fs::remove_dir_all(&out_dir);
    let _ = std::fs::remove_dir_all(&src_dir);
}

/// A program that prints a known value, for JIT run verification.
const RUN_SRC: &str = r#"
namespace test
{
    class Main
    {
        int main()
        {
            int x = 0;
            x += 40;
            x += 2;
            printf("{0}", x);
            return 0;
        }
    }
}
"#;

#[test]
fn e2e_run_via_target_plugin() {
    let base = stage_target("run")
        .expect("target plugin .so artifact not found — run `cargo build --release` first");
    let src_dir = std::env::temp_dir().join(format!("rubyc-plugin-e2e-run-src-{}", std::process::id()));
    std::fs::create_dir_all(&src_dir).unwrap();
    let src = src_dir.join("main.ruby");
    std::fs::write(&src, RUN_SRC).unwrap();

    let mut cmd = crate::tests::common::rubyc();
    cmd.arg("run")
        .arg("--target")
        .arg("linux_x86_64")
        .arg(&src)
        .env("RUBYC_BASE", &base);
    let result = crate::tests::common::exec(&mut cmd);

    assert!(
        result.code == 0,
        "target plugin run failed (exit {}): stderr = {}",
        result.code,
        result.stderr_str()
    );
    assert!(
        !result.timed_out,
        "target plugin run timed out"
    );
    assert_eq!(
        result.stdout_str().trim(),
        "42",
        "expected '42' on stdout, got '{}'",
        result.stdout_str()
    );

    let _ = std::fs::remove_dir_all(&base);
    let _ = std::fs::remove_dir_all(&src_dir);
}

#[test]
fn e2e_bytecode_via_target_plugin() {
    let base = stage_target("bytecode")
        .expect("target plugin .so artifact not found — run `cargo build --release` first");
    let src_dir = std::env::temp_dir().join(format!("rubyc-plugin-e2e-bc-src-{}", std::process::id()));
    std::fs::create_dir_all(&src_dir).unwrap();
    let src = src_dir.join("main.ruby");
    std::fs::write(
        &src,
        "namespace test\n{\n    class Main\n    {\n        int main()\n        {\n            return 0;\n        }\n    }\n}\n",
    )
    .unwrap();

    let out_dir = std::env::temp_dir().join(format!("rubyc-plugin-e2e-bc-out-{}", std::process::id()));
    std::fs::create_dir_all(&out_dir).unwrap();
    let out = out_dir.join("main.bc");

    let mut cmd = crate::tests::common::rubyc();
    cmd.arg("build")
        .arg("--target")
        .arg("linux_x86_64")
        .arg(&src)
        .arg("-O")
        .arg("bytecode")
        .arg("-o")
        .arg(&out)
        .env("RUBYC_BASE", &base);
    let result = crate::tests::common::exec(&mut cmd);

    assert!(
        result.code == 0,
        "target plugin bytecode build failed (exit {}): stderr = {}",
        result.code,
        result.stderr_str()
    );

    let bc = std::fs::read(&out).unwrap_or_else(|_| {
        panic!("bytecode output not written: {:?}", out);
    });
    assert!(
        bc.len() > 4,
        "bytecode output too small ({} bytes)",
        bc.len()
    );

    let _ = std::fs::remove_dir_all(&base);
    let _ = std::fs::remove_dir_all(&out_dir);
    let _ = std::fs::remove_dir_all(&src_dir);
}

#[test]
fn e2e_shared_via_target_plugin() {
    let base = stage_target("shared")
        .expect("target plugin .so artifact not found — run `cargo build --release` first");
    let src_dir = std::env::temp_dir().join(format!("rubyc-plugin-e2e-sh-src-{}", std::process::id()));
    std::fs::create_dir_all(&src_dir).unwrap();
    let src = src_dir.join("main.ruby");
    std::fs::write(
        &src,
        "namespace test\n{\n    class Main\n    {\n        int main()\n        {\n            return 0;\n        }\n    }\n}\n",
    )
    .unwrap();

    let out_dir = std::env::temp_dir().join(format!("rubyc-plugin-e2e-sh-out-{}", std::process::id()));
    std::fs::create_dir_all(&out_dir).unwrap();
    let out = out_dir.join("libmain.so");

    let mut cmd = crate::tests::common::rubyc();
    cmd.arg("build")
        .arg("--target")
        .arg("linux_x86_64")
        .arg(&src)
        .arg("-O")
        .arg("shared")
        .arg("-o")
        .arg(&out)
        .env("RUBYC_BASE", &base);
    let result = crate::tests::common::exec(&mut cmd);

    assert!(
        result.code == 0,
        "target plugin shared build failed (exit {}): stderr = {}",
        result.code,
        result.stderr_str()
    );

    let image = std::fs::read(&out).unwrap_or_else(|_| {
        panic!("shared output not written: {:?}", out);
    });
    assert!(
        image.len() >= 4 && &image[0..4] == &[0x7f, b'E', b'L', b'F'],
        "shared output is not an ELF image ({} bytes)",
        image.len()
    );

    let _ = std::fs::remove_dir_all(&base);
    let _ = std::fs::remove_dir_all(&out_dir);
    let _ = std::fs::remove_dir_all(&src_dir);
}
