//! Negative tests: the compiler must REJECT these programs with a
//! diagnostic, and must never crash (no ICE, no panic, no hang).
//!
//! One case per category the mandate names — syntax, names, types,
//! inheritance, constraints, inline assembly — plus a crash guard: every
//! source here runs through the real binary, so a panic in any pass
//! fails the suite instead of reaching a user as an internal error.

use crate::tests::common::{run, rubyc_path};
use std::process::Command;

/// Compile `src` and return (exit code, stderr). Asserts the compiler did
/// not HANG, which is its own failure mode.
#[track_caller]
fn compile(src: &str) -> (i32, String) {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let out_path = std::env::temp_dir().join(format!("rubyc_neg_{}.out", n));
    let mut cmd = Command::new(rubyc_path());
    cmd.args(["compile", "-o"]).arg(&out_path);
    let out = run(&mut cmd, src.as_bytes());
    let _ = std::fs::remove_file(&out_path);
    assert!(!out.timed_out, "the compiler hung on:\n{src}");
    let stderr = out.stderr_str();
    // A panic is an ICE: reported as such, never as a normal diagnostic.
    assert!(
        !stderr.contains("panicked at") && !stderr.contains("internal error"),
        "the compiler PANICKED (ICE) on:\n{src}\n{stderr}"
    );
    (out.code, stderr)
}

/// Assert the program is REJECTED (non-zero exit) with a diagnostic
/// mentioning `needle`.
#[track_caller]
fn rejects(src: &str, needle: &str) {
    let (code, stderr) = compile(src);
    assert_ne!(code, 0, "expected a rejection for:\n{src}\nstderr: {stderr}");
    assert!(
        stderr.to_lowercase().contains(&needle.to_lowercase()),
        "expected the message to mention {needle:?} for:\n{src}\ngot: {stderr}"
    );
}

#[test]
fn syntax_errors_are_rejected() {
    rejects("class C { int Main( { return 0; } }", "syntax error");
    rejects("class C { int Main() { return 0 } }", "syntax error");
    rejects("class { int Main() { return 0; } }", "syntax error");
}

#[test]
fn unknown_names_are_rejected() {
    rejects(
        "class C { int Main() { return notAThing; } }",
        "unknown name",
    );
    rejects(
        "class C { int Main() { return notAThing(); } }",
        "unknown",
    );
}

#[test]
fn type_errors_are_rejected() {
    // No `main` at all.
    rejects("class C { int Other() { return 0; } }", "main");
    // A member that does not exist on a resolved receiver.
    rejects(
        "class A { public int X; } class C { int Main() { A a = new A(); return a.Missing(); } }",
        "missing",
    );
}

#[test]
fn inheritance_errors_are_rejected() {
    // An unknown base class.
    rejects(
        "class C : NoSuchBase { int Main() { return 0; } }",
        "unknown base",
    );
    // An inheritance cycle.
    rejects(
        "class A : B { } class B : A { } class C { int Main() { return 0; } }",
        "cycle",
    );
}

#[test]
fn constraint_and_generic_errors_are_rejected() {
    // Wrong generic arity in an explicit type argument.
    rejects(
        "class Box<T> { } class C { int Main() { Box<int, string> b = null; return 0; } }",
        "box",
    );
}

#[test]
fn inline_assembly_errors_are_rejected() {
    // An opcode the assembler does not know.
    rejects(
        "class C { int Main() { return 0; } }\n#Assembly\nnot_an_opcode rax\n#EndAssembly",
        "assembly",
    );
}

#[test]
fn duplicate_declarations_are_rejected() {
    rejects(
        "class C { } class C { } class D { int Main() { return 0; } }",
        "declared more than once",
    );
}

#[test]
fn private_access_is_rejected() {
    rejects(
        "class A { private int X; } class C { int Main() { A a = new A(); return a.X; } }",
        "private",
    );
}

#[test]
fn concatenating_a_float_needs_conversion_and_is_reported() {
    // Integer/bool/char now convert; float formatting is still open and
    // must be reported rather than concatenated as a pointer.
    rejects(
        r#"namespace t;
class C { int Main() { double x = 1.5; string s = "a" + x; return 0; } }
"#,
        "value-to-string conversion",
    );
}

#[test]
fn interpolating_a_float_needs_conversion_and_is_reported() {
    rejects(
        r#"namespace t;
class C { int Main() { double x = 1.5; string s = $"{x}"; return 0; } }
"#,
        "value-to-string conversion",
    );
}

#[test]
fn float_tostring_needs_conversion_and_is_reported() {
    rejects(
        r#"namespace t;
class C { int Main() { double x = 1.5; string s = x.ToString(); return 0; } }
"#,
        "value-to-string conversion",
    );
}

#[test]
fn removing_a_delegate_handler_is_reported_not_crashed() {
    // Removal matches a METHOD GROUP by function address. A lambda is
    // a captured closure whose identity is the captured object, so that
    // shape is reported rather than guessed.
    rejects(
        r#"namespace t;
class C {
    static void A() { }
    int Main() { System.Action d = A; d -= () => { }; return 0; }
}
"#,
        "combining delegates",
    );
}

#[test]
fn assigning_to_a_get_only_property_is_reported() {
    // The write used to evaluate the value and DROP it (C# reports
    // CS0200). Both the unqualified and the receiver form are checked.
    rejects(
        r#"namespace t;
class C { int _x; int P { get { return _x; } } int Main() { P = 5; return 0; } }
"#,
        "has no setter",
    );
    rejects(
        r#"namespace t;
class C { int _x; public int P { get { return _x; } } int Main() { C c = new C(); c.P = 5; return 0; } }
"#,
        "has no setter",
    );
    // The declaration may be inherited: the whole base chain is checked.
    rejects(
        r#"namespace t;
class B { int _b; public int Q { get { return _b; } } }
class C : B { int Main() { Q = 5; return 0; } }
"#,
        "has no setter",
    );
    rejects(
        r#"namespace t;
class B { int _b; public int Q { get { return _b; } } }
class C : B { int Main() { C c = new C(); c.Q = 5; return 0; } }
"#,
        "has no setter",
    );
}

#[test]
fn return_inside_a_finally_is_rejected() {
    // C# reports CS0157. Routing it through the finally produced a jump
    // to the same block: the program compiled and then hung.
    rejects(
        r#"namespace t;
class C { static int F() { try { return 1; } finally { return 2; } } int Main() { return F(); } }
"#,
        "finally",
    );
}

#[test]
fn a_lambda_return_is_not_mistaken_for_a_finally_return() {
    // A lambda inside a finally has its own return; the enclosing finally
    // does not leave through it. This must still compile.
    let src = r#"namespace t;
class C {
    int Main() {
        int n = 0;
        try { n = 1; } finally { System.Func<int, int> f = x => { return x + 1; }; n = f(n); }
        return n;
    }
}
"#;
    let (code, stderr) = compile(src);
    assert_eq!(code, 0, "stderr: {stderr}");
}

#[test]
fn printf_reports_formats_it_cannot_render() {
    // A double used to be written as its IEEE bit pattern (a plausible-looking
    // wrong integer); unsupported specs must report too.
    rejects(
        r#"namespace t;
class C { int Main() { double d = 1.5; printf("{0}", d); return 0; } }
"#,
        "not implemented",
    );
    rejects(
        r#"namespace t;
class C { int Main() { printf("{0:N2}", 1234); return 0; } }
"#,
        "unsupported integer format",
    );
    rejects(
        r#"namespace t;
class C { int Main() { printf("{0:G5}", 1234); return 0; } }
"#,
        "G precision",
    );
}
