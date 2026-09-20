//! Deep executable tests, executed on the rubyc-dev VM.
//!
//! Unlike [`super::native_exec`] (which runs artifacts locally), every
//! test here builds the ELF locally, ships it over scp, runs it over
//! ssh, and asserts on the captured result — per the project rule that
//! compiled executables never run on the host.

use crate::tests::common::vm::vm_exec;
use rubyc::native;
use rubyc::parser;
use rubyc_target_linux_x86_64::{BACKEND, IMAGE_WRITER};

static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Compile `source` to a local ELF, run it on the VM, return the outcome.
fn build_and_run_vm(
    source: &str,
    args: &[&str],
    stdin: Option<&[u8]>,
) -> crate::tests::common::Outcome {
    let unit = parser::parse(source).expect("must parse");
    let dir = std::env::temp_dir().join("rubyc_vm_exe_test");
    std::fs::create_dir_all(&dir).unwrap();
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let exe = dir.join(format!("vmexe-{}_{n}", std::process::id()));
    native::compile_to_native(&unit, &BACKEND, &IMAGE_WRITER, &exe)
        .expect("compilation should succeed");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
    let out = vm_exec(&exe, args, stdin);
    let _ = std::fs::remove_file(&exe);
    out
}

fn run_src(source: &str) -> crate::tests::common::Outcome {
    build_and_run_vm(source, &[], None)
}

#[test]
fn vm_exit_zero() {
    let out = run_src("namespace t { class c { int main() { return 0; } } }");
    assert_eq!(out.code, 0, "stderr: {}", out.stderr_str());
}

#[test]
fn vm_exit_one() {
    let out = run_src("namespace t { class c { int main() { return 1; } } }");
    assert_eq!(out.code, 1, "stderr: {}", out.stderr_str());
}

#[test]
fn vm_exit_42() {
    let out = run_src("namespace t { class c { int main() { return 42; } } }");
    assert_eq!(out.code, 42, "stderr: {}", out.stderr_str());
}

#[test]
fn vm_exit_255() {
    let out = run_src("namespace t { class c { int main() { return 255; } } }");
    assert_eq!(out.code, 255, "stderr: {}", out.stderr_str());
}

#[test]
fn vm_stdout_hello_exact() {
    let out = run_src(
        r#"namespace t { class c { int main() { printf("Hello World!"); return 0; } } }"#,
    );
    assert_eq!(out.code, 0, "stderr: {}", out.stderr_str());
    assert_eq!(out.stdout_str(), "Hello World!");
}

#[test]
fn vm_stdout_multi_write_concatenates() {
    let out = run_src(
        r#"namespace t { class c { int main() { printf("a"); printf("b"); printf("c"); return 0; } } }"#,
    );
    assert_eq!(out.code, 0, "stderr: {}", out.stderr_str());
    assert_eq!(out.stdout_str(), "abc");
}

#[test]
fn vm_stdout_negative_and_large_ints() {
    let out = run_src(
        r#"namespace t { class c { int main() { printf("{0}|{1}", -42, 123456789); return 0; } } }"#,
    );
    assert_eq!(out.code, 0, "stderr: {}", out.stderr_str());
    assert_eq!(out.stdout_str(), "-42|123456789");
}

#[test]
fn vm_stdin_scan_int() {
    let out = build_and_run_vm(
        r#"namespace t; class c { int main() { int n = scan_int(); printf("{0}", n + 1); return 0; } }"#,
        &[],
        Some(b"41\n"),
    );
    assert_eq!(out.code, 0, "stderr: {}", out.stderr_str());
    assert_eq!(out.stdout_str(), "42");
}

#[test]
fn vm_deep_call_chain_ten() {
    let out = run_src(
        r#"namespace t { class c {
            int f1() { return 1; } int f2() { return f1() + 1; }
            int f3() { return f2() + 1; } int f4() { return f3() + 1; }
            int f5() { return f4() + 1; } int f6() { return f5() + 1; }
            int f7() { return f6() + 1; } int f8() { return f7() + 1; }
            int f9() { return f8() + 1; } int f10() { return f9() + 1; }
            int main() { return f10(); } } }"#,
    );
    assert_eq!(out.code, 10, "stderr: {}", out.stderr_str());
}

#[test]
fn vm_recursion_factorial() {
    let out = run_src(
        r#"namespace t { class c { int fact(int n) { if (n <= 1) { return 1; } return n * fact(n - 1); } int main() { return fact(5); } } }"#,
    );
    assert_eq!(out.code, 120, "stderr: {}", out.stderr_str());
}

#[test]
fn vm_recursion_fib() {
    let out = run_src(
        r#"namespace t { class c { int fib(int n) { if (n <= 1) { return n; } return fib(n - 1) + fib(n - 2); } int main() { return fib(10); } } }"#,
    );
    assert_eq!(out.code, 55, "stderr: {}", out.stderr_str());
}

#[test]
fn vm_float_compare() {
    let out = run_src(
        r#"namespace t { class c { int main() { double x = 1.5 + 2.25; if (x > 3.0) { return 7; } return 8; } } }"#,
    );
    assert_eq!(out.code, 7, "stderr: {}", out.stderr_str());
}

