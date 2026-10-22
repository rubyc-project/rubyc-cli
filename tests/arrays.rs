//! Bare array initializers keep the declared element width.
//!
//! `int[] a = { 1, 2, 3 };` is parsed by the expression grammar, which
//! has no declared type: the node carried an empty element type, so
//! construction used the 8-byte default while indexed reads used the
//! declared width — `a[1]` answered 0 (the high half of element 0).
//! The declaration fills the element type in.

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

#[test]
fn a_bare_int_initializer_reads_its_elements() {
    assert_eq!(exit_code("namespace t; class C { int Main() { int[] a = { 1, 2, 3 }; return a[1]; } }"), 2);
}

#[test]
fn a_bare_byte_initializer_reads_its_elements() {
    assert_eq!(exit_code("namespace t; class C { int Main() { byte[] a = { 1, 2, 3 }; return a[2]; } }"), 3);
}

#[test]
fn a_bare_char_initializer_reads_its_elements() {
    assert_eq!(exit_code("namespace t; class C { int Main() { char[] c = { 97, 98 }; return c[1] == 98 ? 1 : 0; } }"), 1);
}

#[test]
fn the_new_array_form_is_unchanged() {
    assert_eq!(exit_code("namespace t; class C { int Main() { int[] a = new int[] { 1, 2, 3 }; return a[1]; } }"), 2);
}
