//! End-to-end CLI tests: spawn the real binary with piped stdio and verify
//! every documented flow works.

use std::io::Write;
use std::process::{Command, Stdio};

const SOURCE: &str = "namespace t; class c { int main() { printf(\"pipe ok\"); } }";

fn rubyc() -> Command {
    // The compiler binary of this very package.
    let mut cmd = Command::new(crate::tests::common::rubyc_path());
    cmd.current_dir(std::env::temp_dir());
    cmd
}

fn run_with_stdin(args: &[&str], stdin_data: &[u8]) -> (i32, Vec<u8>, Vec<u8>) {
    let mut child = rubyc()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn failed");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(stdin_data)
        .expect("write stdin failed");
    let out = child.wait_with_output().expect("wait failed");
    (out.status.code().unwrap_or(-1), out.stdout, out.stderr)
}

#[test]
fn source_piped_to_compile_produces_bytecode_on_stdout() {
    let (code, stdout, stderr) = run_with_stdin(&["compile", "-O", "bytecode"], SOURCE.as_bytes());
    assert_eq!(code, 0, "stderr: {}", String::from_utf8_lossy(&stderr));
    // [EB 08][dog][kind=bytecode][version]
    assert_eq!(&stdout[..2], &[0xEB, 0x08]);
    assert_eq!(stdout[10], 0x01);
}

#[test]
fn bytecode_piped_between_compile_and_run() {
    let (_, rbc, err1) = run_with_stdin(&["compile", "-O", "bytecode"], SOURCE.as_bytes());
    assert_eq!(err1.len(), 0);
    let (code, stdout, _) = run_with_stdin(&["run"], &rbc);
    assert_eq!(code, 0);
    assert_eq!(String::from_utf8_lossy(&stdout), "pipe ok");
}

#[test]
fn shebang_wrapped_bytecode_still_runs_through_stdin() {
    // A saved script: shebang line + standard preamble.
    let (_, rbc, _) = run_with_stdin(&["compile", "-O", "bytecode"], SOURCE.as_bytes());
    let mut wrapped = b"#!/usr/bin/env rubyc\n".to_vec();
    wrapped.extend_from_slice(&rbc);
    let (code, stdout, stderr) = run_with_stdin(&["run"], &wrapped);
    assert_eq!(code, 0, "stderr: {}", String::from_utf8_lossy(&stderr));
    assert_eq!(String::from_utf8_lossy(&stdout), "pipe ok");
}

#[test]
fn base64_bytecode_round_trips_through_text_pipes() {
    let (_, rbc, _) = run_with_stdin(
        &["compile", "-O", "bytecode", "--base64"],
        SOURCE.as_bytes(),
    );
    // Output must be pure ASCII base64.
    assert!(
        rbc.iter()
            .all(|&b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=')
    );

    let (code, stdout, stderr) = run_with_stdin(&["run", "--base64"], &rbc);
    assert_eq!(code, 0, "stderr: {}", String::from_utf8_lossy(&stderr));
    assert_eq!(String::from_utf8_lossy(&stdout), "pipe ok");
}

#[test]
fn build_writes_executable_that_runs() {
    let dir = std::env::temp_dir().join("rubyc_cli_test");
    std::fs::create_dir_all(&dir).unwrap();
    let exe_path = dir.join("cli_built");

    // Piped source, explicit output path.
    let mut child = rubyc()
        .args(["build", "-o"])
        .arg(&exe_path)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(SOURCE.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );

    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&exe_path, std::fs::Permissions::from_mode(0o755)).unwrap();
    let ran = Command::new(&exe_path).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&ran.stdout), "pipe ok");
}

#[test]
fn garbage_input_is_rejected_not_miscompiled() {
    let (code, _, _) = run_with_stdin(&["run", "-I", "bytecode"], b"definitely not bytecode");
    assert_ne!(code, 0);
}

