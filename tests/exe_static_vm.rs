//! Standalone static-executable smoke tests, executed on the rubyc-dev VM.
//!
//! Each test builds a fully static executable locally (real pulled-library
//! code merged from sources, CoreCLR runtime natives provided by the
//! `examples/exe_smoke/host_stubs.c` shim), ships it over scp, runs it
//! over ssh, and asserts on the captured result — per the project rule
//! that compiled executables never run on the host.
//!
//! What is covered and why: `vm_static_exe_runs_hello` proves the
//! freestanding `ET_EXEC` pipeline; `vm_static_exe_runs_hashing`
//! proves real pulled-library code (`System.IO.Hashing.Adler32`,
//! known answer) executes correctly in a static image;
//! `vm_static_exe_optimized_matches` proves `--optimize`/`--strip`
//! keep behavior while shrinking the image; `vm_static_exe_runs_fileio`
//! proves real file I/O (`File.WriteAllBytes`/`ReadAllBytes` roundtrip
//! through the host-shimmed PAL) with byte-exact content.

use crate::tests::common::vm::vm_exec;
use std::process::Command;

static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

fn rubyc_bin() -> std::path::PathBuf {
    let dir = std::env::var("CARGO_BIN_EXE_rubyc")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            let mut p = std::env::current_exe().expect("current exe");
            p.pop();
            if p.file_name().is_some_and(|n| n == "deps") {
                p.pop();
            }
            p.join("rubyc")
        });
    assert!(dir.is_file(), "rubyc binary missing: {}", dir.display());
    dir
}

