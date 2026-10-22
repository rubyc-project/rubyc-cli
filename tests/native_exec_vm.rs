//! Deep executable tests, executed on the rubyc-dev VM.
//!
//! Unlike [`super::native_exec`] (which runs artifacts locally), every
//! test here builds the ELF locally, ships it over scp, runs it over
//! ssh, and asserts on the captured result — per the project rule that
//! compiled executables never run on the host.

use crate::tests::common::vm::{vm_exec, vm_exec_bundle};
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

/// `--optimize` / `--strip` parity: the optimized and stripped images
/// behave identically on the VM, are never larger, and the stripped
/// image (no `.symtab`) is strictly smaller.
#[test]
fn vm_optimize_strip_parity_and_shrink() {
    use std::os::unix::fs::PermissionsExt;
    let source = r#"namespace t { class c {
        int main() { printf("opt"); return 0; }
        static int dead() { return 1; }
    } }"#;
    let unit = parser::parse(source).expect("must parse");
    let dir = std::env::temp_dir().join("rubyc_vm_exe_test");
    std::fs::create_dir_all(&dir).unwrap();
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let build = |tag: &str, opts: rubyc::native::EmitOptions| -> std::path::PathBuf {
        let exe = dir.join(format!("vmopt-{}_{}_{n}", std::process::id(), tag));
        let bytes = native::compile_to_bytes_with_libs_preamble_opts(
            &unit,
            &BACKEND,
            &IMAGE_WRITER,
            &[],
            true,
            &[],
            opts,
        )
        .expect("compilation should succeed");
        std::fs::write(&exe, &bytes).unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        exe
    };
    let plain = build(
        "plain",
        rubyc::native::EmitOptions {
            optimize: false,
            strip: false,
        },
    );
    let opt = build(
        "opt",
        rubyc::native::EmitOptions {
            optimize: true,
            strip: false,
        },
    );
    let stripped = build(
        "stripped",
        rubyc::native::EmitOptions {
            optimize: false,
            strip: true,
        },
    );
    let plain_len = std::fs::metadata(&plain).unwrap().len();
    let opt_len = std::fs::metadata(&opt).unwrap().len();
    let stripped_len = std::fs::metadata(&stripped).unwrap().len();
    assert!(
        opt_len <= plain_len,
        "optimized image never larger: {opt_len} vs {plain_len}"
    );
    assert!(
        stripped_len < plain_len,
        "stripped image strictly smaller: {stripped_len} vs {plain_len}"
    );
    let a = vm_exec(&plain, &[], None);
    let b = vm_exec(&opt, &[], None);
    let c = vm_exec(&stripped, &[], None);
    let _ = std::fs::remove_file(&plain);
    let _ = std::fs::remove_file(&opt);
    let _ = std::fs::remove_file(&stripped);
    assert_eq!(a.code, 0, "plain stderr: {}", a.stderr_str());
    assert_eq!(a.stdout_str(), "opt");
    assert_eq!(b.stdout_str(), a.stdout_str(), "optimize keeps behavior");
    assert_eq!(b.code, a.code);
    assert_eq!(c.stdout_str(), a.stdout_str(), "strip keeps behavior");
    assert_eq!(c.code, a.code);
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
fn vm_argv_length_counts_user_args_only() {
    // Real .NET `Main(string[])` excludes the program name (only
    // `Environment.GetCommandLineArgs()` keeps it); the old pinning
    // recorded whatever the implementation did, not the contract.
    let out = build_and_run_vm(
        r#"namespace t { class c { int main(string[] args) { return args.Length; } } }"#,
        &["a", "bb", "ccc"],
        None,
    );
    assert_eq!(out.code, 3, "stderr: {}", out.stderr_str());
}

