//! Extension directory overrides (`--base`, `RUBYC_BASE`, per-kind dirs) and
//! `--target` selection, exercised through the real binary.
//!
//! These keep P1.5/P1.6 testable without `~/.local/share/rubyc` (decision D7).

use std::process::Command;

fn rubyc() -> Command {
    let mut cmd = Command::new(crate::tests::common::rubyc_path());
    cmd.current_dir(std::env::temp_dir());
    cmd
}

fn run(args: &[&str], env: &[(&str, &std::path::Path)]) -> (i32, String, String) {
    let mut cmd = rubyc();
    for (key, value) in env {
        cmd.env(key, *value);
    }
    let out = cmd.args(args).output().expect("spawn failed");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn temp_dir(label: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!("rubyc_{label}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn write_program(dir: &std::path::Path) -> std::path::PathBuf {
    let path = dir.join("hello.rc");
    std::fs::write(&path, "namespace t; class c { int main() { printf(\"ok\"); } }").unwrap();
    path
}

const TARGET_MANIFEST: &str = "[target]\nname = \"wasm32\"\ndisplay-name = \"WebAssembly\"\nauthor = \"Test Author\"\nversion = \"9.9\"\ndescription = \"test target\"\ntriple = \"wasm32-unknown-unknown\"\n";

// ---------- --base ----------

#[test]
fn base_dir_lists_installed_targets() {
    let base = temp_dir("cli_base_targets");
    std::fs::create_dir_all(base.join("targets")).unwrap();
    std::fs::write(base.join("targets/wasm.toml"), TARGET_MANIFEST).unwrap();

    let (code, stdout, stderr) = run(
        &["--list-targets", "--base", base.to_str().unwrap()],
        &[],
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("wasm32"), "stdout: {stdout}");
    assert!(stdout.contains("Test Author"), "stdout: {stdout}");
    assert!(stdout.contains("linux_x86_64"), "builtins must remain: {stdout}");
    let _ = std::fs::remove_dir_all(base);
}

// ---------- RUBYC_BASE / RUBYC_TARGETS_DIR ----------

#[test]
fn env_rubyc_base_is_discovered() {
    let base = temp_dir("cli_env_base");
    std::fs::create_dir_all(base.join("targets")).unwrap();
    std::fs::write(base.join("targets/wasm.toml"), TARGET_MANIFEST).unwrap();

    let (code, stdout, stderr) = run(&["--list-targets"], &[("RUBYC_BASE", &base)]);
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("wasm32"), "stdout: {stdout}");
    let _ = std::fs::remove_dir_all(base);
}

#[test]
fn env_kind_dir_wins_over_env_base() {
    let base = temp_dir("cli_env_base_lose");
    let other = temp_dir("cli_env_other_win");
    std::fs::create_dir_all(base.join("targets")).unwrap();
    std::fs::write(base.join("targets/from_base.toml"), "[target]\nname = \"from_base\"\nversion = \"1\"\n").unwrap();
    std::fs::write(other.join("from_other.toml"), "[target]\nname = \"from_other\"\nversion = \"1\"\n").unwrap();

    let (code, stdout, stderr) = run(
        &["--list-targets"],
        &[("RUBYC_BASE", &base), ("RUBYC_TARGETS_DIR", &other)],
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    assert!(stdout.contains("from_other"), "stdout: {stdout}");
    assert!(!stdout.contains("from_base"), "stdout: {stdout}");
    let _ = std::fs::remove_dir_all(base);
    let _ = std::fs::remove_dir_all(other);
}

// ---------- --target ----------

#[test]
fn unknown_target_is_rejected() {
    let dir = temp_dir("cli_target_bogus");
    let prog = write_program(&dir);
    let (code, _stdout, stderr) = run(&["build", prog.to_str().unwrap(), "--target", "bogus"], &[]);
    assert_eq!(code, 1, "unknown target must fail");
    assert!(stderr.contains("bogus"), "stderr: {stderr}");
    assert!(stderr.contains("linux_x86_64"), "must list available targets: {stderr}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn recognized_target_without_backend_is_rejected() {
    let base = temp_dir("cli_target_no_backend");
    std::fs::create_dir_all(base.join("targets")).unwrap();
    std::fs::write(base.join("targets/wasm.toml"), TARGET_MANIFEST).unwrap();
    let dir = temp_dir("cli_target_no_backend_src");
    let prog = write_program(&dir);

    let (code, _stdout, stderr) = run(
        &["--base", base.to_str().unwrap(), "build", prog.to_str().unwrap(), "--target", "wasm32"],
        &[],
    );
    assert_eq!(code, 1, "wasm32 has no loadable backend yet");
    assert!(stderr.contains("wasm32"), "stderr: {stderr}");
    let _ = std::fs::remove_dir_all(base);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn host_target_flag_is_accepted() {
    let dir = temp_dir("cli_target_host");
    let prog = write_program(&dir);
    let (code, _stdout, stderr) = run(
        &["build", prog.to_str().unwrap(), "--target", "linux_x86_64"],
        &[],
    );
    assert_eq!(code, 0, "stderr: {stderr}");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn mismatched_target_fails_loudly() {
    // The windows_x86_64 stub installs for discovery but has no backend:
    // selecting it must name both targets, never silently build Linux.
    let dir = temp_dir("cli_target_mismatch");
    let prog = write_program(&dir);
    let (code, _stdout, stderr) = run(
        &["build", prog.to_str().unwrap(), "--target", "windows_x86_64"],
        &[],
    );
    assert_eq!(code, 1, "windows stub must refuse: {stderr}");
    assert!(stderr.contains("windows_x86_64"), "stderr: {stderr}");
    assert!(stderr.contains("linux_x86_64"), "stderr: {stderr}");
    let _ = std::fs::remove_dir_all(dir);
}

// ---------- help ----------

#[test]
fn help_mentions_extension_flags() {
    let (code, stdout, stderr) = run(&["--help"], &[]);
    assert_eq!(code, 0, "stderr: {stderr}");
    for flag in ["--base", "--targets-dir", "--target", "--list-targets"] {
        assert!(stdout.contains(flag), "help must mention {flag}:\n{stdout}");
    }
}
