//! Access modifier enforcement with C# semantics:
//! - `public` — accessible from anywhere
//! - `private` — declaring class only
//! - `protected` — declaring class and derived classes
//! - no modifier (`Default`) — behaves like `private` (C# default)
//!
//! Positive cases run the generated code end-to-end; violations assert the
//! stable diagnostic codes (E0017 private/default, E0018 protected).

use std::process::Command;

/// Compile+JIT through the real binary; returns Err(diagnostic text).
fn compile_error(source: &str) -> Result<(), String> {
    let mut cmd = Command::new(crate::tests::common::rubyc_path());
    cmd.arg("run");
    let out = crate::tests::common::run(&mut cmd, source.as_bytes());
    if !out.timed_out && out.code == 0 {
        Ok(())
    } else {
        Err(out.stderr_str())
    }
}

fn runs_clean(source: &str) {
    compile_error(source).unwrap_or_else(|e| panic!("expected success, got: {e}"));
}

fn denied(source: &str, code: &str) {
    match compile_error(source) {
        Ok(()) => panic!("expected {code} denial, but compilation succeeded"),
        Err(msg) => {
            assert!(
                msg.contains(code),
                "expected {code} in diagnostics, got: {msg}"
            );
        }
    }
}

// ---------- public ----------

#[test]
fn public_method_is_callable_across_classes() {
    runs_clean(
        r#"namespace t {
            class lib {
                public int Add(int a, int b) { return a + b; }
            }
            class app {
                int main() {
                    lib l = new lib();
                    printf("{0}", l.Add(2, 3));
                }
            }
        }"#,
    );
}

#[test]
fn public_field_is_readable_across_classes() {
    runs_clean(
        r#"namespace t {
            class lib {
                public int value = 9;
            }
            class app {
                int main() {
                    lib l = new lib();
                    printf("{0}", l.value);
                }
            }
        }"#,
    );
}

// ---------- private ----------

#[test]
fn private_method_is_usable_within_its_own_class() {
    runs_clean(
        r#"namespace t; class c {
            private int Secret() { return 11; }
            int main() { printf("{0}", this.Secret()); }
        }"#,
    );
}

#[test]
fn private_method_denied_from_another_class() {
    denied(
        r#"namespace t {
            class lib {
                private int Secret() { return 11; }
            }
            class app {
                int main() { lib l = new lib(); printf("{0}", l.Secret()); }
            }
        }"#,
        "E0017",
    );
}

#[test]
fn private_field_read_denied_from_another_class() {
    denied(
        r#"namespace t {
            class lib { int secret = 1; }
            class app {
                int main() { lib l = new lib(); printf("{0}", l.secret); }
            }
        }"#,
        "E0017",
    );
}

#[test]
fn inherited_private_method_denied_in_derived_class() {
    // C#: a derived class cannot call its base's privates.
    denied(
        r#"namespace t {
            class base0 { private int Hidden() { return 1; } }
            class d : base0 {
                int main() { printf("{0}", this.Hidden()); }
            }
        }"#,
        "E0017",
    );
}

#[test]
fn inherited_private_field_write_denied_in_derived_class() {
    denied(
        r#"namespace t {
            class base0 { int x = 1; }
            class d : base0 {
                int main() { x = 5; }
            }
        }"#,
        "E0017",
    );
}

// ---------- protected ----------

#[test]
fn protected_member_is_usable_in_declaring_class() {
    runs_clean(
        r#"namespace t; class c {
            protected int v = 4;
            protected int V() { return this.v; }
            int main() { printf("{0}", this.V()); }
        }"#,
    );
}

#[test]
fn protected_member_is_usable_through_derivation() {
    runs_clean(
        r#"namespace t {
            class base0 {
                protected int v = 6;
                protected int V() { return this.v; }
            }
            class d : base0 {
                public int Use() { return this.v + this.V(); }
            }
            class app {
                int main() { d x = new d(); printf("{0}", x.Use()); }
            }
        }"#,
    );
}

#[test]
fn protected_method_denied_from_unrelated_class() {
    denied(
        r#"namespace t {
            class base0 { protected int V() { return 1; } }
            class other {
                int main() { base0 b = new base0(); printf("{0}", b.V()); }
            }
        }"#,
        "E0018",
    );
}

#[test]
fn protected_field_denied_from_unrelated_class() {
    denied(
        r#"namespace t {
            class base0 { protected int v = 1; }
            class other {
                int main() { base0 b = new base0(); printf("{0}", b.v); }
            }
        }"#,
        "E0018",
    );
}

#[test]
fn protected_denied_on_sibling_derived_chain_via_base_instance() {
    // C#: protected access must go through the deriving class's own
    // identity, not an arbitrary base instance held elsewhere.
    denied(
        r#"namespace t {
            class base0 { protected int V() { return 1; } }
            class d : base0 { }
            class other {
                int main() { d x = new d(); printf("{0}", x.V()); }
            }
        }"#,
        "E0018",
    );
}

// ---------- default (no modifier) ----------

#[test]
fn unmodified_members_behave_like_private() {
    runs_clean(
        r#"namespace t; class c {
            int v = 3;
            int Triple() { return this.v * 3; }
            int main() { printf("{0}", this.Triple()); }
        }"#,
    );

    denied(
        r#"namespace t {
            class lib { int Value() { return 3; } }
            class app {
                int main() { lib l = new lib(); printf("{0}", l.Value()); }
            }
        }"#,
        "E0017",
    );
}
