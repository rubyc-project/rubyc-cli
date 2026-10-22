//! Lint infrastructure tests: rule levels, CLI overrides, and pipeline
//! behavior (warnings never fail builds unless denied).

use std::io::Write;
use std::process::{Command, Stdio};

fn run_with_lint_flags(source: &str, flags: &[&str]) -> (i32, String) {
    let mut child = Command::new(crate::tests::common::rubyc_path())
        .args(["run"])
        .args(flags)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // The child may exit before reading stdin (an invalid CLI flag is
    // exactly what one test asserts), which closes the pipe. That is not
    // a test failure: the exit code is the result under test.
    let _ = child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(source.as_bytes());
    let out = child.wait_with_output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stderr),
            String::from_utf8_lossy(&out.stdout)
        ),
    )
}

const DIRTY_SOURCE: &str = r#"namespace t {
    class lib { int dead = 1; }
    class app {
        int main() { lib l = new lib(); int unused = 7; printf("ok"); }
    }
}"#;

#[test]
fn w0001_fires_by_default_without_failing() {
    let (code, output) = run_with_lint_flags(DIRTY_SOURCE, &[]);
    assert_eq!(code, 0, "warnings must not fail builds");
    assert!(output.contains("🐕🇨-W0001"), "{output}");
    assert!(output.contains("'unused'"), "{output}");
}

#[test]
fn w0002_fires_for_unused_private_field() {
    let (code, output) = run_with_lint_flags(DIRTY_SOURCE, &["--allow", "W0001"]);
    assert_eq!(code, 0);
    assert!(output.contains("🐕🇨-W0002"), "{output}");
    assert!(output.contains("'lib.dead'"), "{output}");
}

#[test]
fn deny_upgrades_warning_to_hard_error() {
    let (code, output) = run_with_lint_flags(DIRTY_SOURCE, &["--deny", "unused-local"]);
    assert_eq!(code, 1, "--deny must fail the build");
    assert!(output.contains("ERROR [🐕🇨-W0001]"), "{output}");
}

#[test]
fn allow_silences_a_rule_completely() {
    let (code, output) =
        run_with_lint_flags(DIRTY_SOURCE, &["--allow", "W0001", "--allow", "W0002"]);
    assert_eq!(code, 0);
    assert!(!output.contains("W000"), "{output}");
}

#[test]
fn later_flag_wins_over_earlier_one() {
    let (_, output) = run_with_lint_flags(DIRTY_SOURCE, &["--deny", "W0001", "--allow", "W0001"]);
    assert!(!output.contains("error[W0001]"), "{output}");
}

#[test]
fn unknown_lint_key_is_rejected() {
    let (code, _) = run_with_lint_flags(DIRTY_SOURCE, &["--warn", "no-such-lint"]);
    assert_eq!(code, 2, "CLI parse errors exit 2");
}

#[test]
fn used_members_and_locals_are_never_flagged() {
    let source = r#"namespace t; class c {
        int used = 4;
        int Twice() { return this.used * 2; }
        int main() { int x = 3; printf("{0} {1}", x, this.Twice()); }
    }"#;
    let (code, output) = run_with_lint_flags(source, &[]);
    assert_eq!(code, 0);
    assert!(!output.contains("warning["), "{output}");
}

#[test]
fn a_delegate_local_invoked_by_name_is_used() {
    // `f(2)` reads `f`. The read collector pushed only the ARGUMENTS of a
    // call, so a delegate held in a local was reported as "declared but
    // never used" while every call through it worked.
    let source = r#"namespace t; class c {
        int main() {
            System.Func<int, int> f = x => x + 1;
            System.Func<int, int> g = static (a) => a * 2;
            printf("{0}", f(2) + g(3));
            return 0;
        }
    }"#;
    let (code, output) = run_with_lint_flags(source, &[]);
    assert_eq!(code, 0);
    assert!(!output.contains("W0001"), "{output}");
}

#[test]
fn public_members_are_exempt_from_w0002() {
    let source = r#"namespace t; class c {
        public int exposed = 1;
        public int Api() { return 2; }
        int main() { printf("ok"); }
    }"#;
    let (_, output) = run_with_lint_flags(source, &[]);
    assert!(!output.contains("W0002"), "{output}");
}

#[test]
fn a_private_generic_method_called_with_arguments_is_used() {
    // The callee was recorded with its generic suffix (`One<string>`),
    // so `One` looked unreferenced and the conformance corpus printed a
    // false "never used" warning.
    let source = r#"namespace t; class c {
        private static int One<T>(T v) where T : class { return 1; }
        private int Two<T>(T v) where T : class { return 2; }
        int main() {
            int n = One<string>("x") + this.Two<string>("y");
            printf("{0}", n);
            return 0;
        }
    }"#;
    let (code, output) = run_with_lint_flags(source, &[]);
    assert_eq!(code, 0);
    assert!(!output.contains("W0002"), "{output}");
}
