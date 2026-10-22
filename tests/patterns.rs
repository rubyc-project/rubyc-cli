//! `is` patterns: type tests, constant patterns, and `or`/`and` chains.
//!
//! Two silent-wrong-answer paths lived here. A constant pattern
//! (`x is 5`, `sizeof(T) is sizeof(U)`) fell through the `is` lowering to a
//! bare `0`, so the test was always false; and a chained pattern
//! (`x is A or B`) reached the same fallthrough because the lowering only
//! looked at a single identifier. Both compiled and ran, which is worse
//! than a diagnostic. Each case below is a whole program whose `Main`
//! returns the value C# requires.

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

#[test]
fn constant_pattern_compares_values() {
    let src = r#"
namespace t;
class C {
    int Main() {
        int a = 2;
        int r = 0;
        if (a is 2) { r = r + 1; }
        if (a is 3) { r = r + 100; }
        return r;
    }
}
"#;
    assert_eq!(exit_code(src), 1, "`is 2` must be true for 2 and `is 3` false");
}

#[test]
fn or_pattern_is_a_disjunction() {
    let src = r#"
namespace t;
class C {
    int Main() {
        int a = 2;
        int r = 0;
        if (a is 1 or 2) { r = r + 10; }
        if (a is 4 or 5) { r = r + 100; }
        return r;
    }
}
"#;
    assert_eq!(exit_code(src), 10, "`is 1 or 2` is a disjunction");
}

#[test]
fn and_pattern_is_a_conjunction() {
    let src = r#"
namespace t;
class C {
    int Main() {
        int a = 2;
        int r = 0;
        if (a is 2 and 2) { r = r + 10; }
        if (a is 2 and 3) { r = r + 100; }
        return r;
    }
}
"#;
    assert_eq!(exit_code(src), 10, "`is 2 and 2` is a conjunction");
}

#[test]
fn sizeof_patterns_compare_sizes() {
    let src = r#"
namespace t;
class C {
    int Main() {
        int r = 0;
        if (sizeof(int) is sizeof(int)) { r = r + 4; }
        if (sizeof(int) is sizeof(char)) { r = r + 100; }
        return r;
    }
}
"#;
    assert_eq!(exit_code(src), 4, "sizeof patterns compare the two sizes");
}

#[test]
fn not_pattern_negates() {
    let src = r#"
namespace t;
class C {
    int Main() {
        int a = 2;
        int r = 0;
        if (a is not 3) { r = r + 20; }
        if (a is not 2) { r = r + 100; }
        return r;
    }
}
"#;
    assert_eq!(exit_code(src), 20, "`is not` negates the whole test");
}

#[test]
fn type_pattern_still_works_alongside_constants() {
    let src = r#"
namespace t;
class A { public int V; }
class B : A { }
class C {
    int Main() {
        A a = new B();
        int r = 0;
        if (a is B) { r = r + 1; }
        if (a is C) { r = r + 10; }
        return r;
    }
}
"#;
    assert_eq!(exit_code(src), 1, "type tests must not be mistaken for constants");
}

#[test]
fn property_pattern_tests_members_and_binds() {
    let src = r#"
namespace t;
class Box { public int Length; public bool IsInvalid; public Box() { } }
class C {
    int Main() {
        Box a = new Box(); a.Length = 1; a.IsInvalid = false;
        Box b = new Box(); b.Length = 3; b.IsInvalid = true;
        int r = 0;
        if (a is { Length: 1 } x) { r = r + 1; }
        if (b is { Length: 1 }) { r = r + 100; }
        if (b is { IsInvalid: true }) { r = r + 10; }
        if (a is { IsInvalid: false }) { r = r + 4; }
        if (b is { Length: > 0, IsInvalid: true }) { r = r + 20; }
        return r;
    }
}
"#;
    assert_eq!(exit_code(src), 35, "members are ANDed; each compares the subject's value");
}

