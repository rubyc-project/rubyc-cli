//! End-to-end CLI tests: spawn the real binary with piped stdio and verify
//! every documented flow works.

use std::io::Write;
use std::process::{Command, Stdio};

const SOURCE: &str = "namespace t; class c { int main() { printf(\"pipe ok\"); } }";

fn rubyc() -> Command {
    // The compiler binary of this very package.
    let mut cmd = Command::new(crate::tests::common::rubyc_path());
    cmd.current_dir(std::env::temp_dir());
    cmd
}

fn run_with_stdin(args: &[&str], stdin_data: &[u8]) -> (i32, Vec<u8>, Vec<u8>) {
    let mut child = rubyc()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn failed");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(stdin_data)
        .expect("write stdin failed");
    let out = child.wait_with_output().expect("wait failed");
    (out.status.code().unwrap_or(-1), out.stdout, out.stderr)
}

#[test]
fn source_piped_to_compile_produces_bytecode_on_stdout() {
    let (code, stdout, stderr) = run_with_stdin(&["compile", "-O", "bytecode"], SOURCE.as_bytes());
    assert_eq!(code, 0, "stderr: {}", String::from_utf8_lossy(&stderr));
    // [EB 08][dog][kind=bytecode][version]
    assert_eq!(&stdout[..2], &[0xEB, 0x08]);
    assert_eq!(stdout[10], 0x01);
}

#[test]
fn bytecode_piped_between_compile_and_run() {
    let (_, rbc, err1) = run_with_stdin(&["compile", "-O", "bytecode"], SOURCE.as_bytes());
    assert_eq!(err1.len(), 0);
    let (code, stdout, _) = run_with_stdin(&["run"], &rbc);
    assert_eq!(code, 0);
    assert_eq!(String::from_utf8_lossy(&stdout), "pipe ok");
}

#[test]
fn shebang_wrapped_bytecode_still_runs_through_stdin() {
    // A saved script: shebang line + standard preamble.
    let (_, rbc, _) = run_with_stdin(&["compile", "-O", "bytecode"], SOURCE.as_bytes());
    let mut wrapped = b"#!/usr/bin/env rubyc\n".to_vec();
    wrapped.extend_from_slice(&rbc);
    let (code, stdout, stderr) = run_with_stdin(&["run"], &wrapped);
    assert_eq!(code, 0, "stderr: {}", String::from_utf8_lossy(&stderr));
    assert_eq!(String::from_utf8_lossy(&stdout), "pipe ok");
}

#[test]
fn base64_bytecode_round_trips_through_text_pipes() {
    let (_, rbc, _) = run_with_stdin(
        &["compile", "-O", "bytecode", "--base64"],
        SOURCE.as_bytes(),
    );
    // Output must be pure ASCII base64.
    assert!(
        rbc.iter()
            .all(|&b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=')
    );

    let (code, stdout, stderr) = run_with_stdin(&["run", "--base64"], &rbc);
    assert_eq!(code, 0, "stderr: {}", String::from_utf8_lossy(&stderr));
    assert_eq!(String::from_utf8_lossy(&stdout), "pipe ok");
}

#[test]
fn build_writes_executable_that_runs() {
    let dir = std::env::temp_dir().join("rubyc_cli_test");
    std::fs::create_dir_all(&dir).unwrap();
    let exe_path = dir.join("cli_built");

    // Piped source, explicit output path.
    let mut child = rubyc()
        .args(["build", "-o"])
        .arg(&exe_path)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(SOURCE.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&exe_path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let ran = Command::new(&exe_path).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&ran.stdout), "pipe ok");
}

#[test]
fn garbage_input_is_rejected_not_miscompiled() {
    let (code, _, _) = run_with_stdin(&["run", "-I", "bytecode"], b"definitely not bytecode");
    assert_ne!(code, 0);
}

#[test]
fn completion_subcommand_was_removed() {
    let out = Command::new(crate::tests::common::rubyc_path())
        .args(["completion", "bash"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "'completion' should be gone");
}

#[test]
fn help_title_carries_the_version() {
    for flag in ["-h", "--help"] {
        let out = Command::new(crate::tests::common::rubyc_path())
            .arg(flag)
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        let first = text.lines().next().unwrap_or_default();
        assert_eq!(
            first,
            format!("RubyC Compiler v{}", env!("CARGO_PKG_VERSION")),
            "first help line via {flag}"
        );
    }
}
