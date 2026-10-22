//! `catch (T)` must select a clause by the THROWN OBJECT'S TYPE.
//!
//! Before this, lowering ran the first clause unconditionally: any typed
//! catch claimed any thrown object, so `try { throw new MyErr(); }
//! catch (Other e)` took the `Other` branch (measured on both backends).
//! That made every exception-handling program's control flow wrong while
//! looking correct, and it would have made `checked` report its overflow as
//! whatever the nearest catch clause happened to be.
//!
//! Each case below is a whole program whose `Main` returns the number of the
//! branch C# requires; the test runs it and compares.

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

/// Wrap a `Main` body with three unrelated exception classes.
fn program(body: &str) -> String {
    format!(
        "namespace t;\n\
         class MyErr {{ public MyErr() {{ }} }}\n\
         class Other {{ public Other() {{ }} }}\n\
         class DerivedErr : MyErr {{ public DerivedErr() {{ }} }}\n\
         class C {{ int Main() {{ {body} }} }}\n"
    )
}

#[test]
fn an_unrelated_typed_catch_does_not_claim_the_object() {
    let code = exit_code(&program(
        "try { throw new MyErr(); } catch (Other e) { return 9; } catch { return 3; }",
    ));
    assert_eq!(code, 3, "the catch-all must handle it, not `Other`");
}

#[test]
fn a_matching_typed_catch_claims_the_object() {
    let code = exit_code(&program(
        "try { throw new MyErr(); } catch (Other e) { return 9; } catch (MyErr e) { return 7; } return 0;",
    ));
    assert_eq!(code, 7, "the first MATCHING clause wins; earlier clauses are skipped");
}

#[test]
fn a_base_class_catch_accepts_a_derived_exception() {
    let code = exit_code(&program(
        "try { throw new DerivedErr(); } catch (Other e) { return 9; } catch (MyErr e) { return 5; } return 0;",
    ));
    assert_eq!(code, 5, "`catch (Base)` must accept a derived exception");
}

#[test]
fn a_derived_class_catch_does_not_accept_a_base_exception() {
    let code = exit_code(&program(
        "try { throw new MyErr(); } catch (DerivedErr e) { return 9; } catch { return 4; }",
    ));
    assert_eq!(code, 4, "`catch (Derived)` must NOT accept a base exception");
}

#[test]
fn catch_object_accepts_any_thrown_object() {
    let code = exit_code(&program(
        "try { throw new MyErr(); } catch (object o) { return 4; } return 0;",
    ));
    assert_eq!(code, 4, "`catch (object)` is the widest typed clause");
}

#[test]
fn the_bound_variable_is_the_thrown_object() {
    let code = exit_code(&program(
        "try { throw new MyErr(); } catch (MyErr e) { if (e != null) { return 6; } return 2; } return 0;",
    ));
    assert_eq!(code, 6, "the clause variable must be bound to the thrown object");
}

#[test]
fn a_throw_in_the_catch_body_reaches_the_outer_handler() {
    let code = exit_code(&program(
        "try { try { throw new MyErr(); } catch (MyErr e) { throw new Other(); } } catch (Other o) { return 8; } return 0;",
    ));
    assert_eq!(code, 8, "a rethrow must be tested against the enclosing clauses, not the same one");
}

#[test]
fn uncaught_exception_is_currently_swallowed_not_fatal() {
    // DEFECT, pinned on purpose: an exception that no handler claims must
    // terminate the program with a non-zero status. Today `Rethrow` with no
    // enclosing handler falls through and execution continues, so this
    // returns 1 from the statement after the throw — on both backends.
    // When that is fixed this test fails, which is the signal to flip it to
    // "must not reach the line after the throw".
    let code = exit_code(&program("throw new MyErr(); return 1;"));
    assert_eq!(
        code, 1,
        "an uncaught throw currently continues; if this failed, the abort path landed — flip this test"
    );
}

#[test]
fn a_false_exception_filter_falls_through() {
    // `catch (E) when (false)` must NOT catch. The filter used to be
    // parsed and discarded, so the clause caught anyway.
    let src = r#"
namespace System { public class Exception { } }
namespace t {
class C {
    int Main() {
        int r = 0;
        try { throw new System.Exception(); }
        catch (System.Exception) when (false) { r = 1; }
        catch { r = 2; }
        return r;
    }
}
}
"#;
    assert_eq!(exit_code(src), 2, "the false-filtered clause must be skipped");
}

#[test]
fn a_true_exception_filter_selects_the_clause() {
    let src = r#"
namespace System { public class Exception { } }
namespace t {
class C {
    static bool ok = true;
    int Main() {
        int r = 0;
        try { throw new System.Exception(); }
        catch (System.Exception) when (ok) { r = 5; }
        catch { r = 9; }
        return r;
    }
}
}
"#;
    assert_eq!(exit_code(src), 5);
}

#[test]
fn a_return_inside_try_runs_the_finally() {
    // The block used to seal with `Ret` on the way out, so the finally
    // never ran: `f` stayed 0 and the function answered 10, not 15.
    let src = r#"
namespace t;
class C {
    static int f;
    static int F() { try { return 1; } finally { f = 5; } }
    int Main() { return F() * 10 + f; }
}
"#;
    assert_eq!(exit_code(src), 15);
}

#[test]
fn nested_finallys_run_inner_first_on_a_return() {
    let src = r#"
namespace t;
class C {
    static int f;
    static int G() {
        try { try { return 2; } finally { f = f + 1; } }
        finally { f = f + 2; }
    }
    int Main() { int g = G(); return g * 10 + f; }
}
"#;
    assert_eq!(exit_code(src), 23, "value survives both finallys, inner first");
}

#[test]
fn a_return_from_a_catch_runs_the_finally() {
    let src = r#"
namespace t;
class Err { }
class C {
    static int f;
    static int E() {
        try { throw new Err(); }
        catch (Err) { return 4; }
        finally { f = 9; }
    }
    int Main() { return E() * 10 + f; }
}
"#;
    assert_eq!(exit_code(src), 49);
}

#[test]
fn a_normal_completion_still_runs_the_finally_once() {
    let src = r#"
namespace t;
class C {
    static int f;
    static int N() { try { f = 1; } finally { f = f + 5; } return f; }
    int Main() { return N(); }
}
"#;
    assert_eq!(exit_code(src), 6);
}
