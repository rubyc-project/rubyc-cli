//! `using` disposes its resources.
//!
//! The parser used to desugar `using (R r = e) { body }` to a `let` plus
//! the body: `Dispose` was never called and nothing reported it. It now
//! wraps the body in `try { body } finally { r.Dispose(); }`, so disposal
//! happens on normal completion, on `return`, and on a throw. Resources
//! declared together are disposed in reverse order.

use crate::tests::common::{run, rubyc_path};
use std::process::Command;

#[track_caller]
fn exit_code(src: &str) -> i32 {
    let mut cmd = Command::new(rubyc_path());
    cmd.args(["run", "-"]);
    let out = run(&mut cmd, src.as_bytes());
    assert!(!out.timed_out, "the program hung:\n{src}");
    assert!(
        !out.stderr_str().contains("panicked at"),
        "the compiler panicked:\n{src}\n{}",
        out.stderr_str()
    );
    out.code
}

fn program(main: &str) -> String {
    format!(
        r#"namespace t;
class R {{
    public static int closed;
    public void Dispose() {{ closed = closed + 1; }}
}}
class C {{ int Main() {{ {main} }} }}
"#
    )
}

#[test]
fn using_disposes_on_normal_completion() {
    assert_eq!(exit_code(&program("using (R r = new R()) { } return R.closed;")), 1);
}

#[test]
fn using_disposes_when_the_body_returns() {
    // The return must run the finally, and the resource must still be
    // alive while Dispose runs (releasing it first crashed).
    let src = r#"
namespace t;
class R { public static int closed; public void Dispose() { closed = 5; } }
class C {
    static int F() { using (R r = new R()) { return 1; } }
    int Main() { int v = F(); return v * 10 + R.closed; }
}
"#;
    assert_eq!(exit_code(src), 15);
}

#[test]
fn using_disposes_when_the_body_throws() {
    let src = r#"
namespace t;
class R { public static int closed; public void Dispose() { closed = 9; } }
class Err { }
class C {
    int Main() {
        try { using (R r = new R()) { throw new Err(); } }
        catch (Err) { }
        return R.closed;
    }
}
"#;
    assert_eq!(exit_code(src), 9);
}

#[test]
fn using_disposes_when_a_throw_follows_a_taken_return() {
    // The in-flight return slot is shared by every path through the
    // try/finally. A first call that returns leaves the flag set; a later
    // call that throws must not read that stale flag and turn the throw
    // into a return. The flag is now initialized before the handler is
    // installed, so the finally disposes and propagates.
    let src = r#"
namespace t;
class R { public static int closed; public void Dispose() { closed = 9; } }
class Err { }
class C {
    static int F(bool b) {
        using (R r = new R()) { if (b) return 7; throw new Err(); }
    }
    int Main() {
        F(true);
        try { F(false); return 100; } catch (Err) { return R.closed; }
    }
}
"#;
    assert_eq!(exit_code(src), 9);
}

#[test]
fn multiple_resources_dispose_in_reverse_order() {
    let src = r#"
namespace t;
class A { public static int order; public void Dispose() { order = order * 10 + 1; } }
class B { public void Dispose() { A.order = A.order * 10 + 2; } }
class C { int Main() { using (A a = new A(), B b = new B()) { } return A.order; } }
"#;
    assert_eq!(exit_code(src), 21, "B disposes before A");
}

#[test]
fn an_expression_resource_is_disposed() {
    let src = r#"
namespace t;
class R { public static int closed; public void Dispose() { closed = 3; } }
class C {
    static R Make() { return new R(); }
    int Main() { using (Make()) { } return R.closed; }
}
"#;
    assert_eq!(exit_code(src), 3);
}

#[test]
fn a_using_declaration_disposes_at_the_end_of_the_block() {
    // `using Type name = expr;` disposes at the end of the enclosing
    // block, not at the end of the statement.
    let src = r#"
namespace t;
class R { public static int closed; public void Dispose() { closed = closed + 1; } }
class C {
    int Main() {
        { using R r = new R(); }
        return R.closed;
    }
}
"#;
    assert_eq!(exit_code(src), 1);
}

#[test]
fn a_using_declaration_disposes_when_the_block_returns() {
    let src = r#"
namespace t;
class R { public static int closed; public void Dispose() { closed = 7; } }
class C {
    static int F() { using R r = new R(); return 2; }
    int Main() { int v = F(); return v * 10 + R.closed; }
}
"#;
    assert_eq!(exit_code(src), 27);
}

#[test]
fn using_declarations_dispose_in_reverse_order_and_support_var() {
    let src = r#"
namespace t;
class A { public static int order; public int v; public void Dispose() { order = order * 10 + 1; } }
class B { public void Dispose() { A.order = A.order * 10 + 2; } }
class C {
    int Main() {
        { using A a = new A(); a.v = 4; using var b = new B(); }
        return A.order * 10 + 4;
    }
}
"#;
    assert_eq!(exit_code(src), 214, "b (var) disposes before a");
}

#[test]
fn a_using_declaration_disposes_when_the_block_throws() {
    let src = r#"
namespace t;
class R { public static int closed; public void Dispose() { closed = 8; } }
class Err { }
class C {
    int Main() {
        try { using R r = new R(); throw new Err(); }
        catch (Err) { }
        return R.closed;
    }
}
"#;
    assert_eq!(exit_code(src), 8);
}
