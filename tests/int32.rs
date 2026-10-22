//! C# integer widths: `int` arithmetic wraps at 32 bits.
//!
//! Every vreg is a 64-bit slot, so `int x = 2147483647; x + 1` stayed
//! positive. When both operands are `int`, the result is now truncated to
//! 32 bits and sign-extended, and shift counts are masked by 0x1F (C#
//! rules). Mixed `int`/`long` arithmetic stays 64-bit.

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

#[test]
fn int_addition_wraps_to_int_min() {
    assert_eq!(
        exit_code(&program("int x = 2147483647; x = x + 1; return x < 0 ? 1 : 0;")),
        1
    );
}

#[test]
fn int_subtraction_wraps_to_int_max() {
    assert_eq!(
        exit_code(&program("int x = -2147483648; x = x - 1; return x > 0 ? 1 : 0;")),
        1
    );
}

#[test]
fn int_division_wraps_int_min_over_minus_one() {
    // Unchecked C#: INT_MIN / -1 is INT_MIN (wrapped), not a positive
    // 2147483648.
    assert_eq!(
        exit_code(&program("int x = -2147483648; int y = x / -1; return y < 0 ? 1 : 0;")),
        1
    );
}

#[test]
fn int_shifts_mask_their_count() {
    // C# masks the count by 0x1F: 1 << 32 == 1 << 0.
    assert_eq!(exit_code(&program("int z = 1 << 32; return z == 1 ? 1 : 0;")), 1);
    assert_eq!(exit_code(&program("int z = 1 << 31; return z < 0 ? 1 : 0;")), 1);
}

#[test]
fn constant_folded_int_arithmetic_wraps_the_same_way() {
    assert_eq!(
        exit_code(&program("int x = 2147483647 + 1; return x < 0 ? 1 : 0;")),
        1
    );
    assert_eq!(
        exit_code(&program("return (100000 * 100000) == 1410065408 ? 1 : 0;")),
        1
    );
}

#[test]
fn long_arithmetic_is_not_narrowed() {
    assert_eq!(
        exit_code(&program("long a = 100000; long b = 100000; long c = a * b; return c > 4000000000 ? 1 : 0;")),
        1
    );
}

#[test]
fn mixed_int_long_arithmetic_is_64_bit() {
    // Both are 2000000000; an int-narrowed sum would be 4e9 wrapped to
    // -294967296, so `> 2147483648` proves the 64-bit result.
    assert_eq!(
        exit_code(&program("int a = 2000000000; long b = 2000000000; long c = a + b; return c > 2147483648 ? 1 : 0;")),
        1
    );
    assert_eq!(
        exit_code(&program("int a = 2; long b = 3; long c = a + b; return c == 5 ? 1 : 0;")),
        1
    );
}

// ---- uint: 32-bit UNSIGNED ----

#[test]
fn uint_addition_wraps_mod_2_32() {
    assert_eq!(
        exit_code(&program("uint a = 4000000000; uint b = 4000000000; uint c = a + b; return c == 3705032704 ? 1 : 0;")),
        1
    );
}

#[test]
fn uint_multiplication_wraps_mod_2_32() {
    assert_eq!(
        exit_code(&program("uint a = 4000000000; uint b = 2; uint c = a * b; return c == 3705032704 ? 1 : 0;")),
        1
    );
}

#[test]
fn uint_subtraction_underflows_to_uint_max() {
    assert_eq!(
        exit_code(&program("uint a = 1; uint b = 2; uint c = a - b; return c == 4294967295 ? 1 : 0;")),
        1
    );
}

#[test]
fn uint_shift_count_is_masked_by_31() {
    assert_eq!(
        exit_code(&program("uint a = 1; uint c = a << 32; return c == 1 ? 1 : 0;")),
        1
    );
    assert_eq!(
        exit_code(&program("uint a = 1; uint c = a << 31; return c == 2147483648 ? 1 : 0;")),
        1
    );
}

#[test]
fn uint_high_bit_is_positive() {
    assert_eq!(
        exit_code(&program("uint a = 2147483648; return a > 0 ? 1 : 0;")),
        1
    );
}

#[test]
fn long_shift_count_is_masked_by_63() {
    assert_eq!(
        exit_code(&program("long a = 1; long c = a << 64; return c == 1 ? 1 : 0;")),
        1
    );
}

// ---- checked: overflow at the TYPED bounds ----

fn checked_program(op: &str) -> String {
    program(&format!(
        "try {{ checked {{ {op} }} }} catch {{ return 1; }} return 0;"
    ))
}

#[test]
fn checked_int_add_uses_int_bounds() {
    assert_eq!(
        exit_code(&checked_program("int x = 2147483647; int y = x + 1; return y;")),
        1
    );
}

#[test]
fn checked_int_multiply_uses_int_bounds() {
    assert_eq!(
        exit_code(&checked_program("int x = 100000; int y = x * x; return y;")),
        1
    );
}

#[test]
fn checked_uint_add_and_multiply_use_uint_bounds() {
    assert_eq!(
        exit_code(&checked_program("uint a = (uint)-1; uint c = a + 1; return (int)c;")),
        1
    );
    assert_eq!(
        exit_code(&checked_program("uint a = 4000000000; uint b = 2; uint c = a * b; return (int)c;")),
        1
    );
}

#[test]
fn checked_long_add_uses_long_bounds() {
    assert_eq!(
        exit_code(&checked_program("long x = 9223372036854775807; long y = x + 1; return (int)y;")),
        1
    );
}

#[test]
fn checked_arithmetic_still_passes_when_it_fits() {
    assert_eq!(exit_code(&checked_program("int x = 10; int y = x + 1; return y;")), 11);
}

// ---- unsigned comparisons ----

#[test]
fn uint_compares_as_unsigned() {
    assert_eq!(
        exit_code(&program("uint a = 4000000000; return a > 2147483647 ? 1 : 0;")),
        1
    );
    assert_eq!(
        exit_code(&program("uint u = (uint)-1; return u == 4294967295 ? 1 : 0;")),
        1
    );
}

#[test]
fn uint_against_a_negative_int_promotes_to_long() {
    // C# promotes uint+int to long: 1 > -1 is true, 4e9 < -1 is false.
    assert_eq!(
        exit_code(&program("uint u = 1; int i = -1; return u > i ? 1 : 0;")),
        1
    );
    assert_eq!(
        exit_code(&program("uint u = 4000000000; return u < -1 ? 1 : 0;")),
        0
    );
}

#[test]
fn ulong_division_and_remainder_are_unsigned() {
    // The digits of u64::MAX need unsigned `div`; signed `idiv` would
    // answer negative or wrong quotients above i64::MAX.
    assert_eq!(
        exit_code(&program(
            "ulong u = (ulong)-1; return u / 10 == 1844674407370955161 ? 1 : 0;"
        )),
        1
    );
    assert_eq!(
        exit_code(&program("ulong u = (ulong)-1; return u % 10 == 5 ? 1 : 0;")),
        1
    );
}
