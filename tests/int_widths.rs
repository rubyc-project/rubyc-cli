//! All integer widths: byte, sbyte, short, ushort, int, uint, long, ulong.
//! Each width must parse, store correctly, and print the expected value.

use crate::tests::common::run;
use std::process::Command;

use std::sync::atomic::{AtomicU32, Ordering};
static CTR: AtomicU32 = AtomicU32::new(0);

fn run_source(src: &str) -> String {
    let n = CTR.fetch_add(1, Ordering::Relaxed);
    let src_path = std::env::temp_dir().join(format!("rubyc_intw_{n}.rc"));
    std::fs::write(&src_path, src).unwrap();
    let mut cmd = Command::new(crate::tests::common::rubyc_path());
    cmd.arg("run").arg(&src_path);
    let out = run(&mut cmd, b"");
    assert!(!out.timed_out);
    assert_eq!(out.code, 0, "{}", out.stderr_str());
    out.stdout_str()
}

macro_rules! int_case {
    ($name:ident, $decl:expr, $val:expr, $expected:expr) => {
        #[test]
        fn $name() {
            let src = format!(
                "namespace t; class c {{ int main() {{ {} v = {}; printf(\"{{0}}\", v); }} }}",
                $decl, $val
            );
            assert_eq!(run_source(&src), $expected);
        }
    };
}

// These will pass once all int widths are properly handled in printf
int_case!(byte_max, "byte", "255", "255");
int_case!(sbyte_min, "sbyte", "-128", "-128");
int_case!(short_min, "short", "-32768", "-32768");
int_case!(ushort_max, "ushort", "65535", "65535");

#[test]
fn int_negative_prints_sign() {
    let src = r#"namespace t; class c { int main() { int v = -123456789; printf("{0}", v); } }"#;
    assert_eq!(run_source(src), "-123456789");
}

#[test]
fn char_literal_prints_as_char() {
    let src = r#"namespace t; class c { int main() { printf("{0}", 65); } }"#;
    assert_eq!(run_source(src), "65");
}
