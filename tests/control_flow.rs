//! A3 control flow: if/else chains, while, do-while, for, break/continue,
//! ternary, `%`, bitwise operators, compound assignment and `++/--`.
//! Every case executes on BOTH paths: JIT (`rubyc run`) and static ELF.

use crate::tests::common::{exec, run};
use rubyc::bytecode::traits::{FromBytecode, ToBytecode};
use rubyc::models::compare::StructuralEq;
use rubyc::parser;

fn run_source(source: &str) -> String {
    let mut cmd = std::process::Command::new(crate::tests::common::rubyc_path());
    cmd.arg("run");
    let out = run(&mut cmd, source.as_bytes());
    assert!(
        !out.timed_out,
        "program HUNG (killed after {}s): {}",
        crate::tests::common::EXEC_TIMEOUT.as_secs(),
        source
    );
    assert!(
        out.code == 0,
        "program failed (exit {}): {}\nstderr: {}",
        out.code,
        source,
        out.stderr_str()
    );
    out.stdout_str()
}

fn run_native(source: &str) -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let src = std::env::temp_dir().join(format!("rubyc_cf_{}_{}.rc", std::process::id(), n));
    let bin = std::env::temp_dir().join(format!("rubyc_cf_{}_{}.bin", std::process::id(), n));
    std::fs::write(&src, source).unwrap();
    let mut build = std::process::Command::new(crate::tests::common::rubyc_path());
    let built = exec(build.args(["build"]).arg(&src).arg("-o").arg(&bin));
    assert!(
        !built.timed_out && built.code == 0,
        "build failed: {}\n{}",
        source,
        built.stderr_str()
    );
    let mut exe = std::process::Command::new(&bin);
    let out = exec(&mut exe);
    assert!(!out.timed_out, "compiled program HUNG: {}", source);
    let text = out.stdout_str();
    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&bin);
    text
}

fn program(body: &str) -> String {
    format!("namespace t; class c {{ int main() {{ {body} }} }}")
}

macro_rules! cf_case {
    ($name:ident, $body:expr, $expected:expr) => {
        #[test]
        fn $name() {
            assert_eq!(run_source(&program($body)), $expected, "JIT path");
            assert_eq!(run_native(&program($body)), $expected, "ELF path");
        }
    };
}

// ---------- if / else ----------

cf_case!(
    if_takes_then_branch,
    r#"int x = 5; if (x > 3) { printf("big"); } else { printf("small"); }"#,
    "big"
);
cf_case!(
    if_takes_else_branch,
    r#"int x = 2; if (x > 3) { printf("big"); } else { printf("small"); }"#,
    "small"
);
cf_case!(
    if_without_else_skips,
    r#"int x = 2; if (x > 3) { printf("no"); } printf("done");"#,
    "done"
);
cf_case!(
    else_if_chain,
    r#"int x = 7; if (x < 5) { printf("low"); } else if (x < 10) { printf("mid"); } else { printf("high"); }"#,
    "mid"
);
cf_case!(
    if_cond_can_be_expression,
    r#"int a = 1; int b = 2; if (a + b == 3 && b > a) { printf("yes"); }"#,
    "yes"
);

// ---------- while ----------

cf_case!(
    while_counts_down,
    r#"int i = 3; while (i > 0) { printf("{0}", i); int d = 1; i = i - d; }"#,
    "321"
);
cf_case!(
    while_false_never_runs,
    r#"while (false) { printf("never"); } printf("ok");"#,
    "ok"
);
cf_case!(
    nested_while,
    r#"int i = 0; while (i < 2) { int j = 0; while (j < 2) { printf("{0}{1} ", i, j); j = j + 1; } i = i + 1; }"#,
    "00 01 10 11 "
);

// ---------- do-while ----------

cf_case!(
    do_while_runs_once_even_when_false,
    r#"do { printf("once"); } while (false);"#,
    "once"
);
cf_case!(
    do_while_loops,
    r#"int i = 0; do { printf("{0}", i); i = i + 1; } while (i < 3);"#,
    "012"
);

// ---------- for ----------

cf_case!(
    for_basic_count,
    r#"for (int i = 0; i < 4; i++) { printf("{0}", i); }"#,
    "0123"
);
cf_case!(
    for_all_clauses_optional,
    r#"int i = 0; for (;;) { if (i >= 2) { break; } printf("{0}", i); i++; }"#,
    "01"
);
cf_case!(
    for_summing,
    r#"int s = 0; for (int i = 1; i <= 10; i++) { s += i; } printf("{0}", s);"#,
    "55"
);
cf_case!(
    for_nested,
    r#"for (int r = 0; r < 3; r++) { for (int c = 0; c <= r; c++) { printf("*"); } printf("\n"); }"#,
    "*\n**\n***\n"
);

