//! `switch` statements: integer-constant dispatch, default arm, multi-label
//! arms, no-fallthrough enforcement, interaction with loops (break targets
//! the switch; continue skips to the loop), constant-subject folding, and
//! bytecode round-trip. Every exec case runs on BOTH JIT and ELF paths.

use crate::tests::common::{exec, run};
use rubyc::bytecode::traits::{FromBytecode, ToBytecode};
use rubyc::models::compare::StructuralEq;
use rubyc::parser;

fn run_source(source: &str) -> String {
    let mut cmd = std::process::Command::new(crate::tests::common::rubyc_path());
    cmd.arg("run");
    let out = run(&mut cmd, source.as_bytes());
    assert!(
        !out.timed_out,
        "program HUNG: {}\nstderr: {}",
        source,
        out.stderr_str()
    );
    assert!(out.code == 0, "exit {}: {}", out.code, out.stderr_str());
    out.stdout_str()
}

fn run_native(source: &str) -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let src = std::env::temp_dir().join(format!("rubyc_sw_{}_{}.rc", std::process::id(), n));
    let bin = std::env::temp_dir().join(format!("rubyc_sw_{}_{}.bin", std::process::id(), n));
    std::fs::write(&src, source).unwrap();
    let mut build = std::process::Command::new(crate::tests::common::rubyc_path());
    let built = exec(build.args(["build"]).arg(&src).arg("-o").arg(&bin));
    assert!(
        !built.timed_out && built.code == 0,
        "build failed:\n{}",
        built.stderr_str()
    );
    let mut exe = std::process::Command::new(&bin);
    let out = exec(&mut exe);
    assert!(!out.timed_out, "compiled program HUNG");
    let text = out.stdout_str();
    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&bin);
    text
}

fn program(body: &str) -> String {
    format!("namespace t; class c {{ int main() {{ {body} }} }}")
}

macro_rules! sw_case {
    ($name:ident, $body:expr, $expected:expr) => {
        #[test]
        fn $name() {
            assert_eq!(run_source(&program($body)), $expected, "JIT path");
            assert_eq!(run_native(&program($body)), $expected, "ELF path");
        }
    };
}

sw_case!(
    dispatch_basic,
    r#"for (int i = 0; i < 4; i++) { switch (i) { case 0: printf("z"); break; case 1: printf("o"); break; default: printf("m"); break; } }"#,
    "zomm"
);

sw_case!(
    no_default_no_match_falls_through,
    r#"switch (9) { case 1: printf("x"); break; } printf("end");"#,
    "end"
);

sw_case!(
    multi_label_arm,
    r#"int x = 7; switch (x) { case 1: case 7: case 9: printf("hit"); break; default: printf("miss"); break; }"#,
    "hit"
);

sw_case!(
    constant_subject_picks_arm,
    r#"switch (2 + 3) { case 4: printf("four"); break; case 5: printf("five"); break; default: printf("other"); break; }"#,
    "five"
);

sw_case!(
    constant_subject_default,
    r#"switch (100) { case 1: printf("one"); break; default: printf("d"); break; }"#,
    "d"
);

sw_case!(
    break_exits_switch_not_outer_loop,
    r#"for (int i = 0; i < 3; i++) { switch (i) { case 1: break; default: printf("{0}", i); break; } printf("."); }"#,
    "0..2."
);

sw_case!(
    continue_skips_over_switch_to_loop,
    r#"for (int i = 0; i < 5; i++) { switch (i % 2) { case 0: continue; default: printf("{0}", i); break; } printf("-"); }"#,
    "1-3-"
);

sw_case!(
    nested_switch,
    r#"int a = 1; int b = 2; switch (a) { case 1: switch (b) { case 2: printf("inner"); break; default: break; } printf("outer"); break; default: break; }"#,
    "innerouter"
);

#[test]
fn arm_with_locals_and_heap() {
    let src = "namespace t; class box { public int v = 0; } class c { int main() { for (int i = 1; i <= 2; i++) { switch (i) { default: box b = new box(); b.v = i * 10; printf(\"{0}\", b.v); break; } } printf(\"k\"); } }";
    assert_eq!(run_source(src), "1020k", "JIT path");
    assert_eq!(run_native(src), "1020k", "ELF path");
}

#[test]
fn no_fallthrough_is_enforced() {
    let src = program(r#"switch (1) { case 1: printf("a"); printf("b"); }"#);
    let mut cmd = std::process::Command::new(crate::tests::common::rubyc_path());
    cmd.args(["compile", "-o", "/tmp/opencode/nf.out"]);
    let out = run(&mut cmd, src.as_bytes());
    let _ = std::fs::remove_file("/tmp/opencode/nf.out");
    assert!(out.code != 0);
    assert!(
        out.stderr_str().contains("must end with 'break;'"),
        "{}",
        out.stderr_str()
    );
}

#[test]
fn duplicate_default_rejected() {
    let src = program(r#"switch (1) { default: break; default: break; }"#);
    let mut cmd = std::process::Command::new(crate::tests::common::rubyc_path());
    cmd.args(["compile", "-o", "/tmp/opencode/dd.out"]);
    let out = run(&mut cmd, src.as_bytes());
    let _ = std::fs::remove_file("/tmp/opencode/dd.out");
    assert!(
        out.stderr_str().contains("duplicate 'default'"),
        "{}",
        out.stderr_str()
    );
}

#[test]
fn switch_survives_bytecode_round_trip() {
    let src = program(
        r#"for (int i = 0; i < 3; i++) { switch (i) { case 0: printf("A"); break; case 2: printf("C"); break; default: printf("B"); break; } }"#,
    );
    let unit = parser::parse(&src).unwrap();
    let encoded = unit.to_bytecode();
    let decoded = rubyc::models::CompilationUnit::from_bytecode(&encoded).unwrap();
    assert!(unit.structural_eq(&decoded));
}
