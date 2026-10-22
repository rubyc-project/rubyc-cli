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
fn misspelled_directive_is_rejected() {
    // `#Assmbly` / `#EndAssmbly` were accepted as a "compatibility"
    // courtesy. No file in the .NET tree uses them and C# has no such
    // directive, so a typo silently entered inline-assembly mode —
    // everything after it was captured as assembly instead of being
    // compiled. Reported instead.
    let (code, stderr) =
        run("#Assmbly\nnop\n#EndAssmbly\nnamespace t { class c { int main() { return 3; } } }");
    assert_ne!(code, 3, "the misspelling must not assemble and run: {stderr}");
    assert!(stderr.contains("Assmbly"), "names the bad directive: {stderr}");
}

#[test]
fn assembly_block_closes_cleanly() {
    // `#EndAssembly` used to share its arm with two misspellings and
    // then only ever report "without matching #Assembly" — so a
    // well-formed block reported an error and left the capture open,
    // blanking the rest of the file.
    let (code, stderr) =
        run("#Assembly\nnop\n#EndAssembly\nnamespace t { class c { int main() { return 5; } } }");
    assert_eq!(code, 5, "{stderr}");
    assert!(!stderr.contains("without matching"), "{stderr}");
}

#[test]
fn bad_assembly_is_a_compiler_error() {
    let (code, stderr) = run(
        "#Assembly\nnot_an_opcode rax\n#EndAssembly\nnamespace t { class c { int main() { return 0; } } }",
    );
    assert_ne!(code, 0);
    assert!(stderr.contains("assembly block"), "{stderr}");
}
