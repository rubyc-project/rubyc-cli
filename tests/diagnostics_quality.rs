//! Diagnostics quality gates: syntax errors SAY they are syntax errors,
//! carets point at the offending token, multiple errors surface in one
//! run, and the missing-'=' case gets a dedicated message + fix hint.

use crate::tests::common::{exec, run};
use std::process::Command;

fn compile(src: &str) -> (i32, String) {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let out_path = std::env::temp_dir().join(format!("rubyc_diag_{}.out", n));
    let mut cmd = Command::new(crate::tests::common::rubyc_path());
    cmd.args(["compile", "-o"]).arg(&out_path);
    let out = run(&mut cmd, src.as_bytes());
    let _ = std::fs::remove_file(&out_path);
    assert!(!out.timed_out, "compiler hung on:\n{src}");
    (out.code, out.stderr_str())
}

#[track_caller]
fn caret_column_of(stderr: &str) -> usize {
    // Column the `~~` run starts at, measured past the "  N │ " gutter and
    // counting tabs as 4 columns (the expanded rendering).
    for line in stderr.lines() {
        let body = match line.find("│ ") {
            Some(pos) => {
                // `pos` is a byte index; advance to the char boundary just
                // after the gutter ("│ " is 2 chars = 5 bytes).
                let mut i = pos + 1;
                while !line.is_char_boundary(i) {
                    i += 1;
                }
                i += 1; // skip the space
                &line[i..]
            }
            None => continue,
        };
        if let Some(idx) = body.find('~') {
            // Convert 0-based body index to a 1-based visual column,
            // expanding tabs to 4.
            let mut col = 1;
            for ch in body.chars().take(idx) {
                col += if ch == '\t' { 4 } else { 1 };
            }
            return col;
        }
    }
    panic!("no caret line in:\n{stderr}");
}

#[test]
fn stray_statement_is_a_labeled_syntax_error_with_pointing_caret() {
    let src = "namespace t\n{\n    class c\n    {\n        Main()\n        {\n            int i = 10;\n            boom\n        }\n    }\n}\n";
    let (code, err) = compile(src);
    assert_ne!(code, 0);
    assert!(
        err.contains("syntax error:"),
        "must say 'syntax error':\n{err}"
    );
    assert!(err.contains("';'"), "message names the semicolon:\n{err}");
    assert!(
        err.contains("<stdin>:8"),
        "points at the stray statement line:\n{err}"
    );
}

#[test]
fn missing_equals_gets_dedicated_message_and_help() {
    let src = "namespace t { class c { int main() { int n  10; } } }";
    let (code, err) = compile(src);
    assert_ne!(code, 0);
    assert!(err.contains("E0002"), "dedicated code:\n{err}");
    assert!(
        err.contains("missing '=' in declaration of 'n'"),
        "human message:\n{err}"
    );
    assert!(
        err.contains("help: write `int n = <value>;`"),
        "fix hint:\n{err}"
    );
    // The `~~` run starts under `10` — visual column 99 in this one-liner
    // (98 chars of `namespace t { ... int n  ` before the value).
    assert_eq!(
        caret_column_of(&err),
        99,
        "caret must point at the value where '=' is missing:\n{err}"
    );
}

#[test]
fn multiple_errors_are_all_reported() {
    let src = "namespace t { class c {\n    int main() {\n        int a = 1;\n        int b  2;\n        boom\n        int d = 4;\n        printf(\"{0}\", d);\n    }\n} }";
    let (_, err) = compile(src);
    let count = err.matches("ERROR [🐕🇨-E").count();
    assert!(count >= 2, "expected ≥2 diagnostics, got {count}:\n{err}");
    assert!(err.contains("E0002"), "first mistake reported:\n{err}");
    assert!(err.contains("';'"), "second mistake reported:\n{err}");
}

#[test]
fn json_output_carries_syntax_label_and_fields() {
    let mut cmd = Command::new(crate::tests::common::rubyc_path());
    cmd.args(["compile", "-o", "/dev/null", "--error-format", "json"]);
    let out = run(
        &mut cmd,
        b"namespace t { class c { int main() { int n  10; } } }",
    );
    assert!(!out.timed_out && out.code != 0);
    let js = out.stdout_str() + &out.stderr_str();
    assert!(js.contains("\"code\":\"E0002\""), "{js}");
    assert!(js.contains("syntax error:"), "{js}");
    assert!(
        js.contains("\"line\":1") && js.contains("\"column\":"),
        "{js}"
    );
}