#[test]
fn relational_pattern_compares_the_subject() {
    let src = r#"
namespace t;
class C {
    int Main() {
        int n = 7;
        int r = 0;
        if (n is > 5) { r = r + 1; }
        if (n is >= 7) { r = r + 2; }
        if (n is < 5) { r = r + 100; }
        if (n is <= 7) { r = r + 4; }
        return r;
    }
}
"#;
    assert_eq!(exit_code(src), 7, "> >= < <= compare the tested value, not a placeholder");
}

#[test]
fn switch_case_binding_receives_the_subject() {
    let src = r#"
namespace t;
class Animal { }
class Dog : Animal { public int Bark() { return 10; } }
class Cat : Animal { public int Meow() { return 20; } }
class C {
    int Main() {
        Animal a = new Cat();
        switch (a) {
            case Dog d: return 100 + d.Bark();
            case Cat c: return 200 + c.Meow();
            default: return 300;
        }
    }
}
"#;
    assert_eq!(exit_code(src), 220, "the designation is the matched subject");
}

#[test]
fn list_pattern_tests_length_and_elements() {
    let src = r#"
namespace t;
class C {
    int Main() {
        int[] a = new int[] { 10, 20, 30 };
        int[] e = new int[] { };
        int[] one = new int[] { 200 };
        int r = 0;
        if (a is [10, 20, 30]) { r = r + 1; }
        if (a is [10, ..]) { r = r + 2; }
        if (a is [10, 20]) { r = r + 100; }
        if (e is []) { r = r + 4; }
        if (one is [> 100, ..]) { r = r + 8; }
        return r;
    }
}
"#;
    assert_eq!(exit_code(src), 15, "exact length, slice, empty and relational elements");
}

#[test]
fn switch_expression_evaluates_its_arms() {
    let src = r#"
namespace t;
class C {
    int Main() {
        int x = 2;
        int y = x switch { 1 => 10, 2 => 20, _ => 30 };
        int z = 9 switch { > 5 => 7, _ => 0 };
        return y + z;
    }
}
"#;
    assert_eq!(exit_code(src), 27, "a switch expression used to answer its subject");
}

#[test]
fn switch_expression_tuple_arms_and_guards() {
    let src = r#"
namespace t;
class C {
    int Main() {
        bool a = true; bool b = false;
        int r = (a, b) switch { (true, true) => 1, (true, false) => 2, _ => 4 };
        int g = 5 switch { > 3 when true => 10, _ => 0 };
        return r + g;
    }
}
"#;
    assert_eq!(exit_code(src), 12, "tuple arms and `when` guards select the arm");
}

#[test]
fn switch_expression_without_a_matching_arm_throws() {
    // A switch expression must produce a value. The no-match edge used to
    // fall into the join and read an unassigned slot; it now raises
    // SwitchExpressionException, which this catch observes.
    let src = r#"
namespace System.Runtime.CompilerServices { class SwitchExpressionException { } }
namespace t;
class C {
    int Main() {
        try { int y = 5 switch { 1 => 10 }; return y; }
        catch (System.Runtime.CompilerServices.SwitchExpressionException) { return 4; }
    }
}
"#;
    assert_eq!(exit_code(src), 4, "the unmatched subject must throw");
}

#[test]
fn switch_expression_discard_guard_can_fall_through() {
    // `_` with a false guard does not match, so the no-match path must
    // throw rather than answer the last arm.
    let src = r#"
namespace System.Runtime.CompilerServices { class SwitchExpressionException { } }
namespace t;
class C {
    int Main() {
        int x = 5;
        try {
            int y = x switch { 1 => 10, _ when x > 10 => 20 };
            return y;
        } catch (System.Runtime.CompilerServices.SwitchExpressionException) { return 4; }
    }
}
"#;
    assert_eq!(exit_code(src), 4);
    let src = src.replace("int x = 5;", "int x = 11;");
    assert_eq!(exit_code(&src), 20, "the guarded discard still matches");
}

#[test]
fn switch_statement_when_guard_blocks_the_arm() {
    // `case 1 when false:` must NOT match. The guard used to be parsed
    // and dropped, so the arm ran regardless — a silent wrong answer.
    let src = r#"
namespace t;
class C {
    int Main() {
        int x = 1;
        int r = 0;
        switch (x) {
            case 1 when false: r = 100; break;
            case 1 when true: r = 7; break;
            default: r = 42; break;
        }
        return r;
    }
}
"#;
    assert_eq!(exit_code(src), 7, "a false guard must fall through to the next label");
}

