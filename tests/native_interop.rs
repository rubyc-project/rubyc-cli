//! End-to-end interop: builds every artifact from scratch and exercises
//! all four host × library combinations.
//!
//! ```text
//!  cc -shared ──► libnative_add.so ──┬──► rubyc build (static embed) ──► rhost
//!  rubyc -O shared ──► libtestns.so ─┴──► cc + dlopen/dlsym ────────────► chost
//! ```
//!
//! Skipped cleanly on hosts that are not Linux x86-64 or lack a C
//! compiler — the same guard policy as the native_exec suites.

use std::path::PathBuf;
use std::process::Command;

fn rubyc() -> Command {
    Command::new(crate::tests::common::rubyc_path())
}

fn supported_host() -> bool {
    cfg!(all(target_os = "linux", target_arch = "x86_64"))
        && Command::new("cc")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
}

struct Workspace {
    dir: PathBuf,
}

impl Workspace {
    fn fresh(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("rubyc_interop_{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self { dir }
    }

    fn path(&self, file: &str) -> PathBuf {
        self.dir.join(file)
    }
}

fn run(cmd: &mut Command, cwd: Option<&PathBuf>) -> (bool, String) {
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let out = cmd.output().expect("spawn failed");
    (
        out.status.success(),
        format!(
            "{}{}",
            String::from_utf8_lossy(&out.stderr),
            String::from_utf8_lossy(&out.stdout)
        ),
    )
}

fn repo() -> PathBuf {
    // The cli crate now sits at the workspace root; examples sit alongside it.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The full pipeline: build both libraries, both hosts, run everything.
#[test]
fn full_interop_matrix() {
    if !supported_host() {
        eprintln!("skipping: needs linux-x86_64 + cc");
        return;
    }

    let ws = Workspace::fresh("matrix");
    let root = repo();

    // 1. C library: int32_t add(int32_t, int32_t).
    let (ok, log) = run(
        Command::new("cc")
            .args(["-shared", "-fPIC", "-O1"])
            .arg(root.join("examples/interop/native_add/native_add.c"))
            .arg("-o")
            .arg(ws.path("libnative_add.so")),
        None,
    );
    assert!(ok, "cc shared lib failed: {log}");

    // 2. RubyC library: testns.TestLib.add exported as `add`.
    let (ok, log) = run(
        rubyc()
            .args(["compile", "-O", "shared"])
            .arg(root.join("examples/interop/testlib/testlib.rc"))
            .arg("-o")
            .arg(ws.path("libtestns.so")),
        None,
    );
    assert!(ok, "rubyc shared lib failed: {log}");

    // Both libraries carry the 🐕🇨 container preamble where the loader
    // allows it: at offset 0 for bytecode-style artifacts is impossible in
    // ELF, so libraries place it right after the 64-byte ELF header, and
    // executables keep it at their entry point.
    let so = std::fs::read(ws.path("libtestns.so")).unwrap();
    assert_eq!(&so[64..66], &[0xEB, 0x08], "jump-over-magic at offset 64");
    assert_eq!(
        &so[66..74],
        &rubyc::native::container::MAGIC,
        "dog magic at offset 66"
    );

    // 3. RubyC host executable: statically embeds BOTH libraries' symbols
    //    through [LibraryImport] + link-time extraction. Zero runtime deps.
    let (ok, log) = run(
        rubyc()
            .args(["build"])
            .arg("--lib")
            .arg(ws.path("libnative_add.so"))
            .arg("--lib")
            .arg(ws.path("libtestns.so"))
            .arg(root.join("examples/interop/rubyc_host/main.rc"))
            .arg("-o")
            .arg(ws.path("rhost")),
        None,
    );
    assert!(ok, "rubyc host build failed: {log}");

    // 4. C host executable: dlopen/dlsym against both libraries.
    let (ok, log) = run(
        Command::new("cc")
            .arg(root.join("examples/interop/c_host/main.c"))
            .arg("-ldl")
            .arg("-o")
            .arg(ws.path("chost")),
        None,
    );
    assert!(ok, "cc host build failed: {log}");

    // ---------- run all four combinations ----------
    // RubyC host: cadd(2,3)=5 via native lib; rcadd(40,2)=42 via testns.
    let exe = ws.path("rhost");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
    let out = Command::new(&exe).output().unwrap();
    assert!(out.status.success(), "rhost crashed");
    assert_eq!(String::from_utf8_lossy(&out.stdout), "5 42 10 42");

    // C host: dlopen's both libraries and calls each `add`.
    let out = Command::new(ws.path("chost"))
        .current_dir(&ws.dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "chost crashed");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim_end(),
        "5 42 10 42",
        "c host must call through both libraries (add + odd-arg inc)"
    );
}
