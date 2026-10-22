//! Primitive value-to-string: `ToString`, interpolation, and `string + value`.
//!
//! These shapes used to crash (the primitive handle was concatenated as a
//! string pointer) and then reported. The conversion is now a decimal
//! routine built from immediate-offset stores, plus `True`/`False` and
//! one-character strings. `==` is used here because it exercises the
//! string data, not just a length; printf encoding has its own cases in
//! `cli_pipes.rs`.

use crate::tests::common::{run, rubyc_path};
use std::process::Command;

#[track_caller]
fn exit_code(src: &str) -> i32 {
    let mut cmd = Command::new(rubyc_path());
    cmd.args(["run", "-"]);
    let out = run(&mut cmd, src.as_bytes());
    assert!(!out.timed_out, "the program hung:\n{src}");
    assert!(!out.stderr_str().contains("panicked at"), "{}", out.stderr_str());
    out.code
}

fn program(body: &str) -> String {
    format!("namespace t; class C {{ int Main() {{ {body} }} }}")
}

fn equals(expr: &str, lit: &str) -> i32 {
    exit_code(&program(&format!("return {expr} == \"{lit}\" ? 1 : 0;")))
}

#[test]
fn int_to_string_covers_sign_and_zero() {
    assert_eq!(equals("(5).ToString()", "5"), 1);
    assert_eq!(equals("(0).ToString()", "0"), 1);
    assert_eq!(equals("(-7).ToString()", "-7"), 1);
    assert_eq!(equals("(2147483647).ToString()", "2147483647"), 1);
    assert_eq!(equals("(-2147483648).ToString()", "-2147483648"), 1);
}

#[test]
fn uint_and_long_to_string() {
    assert_eq!(equals("((uint)-1).ToString()", "4294967295"), 1);
    assert_eq!(
        equals("(9223372036854775807).ToString()", "9223372036854775807"),
        1
    );
    assert_eq!(
        equals("(-9223372036854775808).ToString()", "-9223372036854775808"),
        1
    );
}

#[test]
fn bool_and_char_to_string() {
    assert_eq!(equals("true.ToString()", "True"), 1);
    assert_eq!(equals("false.ToString()", "False"), 1);
    assert_eq!(equals("((char)65).ToString()", "A"), 1);
}

#[test]
fn interpolation_converts_primitives() {
    assert_eq!(equals("$\"{5}\"", "5"), 1);
    assert_eq!(equals("$\"{-42}\"", "-42"), 1);
    assert_eq!(equals("$\"a{1}b{23}c\"", "a1b23c"), 1);
    assert_eq!(equals("$\"{true}\"", "True"), 1);
}

#[test]
fn string_concatenation_converts_primitives() {
    assert_eq!(equals("\"n=\" + 5", "n=5"), 1);
    assert_eq!(equals("5 + \"!\"", "5!"), 1);
    assert_eq!(equals("\"u=\" + (uint)-1", "u=4294967295"), 1);
    assert_eq!(equals("false + \"!\"", "False!"), 1);
    assert_eq!(equals("(char)66 + \"!\"", "B!"), 1);
}

#[test]
fn unsupported_primitive_conversion_is_reported() {
    let src = program("double d = 1.5; return d.ToString() == \"1.5\" ? 1 : 0;");
    let mut cmd = Command::new(rubyc_path());
    cmd.args(["build", "-"]);
    let out = run(&mut cmd, src.as_bytes());
    assert_ne!(out.code, 0, "float conversion must be reported");
}

#[test]
fn ulong_to_string_covers_the_unsigned_range() {
    // The decimal routine divides with `div` for unsigned values, so
    // u64::MAX renders as 20 digits instead of a negative number.
    assert_eq!(equals("((ulong)-1).ToString()", "18446744073709551615"), 1);
    assert_eq!(equals("((ulong)9000000000).ToString()", "9000000000"), 1);
}