#[test]
fn completion_subcommand_was_removed() {
    let out = Command::new(crate::tests::common::rubyc_path())
        .args(["completion", "bash"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2), "'completion' should be gone");
}

#[test]
fn help_title_carries_the_version() {
    for flag in ["-h", "--help"] {
        let out = Command::new(crate::tests::common::rubyc_path())
            .arg(flag)
            .output()
            .unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        let first = text.lines().next().unwrap_or_default();
        assert_eq!(
            first,
            format!("RubyC Compiler v{}", env!("CARGO_PKG_VERSION")),
            "first help line via {flag}"
        );
    }
}

#[test]
fn preprocessor_error_directive_fails_the_build() {
    // `#error` carries Error severity; the driver used to print every
    // preprocessor diagnostic to stderr and continue, so an unsupported
    // platform branch still produced a runnable binary.
    let src = "namespace t;\n#error this target is not supported\nclass c { int main() { return 0; } }\n";
    let (code, _stdout, stderr) = run_with_stdin(&["run", "-"], src.as_bytes());
    let text = String::from_utf8_lossy(&stderr);
    assert_ne!(code, 0, "#error must fail the build; stderr: {text}");
    assert!(
        text.contains("this target is not supported"),
        "the directive text must reach the user: {text}"
    );
}

#[test]
fn preprocessor_unknown_directive_fails_the_build() {
    let src = "namespace t;\n#frobnicate x\nclass c { int main() { return 0; } }\n";
    let (code, _stdout, stderr) = run_with_stdin(&["run", "-"], src.as_bytes());
    let text = String::from_utf8_lossy(&stderr);
    assert_ne!(code, 0, "an unknown directive must fail the build: {text}");
    assert!(
        text.contains("frobnicate"),
        "names the offending directive: {text}"
    );
}

#[test]
fn preprocessor_warning_directive_only_warns() {
    let src = "namespace t;\n#warning heads up\nclass c { int main() { printf(\"ran\"); } }\n";
    let (code, stdout, stderr) = run_with_stdin(&["run", "-"], src.as_bytes());
    let text = String::from_utf8_lossy(&stderr);
    assert_eq!(code, 0, "#warning must not fail the build: {text}");
    assert!(
        text.contains("heads up"),
        "the warning text must reach the user: {text}"
    );
    assert!(stdout.windows(3).any(|w| w == b"ran"), "program ran");
}

#[test]
fn run_printf_encodes_non_ascii_string_arguments() {
    // The argument is a UTF-16 string object; printf converts it to UTF-8
    // before writing. Shared constants used by the encoder were defined
    // inside one branch and read from a sibling, so `é` came out as the
    // first byte of a three-byte sequence plus uninitialized bytes.
    let src = r#"namespace t; class C { int Main() { printf("D[{0}]", "é"); return 0; } }"#;
    let (code, stdout, stderr) = run_with_stdin(&["run", "-"], src.as_bytes());
    assert_eq!(code, 0, "stderr: {}", String::from_utf8_lossy(&stderr));
    assert_eq!(stdout, "D[é]".as_bytes());
}

#[test]
fn run_supports_main_with_args_and_a_string_local() {
    // `Main(string[] args)` makes @start build argv. The JIT has no kernel
    // argv stack and used to dereference the zero startup slot; a string
    // local in that entry point then faulted as well.
    let src = r#"namespace t; class C { int Main(string[] args) { string e = "é"; return e.Length + args.Length; } }"#;
    let (code, _stdout, stderr) = run_with_stdin(&["run", "-"], src.as_bytes());
    assert_eq!(
        code,
        1,
        "one-char string, no arguments: {}",
        String::from_utf8_lossy(&stderr)
    );
}

