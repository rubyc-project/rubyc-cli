//! Generic classes and generic methods: type arguments, fields, and
//! the runtime types they carry.
//!
//! Three silent-wrong-answer paths lived here. A field write through a
//! generic-typed receiver (`Box<int> b; b.Value = 7;`) resolved the
//! receiver's spelling against the type table, which holds the
//! DEFINITION of a generic class and not its instantiations — so no
//! field was found, the store was dropped, and the field read back
//! zero. A generic method specialized at the call site (`Make<int>(7)`)
//! kept its type parameter inside a constructed type (`Box<T>`), so
//! the composed type had no runtime type to build from. And a
//! property getter that names its own class's type parameter read it
//! from the reader instead, so `typeof(T)` inside the getter had no
//! type to name.

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

/// The compiler's stderr for a source file (empty when the build
/// succeeds without warnings).
#[track_caller]
fn compile_stderr(src: &str) -> String {
    let mut cmd = Command::new(rubyc_path());
    cmd.args(["build", "-", "-o", "/dev/null"]);
    let out = run(&mut cmd, src.as_bytes());
    assert!(
        !out.timed_out,
        "the compiler hung:\n{src}"
    );
    out.stderr_str().to_owned()
}

#[test]
fn a_generic_class_field_keeps_its_value() {
    let src = r#"
namespace t;
class Box<T> { public T Value; }
class C {
    int Main() {
        Box<int> b = new Box<int>();
        b.Value = 7;
        return b.Value;
    }
}
"#;
    assert_eq!(
        exit_code(src),
        7,
        "a field write through a generic-typed receiver must reach the field"
    );
}

#[test]
fn a_generic_class_field_reads_back_across_two_instances() {
    let src = r#"
namespace t;
class Box<T> { public T Value; }
class C {
    int Main() {
        Box<int> a = new Box<int>();
        Box<int> b = new Box<int>();
        a.Value = 3;
        b.Value = 4;
        return a.Value * 10 + b.Value;
    }
}
"#;
    assert_eq!(exit_code(src), 34, "each instance keeps its own field");
}

#[test]
fn a_specialized_generic_method_builds_the_constructed_type() {
    let src = r#"
namespace t;
class Box<T> { public T Value; }
class C {
    static Box<T> Make<T>(T value) {
        Box<T> b = new Box<T>();
        b.Value = value;
        return b;
    }
    int Main() {
        Box<int> b = Make<int>(7);
        return b.Value;
    }
}
"#;
    assert_eq!(
        exit_code(src),
        7,
        "the specialization substitutes the type argument inside `Box<T>`"
    );
}

#[test]
fn a_property_getter_reads_its_own_class_type_parameter() {
    let src = r#"
namespace t;
class Scalar<T> {
    public static bool IsInt {
        get => typeof(T) == typeof(int);
    }
}
class C {
    static int Check<T>(T value) {
        if (Scalar<T>.IsInt) { return 1; }
        return 0;
    }
    int Main() {
        int r = 0;
        r = r + Check<int>(1);
        r = r + 10 * Check<string>("x");
        return r;
    }
}
"#;
    assert_eq!(
        exit_code(src),
        1,
        "the getter's `T` is the class's parameter, bound by the receiver's spelling"
    );
}

#[test]
fn an_inlined_generic_body_may_call_a_private_member() {
    let src = r#"
namespace t;
class C {
    private static int Secret() { return 42; }
    static int Get<T>(T v) { return Secret(); }
    int Main() { return Get<int>(1); }
}
"#;
    assert_eq!(
        exit_code(src),
        42,
        "the specialized body is still the class's own code"
    );
}

#[test]
fn a_conflicting_type_argument_infers_the_best_common_type() {
    // The candidates for `T` are `long` (the vector) and `int` (the
    // literal). C# fixes them to the best common type — the one every
    // candidate converts to — which is `long`. The inference compared
    // the candidates for equality and reported the call as ambiguous.
    let src = r#"
namespace t;
class Vec<T> { }
class C {
    static bool All<T>(Vec<T> v, T value) { return true; }
    int Main() {
        Vec<long> v = new Vec<long>();
        return All(v, 1) ? 1 : 0;
    }
}
"#;
    assert_eq!(exit_code(src), 1, "the call infers `long` and compiles");
}

#[test]
fn a_dereference_types_as_the_pointed_to_type() {
    // `*(byte*)p` is a byte. The dereference degraded to `object`,
    // which poisoned the generic inference: `Memmove(ref *(byte*)dst,
    // ...)` inferred nothing and the call was refused.
    let src = r#"
namespace t;
class C {
    static void Memmove<T>(ref T destination, ref T source, long count) { }
    static void Copy(byte* destination, byte* source) {
        Memmove(ref *(byte*)destination, ref *(byte*)source, 4);
    }
    int Main() { return 0; }
}
"#;
    let out = compile_stderr(src);
    assert!(
        out.is_empty() || !out.contains("cannot infer"),
        "the call infers `byte` and compiles, got:\n{out}"
    );
}

#[test]
fn a_type_parameter_pattern_is_reported_rather_than_answered() {
    let src = r#"
namespace t;
class Box<T> {
    public bool Is(object o) { return o is T; }
}
class C {
    int Main() { return 0; }
}
"#;
    let err = compile_stderr(src);
    assert!(
        err.contains("type parameter"),
        "expected the diagnostic to name the type parameter, got:\n{err}"
    );
}

#[test]
fn a_nested_type_reads_its_enclosing_types_private_members() {
    // C# lets a nested type read the enclosing type's private
    // members. The viewer's simple name is ambiguous (`AlternateLookup`
    // exists in three collections), so the access rule gave up and
    // denied the read.
    let src = r#"
namespace t;
class Outer {
    private int _value = 7;
    private int Read() { return _value; }
    public sealed class Inner {
        public int Get(Outer o) { return o.Read(); }
    }
}
class C {
    int Main() {
        Outer o = new Outer();
        Outer.Inner i = new Outer.Inner();
        return i.Get(o);
    }
}
"#;
    assert_eq!(exit_code(src), 7);
}
