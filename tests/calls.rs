//! Call semantics: `params` packing and argument evaluation.
//!
//! `void Sum(params int[] xs)` called as `Sum(1, 2, 3)` used to pass
//! three vregs to a one-parameter callee: the callee read argument 1 as
//! the array and faulted. `Sum()` passed nothing at all.

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
class C {{
    static int Sum(params int[] xs) {{
        int s = 0;
        foreach (int x in xs) {{ s += x; }}
        return s;
    }}
    int Main() {{ {body} }}
}}
"#
    )
}

#[test]
fn params_packs_extra_arguments() {
    assert_eq!(exit_code(&program("return Sum(1, 2, 3);")), 6);
}

#[test]
fn params_with_no_arguments_is_an_empty_array() {
    assert_eq!(exit_code(&program("return Sum();")), 0);
}

#[test]
fn an_explicit_array_is_not_packed_again() {
    assert_eq!(exit_code(&program("return Sum(new int[] { 4, 5 });")), 9);
}

#[test]
fn params_packs_strings_and_fixed_parameters() {
    let src = r#"
namespace t;
class C {
    static int Count(int prefix, params string[] rest) {
        int n = prefix;
        foreach (string s in rest) { n += s.Length; }
        return n;
    }
    int Main() { return Count(10, "ab", "c"); }
}
"#;
    assert_eq!(exit_code(src), 13);
}

fn interface_program(body: &str) -> String {
    format!(
        r#"namespace t;
interface I {{ int M(); }}
class A : I {{
    public int M() {{ return 3; }}
    int I.M() {{ return 7; }}
}}
class B : I {{ public int M() {{ return 5; }} }}
class D : B {{ }}
class C {{ int Main() {{ {body} }} }}
"#
    )
}

#[test]
fn explicit_implementation_dispatches_through_the_interface() {
    // The method is registered as `I.M`; the interface call looked for
    // `M` and fell through to an unrelated slot.
    assert_eq!(exit_code(&interface_program("I i = new A(); return i.M();")), 7);
}

#[test]
fn explicit_implementation_wins_over_a_plain_method_through_the_interface() {
    assert_eq!(
        exit_code(&interface_program("I i = new A(); A a = new A(); return i.M() * 10 + a.M();")),
        73
    );
}

#[test]
fn a_plain_implementation_in_a_base_class_still_dispatches() {
    assert_eq!(exit_code(&interface_program("I i = new D(); return i.M();")), 5);
}

#[test]
fn interface_overloads_dispatch_by_argument_type() {
    // An interface receiver has no table entry of its own, so the slot
    // used to fall back to the first member of a guessed implementer:
    // both overloads landed on M(int).
    let src = r#"
namespace t;
interface I { int M(int x); int M(string s); }
class A : I {
    public int M(int x) { return 1; }
    public int M(string s) { return 2; }
}
class C { int Main() { I i = new A(); return i.M(5) * 10 + i.M("x"); } }
"#;
    assert_eq!(exit_code(src), 12);
}

#[test]
fn two_interfaces_with_the_same_member_name_keep_their_own_slots() {
    let src = r#"
namespace t;
interface IFoo { int M(); }
interface IBar { int M(); }
class A : IFoo, IBar {
    int IFoo.M() { return 1; }
    int IBar.M() { return 2; }
}
class C { int Main() { IFoo f = new A(); IBar b = new A(); return f.M() * 10 + b.M(); } }
"#;
    assert_eq!(exit_code(src), 12);
}

#[test]
fn an_explicit_implementation_is_inherited() {
    let src = r#"
namespace t;
interface I { int M(); }
class A : I { int I.M() { return 7; } }
class B : A { }
class C { int Main() { I i = new B(); return i.M(); } }
"#;
    assert_eq!(exit_code(src), 7);
}