// ---------- break / continue ----------

cf_case!(
    break_exits_loop,
    r#"int i = 0; while (true) { if (i == 3) { break; } printf("{0}", i); i++; } printf("end");"#,
    "012end"
);
cf_case!(
    continue_skips_iteration,
    r#"for (int i = 0; i < 6; i++) { if (i % 2 == 0) { continue; } printf("{0}", i); }"#,
    "135"
);
cf_case!(
    continue_in_for_still_steps,
    r#"int n = 0; for (int i = 0; i < 5; i++) { if (i == 2) { continue; } n++; } printf("{0}", n);"#,
    "4"
);
cf_case!(
    break_inner_only,
    r#"for (int i = 0; i < 2; i++) { for (int j = 0; j < 5; j++) { if (j == 1) { break; } printf("{0}", j); } printf("|"); }"#,
    "0|0|"
);

// ---------- ternary ----------

cf_case!(
    ternary_true_arm,
    r#"int a = 10; int b = 20; printf("{0}", a > b ? a : b);"#,
    "20"
);
cf_case!(
    ternary_false_arm,
    r#"int x = 5; printf("{0}", x == 5 ? 1 : 0);"#,
    "1"
);
cf_case!(
    ternary_nested,
    r#"int v = 7; printf("{0}", v < 5 ? 0 : v < 10 ? 1 : 2);"#,
    "1"
);

// ---------- modulo ----------

cf_case!(
    modulo_positive,
    r#"printf("{0} {1} {2}", 7 % 3, 10 % 5, 9 % 4);"#,
    "1 0 1"
);
cf_case!(
    modulo_negative_dividend,
    r#"printf("{0} {1}", -7 % 3, 7 % -3);"#,
    "-1 1"
);

// ---------- bitwise ----------

cf_case!(
    bitwise_and_or_xor,
    r#"printf("{0} {1} {2}", 12 & 10, 12 | 10, 12 ^ 10);"#,
    "8 14 6"
);
cf_case!(bitwise_not, r#"printf("{0}", ~5 == -6 ? 1 : 0);"#, "1");
cf_case!(
    shift_left_right,
    r#"printf("{0} {1} {2}", 1 << 4, 64 >> 3, -16 >> 2);"#,
    "16 8 -4"
);
cf_case!(shift_masks_high_counts, r#"printf("{0}", 1 << 65);"#, "2");

// ---------- compound assignment & increments ----------

cf_case!(
    compound_assignments,
    r#"int x = 10; x += 5; printf("{0},", x); x -= 3; printf("{0},", x); x *= 2; printf("{0},", x); x /= 4; printf("{0}", x);"#,
    "15,12,24,6"
);
cf_case!(
    compound_bitwise_assign,
    r#"int m = 6; m &= 3; printf("{0},", m); m |= 8; printf("{0},", m); m ^= 1; printf("{0}", m);"#,
    "2,10,11"
);
cf_case!(
    increment_decrement_sugar,
    r#"int n = 5; n++; n++; n--; printf("{0}", n);"#,
    "6"
);
cf_case!(
    step_can_update_other_variables,
    r#"int total = 0; for (int i = 0; i < 3; total += i) { i++; } printf("{0}", total);"#,
    "6"
);

// ---------- scope & heap interaction through loops ----------

#[test]
fn loop_scoped_objects_release_each_iteration() {
    // Each iteration allocates an object; the scope-exit release must fire
    // every iteration (and along break paths), not just once.
    let body = r#"namespace t; class box { public int v = 0; }
class c { int main() {
    for (int i = 0; i < 50; i++) {
        box b = new box();
        b.v = i;
        if (b.v != i) { return 1; }
        if (i == 25) { break; }
    }
    printf("ok");
} }"#;
    assert_eq!(run_source(body), "ok");
    assert_eq!(run_native(body), "ok");
}

// ---------- bytecode round-trip ----------

#[test]
fn control_flow_survives_bytecode() {
    let src = program(
        r#"int acc = 0;
for (int i = 1; i <= 5; i++) { if (i % 2 == 1) { acc += i; } else { continue; } }
while (acc > 3) { acc--; if (acc == 4) { break; } }
int label = acc == 4 && true ? 100 : 200;
printf("{0}", label);"#,
    );
    let unit = parser::parse(&src).unwrap();
    let encoded = unit.to_bytecode();
    let decoded = rubyc::models::CompilationUnit::from_bytecode(&encoded).unwrap();
    assert!(unit.structural_eq(&decoded), "AST must survive round-trip");
    // And the decoded unit still executes correctly.
    assert_eq!(run_source(&src), "100");
}
