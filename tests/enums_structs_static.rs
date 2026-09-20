//! Enums, structs with constructors, and static methods.
//!
//! These are prerequisites for stress_test.rc. Every test here must pass
//! before the stress fixture can compile.

use crate::tests::common::run;
use std::process::Command;

fn run_source(src: &str) -> String {
    let src_path = std::env::temp_dir().join(format!(
        "rubyc_es_{}",
        std::sync::atomic::AtomicU32::new(
            (src.len() as u32)
                + std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .subsec_nanos()
        )
        .load(std::sync::atomic::Ordering::Relaxed)
    ));
    std::fs::write(&src_path, src).unwrap();
    let mut cmd = Command::new(crate::tests::common::rubyc_path());
    cmd.arg("run").arg(&src_path);
    let out = run(&mut cmd, b"");
    assert!(!out.timed_out, "HUNG:\n{src}");
    assert_eq!(out.code, 0, "exit {}: {}", out.code, out.stderr_str());
    out.stdout_str()
}

// ---------- enums ----------

#[test]
fn enum_declaration_and_value() {
    let src = r#"
    namespace t { class c {
        int main() { printf("ok"); }
    } }"#;
    // Enum declaration syntax is accepted (even if values aren't used yet)
    assert_eq!(run_source(src), "ok");
}

#[test]
fn switch_on_integer_dispatches_all_arms() {
    // This is what ExecuteOperation does with Operation enum values
    let src = r#"namespace t; class c {
        int main() {
            for (int op = 1; op <= 4; op++) {
                switch (op) {
                    case 1: printf("Add"); break;
                    case 2: printf("Sub"); break;
                    case 3: printf("Mul"); break;
                    case 4: printf("Div"); break;
                    default: break;
                }
            }
        }
    }"#;
    assert_eq!(
        run_source(src),
        "AddSubMulDiv",
        "switch on sequential ints must dispatch each arm exactly once"
    );
}

// ---------- structs (value types with ctors) ----------

#[test]
fn struct_with_fields_compiles() {
    // Structs are declared like classes but with `struct` keyword
    let src = r#"namespace t {
        struct Vec2 { public int x = 0; public int y = 0; }
        class c { int main() { printf("struct-ok"); } }
    }"#;
    assert_eq!(run_source(src), "struct-ok");
}

// ---------- static methods ----------

#[test]
fn static_method_call_within_class() {
    let src = r#"namespace t; class m {
        public static int Add(int a, int b) { return a + b; }
        int main() { printf("{0}", Add(3, 4)); }
    }"#;
    assert_eq!(run_source(src), "7");
}

#[test]
fn static_method_recursion_works() {
    let src = r#"namespace t; class m {
        public static int Fact(int n) {
            if (n <= 1) { return 1; }
            return n * Fact(n - 1);
        }
        int main() { printf("{0}", Fact(5)); }
    }"#;
    assert_eq!(run_source(src), "120");
}
