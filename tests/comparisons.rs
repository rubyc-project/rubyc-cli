//! `bool`, comparison and logical operators, plus the A3 control-flow
//! surface: parsing, bytecode survival, native execution (both JIT and
//! static ELF), and constant folding.
//!
//! All program execution goes through `crate::tests::common::run` / `crate::tests::common::exec` —
//! hard timeout, capped capture — so a miscompiled loop can't take the
//! machine down.

use crate::tests::common::{exec, run};
use rubyc::bytecode::traits::{FromBytecode, ToBytecode};
use rubyc::models::compare::StructuralEq;
use rubyc::parser;

pub(crate) fn run_source(source: &str) -> String {
    let mut cmd = std::process::Command::new(crate::tests::common::rubyc_path());
    cmd.arg("run");
    let out = run(&mut cmd, source.as_bytes());
    assert!(
        !out.timed_out,
        "program hung (killed after {}s): {}",
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

/// Static ELF path — exercises the encoder, not just the JIT.
fn run_native(source: &str) -> String {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let src = std::env::temp_dir().join(format!("rubyc_cmp_{}_{}.rc", std::process::id(), n));
    let bin = std::env::temp_dir().join(format!("rubyc_cmp_{}_{}.bin", std::process::id(), n));
    std::fs::write(&src, source).unwrap();
    let mut build = std::process::Command::new(crate::tests::common::rubyc_path());
    let built = exec(build.args(["build"]).arg(&src).arg("-o").arg(&bin));
    assert!(
        !built.timed_out && built.code == 0,
        "build failed: {}",
        built.stderr_str()
    );
    let mut exe = std::process::Command::new(&bin);
    let out = exec(&mut exe);
    assert!(!out.timed_out, "compiled program hung: {}", source);
    let text = out.stdout_str();
    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&bin);
    text
}

pub(crate) fn program(body: &str) -> String {
    format!("namespace t; class c {{ int main() {{ {body} }} }}")
}

macro_rules! exec_case {
    ($name:ident, $body:expr, $expected:expr) => {
        #[test]
        fn $name() {
            assert_eq!(run_source(&program($body)), $expected);
            assert_eq!(run_native(&program($body)), $expected);
        }
    };
}

// ---------- bool literals and locals ----------

exec_case!(bool_true_prints_one, r#"printf("{0}", true);"#, "1");
exec_case!(bool_false_prints_zero, r#"printf("{0}", false);"#, "0");

exec_case!(
    bool_local_round_trips,
    r#"bool f = true; printf("{0}", f);"#,
    "1"
);

// ---------- comparisons ----------

exec_case!(eq_holds, r#"printf("{0}", 2 + 3 == 5);"#, "1");
exec_case!(eq_fails, r#"printf("{0}", 2 == 3);"#, "0");
exec_case!(ne_works, r#"printf("{0}", 2 != 3);"#, "1");
exec_case!(lt_works, r#"printf("{0}", -7 < 3);"#, "1");
exec_case!(gt_works, r#"printf("{0}", 3 > 7);"#, "0");
exec_case!(le_equal, r#"printf("{0}", 4 <= 4);"#, "1");
exec_case!(ge_greater, r#"printf("{0}", 5 >= 4);"#, "1");
exec_case!(
    comparison_of_variables,
    r#"int a = 10; int b = 3; printf("{0}{1}{2}", a > b, a < b, a == b);"#,
    "100"
);

// ---------- logical operators ----------

exec_case!(
    and_truth_table,
    r#"printf("{0}{1}{2}{3}", true && true, true && false, false && true, false && false);"#,
    "1000"
);
exec_case!(
    or_truth_table,
    r#"printf("{0}{1}{2}{3}", true || true, true || false, false || true, false || false);"#,
    "1110"
);
exec_case!(and_normalizes_ints, r#"printf("{0}", 5 && 7);"#, "1");
exec_case!(or_normalizes_ints, r#"printf("{0}", 0 || -2);"#, "1");

// ---------- unary not ----------

exec_case!(not_bool, r#"printf("{0}{1}", !true, !false);"#, "01");
exec_case!(not_number, r#"int x = 5; printf("{0}{1}", !x, !0);"#, "01");

// ---------- precedence ----------

#[test]
fn precedence_and_binds_tighter_than_or() {
    // true || false && false → true (&& first)
    assert_eq!(
        run_source(&program(r#"printf("{0}", true || false && false);"#)),
        "1"
    );
    // (true || false) && false → false
    assert_eq!(
        run_source(&program(r#"printf("{0}", (true || false) && false);"#)),
        "0"
    );
}

#[test]
fn precedence_comparison_below_arithmetic() {
    // 2 + 3 * 4 == 14 → 1; equality binds looser than +
    assert_eq!(
        run_source(&program(r#"printf("{0}", 2 + 3 * 4 == 14);"#)),
        "1"
    );
    assert_eq!(
        run_source(&program(r#"printf("{0}", 1 + 1 == 2 == true);"#)),
        "1"
    );
}

// ---------- constant folding ----------

#[test]
fn literal_comparisons_fold_at_compile_time() {
    let src = program(r#"int z = 3 < 5; printf("{0}", z);"#);
    assert_eq!(run_source(&src), "1");
}

// ---------- bytecode round-trip ----------

#[test]
fn comparison_expressions_survive_bytecode() {
    let src = program(r#"bool ok = 1 < 2 && !(3 >= 4); printf("{0}", ok);"#);
    let unit = parser::parse(&src).unwrap();
    let encoded = unit.to_bytecode();
    let decoded = rubyc::models::CompilationUnit::from_bytecode(&encoded).unwrap();
    assert!(unit.structural_eq(&decoded));
}