#[test]
fn vm_typeof_descriptors_have_identity_and_metadata() {
    // One canonical descriptor per type; names are .NET spellings,
    // and the common Type predicates read descriptor flags.
    let out = build_and_run_vm(
        r#"namespace t; class C {
            static int Main() {
                System.Type a = typeof(int);
                System.Type b = typeof(int);
                System.Type s = typeof(string);
                if (a != b) return 1;
                if (a == s) return 2;
                if (a.Name != "Int32") return 3;
                if (s.Name != "String") return 4;
                if (!a.IsValueType) return 5;
                if (!a.IsPrimitive) return 6;
                if (s.IsPrimitive) return 7;
                if (!s.IsClass) return 8;
                System.Type arr = typeof(int[]);
                if (arr != typeof(int[])) return 9;
                if (!arr.IsArray || !arr.IsSZArray) return 10;
                if (arr.Name != "Int32[]") return 11;
                if (arr.FullName != "System.Int32[]") return 12;
                if (!(a is System.Type)) return 13;
                return 0;
            }
        }"#,
        &[],
        None,
    );
    assert_eq!(out.code, 0, "stderr: {}", out.stderr_str());
}

#[test]
fn vm_is_not_null_pattern_lowers() {
    let out = build_and_run_vm(
        r#"namespace t;
        class C {
            static int Main() {
                object o = "x";
                object n = null;
                if (!(o is not null)) return 1;
                if (n is not null) return 2;
                return 0;
            }
        }"#,
        &[],
        None,
    );
    assert_eq!(out.code, 0, "stderr: {}", out.stderr_str());
}

#[test]
fn vm_unsafe_accessor_calls_its_target() {
    // An `[UnsafeAccessor]` extern local function names its target:
    // the attribute carries the method name, the first parameter's
    // `[UnsafeAccessorType]` the target type.
    let out = build_and_run_vm(
        r#"namespace t;
        class Target { public static int Calc(int x) { return x * 2; } }
        class C {
            static int Main() {
                return Doubler(null, 21) == 42 ? 0 : 1;
                [System.Runtime.CompilerServices.UnsafeAccessor(System.Runtime.CompilerServices.UnsafeAccessorKind.StaticMethod, Name = "Calc")]
                static extern int Doubler([System.Runtime.CompilerServices.UnsafeAccessorType("t.Target, t")] object? _, int x);
            }
        }"#,
        &[],
        None,
    );
    assert_eq!(out.code, 0, "stderr: {}", out.stderr_str());
}

#[test]
fn vm_unsafe_accessor_without_target_throws() {
    // A target that is not in the unit cannot be called: the call
    // throws, as .NET does for a missing accessor target.
    let out = build_and_run_vm(
        r#"namespace System { public class MissingMethodException { } }
        namespace t;
        class C {
            static int Main() {
                int r = 0;
                try { r = Missing(null, 1); }
                catch (System.MissingMethodException) { r = 42; }
                return r == 42 ? 0 : 1;
                [System.Runtime.CompilerServices.UnsafeAccessor(System.Runtime.CompilerServices.UnsafeAccessorKind.StaticMethod, Name = "NoSuchMethod")]
                static extern int Missing([System.Runtime.CompilerServices.UnsafeAccessorType("t.Nope, t")] object? _, int x);
            }
        }"#,
        &[],
        None,
    );
    assert_eq!(out.code, 0, "stderr: {}", out.stderr_str());
}

#[test]
fn vm_property_override_and_hiding_follow_csharp() {
    // `override` replaces the base member; `new` HIDES it, so a
    // base-typed receiver still sees the base's implementation.
    let out = build_and_run_vm(
        r#"namespace t;
        class Base { public virtual int P { get { return 1; } } }
        class Hider : Base { public new int P { get { return 2; } } }
        class Overrider : Base { public override int P { get { return 3; } } }
        class C {
            static int Main() {
                Base b = new Hider();
                if (b.P != 1) return 1;
                Base o = new Overrider();
                if (o.P != 3) return 2;
                Hider h = new Hider();
                if (h.P != 2) return 3;
                return 0;
            }
        }"#,
        &[],
        None,
    );
    assert_eq!(out.code, 0, "stderr: {}", out.stderr_str());
}