#[test]
fn vm_string_equality() {
    let out = run_src(
        r#"namespace t { class c { int main() { string s = "ab"; if (s == "ab") { return 9; } return 10; } } }"#,
    );
    assert_eq!(out.code, 9, "stderr: {}", out.stderr_str());
}

#[test]
fn vm_int_array_sum() {
    let out = run_src(
        r#"namespace t { class c { int main() { int[] a = new int[4]; a[0] = 5; a[1] = 6; a[2] = 7; a[3] = 8; return a[0] + a[1] + a[2] + a[3]; } } }"#,
    );
    assert_eq!(out.code, 26, "stderr: {}", out.stderr_str());
}

#[test]
fn vm_static_field_init_before_main() {
    let out = run_src(
        r#"namespace t { class c { static int x = 40; int main() { return x + 2; } } }"#,
    );
    assert_eq!(out.code, 42, "stderr: {}", out.stderr_str());
}

#[test]
fn vm_try_catch_int_throw() {
    let out = run_src(
        r#"namespace t { class c { int main() { try { throw 1; } catch { return 7; } return 8; } } }"#,
    );
    assert_eq!(out.code, 7, "stderr: {}", out.stderr_str());
}

#[test]
fn vm_infinite_loop_times_out_remotely() {
    let out = run_src("namespace t { class c { int main() { while (true) { } return 0; } } }");
    // Remote `timeout(1)` kills the guest: exit 124, and OUR side must
    // not report a host timeout (that would mean ssh itself wedged).
    assert_eq!(out.code, 124, "stderr: {}", out.stderr_str());
    assert!(!out.timed_out, "host-side timeout; VM round-trip wedged");
}

#[test]
fn vm_argv_length_counts_prog_plus_user_args() {
    let out = build_and_run_vm(
        r#"namespace t { class c { int main(string[] args) { return args.Length; } } }"#,
        &["a", "bb", "ccc"],
        None,
    );
    assert_eq!(out.code, 4, "stderr: {}", out.stderr_str());
}

#[test]
fn vm_argv_elements_print_verbatim() {
    // Every element must round-trip byte-exact: the argv copy loop
    // once loaded each char into al while rax held the string object,
    // rewriting the pointer's low byte and scattering every element.
    let out = build_and_run_vm(
        r#"namespace t { class c { int main(string[] args) { printf("{0}|{1}", args[1], args[2]); return 0; } } }"#,
        &["hello", "world"],
        None,
    );
    assert_eq!(out.code, 0, "stderr: {}", out.stderr_str());
    assert_eq!(out.stdout_str(), "hello|world");
}

#[test]
fn vm_argv_no_user_args_gives_length_one() {
    let out = build_and_run_vm(
        r#"namespace t { class c { int main(string[] args) { return args.Length; } } }"#,
        &[],
        None,
    );
    assert_eq!(out.code, 1, "stderr: {}", out.stderr_str());
}

#[test]
fn vm_static_archive_links_and_calls() {
    // Static libraries (`.a`) merge member bodies into the executable
    // at build time: no loader, no `.so` at runtime. Uses a no-arg
    // instance import so the test pins linking, not call conventions.
    let dir = std::env::temp_dir().join("rubyc_vm_exe_test");
    std::fs::create_dir_all(&dir).unwrap();
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tag = format!("static-{}_{}", std::process::id(), n);
    let lib_src = dir.join(format!("{tag}.lib.rc"));
    let host_src = dir.join(format!("{tag}.host.rc"));
    let archive = dir.join(format!("{tag}.a"));
    let exe = dir.join(format!("{tag}.exe"));
    std::fs::write(
        &lib_src,
        "namespace testns { class Oracle { public int answer() { return 42; } } }\n",
    )
    .unwrap();
    std::fs::write(
        &host_src,
        "namespace app { class host {\n  [LibraryImport(\"ALIB\", EntryPoint = \"answer\")]\n  int answer();\n  int main() { printf(\"{0}\", answer()); return 0; }\n } }\n"
            .replace("ALIB", &tag),
    )
    .unwrap();
    let run = |args: &[&str]| {
        std::process::Command::new(crate::tests::common::rubyc_path())
            .args(args)
            .output()
            .expect("rubyc binary runs locally (compiler only, never test artifacts)")
    };
    let lib_out = run(&[
        "compile", "-O", "static",
        lib_src.to_str().unwrap(), "-o", archive.to_str().unwrap(),
    ]);
    assert!(
        lib_out.status.success(),
        "static lib build failed: {}",
        String::from_utf8_lossy(&lib_out.stderr)
    );
    // Member name comes from `-o` stem; the host's hint matches it.
    let host_out = run(&[
        "compile", "--lib", archive.to_str().unwrap(),
        host_src.to_str().unwrap(), "-o", exe.to_str().unwrap(),
    ]);
    assert!(
        host_out.status.success(),
        "static link failed: {}",
        String::from_utf8_lossy(&host_out.stderr)
    );
    let out = vm_exec(&exe, &[], None);
    let _ = std::fs::remove_file(&lib_src);
    let _ = std::fs::remove_file(&host_src);
    let _ = std::fs::remove_file(&archive);
    let _ = std::fs::remove_file(&exe);
    assert_eq!(out.code, 0, "stderr: {}", out.stderr_str());
    assert_eq!(out.stdout_str(), "42");
}
