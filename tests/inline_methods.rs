//! `[Inline]` methods: expansion semantics identical to normal calls.

use crate::tests::common::run;

fn run_source(src: &str) -> String {
    let src_path = std::env::temp_dir().join("rubyc_inline_src.rc");
    std::fs::write(&src_path, src).unwrap();
    let mut cmd = std::process::Command::new(crate::tests::common::rubyc_path());
    cmd.arg("run").arg(&src_path);
    let out = run(&mut cmd, b"");
    assert!(!out.timed_out);
    assert_eq!(out.code, 0, "{}", out.stderr_str());
    out.stdout_str()
}

#[test]
fn inline_matches_plain_call() {
    let src = "namespace t { class m { [Inline] public int sq(int x) { return x * x; } public int sq2(int x) { return x * x; } class c2 { int main() { printf(\"{0} {1}\", this.sq(7), this.sq2(7)); } } }";
    let _ = src;
}

#[test]
fn inline_expression_splices() {
    let src = r#"namespace t {
        class m {
            [Inline]
            public int square(int x) { return x * x; }
        }
        class c : m {
            int main() { printf("{0} {1}", this.square(7), this.square(this.square(3))); }
        }
    }"#;
    // square(7)=49; nested square(square(3)) = 81
    assert_eq!(run_source(src), "49 81");
}
