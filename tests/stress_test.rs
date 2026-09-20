//! Acceptance suite for the non-async C# 11 mega fixture.
//!
//! Parser, JIT, native build, and native execution live together so this
//! fixture has one graduation gate. `stress_test_async.rc` is deliberately
//! excluded.

use crate::tests::common::{Outcome, exec, rubyc_path, run};
use rubyc::bytecode::traits::{FromBytecode, ToBytecode};
use rubyc::models::compare::StructuralEq;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

const SOURCE: &str = include_str!("fixtures/stress_test.rc");
static SEQ: AtomicU32 = AtomicU32::new(0);

fn paths(label: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("rubyc_mega_{label}_{}_{n}", std::process::id()));
    (root.with_extension("rc"), root.with_extension("bin"))
}

fn run_jit(stdin: &[u8]) -> Outcome {
    let (source, _) = paths("jit");
    std::fs::write(&source, SOURCE).unwrap();
    let mut command = Command::new(rubyc_path());
    command.arg("run").arg(&source);
    let outcome = run(&mut command, stdin);
    let _ = std::fs::remove_file(source);
    outcome
}

fn build_native() -> (std::path::PathBuf, std::path::PathBuf) {
    let (source, binary) = paths("native");
    std::fs::write(&source, SOURCE).unwrap();
    let mut command = Command::new(rubyc_path());
    let outcome = exec(command.arg("build").arg(&source).arg("-o").arg(&binary));
    assert!(!outcome.timed_out, "native compilation timed out");
    assert_eq!(
        outcome.code,
        0,
        "native compilation failed:\n{}",
        outcome.stderr_str()
    );
    (source, binary)
}

#[test]
fn parser_and_bytecode_accept_complete_stress_fixture() {
    let preprocessed = rubyc::preprocessor::preprocess(SOURCE, &[], "stress_test.rc")
        .unwrap_or_else(|errors| panic!("preprocessing failed: {errors:#?}"));
    std::fs::write("/tmp/stress_preprocessed.rc", &preprocessed.source).ok();
    let unit = rubyc::parser::parse(&preprocessed.source).unwrap_or_else(|errors| {
        let total = errors.len();
        let all: String = errors
            .iter()
            .map(|e| format!("@{}: {}", e.span.start, e.message))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write("/tmp/stress_errors.txt", &all).ok();
        let first = errors.into_iter().take(20).collect::<Vec<_>>();
        panic!("parser failed with {total} diagnostics; first 20: {first:#?}")
    });
    let encoded = unit.to_bytecode();
    let decoded = rubyc::models::CompilationUnit::from_bytecode(&encoded).unwrap();
    assert!(unit.structural_eq(&decoded));
}

#[test]
fn complete_stress_fixture_compiles_to_native_executable() {
    let (source, binary) = build_native();
    let bytes = std::fs::read(&binary).unwrap();
    assert_eq!(&bytes[..4], b"\x7fELF");
    let _ = std::fs::remove_file(source);
    let _ = std::fs::remove_file(binary);
}

#[test]
fn jit_and_native_executable_match_all_deterministic_printf_output() {
    let jit = run_jit(b"42\n");
    assert!(!jit.timed_out, "JIT execution timed out");
    assert_eq!(jit.code, 0, "JIT failed:\n{}", jit.stderr_str());

    let (source, binary) = build_native();
    let mut command = Command::new(&binary);
    let native = run(&mut command, b"42\n");
    assert!(!native.timed_out, "native execution timed out");
    assert_eq!(native.code, 0, "native failed:\n{}", native.stderr_str());

    let deterministic = |bytes: &[u8]| {
        String::from_utf8_lossy(bytes)
            .lines()
            .filter(|line| !line.starts_with("FINALIZER|"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let jit_stdout = deterministic(&jit.stdout);
    let native_stdout = deterministic(&native.stdout);
    assert!(jit_stdout.contains("TRACE|PROGRAM|MAIN|BEGIN"));
    assert!(jit_stdout.contains("TRACE|PROGRAM|MAIN|END|return=0"));
    assert_eq!(
        jit_stdout, native_stdout,
        "JIT/native printf output diverged"
    );

    let _ = std::fs::remove_file(source);
    let _ = std::fs::remove_file(binary);
}