#[test]
fn vm_get_type_answers_the_runtime_type() {
    // The runtime type, not the static one: a derived instance seen
    // through a base-typed receiver keeps its own type, a boxed value
    // answers its own, and an array answers its element type.
    let out = build_and_run_vm(
        r#"namespace t;
        class Base { }
        class Derived : Base { }
        class C {
            static int Main() {
                Base b = new Derived();
                if (b.GetType().Name != "Derived") return 1;
                Base b2 = new Base();
                if (b2.GetType().Name != "Base") return 2;
                int[] a = new int[2];
                if (a.GetType().Name != "Int32[]") return 3;
                object o = 5;
                if (o.GetType().Name != "Int32") return 4;
                return 0;
            }
        }"#,
        &[],
        None,
    );
    assert_eq!(out.code, 0, "stderr: {}", out.stderr_str());
}

#[test]
fn vm_primitive_boxing_and_unboxing() {
    let out = build_and_run_vm(
        r#"namespace t;
        class Holder { public object Value = 5; public static object StaticValue; }
        class C {
            static object Box(int x) { return x; }
            static int Take(object o) {
                if (o is int v) return v;
                return -1;
            }
            static int Main() {
                object o = 5;
                if (!(o is int)) return 1;
                if (o is long) return 2;
                if (o is string) return 3;
                if (o is int i) { if (i != 5) return 4; }
                if ((int)o != 5) return 5;
                object n = null;
                if (n is int) return 6;
                long l = 9;
                object lo = l;
                if (!(lo is long)) return 7;
                if (lo is int) return 8;
                double d = 1.5;
                object dobj = d;
                if (!(dobj is double)) return 9;
                bool b = true;
                object bobj = b;
                if (!(bobj is bool)) return 10;
                char ch = 'A';
                object cobj = ch;
                if (!(cobj is char)) return 11;
                object s = "hi";
                if (!(s is string)) return 12;
                if (s is int) return 13;
                if (Box(7) is not int) return 14;
                if (Take(8) != 8) return 15;
                if (Take("x") != -1) return 16;
                Holder h = new Holder();
                if (!(h.Value is int)) return 17;
                if ((int)h.Value != 5) return 18;
                Holder.StaticValue = 21;
                if (!(Holder.StaticValue is int)) return 19;
                if ((int)Holder.StaticValue != 21) return 20;
                object[] arr = new object[] { 31, 32 };
                if (!(arr[0] is int) || (int)arr[0] != 31) return 21;
                if (!(arr[1] is int) || (int)arr[1] != 32) return 22;
                object[] sized = new object[1];
                sized[0] = 41;
                if (!(sized[0] is int) || (int)sized[0] != 41) return 23;
                return 0;
            }
        }"#,
        &[],
        None,
    );
    assert_eq!(out.code, 0, "stderr: {}", out.stderr_str());
}

#[test]
fn vm_static_local_function_uses_enclosing_constants() {
    // C# forbids capturing variables in a static local function, but a
    // local constant is not a variable. The constant is visible in the
    // nested function and must retain its value.
    let out = build_and_run_vm(
        r#"namespace t; class C {
            static int Local(int x) {
                const int K = 3;
                const string S = "ok";
                static int Helper() {
                    if (S != "ok") return 1;
                    return K;
                }
                return Helper();
            }
            static int Main() { return Local(0) == 3 ? 0 : 1; }
        }"#,
        &[],
        None,
    );
    assert_eq!(out.code, 0, "stderr: {}", out.stderr_str());
}

#[test]
fn vm_generic_type_handles_reach_methods_and_local_functions() {
    let out = build_and_run_vm(
        r#"namespace t; class C {
            static int Kind<T>() {
                if (typeof(T) == typeof(int)) return 1;
                if (typeof(T) == typeof(string)) return 2;
                return 0;
            }
            static int KindOf<T>(T value) { return Kind<T>(); }
            static int Relay<T>(T value) { return KindOf(value); }
            static int Local<T>(T value) {
                static int Helper() {
                    if (typeof(T) == typeof(int)) return 1;
                    if (typeof(T) == typeof(string)) return 2;
                    return 0;
                }
                return Helper();
            }
            static int ArrayKind<T>() {
                System.Type a = typeof(T[]);
                if (a != typeof(T[])) return 9;
                if (!a.IsArray || !a.IsSZArray) return 10;
                if (typeof(T) == typeof(int) && a.Name != "Int32[]") return 11;
                return 0;
            }
            static int Main() {
                if (Kind<int>() != 1) return 1;
                if (Kind<string>() != 2) return 2;
                if (KindOf(5) != 1) return 3;
                if (KindOf("x") != 2) return 4;
                if (Relay(7) != 1) return 5;
                if (Relay("y") != 2) return 6;
                if (Local(8) != 1) return 7;
                if (Local("z") != 2) return 8;
                if (ArrayKind<int>() != 0) return 12;
                if (ArrayKind<string>() != 0) return 13;
                return 0;
            }
        }"#,
        &[],
        None,
    );
    assert_eq!(out.code, 0, "stderr: {}", out.stderr_str());
}