#[test]
fn valid_program_produces_no_diagnostics() {
    let (code, err) = compile("namespace t; class c { int main() { printf(\"ok\"); } }");
    assert_eq!(code, 0, "{err}");
    assert!(!err.contains("error["), "{err}");
}

#[test]
fn human_token_names_in_messages() {
    let (_, err) = compile("namespace t { class c { int main() { if true ) { } } } }");
    assert!(
        err.contains("'('"),
        "punctuation should read as punctuation:\n{err}"
    );
    assert!(!err.contains("LeftParen"), "no Debug enum leakage:\n{err}");
}

// Keep the exec helper referenced so the import stays honest across
// future edits to this file.
#[test]
fn helper_wiring() {
    let mut cmd = Command::new(crate::tests::common::rubyc_path());
    cmd.arg("--help");
    let out = exec(&mut cmd);
    assert!(out.code == 0 || out.code == 2);
}

// ---------- user-reported regression cases (tab-indented, multi-error) ----------

#[test]
fn missing_semicolon_points_at_insertion_spot_on_previous_line() {
    // Exact user repro: printf("i") without ';', closing brace next line.
    let src = "namespace t\n{\n\tclass c\n\t{\n\t\tvoid main()\n\t\t{\n\t\t\tint i  10;\n\t\t\tdas;\n\t\t\tagdasaagdfs();\n\t\t\tprintf(\"i\")\n\t\t}\n\t}\n}\n";
    let (_, err) = compile(src);
    assert!(err.contains("missing ';' after statement"), "{err}");
    // The location line for this diagnostic must be `<stdin>:10` (the
    // printf line), not `:11` ('}').
    let mut saw_line10 = false;
    for line in err.lines() {
        if line.trim() == "<stdin>:10" {
            saw_line10 = true;
            break;
        }
    }
    assert!(saw_line10, "missing-; must point at line 10:\n{err}");
}

#[test]
fn tabs_expand_so_carets_align() {
    // E0002 on a tab-indented declaration: caret column measured in the
    // EXPANDED rendering (tabs = 4 columns) must land under `10`.
    let src =
        "namespace t\n{\n\tclass c\n\t{\n\t\tvoid main()\n\t\t{\n\t\t\tint i  10;\n\t\t}\n\t}\n}\n";
    let (_, err) = compile(src);
    assert!(err.contains("E0002"), "{err}");
    // Line: two tabs + "int i  10;" → the `~~` run sits directly under `10`.
    // In the expanded rendering (tabs = 4 cols) the `~` is at 1-based visual
    // column 43.
    assert_eq!(
        caret_column_of(&err),
        43,
        "caret under 10 with expanded tabs:\n{err}"
    );
}

#[test]
fn semantic_errors_surface_alongside_syntax_errors() {
    let src = "namespace t\n{\n\tclass c\n\t{\n\t\tvoid main()\n\t\t{\n\t\t\tint i  10;\n\t\t\tdas;\n\t\t\tagdasaagdfs();\n\t\t\tprintf(\"i\")\n\t\t}\n\t}\n}\n";
    let (_, err) = compile(src);
    assert!(err.contains("E0025"), "unknown name reported:\n{err}");
    assert!(err.contains("unknown name 'das'"), "{err}");
    assert!(err.contains("E0026"), "unknown function reported:\n{err}");
    assert!(err.contains("unknown function 'agdasaagdfs'"), "{err}");
    let total = err.matches("ERROR [🐕🇨-E").count();
    assert_eq!(total, 4, "all four diagnostics present:\n{err}");
}

#[test]
fn unknown_function_reported_without_syntax_errors() {
    let src = "namespace t; class c { int main() { nosuchfn(1); } }";
    let (_, err) = compile(src);
    assert!(err.contains("E0026"), "{err}");
    assert!(err.contains("unknown function 'nosuchfn'"), "{err}");
}

#[test]
fn unknown_name_gets_did_you_mean() {
    let src = "namespace t { class c { int value = 0; int main() { valu = 1; return value; } } }";
    let (_, err) = compile(src);
    assert!(
        err.contains("did you mean 'value'?"),
        "suggestion expected:\n{err}"
    );
}

#[test]
fn unknown_function_gets_did_you_mean() {
    let src = "namespace t { class c { int main() { printfx(\"x\"); } } }";
    let (_, err) = compile(src);
    assert!(err.contains("did you mean 'printf'?"), "{}", err);
}