#[test]
fn switch_guard_runs_only_after_the_label_matches() {
    let src = r#"
namespace t;
class C {
    static int calls;
    static bool Probe() { calls = calls + 1; return false; }
    int Main() {
        int x = 2;
        int r = 0;
        switch (x) {
            case 1 when Probe(): r = 100; break;
            default: r = 5; break;
        }
        return r * 10 + calls;
    }
}
"#;
    // The label 1 does not match x = 2, so Probe never runs.
    assert_eq!(exit_code(src), 50);
}

#[test]
fn switch_expression_false_guard_tries_the_next_arm() {
    // A false guard used to jump to the end, leaving the result at its
    // default (0) instead of continuing to the next arm.
    let src = r#"
namespace t;
class C {
    int Main() {
        int x = 1;
        int r = x switch { 1 when false => 10, 1 => 7, _ => 20 };
        return r;
    }
}
"#;
    assert_eq!(exit_code(src), 7);
}

#[test]
fn switch_expression_guard_selects_the_arm_and_falls_back() {
    let src = r#"
namespace t;
class C {
    static bool ok = true;
    int Main() {
        int x = 2;
        int a = x switch { 2 when ok => 5, _ => 9 };
        int b = x switch { 2 when !ok => 5, _ => 9 };
        return a * 10 + b;
    }
}
"#;
    assert_eq!(exit_code(src), 59, "true guard 5, false guard falls to _ = 9");
}

#[test]
fn switch_on_string_compares_case_labels_by_value() {
    // The switch lowering compared the subject and the label with a raw
    // pointer `Cmp`, so two equal strings never matched.
    let src = r#"
namespace t;
class C {
    int Main() {
        string s = "b";
        switch (s) {
            case "a": return 1;
            case "b": return 2;
            default: return 0;
        }
    }
}
"#;
    assert_eq!(exit_code(src), 2);
}

#[test]
fn tuple_item_reads_work_for_explicit_and_inferred_types() {
    let explicit = r#"
namespace t;
class C { int Main() { (int, int) t = (1, 2); return t.Item1 * 10 + t.Item2; } }
"#;
    assert_eq!(exit_code(explicit), 12);
    let inferred = r#"
namespace t;
class C { int Main() { var t = (1, 2); return t.Item1 * 10 + t.Item2; } }
"#;
    assert_eq!(exit_code(inferred), 12);
}

#[test]
fn an_object_pattern_is_a_null_test() {
    // `x is object` names the one type every reference derives from,
    // so it holds for every value but a null one. The lowering looked
    // `object` up in the type table, found no entry, and reported the
    // pattern as an unknown type.
    let src = r#"
namespace t;
class C {
    int Main() {
        object live = new C();
        object dead = null;
        int r = 0;
        if (live is object) { r = r + 1; }
        if (dead is object) { r = r + 100; }
        return r;
    }
}
"#;
    assert_eq!(exit_code(src), 1, "a null reference is not an object");
}

#[test]
fn an_object_pattern_on_a_value_type_always_holds() {
    // A value type is never null, so it is always an object; a null
    // nullable is a null reference and is not.
    let src = r#"
namespace t;
class C {
    int Main() {
        int x = 5;
        int? y = null;
        int r = 0;
        if (x is object) { r = r + 1; }
        if (y is object) { r = r + 10; }
        return r;
    }
}
"#;
    assert_eq!(exit_code(src), 1);
}

#[test]
fn a_discard_pattern_matches_every_value() {
    // `is _` is the discard pattern: it matches whatever it is
    // applied to. The lowering read it as a type name.
    let src = r#"
namespace t;
class C {
    int Main() {
        object o = new C();
        int r = 0;
        if (o is _) { r = r + 1; }
        return r;
    }
}
"#;
    assert_eq!(exit_code(src), 1);
}
