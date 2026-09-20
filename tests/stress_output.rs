//! Golden-file verification for the C# 11 mega fixture (`stress_test.rc`).
//!
//! Every `printf` in the fixture ends its format string with `\n`, so the
//! program's stdout is exactly one deterministic `TRACE|...` line per
//! `printf`. `FINALIZER|...` lines are excluded from the comparison because
//! their order and timing are runtime-defined. The golden file
//! (`fixtures/stress_test.expected.txt`) is the single source of truth for
//! the fixture's full output and grows section by section as the parser
//! graduates the fixture; the two execution gates below stay red until the
//! whole fixture compiles and matches it byte-for-byte.

use crate::tests::common::{Outcome, exec, rubyc_path, run};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

const GOLDEN: &str = include_str!("fixtures/stress_test.expected.txt");
const STRESS: &str = include_str!("fixtures/stress_test.rc");

static SEQ: AtomicU32 = AtomicU32::new(0);

fn temp_path(label: &str) -> (std::path::PathBuf, std::path::PathBuf) {
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("rubyc_golden_{label}_{}_{n}", std::process::id()));
    (root.with_extension("rc"), root.with_extension("bin"))
}

fn run_jit() -> Outcome {
    let (source, _) = temp_path("jit");
    std::fs::write(&source, STRESS).unwrap();
    let mut command = Command::new(rubyc_path());
    command.arg("run").arg(&source);
    let outcome = run(&mut command, b"42\n");
    let _ = std::fs::remove_file(source);
    outcome
}

fn run_native() -> Outcome {
    let (source, binary) = temp_path("native");
    std::fs::write(&source, STRESS).unwrap();
    let mut command = Command::new(rubyc_path());
    let built = exec(command.arg("build").arg(&source).arg("-o").arg(&binary));
    assert!(!built.timed_out, "native compilation timed out");
    assert_eq!(
        built.code,
        0,
        "native compilation failed:\n{}",
        built.stderr_str()
    );
    let mut command = Command::new(&binary);
    let outcome = run(&mut command, b"42\n");
    let _ = std::fs::remove_file(source);
    let _ = std::fs::remove_file(binary);
    outcome
}

/// Deterministic output: all lines except the runtime-ordered finalizers.
fn deterministic(stdout: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(stdout)
        .lines()
        .filter(|line| !line.starts_with("FINALIZER|"))
        .map(ToOwned::to_owned)
        .collect()
}

fn assert_golden_match(actual: &[String], expected: &[&str]) {
    if actual.len() != expected.len() {
        let empty = String::new();
        panic!(
            "line count diverged: got {} lines, golden has {}\nfirst actual: {:?}\nlast actual: {:?}",
            actual.len(),
            expected.len(),
            actual.first().unwrap_or(&empty),
            actual.last().unwrap_or(&empty),
        );
    }
    for (index, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
        if a != e {
            panic!("first divergence at line {index}:\n  got:  {a}\n  want: {e}");
        }
    }
}

#[test]
fn golden_seed_first_line_is_the_first_trace() {
    let expected: Vec<&str> = GOLDEN.lines().collect();
    assert_eq!(
        expected.first().copied(),
        Some("TRACE|PROGRAM|MAIN|BEGIN|version=CSharp11"),
        "the program's first print is its first output line; the golden seed must start with it"
    );
    assert!(GOLDEN.ends_with('\n'), "golden file must end with a newline");
    for line in &expected {
        assert!(!line.is_empty(), "golden file contains a blank line");
    }
}

#[test]
fn jit_stdout_matches_golden_file() {
    let out = run_jit();
    assert!(!out.timed_out, "JIT run HUNG — a loop or branch went wrong");
    assert_eq!(out.code, 0, "JIT run failed:\n{}", out.stderr_str());
    let actual = deterministic(&out.stdout);
    let expected: Vec<&str> = GOLDEN.lines().collect();
    assert_golden_match(&actual, &expected);
}

#[test]
fn native_stdout_matches_golden_file() {
    let out = run_native();
    assert!(!out.timed_out, "native run HUNG — a loop or branch went wrong");
    assert_eq!(out.code, 0, "native run failed:\n{}", out.stderr_str());
    let actual = deterministic(&out.stdout);
    let expected: Vec<&str> = GOLDEN.lines().collect();
    assert_golden_match(&actual, &expected);
}
