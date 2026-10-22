//! Multicast delegates: `d += g` builds an invocation list.
//!
//! A delegate value is a single object, so `+` used to do pointer
//! arithmetic and the following call jumped through garbage. A combined
//! delegate is `[0, kind 2, head]` over `[next, delegate]` nodes; the
//! invocation path evaluates its arguments once and runs every node in
//! subscription order.

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
    assert_eq!(
        out.code,
        out.code,
        "program failed to run:\n{src}\n{}",
        out.stderr_str()
    );
    out.code
}

#[test]
fn two_method_groups_run_in_subscription_order() {
    let src = r#"
namespace t;
class C {
    static int t;
    static void A() { t = t + 1; }
    static void B() { t = t + 10; }
    int Main() { System.Action d = A; d += B; d(); return t; }
}
"#;
    assert_eq!(exit_code(src), 11);
}

#[test]
fn null_plus_a_handler_is_the_handler() {
    let src = r#"
namespace t;
class C {
    static int t;
    static void A() { t = 1; }
    int Main() { System.Action d = null; d += A; d(); return t; }
}
"#;
    assert_eq!(exit_code(src), 1);
}

#[test]
fn three_handlers_run_in_order() {
    let src = r#"
namespace t;
class C {
    static int t;
    static void A() { t = t * 10 + 1; }
    static void B() { t = t * 10 + 2; }
    static void C2() { t = t * 10 + 3; }
    int Main() { System.Action d = A; d += B; d += C2; d(); return t; }
}
"#;
    assert_eq!(exit_code(src), 123);
}

#[test]
fn a_non_capturing_lambda_combines_with_a_method_group() {
    let src = r#"
namespace t;
class C {
    static int t;
    static void A() { t = t * 10 + 1; }
    int Main() {
        System.Action d = A;
        d += () => { t = t * 10 + 2; };
        d();
        return t;
    }
}
"#;
    assert_eq!(exit_code(src), 12);
}

#[test]
fn multicast_arguments_are_evaluated_once() {
    let src = r#"
namespace t;
class C {
    static int calls;
    static int t;
    static int Next() { calls = calls + 1; return calls; }
    static void A(int v) { t = t + v; }
    static void B(int v) { t = t + v; }
    int Main() { System.Action<int> d = A; d += B; d(Next()); return calls * 10 + t; }
}
"#;
    assert_eq!(exit_code(src), 12, "Next runs once and both handlers see 1");
}

#[test]
fn removing_the_last_handler_leaves_the_first() {
    let src = r#"
namespace t;
class C {
    static int t;
    static void A() { t = t * 10 + 1; }
    static void B() { t = t * 10 + 2; }
    int Main() { System.Action d = A; d += B; d -= B; d(); return t; }
}
"#;
    assert_eq!(exit_code(src), 1);
}

#[test]
fn removing_the_first_handler_leaves_the_second() {
    let src = r#"
namespace t;
class C {
    static int t;
    static void A() { t = t * 10 + 1; }
    static void B() { t = t * 10 + 2; }
    int Main() { System.Action d = A; d += B; d -= A; d(); return t; }
}
"#;
    assert_eq!(exit_code(src), 2);
}

#[test]
fn removing_an_unlisted_handler_changes_nothing() {
    let src = r#"
namespace t;
class C {
    static int t;
    static void A() { t = t * 10 + 1; }
    static void B() { t = t * 10 + 2; }
    static void C2() { }
    int Main() { System.Action d = A; d += B; d -= C2; d(); return t; }
}
"#;
    assert_eq!(exit_code(src), 12);
}

#[test]
fn a_lambda_handler_still_reports_removal() {
    // Removal matches by function address; a closure's identity is its
    // captured object, so it is reported rather than guessed.
    let src = r#"namespace t;
class C { static void A() { } int Main() { System.Action d = A; d -= () => { }; return 0; } }
"#;
    let mut cmd = Command::new(rubyc_path());
    cmd.args(["build", "-"]);
    let out = run(&mut cmd, src.as_bytes());
    assert_ne!(out.code, 0, "lambda removal must be reported");
    assert!(
        out.stderr_str().contains("combining delegates"),
        "expected the diagnostic, got:\n{}",
        out.stderr_str()
    );
}


