use crate::tests::common::{exec, rubyc_path};
use std::process::Command;

fn run(source: &str) -> (i32, String) {
    static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "rubyc_assembly_block_{}_{n}.rc",
        std::process::id()
    ));
    std::fs::write(&path, source).unwrap();
    let mut command = Command::new(rubyc_path());
    command.arg("run").arg(&path);
    let output = exec(&mut command);
    let _ = std::fs::remove_file(path);
    (output.code, output.stderr_str())
}

#[test]
fn assembly_block_flows_through_jit_raw_instruction() {
    let (code, stderr) =
        run("#Assembly\nnop\n#EndAssembly\nnamespace t { class c { int main() { return 7; } } }");
    assert_eq!(code, 7, "{stderr}");
}

#[test]
fn compatibility_misspelling_is_accepted() {
    let (code, stderr) =
        run("#Assmbly\nnop\n#EndAssmbly\nnamespace t { class c { int main() { return 3; } } }");
    assert_eq!(code, 3, "{stderr}");
}

#[test]
fn bad_assembly_is_a_compiler_error() {
    let (code, stderr) = run(
        "#Assembly\nnot_an_opcode rax\n#EndAssembly\nnamespace t { class c { int main() { return 0; } } }",
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("assembly block"), "{stderr}");
}