#[test]
fn vm_is_string_uses_the_refcount_tag() {
    // A string marks the high bit of its refcount word; class objects
    // keep a plain count and null is excluded by the branch. The
    // one-character string matters: a tag stored in the header word
    // would collide with a length of 1.
    let out = build_and_run_vm(
        r#"namespace t { class Box { } class c { int main() {
            object s = "x";
            object n = null;
            object b = new Box();
            object t = s;
            return (s is string ? 1 : 0) + (t is string ? 2 : 0)
                 + (n is string ? 4 : 0) + (b is string ? 8 : 0);
        } } }"#,
        &[],
        None,
    );
    assert_eq!(out.code, 3, "stderr: {}", out.stderr_str());
}

#[test]
fn vm_argv_elements_print_verbatim() {
    // Every element must round-trip byte-exact: the argv copy loop
    // once loaded each char into al while rax held the string object,
    // rewriting the pointer's low byte and scattering every element.
    let out = build_and_run_vm(
        r#"namespace t { class c { int main(string[] args) { printf("{0}|{1}", args[0], args[1]); return 0; } } }"#,
        &["hello", "world"],
        None,
    );
    assert_eq!(out.code, 0, "stderr: {}", out.stderr_str());
    assert_eq!(out.stdout_str(), "hello|world");
}