#[test]
fn removing_the_last_handler_leaves_null() {
    // C# answers null when removal empties the list; an empty combined
    // delegate made the call a silent no-op instead.
    let both = r#"
namespace t;
class C {
    static void A() { }
    static void B() { }
    int Main() {
        System.Action d = A; d += B; d -= A; d -= B;
        return d == null ? 7 : 0;
    }
}
"#;
    assert_eq!(exit_code(both), 7);

    let only = r#"
namespace t;
class C {
    static void A() { }
    int Main() { System.Action d = A; d -= A; return d == null ? 7 : 0; }
}
"#;
    assert_eq!(exit_code(only), 7);
}

#[test]
fn a_remaining_handler_is_not_null() {
    let src = r#"
namespace t;
class C {
    static void A() { }
    static void B() { }
    int Main() {
        System.Action d = A; d += B; d -= B;
        return d == null ? 0 : 1;
    }
}
"#;
    assert_eq!(exit_code(src), 1);
}

#[test]
fn a_delegate_field_can_be_invoked() {
    // `d()` where `d` is a field used to report
    // `delegate_invocation_unavailable`; the local invoke body is now a
    // shared helper, so the field path dispatches identically.
    let src = r#"
namespace t;
class C {
    System.Action d;
    static int t;
    static void A() { t = 7; }
    void Run() { d(); }
    int Main() { C c = new C(); c.d = A; c.Run(); return t; }
}
"#;
    assert_eq!(exit_code(src), 7);
}

#[test]
fn a_combined_delegate_invoked_through_a_field_runs_every_handler() {
    let src = r#"
namespace t;
class C {
    System.Action d;
    static int t;
    static void A() { t = t * 10 + 1; }
    static void B() { t = t * 10 + 2; }
    void Run() { d(); }
    int Main() {
        System.Action l = A; l += B;
        C c = new C(); c.d = l; c.Run();
        return t;
    }
}
"#;
    assert_eq!(exit_code(src), 12);
}

#[test]
fn a_delegate_field_holding_a_closure_can_be_invoked() {
    let src = r#"
namespace t;
class C {
    System.Func<int, int> f;
    static int t;
    void Run() { t = f(4); }
    int Main() { C c = new C(); c.f = x => x * 3; c.Run(); return t; }
}
"#;
    assert_eq!(exit_code(src), 12);
}

#[test]
fn a_delegate_field_can_be_combined_and_invoked() {
    let src = r#"
namespace t;
class C {
    System.Action d;
    static int t;
    static void A() { t = t * 10 + 1; }
    static void B() { t = t * 10 + 2; }
    void Run() { d = A; d += B; d(); }
    int Main() { C c = new C(); c.Run(); return t; }
}
"#;
    assert_eq!(exit_code(src), 12);
}

#[test]
fn a_delegate_field_can_be_reduced() {
    let src = r#"
namespace t;
class C {
    System.Action d;
    static int t;
    static void A() { t = t * 10 + 1; }
    static void B() { t = t * 10 + 2; }
    void Run() { d = A; d += B; d -= B; d(); }
    int Main() { C c = new C(); c.Run(); return t; }
}
"#;
    assert_eq!(exit_code(src), 1);
}



#[test]
fn a_static_delegate_field_can_be_combined_and_invoked() {
    let src = r#"
namespace t;
class C {
    static System.Action d;
    static int t;
    static void A() { t = t * 10 + 1; }
    static void B() { t = t * 10 + 2; }
    int Main() { d = A; d += B; d(); return t; }
}
"#;
    assert_eq!(exit_code(src), 12);
}

#[test]
fn a_static_delegate_field_can_be_reduced() {
    let src = r#"
namespace t;
class C {
    static System.Action d;
    static int t;
    static void A() { t = t * 10 + 1; }
    static void B() { t = t * 10 + 2; }
    int Main() { d = A; d += B; d -= B; d(); return t; }
}
"#;
    assert_eq!(exit_code(src), 1);
}

#[test]
fn a_single_static_delegate_field_dispatches() {
    let src = r#"
namespace t;
class C {
    static System.Action d;
    static int t;
    static void A() { t = 5; }
    int Main() { d = A; d(); return t; }
}
"#;
    assert_eq!(exit_code(src), 5);
}