#[track_caller]
fn compile_error(src: &str) -> String {
    let mut cmd = Command::new(rubyc_path());
    cmd.args(["build", "-"]);
    let out = run(&mut cmd, src.as_bytes());
    assert!(!out.timed_out, "the compiler hung:\n{src}");
    assert_ne!(out.code, 0, "the build should have failed:\n{src}");
    out.stderr_str()
}

#[test]
fn a_generic_class_knows_its_runtime_type_argument() {
    // `typeof(T)` inside a method of a generic class asks for the
    // runtime type argument: the answer depends on the INSTANTIATION
    // (`Box<int>` says int, `Box<string>` says string), so the
    // instance has to carry its own runtime type. The compiler read
    // the spelling and answered "T" for both.
    let src = r#"
namespace t;
class Box<T> {
    public string Describe() {
        if (typeof(T) == typeof(int)) { return "int"; }
        if (typeof(T) == typeof(string)) { return "string"; }
        return "other";
    }
}
class C {
    int Main() {
        Box<int> bi = new Box<int>();
        Box<string> bs = new Box<string>();
        printf("{0} {1}", bi.Describe(), bs.Describe());
        return 0;
    }
}
"#;
    let mut cmd = Command::new(rubyc_path());
    cmd.args(["run", "-"]);
    let out = run(&mut cmd, src.as_bytes());
    assert!(!out.timed_out, "the program hung:\n{src}");
    assert_eq!(out.code, 0, "the program failed:\n{}", out.stderr_str());
    assert_eq!(out.stdout_str(), "int string");
}

#[test]
fn a_generic_struct_knows_its_runtime_type_argument() {
    // A generic STRUCT's box is materialized from the class entry,
    // which does not name the instantiation, so the box had to be
    // given the runtime type from the spelling it was asked for.
    let src = r#"
namespace t;
struct Pair<T> {
    public T First;
    public T Second;
    public Pair(T a, T b) { First = a; Second = b; }
    public string Kind() {
        if (typeof(T) == typeof(int)) { return "int"; }
        return "other";
    }
}
class C {
    int Main() {
        Pair<int> p = new Pair<int>(1, 2);
        printf("{0}", p.Kind());
        return 0;
    }
}
"#;
    let mut cmd = Command::new(rubyc_path());
    cmd.args(["run", "-"]);
    let out = run(&mut cmd, src.as_bytes());
    assert!(!out.timed_out, "the program hung:\n{src}");
    assert_eq!(out.code, 0, "the program failed:\n{}", out.stderr_str());
    assert_eq!(out.stdout_str(), "int");
}

#[test]
fn a_delegate_knows_its_runtime_type() {
    // `x is SomeDelegate` compares the value's runtime type, so the
    // delegate has to carry one. The value is a bare function pointer
    // without it, and the test could not answer at all.
    let src = r#"
namespace t;
public delegate int Op(int x);
public delegate int Op2(int x);
class C {
    int Main() {
        Op o = (int x) => x + 1;
        Op2 p = (int x) => x + 2;
        printf("{0} {1}", o is Op ? 1 : 0, p is Op ? 1 : 0);
        return 0;
    }
}
"#;
    let mut cmd = Command::new(rubyc_path());
    cmd.args(["run", "-"]);
    let out = run(&mut cmd, src.as_bytes());
    assert!(!out.timed_out, "the program hung:\n{src}");
    assert_eq!(out.code, 0, "the program failed:\n{}", out.stderr_str());
    assert_eq!(out.stdout_str(), "1 0");
}

#[test]
fn a_delegate_keeps_its_type_across_a_call() {
    // The type word travels with the value: the callee sees the
    // delegate's runtime type, so `o is Op` answers there too.
    let src = r#"
namespace t;
public delegate int Op(int x);
class C {
    static int IsOp(Op o) { return o is Op ? 1 : 0; }
    int Main() {
        Op o = (int x) => x + 1;
        printf("{0} {1}", IsOp(o), o is Op ? 1 : 0);
        return 0;
    }
}
"#;
    let mut cmd = Command::new(rubyc_path());
    cmd.args(["run", "-"]);
    let out = run(&mut cmd, src.as_bytes());
    assert!(!out.timed_out, "the program hung:\n{src}");
    assert_eq!(out.code, 0, "the program failed:\n{}", out.stderr_str());
    assert_eq!(out.stdout_str(), "1 1");
}

