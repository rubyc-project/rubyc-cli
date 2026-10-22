//! Events: storage, subscription, removal and invocation.
//!
//! An event used to be accessor-backed with NO storage: reading it was
//! reported as a missing feature, and even after a slot existed the
//! parser's `E += h` desugaring (`E = E + h`) did pointer arithmetic on
//! delegate handles. A method group converted to a delegate object with
//! no function address, so `D d = H; d(x)` called address 0.
//!
//! Each case is a whole program whose `Main` returns the value C#
//! requires; every expectation below was measured by running it.

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

fn program(body: &str) -> String {
    format!(
        r#"namespace t;
delegate void D(int x);
class Src {{
    public event D A;
    public void Fire(int x) {{ A?.Invoke(x); }}
    public void Sub(D d) {{ A += d; }}
    public void Rem(D d) {{ A -= d; }}
}}
class C {{
    static int total;
    static void H(int x) {{ total = total + x; }}
    static void G(int x) {{ total = total * 10 + x; }}
    int Main() {{ {body} }}
}}
"#
    )
}

#[test]
fn a_null_event_invocation_is_a_no_op() {
    // `A?.Invoke(...)` with no subscribers must skip, not fault.
    assert_eq!(
        exit_code(&program("Src s = new Src(); s.Fire(1); return 5;")),
        5
    );
}

#[test]
fn one_subscriber_receives_the_arguments() {
    assert_eq!(
        exit_code(&program("Src s = new Src(); s.Sub(H); s.Fire(3); return total;")),
        3
    );
}

#[test]
fn subscribers_run_in_subscription_order() {
    // H then G: 0 + 2 = 2, then 2 * 10 + 2 = 22. Prepending the list
    // (the first implementation) ran G first and answered 4.
    assert_eq!(
        exit_code(&program(
            "Src s = new Src(); s.Sub(H); s.Sub(G); s.Fire(2); return total;"
        )),
        22
    );
}

#[test]
fn removal_stops_only_that_subscriber() {
    // r = 22 (both ran); remove G; Fire(3) reaches only H: 25.
    // 22 * 100 + 25 = 2225, which is 177 modulo 256.
    assert_eq!(
        exit_code(&program(
            "Src s = new Src(); s.Sub(H); s.Sub(G); s.Fire(2); int r=total; s.Rem(G); s.Fire(3); return r*100+total;"
        )),
        177
    );
}

#[test]
fn a_method_group_is_a_real_delegate() {
    // `D d = H; d(x)` — the delegate object used to be allocated EMPTY,
    // so the call went to address 0.
    let src = r#"
namespace t;
delegate void D(int x);
class C {
    static int total;
    static void H(int x) { total = total + x; }
    int Main() { D d = H; d(3); return total; }
}
"#;
    assert_eq!(exit_code(src), 3);
}

#[test]
fn event_arguments_are_evaluated_once() {
    // C# evaluates the invocation arguments once and passes them to
    // every handler.
    let src = r#"
namespace t;
delegate void D(int x);
class Src {
    public event D A;
    public void Fire(int x) { A?.Invoke(x); }
    public void Sub(D d) { A += d; }
}
class C {
    static int total;
    static int calls;
    static int Next() { calls = calls + 1; return calls; }
    static void H(int x) { total = total + x; }
    static void G(int x) { total = total + x; }
    int Main() {
        Src s = new Src();
        s.Sub(H); s.Sub(G);
        s.Fire(Next());
        return total * 10 + calls;
    }
}
"#;
    // Next() runs once (1), both handlers add it: total = 2 -> 21.
    assert_eq!(exit_code(src), 21);
}