#[test]
fn vm_argv_no_user_args_gives_empty() {
    let out = build_and_run_vm(
        r#"namespace t { class c { int main(string[] args) { return args.Length; } } }"#,
        &[],
        None,
    );
    assert_eq!(out.code, 0, "stderr: {}", out.stderr_str());
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

/// Differential equivalence: the same program built as a static
/// closure (all sources, one unit) and as a static link (archive +
/// import stubs) must behave identically on the VM. Every artifact is
/// built fresh in-test (unique names) so stale files can never mask
/// a divergence the way they did during development.
fn build_cli(args: &[&str]) -> crate::tests::common::Outcome {
    let mut cmd = std::process::Command::new(crate::tests::common::rubyc_path());
    cmd.args(args);
    let out = cmd.output().expect("rubyc binary runs locally");
    crate::tests::common::Outcome {
        code: out.status.code().unwrap_or(-1),
        stdout: out.stdout,
        stderr: out.stderr,
        timed_out: false,
    }
}

#[test]
fn vm_static_link_matches_closure_int_and_string() {
    let dir = std::env::temp_dir().join("rubyc_vm_exe_test");
    std::fs::create_dir_all(&dir).unwrap();
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tag = format!("difflink-{}_{}", std::process::id(), n);
    let lib_src = dir.join(format!("{tag}.lib.rc"));
    let stub_src = dir.join(format!("{tag}.stub.rc"));
    let closed_src = dir.join(format!("{tag}.closed.rc"));
    let archive = dir.join(format!("{tag}.a"));
    let exe_static = dir.join(format!("{tag}.static.exe"));
    let exe_closed = dir.join(format!("{tag}.closed.exe"));
    std::fs::write(
        &lib_src,
        "namespace LibNS { class Calc { public int answer() { return 42; } public string greet() { return \"hi\"; } } }\n",
    )
    .unwrap();
    std::fs::write(
        &stub_src,
        format!(
            "namespace app {{ class Host {{ [LibraryImport(\"{tag}\", EntryPoint = \"answer\")] int answer(); [LibraryImport(\"{tag}\", EntryPoint = \"greet\")] string greet(); int main() {{ printf(\"{{0}}\", answer()); printf(\"{{0}}\", greet()); return 0; }} }} }}\n"
        ),
    )
    .unwrap();
    std::fs::write(
        &closed_src,
        "namespace LibNS { class Calc { public int answer() { return 42; } public string greet() { return \"hi\"; } } }\nnamespace app { class Host { int main() { LibNS.Calc c = new LibNS.Calc(); printf(\"{0}\", c.answer()); printf(\"{0}\", c.greet()); return 0; } } }\n",
    )
    .unwrap();
    let s = |p: &std::path::Path| p.to_str().unwrap().to_owned();
    let lib_out = build_cli(&["compile", "-O", "static", &s(&lib_src), "-o", &s(&archive)]);
    assert!(lib_out.code == 0, "lib build failed: {}", lib_out.stderr_str());
    let link_out = build_cli(&[
        "compile", "--lib", &s(&archive), &s(&stub_src), "-o", &s(&exe_static),
    ]);
    assert!(link_out.code == 0, "static link failed: {}", link_out.stderr_str());
    let closed_out = build_cli(&["compile", &s(&closed_src), "-o", &s(&exe_closed)]);
    assert!(closed_out.code == 0, "closure build failed: {}", closed_out.stderr_str());
    let a = vm_exec(&exe_static, &[], None);
    let b = vm_exec(&exe_closed, &[], None);
    for f in [&lib_src, &stub_src, &closed_src, &archive, &exe_static, &exe_closed] {
        let _ = std::fs::remove_file(f);
    }
    assert_eq!(a.code, 0, "static stderr: {}", a.stderr_str());
    assert_eq!(b.code, 0, "closed stderr: {}", b.stderr_str());
    assert_eq!(a.stdout_str(), "42hi");
    assert_eq!(a.stdout_str(), b.stdout_str(), "linkage must not change behavior");
}

#[test]
fn vm_static_link_transitive_matches_closure() {
    // main -> Mid.run (archive A) -> Base.fetch (archive B): the
    // fixpoint must merge both archives; the closure variant merges
    // sources. Qualified entry points (immune to simple-name rules).
    let dir = std::env::temp_dir().join("rubyc_vm_exe_test");
    std::fs::create_dir_all(&dir).unwrap();
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tag = format!("difftra-{}_{}", std::process::id(), n);
    let (libb_src, liba_src, stub_src, closed_src) = (
        dir.join(format!("{tag}.b.rc")),
        dir.join(format!("{tag}.a.rc")),
        dir.join(format!("{tag}.stub.rc")),
        dir.join(format!("{tag}.closed.rc")),
    );
    let (arch_b, arch_a) = (
        dir.join(format!("{tag}b.a")),
        dir.join(format!("{tag}a.a")),
    );
    let (exe_static, exe_closed) = (
        dir.join(format!("{tag}.static.exe")),
        dir.join(format!("{tag}.closed.exe")),
    );
    std::fs::write(
        &libb_src,
        "namespace LibB { class Base { public int fetch() { return 7; } } }\n",
    )
    .unwrap();
    std::fs::write(
        &liba_src,
        format!(
            "namespace LibA {{ class Mid {{ [LibraryImport(\"{tag}b\", EntryPoint = \"LibB.Base::fetch#0\")] int fetch(); public int run() {{ return fetch(); }} }} }}\n"
        ),
    )
    .unwrap();
    std::fs::write(
        &stub_src,
        format!(
            "namespace app {{ class Host {{ [LibraryImport(\"{tag}a\", EntryPoint = \"LibA.Mid::run#0\")] int run(); int main() {{ printf(\"{{0}}\", run()); return 0; }} }} }}\n"
        ),
    )
    .unwrap();
    std::fs::write(
        &closed_src,
        "namespace LibB { class Base { public int fetch() { return 7; } } }\nnamespace LibA { class Mid { public int run() { LibB.Base b = new LibB.Base(); return b.fetch(); } } }\nnamespace app { class Host { int main() { LibA.Mid m = new LibA.Mid(); printf(\"{0}\", m.run()); return 0; } } }\n",
    )
    .unwrap();
    let s = |p: &std::path::Path| p.to_str().unwrap().to_owned();
    let b_out = build_cli(&["compile", "-O", "static", &s(&libb_src), "-o", &s(&arch_b)]);
    assert!(b_out.code == 0, "libB build failed: {}", b_out.stderr_str());
    let a_out = build_cli(&["compile", "-O", "static", &s(&liba_src), "-o", &s(&arch_a)]);
    assert!(a_out.code == 0, "libA build failed: {}", a_out.stderr_str());
    let link_out = build_cli(&[
        "compile", "--lib", &s(&arch_a), "--lib", &s(&arch_b), &s(&stub_src), "-o",
        &s(&exe_static),
    ]);
    assert!(link_out.code == 0, "transitive link failed: {}", link_out.stderr_str());
    let closed_out = build_cli(&["compile", &s(&closed_src), "-o", &s(&exe_closed)]);
    assert!(closed_out.code == 0, "closure build failed: {}", closed_out.stderr_str());
    let a = vm_exec(&exe_static, &[], None);
    let b = vm_exec(&exe_closed, &[], None);
    for f in [&libb_src, &liba_src, &stub_src, &closed_src, &arch_b, &arch_a, &exe_static, &exe_closed] {
        let _ = std::fs::remove_file(f);
    }
    assert_eq!(a.code, 0, "static stderr: {}", a.stderr_str());
    assert_eq!(b.code, 0, "closed stderr: {}", b.stderr_str());
    assert_eq!(a.stdout_str(), "7");
    assert_eq!(a.stdout_str(), b.stdout_str(), "linkage must not change behavior");
}

#[test]
fn vm_static_link_scale_matches_closure() {
    // Linker at volume: 4 archives x 75 functions with cross-archive
    // chains (D->C->B->A->main), string + int traffic. The closure
    // variant merges all sources; both must agree on the VM.
    const PER: usize = 75;
    const ARCHIVES: usize = 4;
    let dir = std::env::temp_dir().join("rubyc_vm_exe_test");
    std::fs::create_dir_all(&dir).unwrap();
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tag = format!("diffscale-{}_{}", std::process::id(), n);
    let s = |p: &std::path::Path| p.to_str().unwrap().to_owned();
    // Archive variant: chain() goes through an import stub.
    // Closure variant: chain() calls the next class directly.
    // Both print chain(), f0_0, f1_37, f2_74, f3_11 = "0" "0" "37" "74" "11".
    let mut arch_paths = Vec::new();
    let mut closed_src = String::new();
    for k in 0..ARCHIVES {
        let mut fns = String::new();
        for i in 0..PER {
            fns.push_str(&format!("public int f{k}_{i}() {{ return {i}; }}\n"));
        }
        let stub = format!(
            "[LibraryImport(\"scale-{tag}-{0}\", EntryPoint = \"Scale{0}.C{0}::f{0}_0#0\")] int next();\n",
            k + 1,
        );
        let direct = if k + 1 < ARCHIVES {
            format!(
                "public int chain() {{ Scale{0}.C{0} c = new Scale{0}.C{0}(); return c.f{0}_0(); }}\n",
                k + 1,
            )
        } else {
            String::new()
        };
        let chain_stub = if k + 1 < ARCHIVES {
            format!("public int chain() {{ return next(); }}\n")
        } else {
            String::new()
        };
        let src_path = dir.join(format!("{tag}.s{k}.rc"));
        std::fs::write(
            &src_path,
            format!("namespace Scale{k} {{ class C{k} {{\n{fns}{stub}{chain_stub}}} }}\n"),
        )
        .unwrap();
        closed_src.push_str(&format!("namespace Scale{k} {{ class C{k} {{\n{fns}{direct}}} }}\n"));
        let arch = dir.join(format!("scale-{tag}-{k}.a"));
        let out = build_cli(&["compile", "-O", "static", &s(&src_path), "-o", &s(&arch)]);
        assert!(out.code == 0, "archive {k} build failed: {}", out.stderr_str());
        arch_paths.push((src_path, arch));
    }
    // Static main: chain() on archive 0 + direct stubs for spread functions.
    // Prints chain(), f0_0, f1_37, f2_74, f3_11 = "0" "0" "37" "74" "11".
    let mut stub_calls = String::new();
    for (k, i) in [(0usize, 0usize), (1, 37), (2, 74), (3, 11)] {
        stub_calls.push_str(&format!("printf(\"{{0}}\", f{k}_{i}());"));
    }
    // chain() head returns f1_0 = 0; add nothing for it.
    let stub_src = dir.join(format!("{tag}.stub.rc"));
    std::fs::write(
        &stub_src,
        format!(
            "namespace app {{ class Host {{\n[LibraryImport(\"scale-{tag}-0\", EntryPoint = \"Scale0.C0::chain#0\")] int chain0();\n[LibraryImport(\"scale-{tag}-0\", EntryPoint = \"Scale0.C0::f0_0#0\")] int f0_0();\n[LibraryImport(\"scale-{tag}-1\", EntryPoint = \"Scale1.C1::f1_37#0\")] int f1_37();\n[LibraryImport(\"scale-{tag}-2\", EntryPoint = \"Scale2.C2::f2_74#0\")] int f2_74();\n[LibraryImport(\"scale-{tag}-3\", EntryPoint = \"Scale3.C3::f3_11#0\")] int f3_11();\nint main() {{ printf(\"{{0}}\", chain0()); {stub_calls} return 0; }} }} }}\n"
        ),
    )
    .unwrap();
    let closed_src_path = dir.join(format!("{tag}.closed.rc"));
    let closed_main = "namespace app { class Host { int main() { Scale0.C0 z0 = new Scale0.C0(); printf(\"{0}\", z0.chain()); printf(\"{0}\", z0.f0_0()); Scale1.C1 z1 = new Scale1.C1(); printf(\"{0}\", z1.f1_37()); Scale2.C2 z2 = new Scale2.C2(); printf(\"{0}\", z2.f2_74()); Scale3.C3 z3 = new Scale3.C3(); printf(\"{0}\", z3.f3_11()); return 0; } } }\n";
    std::fs::write(&closed_src_path, format!("{closed_src}{closed_main}")).unwrap();
    let exe_static = dir.join(format!("{tag}.static.exe"));
    let exe_closed = dir.join(format!("{tag}.closed.exe"));
    let mut lib_args: Vec<String> = Vec::new();
    for (_, arch) in &arch_paths {
        lib_args.push("--lib".to_owned());
        lib_args.push(s(&arch));
    }
    let mut link_cmd: Vec<&str> = vec!["compile"];
    link_cmd.extend(lib_args.iter().map(|a| a.as_str()));
    link_cmd.extend([s(&stub_src).leak() as &str, "-o", s(&exe_static).leak() as &str]);
    let link_out = build_cli(&link_cmd);
    assert!(link_out.code == 0, "scale link failed: {}", link_out.stderr_str());
    let closed_out = build_cli(&["compile", &s(&closed_src_path), "-o", &s(&exe_closed)]);
    assert!(closed_out.code == 0, "closure build failed: {}", closed_out.stderr_str());
    let a = vm_exec(&exe_static, &[], None);
    let b = vm_exec(&exe_closed, &[], None);
    let mut doomed: Vec<std::path::PathBuf> = arch_paths.into_iter().flat_map(|(s, a)| [s, a]).collect();
    doomed.extend([stub_src, closed_src_path, exe_static, exe_closed]);
    for f in &doomed {
        let _ = std::fs::remove_file(f);
    }
    for k in 0..ARCHIVES {
        let _ = std::fs::remove_file(dir.join(format!("{tag}.s{k}.rc")));
    }
    assert_eq!(a.code, 0, "static stderr: {}", a.stderr_str());
    assert_eq!(b.code, 0, "closed stderr: {}", b.stderr_str());
    assert_eq!(a.stdout_str(), "00377411");
    assert_eq!(a.stdout_str(), b.stdout_str(), "linkage must not change behavior");
}


#[test]
fn vm_shared_dependency_matrix() {
    // .NET model: consumer.so NEEDS dep1.so + dep2.so, resolves the bodies
    // at load time (GLOB_DAT), and a C host runs it on the VM. Then the
    // same consumer without dep2.so must fail at load naming the image.
    let dir = std::env::temp_dir().join("rubyc_vm_link_matrix");
    std::fs::create_dir_all(&dir).unwrap();
    let dep1_src = dir.join("dep1.rc");
    let dep2_src = dir.join("dep2.rc");
    let cons_src = dir.join("cons.rc");
    let host_c = dir.join("host.c");
    let dep1 = dir.join("dep1.so");
    let dep2 = dir.join("dep2.so");
    let cons = dir.join("cons.so");
    let host = dir.join("host");
    std::fs::write(
        &dep1_src,
        "namespace testns { class Oracle { public int answer() { return 42; } } }\n",
    )
    .unwrap();
    std::fs::write(
        &dep2_src,
        "namespace other { class O { public int n() { return 7; } } }\n",
    )
    .unwrap();
    std::fs::write(
        &cons_src,
        r#"namespace app { class C {
    [LibraryImport("dep1.so", EntryPoint = "testns.Oracle::answer#0")]
    int answer();
    [LibraryImport("dep2.so", EntryPoint = "other.O::n#0")]
    int n();
    public int run() { return answer() * 10 + n(); }
    int main() { printf("{0}", run()); return 0; }
} }
"#,
    )
    .unwrap();
    std::fs::write(
        &host_c,
        r#"#include <dlfcn.h>
#include <stdio.h>
int main(int argc, char **argv) {
    void *h = dlopen(argv[1], RTLD_NOW);
    if (!h) { fprintf(stderr, "dlopen: %s\n", dlerror()); return 2; }
    int (*run)(void) = (int (*)(void))dlsym(h, "run");
    if (!run) { fprintf(stderr, "dlsym: %s\n", dlerror()); return 3; }
    printf("%d\n", run());
    return 0;
}
"#,
    )
    .unwrap();
    for (src, out) in [(&dep1_src, &dep1), (&dep2_src, &dep2), (&cons_src, &cons)] {
        let o = build_cli(&[
            "compile",
            "-O",
            "shared",
            src.to_str().unwrap(),
            "-o",
            out.to_str().unwrap(),
        ]);
        assert_eq!(o.code, 0, "{}: {}", src.display(), o.stderr_str());
    }
    let cc = std::process::Command::new("cc")
        .args(["-O2", "-o"])
        .arg(&host)
        .arg(&host_c)
        .arg("-ldl")
        .output()
        .expect("cc runs");
    assert!(
        cc.status.success(),
        "host build failed: {}",
        String::from_utf8_lossy(&cc.stderr)
    );

    let out = vm_exec_bundle(
        &[
            ("host", &host),
            ("cons.so", &cons),
            ("dep1.so", &dep1),
            ("dep2.so", &dep2),
        ],
        "host",
        &["./cons.so"],
    );
    assert_eq!(out.code, 0, "stderr: {}", out.stderr_str());
    assert_eq!(out.stdout_str(), "427\n");

    let missing = vm_exec_bundle(
        &[
            ("host", &host),
            ("cons.so", &cons),
            ("dep1.so", &dep1),
        ],
        "host",
        &["./cons.so"],
    );
    assert_ne!(missing.code, 0, "a missing dependency must not run");
    assert!(
        missing.stderr_str().contains("dep2.so"),
        "the loader must name the missing image: {}",
        missing.stderr_str()
    );
    // The JIT resolves the same images from RUBYC_BASE/libraries and
    // must agree with the produced image's stdout.
    let base = dir.join("base");
    std::fs::create_dir_all(base.join("libraries")).unwrap();
    std::fs::copy(&dep1, base.join("libraries").join("dep1.so")).unwrap();
    std::fs::copy(&dep2, base.join("libraries").join("dep2.so")).unwrap();
    let jit = std::process::Command::new(crate::tests::common::rubyc_path())
        .arg("run")
        .arg(&cons_src)
        .env("RUBYC_BASE", &base)
        .output()
        .expect("rubyc run");
    assert!(
        jit.status.success(),
        "JIT run failed: {}",
        String::from_utf8_lossy(&jit.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&jit.stdout).trim(),
        out.stdout_str().trim(),
        "run and the produced image must agree"
    );
    for f in [&dep1_src, &dep2_src, &cons_src, &host_c, &dep1, &dep2, &cons, &host] {
        let _ = std::fs::remove_file(f);
    }
}