#[test]
fn run_encodes_astral_characters_and_lone_surrogates() {
    // A surrogate pair in a string is ONE code point: four UTF-8 bytes and
    // Length 2. An unpaired surrogate is not valid UTF-16 text and becomes
    // U+FFFD, never an encoded surrogate.
    let src = r#"namespace t; class C { int Main() {
        string p = "" + (char)0xD83D + (char)0xDE00;
        string l = "" + (char)0xD800;
        printf("{0}|{1}|{2}|{3}", p, p.Length, l, l.Length);
        return 0;
    } }"#;
    let (code, stdout, stderr) = run_with_stdin(&["run", "-"], src.as_bytes());
    assert_eq!(code, 0, "stderr: {}", String::from_utf8_lossy(&stderr));
    assert_eq!(
        stdout,
        "😀|2|\u{FFFD}|1".as_bytes(),
        "pair combines, lone surrogate replaces"
    );
}

#[test]
fn printf_formats_integers_and_alignment() {
    // Expectations verified against .NET 10: D zero-pads after the sign,
    // X is the type's two's-complement width (short -> 4 digits), F pads
    // with fractions of zero, and alignment is spaces on the requested
    // side.
    let src = r#"namespace t; class C { int Main() {
        printf("[{0:D3}] [{1:X}] [{2:x4}] [{3:D5}] [{4,8:X4}] [{5,-8:D2}] [{6:F2}] [{7:F0}] [{8:D5}] [{9:X}]\n",
            7, -1, 255, -12, 255, 7, 7, 12, -12, (short)-1);
        printf("[{0,5}] [{1,6}] [{2,3}]\n", "ab", true, (char)65);
        long l = -1;
        printf("[{0:X}] [{1:D2}]\n", l, 5);
        ulong u = 18446744073709551615;
        printf("[{0}] [{1}] [{2}] [{3}]\n", u, u / 10, u % 10, u.ToString() == "18446744073709551615" ? 1 : 0);
        long m = -9223372036854775808;
        printf("[{0:D22}]\n", m);
        return 0;
    } }"#;
    let (code, stdout, stderr) = run_with_stdin(&["run", "-"], src.as_bytes());
    assert_eq!(code, 0, "stderr: {}", String::from_utf8_lossy(&stderr));
    assert_eq!(
        String::from_utf8_lossy(&stdout),
        "[007] [FFFFFFFF] [00ff] [-00012] [    00FF] [07      ] [7.00] [12] [-00012] [FFFF]\n\
         [   ab] [  True] [  A]\n\
         [FFFFFFFFFFFFFFFF] [05]\n\
         [18446744073709551615] [1844674407370955161] [5] [1]\n\
         [-0009223372036854775808]\n"
    );
}

#[test]
fn char_literals_are_chars() {
    // The lexer used to hand `'A'` to the parser as an integer, so
    // `var c = 'A'` was an int and `c.ToString()` was "65".
    let src = r#"namespace t; class C {
        static int Pick(char c) { return 1; }
        static int Pick(int c) { return 2; }
        int Main() {
            var c = 'A';
            int a = 'A' + 1;
            char b = (char)('A' + 1);
            string s = "x" + 'A';
            string i = $"{'A'}";
            printf("{0}|{1}|{2}|{3}|{4}", c, c.ToString(), s, i, Pick('A'));
            return (a == 66 && b == 'B' && c == 'A' && Pick('A') == 1) ? 1 : 0;
        } }"#;
    let (code, stdout, stderr) = run_with_stdin(&["run", "-"], src.as_bytes());
    assert_eq!(code, 1, "stderr: {}", String::from_utf8_lossy(&stderr));
    assert_eq!(stdout, b"A|A|xA|A|1");

    // The CHAR tag must survive `compile -O bytecode` as well.
    let (cc, rbc, err) = run_with_stdin(&["compile", "-O", "bytecode"], src.as_bytes());
    assert_eq!(cc, 0, "stderr: {}", String::from_utf8_lossy(&err));
    let (code, stdout, stderr) = run_with_stdin(&["run"], &rbc);
    assert_eq!(code, 1, "stderr: {}", String::from_utf8_lossy(&stderr));
    assert_eq!(stdout, b"A|A|xA|A|1");
}
