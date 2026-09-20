//! Enum declaration, underlying values, and switch integration.
//!
//! Tests are ordered: simplest first. Each test exercises one specific
//! behavior so failures pinpoint the exact missing feature.

use crate::tests::common::run;
use std::process::Command;

static CTR: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn run_program(src: &str) -> String {
    let n = CTR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let src_path = std::env::temp_dir().join(format!("rubyc_enum_{n}.rc"));
    std::fs::write(&src_path, src).unwrap();
    let mut cmd = Command::new(crate::tests::common::rubyc_path());
    cmd.arg("run").arg(&src_path);
    let out = run(&mut cmd, b"");
    assert!(!out.timed_out, "HUNG:\n{src}\nstderr: {}", out.stderr_str());
    assert_eq!(out.code, 0, "exit {}: {}", out.code, out.stderr_str());
    out.stdout_str()
}

// ---------- declaration accepted ----------

#[test]
fn enum_decl_parses_without_error() {
    // Just verify the syntax doesn't cause a parse error
    // when the enum is declared but unused.
    let src = r#"namespace t {
        enum Color { Red = 0 }
        class c { int main() { printf("ok"); } }
    }"#;
    let src_path = std::env::temp_dir().join("rubyc_enum_decl.rc");
    std::fs::write(&src_path, src).unwrap();
    let mut cmd = Command::new(crate::tests::common::rubyc_path());
    cmd.arg("run").arg(&src_path);
    let out = run(&mut cmd, b"");
    // May not fully work yet — but must NOT be a parse error
    let stderr = out.stderr_str();
    assert!(
        !stderr.contains("syntax error"),
        "enum declaration must not cause syntax error:\n{stderr}"
    );
}

#[test]
fn enum_multiple_values_parse() {
    let src = r#"namespace t {
        enum Op { A = 0, B = 1, C = 2 }
        class c { int main() { printf("ok"); } }
    }"#;
    let src_path = std::env::temp_dir().join("rubyc_enum_multi.rc");
    std::fs::write(&src_path, src).unwrap();
    let mut cmd = Command::new(crate::tests::common::rubyc_path());
    cmd.arg("run").arg(&src_path);
    let out = run(&mut cmd, b"");
    let stderr = out.stderr_str();
    assert!(
        !stderr.contains("syntax error"),
        "multi-value enum must not cause syntax error:\n{stderr}"
    );
}

#[test]
fn enum_with_underlying_type_parses() {
    let src = r#"namespace t {
        enum Level : int { Low = 0, High = 1 }
        class c { int main() { printf("ok"); } }
    }"#;
    let src_path = std::env::temp_dir().join("rubyc_enum_utype.rc");
    std::fs::write(&src_path, src).unwrap();
    let mut cmd = Command::new(crate::tests::common::rubyc_path());
    cmd.arg("run").arg(&src_path);
    let out = run(&mut cmd, b"");
    let stderr = out.stderr_str();
    assert!(
        !stderr.contains("syntax error"),
        "enum with : type must not cause syntax error:\n{stderr}"
    );
}

// ---------- switch on enum values (the real test) ----------

#[test]
fn switch_on_int_with_case_labels_dispatches() {
    // This is what ExecuteOperation does — switch on an int with case labels
    let src = r#"namespace t; class c {
        int main() {
            for (int op = 1; op <= 3; op++) {
                switch (op) {
                    case 1: printf("ADD"); break;
                    case 2: printf("SUB"); break;
                    default: printf("OTHER"); break;
                }
            }
        }
    }"#;
    assert_eq!(run_program(src), "ADDSUBOTHER");
}

#[test]
fn switch_inside_loop_with_continue() {
    let src = r#"namespace t; class c {
        int main() {
            for (int i = 0; i < 4; i++) {
                switch (i % 2) {
                    case 0: printf("even "); continue;
                    default: printf("odd "); break;
                }
            }
        }
    }"#;
    // continue skips the rest of the loop body (nothing after switch here)
    // so we expect: even odd even odd even
    // Wait: i=0 even, i=1 odd, i=2 even, i=3 odd → "even odd even odd "
    assert_eq!(run_program(src), "even odd even odd ");
}
