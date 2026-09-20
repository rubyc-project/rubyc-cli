//! Native executable tests: build a real Linux x86-64 binary from a sample
//! and execute it.

use rubyc::native;
use rubyc::parser;
use rubyc::testdata::SAMPLES;
use rubyc_target_linux_x86_64::{BACKEND, IMAGE_WRITER};
use std::process::Command;

fn e_entry(bytes: &[u8]) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&bytes[24..32]);
    u64::from_le_bytes(b)
}

#[test]
fn hello_world_executable_runs_and_prints() {
    let source = &SAMPLES[0].source;
    let unit = parser::parse(source).expect("sample must parse");

    let out_dir = std::env::temp_dir().join("rubyc_native_test");
    std::fs::create_dir_all(&out_dir).unwrap();
    let exe = out_dir.join(format!("hello_rubyc-{}", std::process::id()));
    native::compile_to_native(&unit, &BACKEND, &IMAGE_WRITER, &exe)
        .expect("compilation should succeed");

    // The image is a real ELF and embeds the dog preamble at its entry.
    let bytes = std::fs::read(&exe).unwrap();
    assert_eq!(&bytes[..4], b"\x7fELF");
    // Entry point sits at file offset 232 (ELF header + 3 program
    // headers); preamble (jump + magic) starts there. e_entry is the
    // corresponding vaddr: image base 0x400000 plus that offset.
    let entry = 64 + 3 * 56;
    assert_eq!(e_entry(&bytes), 0x0040_0000 + entry as u64);
    assert_eq!(&bytes[entry..entry + 2], &[0xEB, 0x08]);
    assert_eq!(
        &bytes[entry + 2..entry + 10],
        &rubyc::native::container::MAGIC
    );

    // Make it executable and run it.
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();

    let output = (0..100)
        .find_map(|_| match std::process::Command::new(&exe).output() {
            Ok(output) => Some(output),
            Err(error) if error.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                std::thread::sleep(std::time::Duration::from_millis(10));
                None
            }
            Err(error) => panic!("spawn failed: {error}"),
        })
        .expect("spawn remained ETXTBSY after retries");
    assert!(
        output.status.success(),
        "exit status {:?}, stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "Hello World!");
}

// ---------- codegen regression tests ----------
//
// Each of these exercises a bug that once produced silent corruption or
// segfaults (itoa jump displacement, double ELF relocation, vtable words
// never populated, receiver offset math). Exact stdout is asserted so
// value-corrupting bugs fail loudly.

fn build_and_run(source: &str, name: &str) -> String {
    let unit = parser::parse(source).expect("must parse");
    let dir = std::env::temp_dir().join("rubyc_native_test");
    std::fs::create_dir_all(&dir).unwrap();
    // Process-unique name: parallel test binaries share the temp dir and
    // rewriting a mapped executable yields ETXTBSY.
    let exe = dir.join(format!("{name}-{}", std::process::id()));
    native::compile_to_native(&unit, &BACKEND, &IMAGE_WRITER, &exe).expect("must compile");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
    // Parallel harnesses can transiently race a fresh executable with
    // page-cache flushes (ETXTBSY); retry briefly before giving up.
    let mut out = None;
    for _ in 0..100 {
        match std::process::Command::new(&exe).output() {
            Ok(o) => {
                out = Some(o);
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::ExecutableFileBusy => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(e) => panic!("spawn failed: {e}"),
        }
    }
    let out = out.expect("executable stayed busy");
    assert!(
        out.status.success(),
        "{name} exited {:?}: {}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn int_printing_exact_on_both_paths() {
    let src =
        r#"namespace t { class c { int main() { printf("{0}|{1}|{2}", 7, -42, 123456789); } } }"#;
    assert_eq!(build_and_run(src, "reg_int"), "7|-42|123456789");
}

#[test]
fn cross_method_call_returns_and_multi_function_frames() {
    // Two methods plus @start: exercises CallFn rel32 patching and frame
    // setup for parameterized functions.
    let src = r#"namespace t { class c { int main() { printf("{0}", Add(2, 3)); } int Add(int a, int b) { printf(""); } } }"#;
    assert_eq!(build_and_run(src, "reg_call"), "0");
}

/// Non-commutative argument use catches push-order/convention mismatches:
/// params arrive as this@[rbp+16], param_i@[rbp+16+8i].
#[test]
fn arguments_arrive_in_declaration_order() {
    let src = r#"namespace t { class c { int Sub(int a, int b) { return a - b; } int main() { printf("{0}|{1}", Sub(10, 4), Sub(4, 10)); } } }"#;
    assert_eq!(build_and_run(src, "reg_argorder"), "6|-6");
}

/// Instance calls through object references dispatch virtually and pass
/// both receiver state and arguments correctly.
#[test]
fn instance_calls_read_receiver_state() {
    let src = r#"namespace t {
        class mathlib {
            int offset_base = 100;
            public int AddBase(int a) { return a + this.offset_base; }
        }
        class app {
            int main() {
                mathlib m = new mathlib();
                printf("{0}", m.AddBase(7));
            }
        }
    }"#;
    assert_eq!(build_and_run(src, "reg_instance"), "107");
}

#[test]
fn virtual_dispatch_picks_the_override() {
    let src = r#"namespace t { class b { int Who() { printf("B"); } } class d : b { int Who() { printf("D"); } int main() { Who(); } } }"#;
    assert_eq!(build_and_run(src, "reg_virt"), "D");
}

#[test]
fn scan_int_reads_piped_stdin() {
    let src =
        r#"namespace t; class c { int main() { int n = scan_int(); printf("{0}", n + 1); } }"#;
    use std::os::unix::fs::PermissionsExt;

    let dir = std::env::temp_dir().join("rubyc_native_test");
    std::fs::create_dir_all(&dir).unwrap();
    let src_path = dir.join(format!("scan-{}.rc", std::process::id()));
    let exe_path = dir.join(format!("scan-{}", std::process::id()));
    std::fs::write(&src_path, src).unwrap();

    let rc = Command::new(crate::tests::common::rubyc_path())
        .args(["build", "-o"])
        .arg(&exe_path)
        .arg(&src_path)
        .status()
        .unwrap();
    assert!(rc.success(), "build failed");

    std::fs::set_permissions(&exe_path, std::fs::Permissions::from_mode(0o755)).unwrap();

    // Feed runtime stdin to the compiled program (not to the compiler).
    let mut exe = Command::new(&exe_path);
    let out = crate::tests::common::run(&mut exe, b"41\n");
    assert!(!out.timed_out, "compiled program hung");
    assert_eq!(out.stdout_str(), "42");
}