fn workdir(tag: &str) -> std::path::PathBuf {
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rubyc_exe_smoke_{}_{}_{n}", std::process::id(), tag));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Build `examples/exe_smoke/host_stubs.c` into a shared object with the
/// system compiler (same `cc` precedent as the interop tests).
fn build_host_stubs(dir: &std::path::Path) -> std::path::PathBuf {
    let root = std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir");
    let src = std::path::PathBuf::from(root).join("examples/exe_smoke/host_stubs.c");
    assert!(src.is_file(), "missing {}", src.display());
    let so = dir.join("host_stubs.so");
    let out = Command::new("cc")
        .arg("-shared")
        .arg("-fPIC")
        .arg("-o")
        .arg(&so)
        .arg(&src)
        .arg("-ldl")
        .output()
        .expect("cc must run");
    assert!(
        out.status.success(),
        "cc host_stubs failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    so
}

/// Build an exe project (`main.rc` + `app.rcp` with the given extra
/// `ruby_c` deps) with extra CLI args; returns the exe path.
fn build_exe(
    dir: &std::path::Path,
    main_rc: &str,
    ruby_c: &[&str],
    extra_args: &[&str],
    exe_name: &str,
) -> std::path::PathBuf {
    std::fs::write(dir.join("main.rc"), main_rc).unwrap();
    let deps = ruby_c
        .iter()
        .map(|d| format!("\"{d}\""))
        .collect::<Vec<_>>()
        .join(", ");
    std::fs::write(
        dir.join("app.rcp"),
        format!(
            "[meta]\nname = \"exesmoke\"\nversion = \"1.0.0\"\n\
             [target]\nplatform = \"linux_x86_64\"\noutput = \"exe\"\n\
             [build]\nsources = [\"main.rc\"]\nentry-point = \"main\"\n\
             [libs]\nruby_c = [{deps}]\n"
        ),
    )
    .unwrap();
    let exe = dir.join(exe_name);
    let mut cmd = Command::new(rubyc_bin());
    cmd.arg("build").arg(dir.join("app.rcp")).arg("-o").arg(&exe);
    // Linking is strict: an unresolved import is a hard error, so a
    // build here proves every native it references was provided. The
    // smoke tests deliberately link against examples/exe_smoke/host_stubs.c
    // to supply the ones they exercise.
    for a in extra_args {
        cmd.arg(a);
    }
    let out = cmd.output().expect("rubyc must run");
    assert!(
        out.status.success(),
        "exe build failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(exe.is_file(), "no exe emitted");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
    exe
}

fn tree_lib(name: &str) -> String {
    let root = std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir");
    format!("{root}/dotnet-runtime/src/libraries/{name}/{name}.rcp")
}

/// Freestanding hello-world: no libraries, pure host call.
#[test]
fn vm_static_exe_runs_hello() {
    let dir = workdir("hello");
    let exe = build_exe(
        &dir,
        "namespace app { class Program { int main() { printf(\"hello-exe\"); return 0; } } }",
        &[],
        &[],
        "hello",
    );
    let out = vm_exec(&exe, &[], None);
    assert_eq!(out.code, 0, "stderr: {}", out.stderr_str());
    assert_eq!(out.stdout_str(), "hello-exe");
}

/// Real pulled-library code in a static image: Adler32("abc") is
/// 38600999. Needs the host-stub shim for CoreCLR runtime natives.
#[test]
fn vm_static_exe_runs_hashing() {
    let dir = workdir("hashing");
    let stubs = build_host_stubs(&dir);
    let stubs_arg = format!("--lib={}", stubs.display());
    let exe = build_exe(
        &dir,
        "using System.IO.Hashing;\nnamespace app { class Program { int main() { \
         byte[] data = new byte[3]; data[0] = 97; data[1] = 98; data[2] = 99; \
         uint h = Adler32.HashToUInt32(data); printf(\"{0}\", h); return 0; } } }",
        &[&tree_lib("System.IO.Hashing")],
        &[stubs_arg.as_str(), "--optimize"],
        "hashing",
    );
    let out = vm_exec(&exe, &[], None);
    assert_eq!(out.code, 0, "stderr: {}", out.stderr_str());
    assert_eq!(out.stdout_str(), "38600999");
}

/// `System.Console.WriteLine` in a static image: exercises the full
/// managed console path (formatting, PAL shims) end to end.
#[test]
fn vm_static_exe_runs_console() {
    let dir = workdir("console");
    let stubs = build_host_stubs(&dir);
    let stubs_arg = format!("--lib={}", stubs.display());
    let exe = build_exe(
        &dir,
        "using System;\nnamespace app { class Program { int main() { Console.WriteLine(\"hello-static-exe\"); return 0; } } }",
        &[&tree_lib("System.Console")],
        &[stubs_arg.as_str(), "--optimize"],
        "console",
    );
    let out = vm_exec(&exe, &[], None);
    assert_eq!(out.code, 0, "stderr: {}", out.stderr_str());
    assert!(
        out.stdout_str().contains("hello-static-exe"),
        "unexpected stdout: {:?}",
        out.stdout_str()
    );
}

/// `--optimize` and `--strip` keep behavior while shrinking the image.
#[test]
fn vm_static_exe_optimized_matches() {
    let dir = workdir("optmatch");
    let stubs = build_host_stubs(&dir);
    let stubs_arg = format!("--lib={}", stubs.display());
    let src = "using System.IO.Hashing;\nnamespace app { class Program { int main() { \
         byte[] data = new byte[3]; data[0] = 97; data[1] = 98; data[2] = 99; \
         uint h = Adler32.HashToUInt32(data); printf(\"{0}\", h); return 0; } } }";
    let plain = build_exe(&dir, src, &[&tree_lib("System.IO.Hashing")], &[stubs_arg.as_str()], "plain");
    let small = build_exe(
        &dir,
        src,
        &[&tree_lib("System.IO.Hashing")],
        &[stubs_arg.as_str(), "--optimize", "--strip"],
        "small",
    );
    let plain_len = std::fs::metadata(&plain).unwrap().len();
    let small_len = std::fs::metadata(&small).unwrap().len();
    assert!(
        small_len < plain_len,
        "optimized+stripped image smaller: {small_len} vs {plain_len}"
    );
    let a = vm_exec(&plain, &[], None);
    let b = vm_exec(&small, &[], None);
    assert_eq!(a.code, 0, "stderr: {}", a.stderr_str());
    assert_eq!(a.stdout_str(), "38600999");
    assert_eq!(b.stdout_str(), a.stdout_str(), "flags keep behavior");
    assert_eq!(b.code, a.code);
}

/// File I/O roundtrip in a static image: `File.WriteAllBytes` then
/// `File.ReadAllBytes` with byte-exact content (`65 66 67`), through
/// the real PAL shims (`open`/`pwrite`/`pread`/`close`) and the pooled
/// `ArrayPool` read path. Exact stdout `m1m2m3 len=365 66 67OK`.
#[test]
fn vm_static_exe_runs_fileio() {
    let dir = workdir("fileio");
    let stubs = build_host_stubs(&dir);
    let stubs_arg = format!("--lib={}", stubs.display());
    let src = "using System;\nusing System.IO;\nnamespace app { class Program { int main() { \
         printf(\"m1\"); byte[] data = new byte[3]; data[0]=65; data[1]=66; data[2]=67; \
         File.WriteAllBytes(\"/tmp/rcfileio.bin\", data); printf(\"m2\"); \
         byte[] back = File.ReadAllBytes(\"/tmp/rcfileio.bin\"); printf(\"m3 len={0}\", back.Length); \
         printf(\"{0} {1} {2}\", back[0], back[1], back[2]); \
         bool ok = back.Length == 3 && back[0] == 65 && back[1] == 66 && back[2] == 67; \
         if (ok) { printf(\"OK\"); } else { printf(\"MISMATCH\"); } return 0; } } }";
    let exe = build_exe(
        &dir,
        src,
        &[&tree_lib("System.Console")],
        &[stubs_arg.as_str()],
        "fileio",
    );
    let out = vm_exec(&exe, &[], None);
    assert_eq!(out.code, 0, "stderr: {}", out.stderr_str());
    assert_eq!(out.stdout_str(), "m1m2m3 len=365 66 67OK");
}

/// `--optimize`/`--strip` keep file-I/O behavior while shrinking.
#[test]
fn vm_static_exe_fileio_optimized_matches() {
    let dir = workdir("fileioopt");
    let stubs = build_host_stubs(&dir);
    let stubs_arg = format!("--lib={}", stubs.display());
    let src = "using System;\nusing System.IO;\nnamespace app { class Program { int main() { \
         printf(\"m1\"); byte[] data = new byte[3]; data[0]=65; data[1]=66; data[2]=67; \
         File.WriteAllBytes(\"/tmp/rcfileio_opt.bin\", data); printf(\"m2\"); \
         byte[] back = File.ReadAllBytes(\"/tmp/rcfileio_opt.bin\"); printf(\"m3 len={0}\", back.Length); \
         printf(\"{0} {1} {2}\", back[0], back[1], back[2]); \
         bool ok = back.Length == 3 && back[0] == 65 && back[1] == 66 && back[2] == 67; \
         if (ok) { printf(\"OK\"); } else { printf(\"MISMATCH\"); } return 0; } } }";
    let plain = build_exe(
        &dir,
        src,
        &[&tree_lib("System.Console")],
        &[stubs_arg.as_str()],
        "plain",
    );
    let small = build_exe(
        &dir,
        src,
        &[&tree_lib("System.Console")],
        &[stubs_arg.as_str(), "--optimize", "--strip"],
        "small",
    );
    let plain_len = std::fs::metadata(&plain).unwrap().len();
    let small_len = std::fs::metadata(&small).unwrap().len();
    assert!(
        small_len < plain_len,
        "optimized+stripped image smaller: {small_len} vs {plain_len}"
    );
    let a = vm_exec(&plain, &[], None);
    let b = vm_exec(&small, &[], None);
    assert_eq!(a.code, 0, "stderr: {}", a.stderr_str());
    assert_eq!(a.stdout_str(), "m1m2m3 len=365 66 67OK");
    assert_eq!(b.stdout_str(), a.stdout_str(), "flags keep behavior");
    assert_eq!(b.code, a.code);
}

/// File-I/O + hashing composition: write, read back, Adler32 the
/// read-back bytes (`ABC` -> `26017991`). Proves pooled-read output
/// feeds real pulled-library compute byte-exactly.
#[test]
fn vm_static_exe_runs_fileio_hash() {
    let dir = workdir("fileiohash");
    let stubs = build_host_stubs(&dir);
    let stubs_arg = format!("--lib={}", stubs.display());
    let src = "using System;\nusing System.IO;\nusing System.IO.Hashing;\nnamespace app { class Program { int main() { \
         printf(\"m1\"); byte[] data = new byte[3]; data[0]=65; data[1]=66; data[2]=67; \
         File.WriteAllBytes(\"/tmp/rcfileio_hash.bin\", data); printf(\"m2\"); \
         byte[] back = File.ReadAllBytes(\"/tmp/rcfileio_hash.bin\"); printf(\"m3 len={0}\", back.Length); \
         printf(\"{0} {1} {2}\", back[0], back[1], back[2]); \
         bool ok = back.Length == 3 && back[0] == 65 && back[1] == 66 && back[2] == 67; \
         if (ok) { printf(\"OK\"); } else { printf(\"MISMATCH\"); } \
         uint h = Adler32.HashToUInt32(back); printf(\" h={0}\", h); return 0; } } }";
    let exe = build_exe(
        &dir,
        src,
        &[&tree_lib("System.Console"), &tree_lib("System.IO.Hashing")],
        &[stubs_arg.as_str()],
        "fileiohash",
    );
    let out = vm_exec(&exe, &[], None);
    assert_eq!(out.code, 0, "stderr: {}", out.stderr_str());
    assert_eq!(out.stdout_str(), "m1m2m3 len=365 66 67OK h=26017991");
}

/// `Main(string[])` entry marshalling: kernel argv becomes a managed
/// `string[]` with .NET semantics (program name stripped). Echoes its
/// arguments back byte-exactly, including spaces and symbols.
#[test]
fn vm_static_exe_runs_argv() {
    let dir = workdir("argv");
    let stubs = build_host_stubs(&dir);
    let stubs_arg = format!("--lib={}", stubs.display());
    let src = "using System;\nnamespace app { class Program { static int Main(string[] args) { \
         printf(\"n={0}\", args.Length); \
         for (int i = 0; i < args.Length; i++) { printf(\" a={0}\", args[i]); } return 0; } } }";
    let exe = build_exe(
        &dir,
        src,
        &[&tree_lib("System.Console")],
        &[stubs_arg.as_str()],
        "argv",
    );
    let out = vm_exec(&exe, &["hello", "world"], None);
    assert_eq!(out.code, 0, "stderr: {}", out.stderr_str());
    assert_eq!(out.stdout_str(), "n=2 a=hello a=world");
    let bare = vm_exec(&exe, &[], None);
    assert_eq!(bare.code, 0, "stderr: {}", bare.stderr_str());
    assert_eq!(bare.stdout_str(), "n=0");
    let quoted = vm_exec(&exe, &["a b", "c@d"], None);
    assert_eq!(quoted.code, 0, "stderr: {}", quoted.stderr_str());
    assert_eq!(quoted.stdout_str(), "n=2 a=a b a=c@d");
}