#[test]
fn a_constructed_type_composes_at_run_time() {
    // `typeof(Box<T>)` names the instantiation, and the instantiation
    // is only known when the method runs: the descriptor is composed
    // from the definition and the runtime argument.
    let src = r#"
namespace t;
class Box<T> {
    public string Kind() {
        if (typeof(Box<T>) == typeof(Box<int>)) { return "int"; }
        return "other";
    }
}
class C {
    int Main() {
        Box<int> b = new Box<int>();
        printf("{0}", b.Kind());
        return 0;
    }
}
"#;
    let mut cmd = Command::new(rubyc_path());
    cmd.args(["run", "-"]);
    let out = run(&mut cmd, src.as_bytes());
    assert!(!out.timed_out, "the program hung:\n{src}");
    assert_eq!(out.code, 0, "the program failed:\n{}", out.stderr_str());
    assert_eq!(out.stdout_str(), "int");
}

#[test]
fn named_arguments_are_bound_by_name_not_position() {
    let src = r#"
namespace t;
class C {
    static int Sub(int a, int b) { return a - b; }
    int Main() { return Sub(b: 1, a: 5); }
}
"#;
    assert_eq!(exit_code(src), 4);
}

#[test]
fn named_arguments_work_on_instance_and_constructor_calls() {
    let src = r#"
namespace t;
class C {
    public int V;
    public C(int a, int b) { V = a * 10 + b; }
    int Sub(int a, int b) { return a - b; }
    int Main() {
        C c = new C(b: 1, a: 5);
        return c.Sub(b: 2, a: 9) * 10 + c.V;
    }
}
"#;
    assert_eq!(exit_code(src), 121, "Sub 7, V 51");
}

#[test]
fn named_arguments_fill_skipped_defaults() {
    let src = r#"
namespace t;
class C {
    static int F(int a = 1, int b = 2, int c = 3) { return a * 100 + b * 10 + c; }
    int Main() { return F(b: 7); }
}
"#;
    assert_eq!(exit_code(src), 173);
}

#[test]
fn named_arguments_keep_ref_and_params_semantics() {
    let src = r#"
namespace t;
class C {
    static void Inc(ref int x) { x = x + 1; }
    static int Sum(params int[] xs) {
        int s = 0;
        foreach (int x in xs) { s += x; }
        return s;
    }
    int Main() {
        int v = 4;
        Inc(x: ref v);
        return v * 10 + Sum(xs: new int[] { 2, 3 });
    }
}
"#;
    assert_eq!(exit_code(src), 55, "v is 5 after the ref call, Sum is 5");
}

#[test]
fn an_unknown_named_argument_is_reported() {
    let src = r#"
namespace t;
class C {
    static int F(int a) { return a; }
    int Main() { return F(z: 1); }
}
"#;
    let err = compile_error(src);
    assert!(
        err.contains("no parameter named `z`"),
        "expected the diagnostic to name the parameter, got:\n{err}"
    );
}

#[test]
fn a_duplicated_named_argument_is_reported() {
    let src = r#"
namespace t;
class C {
    static int F(int a) { return a; }
    int Main() { return F(a: 1, a: 2); }
}
"#;
    // Overload selection sees two arguments for a one-parameter method
    // and rejects the call before the reordering runs; either diagnostic
    // is a loud failure, which is the point.
    let err = compile_error(src);
    assert!(
        err.contains("F") && (err.contains("is given twice") || err.contains("unknown call")),
        "expected a diagnostic naming the method, got:\n{err}"
    );
}