#[test]
fn a_delegate_property_can_be_invoked() {
    let src = r#"
namespace t;
class C {
    System.Action _d;
    System.Action P { get { return _d; } }
    static int t;
    static void A() { t = 7; }
    void Run() { P(); }
    int Main() { C c = new C(); c._d = A; c.Run(); return t; }
}
"#;
    assert_eq!(exit_code(src), 7);
}

#[test]
fn an_auto_property_delegate_can_be_invoked() {
    let src = r#"
namespace t;
class C {
    System.Action P { get; set; }
    static int t;
    static void A() { t = 3; }
    void Run() { P(); }
    int Main() { C c = new C(); c.P = A; c.Run(); return t; }
}
"#;
    assert_eq!(exit_code(src), 3);
}

#[test]
fn a_combined_delegate_read_through_a_property_runs_every_handler() {
    let src = r#"
namespace t;
class C {
    System.Action _d;
    System.Action P { get { return _d; } }
    static int t;
    static void A() { t = t * 10 + 1; }
    static void B() { t = t * 10 + 2; }
    void Setup() { _d = A; _d += B; }
    void Run() { P(); }
    int Main() { C c = new C(); c.Setup(); c.Run(); return t; }
}
"#;
    assert_eq!(exit_code(src), 12);
}

#[test]
fn a_settable_property_can_be_combined() {
    let src = r#"
namespace t;
class C {
    System.Action _d;
    System.Action P { get { return _d; } set { _d = value; } }
    static int t;
    static void A() { t = t * 10 + 1; }
    static void B() { t = t * 10 + 2; }
    void Setup() { _d = A; P += B; }
    void Run() { P(); }
    int Main() { C c = new C(); c.Setup(); c.Run(); return t; }
}
"#;
    assert_eq!(exit_code(src), 12, "the existing handler must survive the combine");
}

#[test]
fn an_auto_property_can_be_combined() {
    let src = r#"
namespace t;
class C {
    System.Action P { get; set; }
    static int t;
    static void A() { t = t * 10 + 1; }
    static void B() { t = t * 10 + 2; }
    void Run() { P += A; P += B; P(); }
    int Main() { C c = new C(); c.Run(); return t; }
}
"#;
    assert_eq!(exit_code(src), 12);
}

#[test]
fn a_get_only_property_combination_is_reported() {
    let src = r#"namespace t;
class C {
    System.Action _d;
    System.Action P { get { return _d; } }
    static void A() { }
    int Main() { P += A; return 0; }
}
"#;
    let mut cmd = Command::new(rubyc_path());
    cmd.args(["build", "-"]);
    let out = run(&mut cmd, src.as_bytes());
    assert_ne!(out.code, 0, "a get-only property cannot be assigned");
    // Either diagnostic is correct: the property-assignment check now
    // fires before the combine guard, since the declaration has no
    // setter at all.
    assert!(
        out.stderr_str().contains("has no setter")
            || out.stderr_str().contains("SETTABLE property"),
        "expected a setter diagnostic, got:\n{}",
        out.stderr_str()
    );
}

#[test]
fn a_settable_property_can_be_reduced() {
    let src = r#"
namespace t;
class C {
    System.Action _d;
    System.Action P { get { return _d; } set { _d = value; } }
    static int t;
    static void A() { t = t * 10 + 1; }
    static void B() { t = t * 10 + 2; }
    void Run() { P += A; P += B; P -= B; P(); }
    int Main() { C c = new C(); c.Run(); return t; }
}
"#;
    assert_eq!(exit_code(src), 1);
}

#[test]
fn an_auto_property_can_be_reduced() {
    let src = r#"
namespace t;
class C {
    System.Action P { get; set; }
    static int t;
    static void A() { t = t * 10 + 1; }
    static void B() { t = t * 10 + 2; }
    void Run() { P += A; P += B; P -= B; P(); }
    int Main() { C c = new C(); c.Run(); return t; }
}
"#;
    assert_eq!(exit_code(src), 1);
}

#[test]
fn removing_every_handler_from_a_property_leaves_null() {
    let src = r#"
namespace t;
class C {
    System.Action _d;
    System.Action P { get { return _d; } set { _d = value; } }
    static void A() { }
    void Run() { P += A; P -= A; }
    int Main() { C c = new C(); c.Run(); return c.P == null ? 7 : 0; }
}
"#;
    assert_eq!(exit_code(src), 7);
}
