//! JIT tests: compile a sample in-process and run the generated code.

use rubyc::native::jit::jit_run;
use rubyc::parser;
use rubyc::testdata::SAMPLES;
use rubyc_target_linux_x86_64::{BACKEND, MEMORY};

#[test]
fn jit_compiles_and_runs_hello_world() {
    // Spawned rather than in-process: a completed program ends with an
    // `exit_group` syscall that would terminate this harness.
    let sample = &SAMPLES[0];
    let (code, stdout) = run_source(sample.source);
    assert_eq!(code, 0);
    assert_eq!(stdout, "Hello World!");
}

#[test]
fn program_without_main_is_reported() {
    let unit = parser::parse("namespace t; class c { int start() { printf(\"x\"); } }").unwrap();
    let err = jit_run(&unit, &BACKEND, &MEMORY).unwrap_err();
    assert!(err.to_string().contains("'main'"));
}

#[test]
fn unsupported_construct_is_reported() {
    let src = "namespace t; class c { int main() { foo(bar); } }";
    let unit = parser::parse(src).unwrap();
    let err = jit_run(&unit, &BACKEND, &MEMORY).unwrap_err();
    assert!(err.to_string().contains("not supported"), "{err}");
}

// ---------- codegen regression tests ----------
//
// Each of these exercises a bug that once produced silent corruption or
// segfaults (itoa jump displacement, double ELF relocation, vtable words
// never populated, receiver offset math).
//
// NOTE: a completed program ends with an `exit_group` syscall, which under
// in-process JIT terminates the *harness* process. Exact-output checks must
// therefore spawn the compiler binary (`run`) instead of calling jit_run;
// jit_run in-process stays reserved for error-path assertions above.

fn run_source(source: &str) -> (i32, String) {
    let mut cmd = std::process::Command::new(crate::tests::common::rubyc_path());
    cmd.arg("run");
    let out = crate::tests::common::run(&mut cmd, source.as_bytes());
    (out.code, out.stdout_str())
}

/// ITOA digit loop: `rcx` must be zeroed on the positive path too, and the
/// `jns` displacement must land exactly on the digit-loop head.
#[test]
fn jit_int_printing_exact_digits() {
    let src =
        r#"namespace t { class c { int main() { printf("{0}|{1}|{2}", 7, -42, 123456789); } } }"#;
    let (code, stdout) = run_source(src);
    assert_eq!(code, 0);
    assert_eq!(stdout, "7|-42|123456789");
}

/// The alloc helper's lazy-init skip must land on the full 64-bit
/// `mov rax,[rcx]`, and the vtable pointer must be stored into the object.
#[test]
fn jit_virtual_dispatch_picks_the_override() {
    let src = r#"namespace t { class b { int Who() { printf("B"); } } class d : b { int Who() { printf("D"); } int main() { Who(); } } }"#;
    let (code, stdout) = run_source(src);
    assert_eq!(code, 0);
    assert_eq!(stdout, "D");
}
