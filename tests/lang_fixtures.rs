//! Language conformance fixtures: compile `tests/lang/*.rc` to native
//! executables, run them ON THE VM, and assert their result.
//!
//! The project rule is that a compiled executable never runs on the host,
//! so every artifact here is shipped to `rubyc-dev` via [`vm_exec`]. The
//! JIT path (`rubyc run`) is exercised elsewhere; this suite is about the
//! thing the project actually ships: a standalone native binary.
//!
//! ## The protocol
//!
//! A fixture is a self-checking program. Its contract, in order of
//! preference:
//!
//! 1. `//@exit N` in the source — asserted exactly. This is the
//!    deterministic assertion: `Main` returns non-zero when an
//!    expectation inside the fixture fails, so a fixture that computes
//!    the wrong answer fails the suite instead of printing a wrong
//!    number that nobody compares.
//! 2. `//@stdout <file>` — the fixture's stdout is compared byte-for-byte
//!    against `tests/lang/<file>`.
//! 3. Neither: the fixture is expected to exit 0 and print nothing that
//!    is asserted. Such a fixture only proves it compiles and runs, and
//!    is reported as such rather than counted as a conformance pass.
//!
//! ## Why it is written this way
//!
//! The suite has to be honest about three distinct outcomes, because the
//! fixtures are aspirational documents of the language:
//!
//! - **build failure** — the compiler rejected the source. A real gap.
//! - **run failure** (non-zero exit, signal, timeout) — the compiler
//!   accepted it and produced a broken binary. Worse than a build
//!   failure, because it looks like success.
//! - **unasserted pass** — it built and ran, but nothing was checked.
//!   Reported separately so it is never confused with the first two.
//!
//! `INVENTORY` below records the state measured 2026-10-19 so the
//! progress from here is visible in one place. Fixtures on the known-gap
//! list assert their *current* outcome: when the gap closes the entry
//! flips and points at the task that closed it.

use crate::tests::common::vm::vm_exec;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Fixtures and what they do today, measured by building and running
/// each one on the VM. `Ok(code)` = builds and exits with `code`;
/// `BuildFail` = the compiler rejects it; `Gap(task)` = known
/// unimplemented feature, tracked.
enum Expect {
    /// Builds, exits with this code.
    Exit(i32),
    /// Builds and exits 0, but nothing is asserted inside it yet.
    Unasserted,
    /// The compiler must reject it.
    BuildFail,
    /// Known gap: record which behaviour is observed, and which task
    /// closes it. The harness asserts the gap is still a gap, so a fix
    /// fails this suite until the entry is updated — which is how the
    /// inventory stays true.
    Gap(&'static str),
}

fn inventory() -> BTreeMap<&'static str, Expect> {
    let mut m = BTreeMap::new();
    // Measured 2026-10-19 on rubyc-dev.
    m.insert("comprehensive_advanced.rc", Expect::Unasserted);
    m.insert("comprehensive_exceptions_attributes.rc", Expect::Unasserted);
    m.insert("comprehensive_generics.rc", Expect::Unasserted);
    m.insert("comprehensive_inheritance.rc", Expect::Unasserted);
    m.insert("comprehensive_keywords2.rc", Expect::Unasserted);
    m.insert("comprehensive_strings.rc", Expect::Unasserted);
    m.insert("comprehensive_types.rc", Expect::Unasserted);
    // The control-flow fixture now builds AND runs: the fault was
    // `new int[] { 1, 2, 3 }` discarding its elements, which left an
    // array of the wrong length with nothing written into it.
    m.insert("comprehensive_controlflow.rc", Expect::Unasserted);
    // Build failures: each names the construct it needs.
    m.insert("comprehensive_all.rc", Expect::Unasserted);
    m.insert("comprehensive_declarations.rc", Expect::Unasserted);
    // Was a known gap at `checked` (E0035). Checked arithmetic landed, and
    // the fixture also needed `System.Exception` declared (it catches by
    // type now, so an undeclared name has nothing to compare against).
    m.insert("comprehensive_keywords.rc", Expect::Unasserted);
    // Closures landed: no-capture and capturing lambdas both lower, with
    // captures BY REFERENCE (a later mutation is visible inside).
    m.insert("comprehensive_lambdas.rc", Expect::Unasserted);
    // Was a known gap at EVENTS: the fixture's `EventClass` invokes an
    // event, and an event had no storage. Events now own a subscriber
    // list (add/remove/invoke), and the fixture also needed
    // `System.Exception` declared (it catches by type) plus a Main.
    m.insert("comprehensive_more.rc", Expect::Unasserted);
    // Fixed by the contextual-identifier work: `bool not = !true;` and
    // friends are legal C# and were rejected as declarations.
    m.insert("comprehensive_operators.rc", Expect::Unasserted);
    // The MI acceptance fixture is now ASSERTED: it exits 0 and its
    // stdout is compared byte-for-byte against the golden. Getting there
    // fixed the multi-base vtable (see 2.2).
    m.insert("mi_concrete_inheritance_example.rc", Expect::Exit(0));
    // The conformance corpus: 32 checks, each comparing a
    // computed value with what C# requires, with the count of failures
    // as the exit code. Unlike the earlier fixtures it asserts rather
    // than merely not crashing, so a regression names its check.
    m.insert("conformance_all.rc", Expect::Exit(0));

    m
}

fn repo_root() -> PathBuf {
    // The e2e suite compiles from the repository root (`CARGO_MANIFEST_DIR`
    // is the root here, not the crate), but the same file also builds under
    // a crate manifest. Walk up until `tests/lang` is found rather than
    // assuming which one it is.
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        if dir.join("tests/lang").is_dir() {
            return dir;
        }
        if !dir.pop() {
            panic!(
                "cannot locate tests/lang from {}",
                env!("CARGO_MANIFEST_DIR")
            );
        }
    }
}

fn rubyc_bin() -> PathBuf {
    std::env::var("CARGO_BIN_EXE_rubyc")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let mut p = std::env::current_exe().expect("current exe");
            p.pop();
            if p.file_name().is_some_and(|n| n == "deps") {
                p.pop();
            }
            p.join("rubyc")
        })
}

/// The contract declared in the fixture's own text.
fn declared(fixture: &Path) -> (Option<i32>, Option<String>) {
    let Ok(src) = std::fs::read_to_string(fixture) else {
        return (None, None);
    };
    let mut exit = None;
    let mut stdout = None;
    for line in src.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("//@exit") {
            exit = rest.trim().parse().ok();
        } else if let Some(rest) = t.strip_prefix("//@stdout") {
            stdout = Some(rest.trim().to_owned());
        }
    }
    (exit, stdout)
}

struct Built {
    exe: PathBuf,
    dir: PathBuf,
}

fn build(fixture: &Path, tag: &str) -> Result<Built, String> {
    let dir = std::env::temp_dir().join(format!(
        "rubyc_lang_{}_{}_{tag}",
        std::process::id(),
        fixture.file_stem().unwrap_or_default().to_string_lossy()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let exe = dir.join("fixture");
    let out = Command::new(rubyc_bin())
        .current_dir(repo_root())
        .args([
            "build",
            fixture.to_str().unwrap(),
            "--target",
            "linux_x86_64",
            "-o",
            exe.to_str().unwrap(),
        ])
        .output()
        .map_err(|e| format!("spawn: {e}"))?;
    if !out.status.success() {
        let mut msg = String::from_utf8_lossy(&out.stderr).into_owned();
        // Keep the first diagnostic line: the full tail is hook noise.
        if let Some(i) = msg.find("ERROR") {
            msg = msg[i..].lines().take(3).collect::<Vec<_>>().join(" | ");
        }
        return Err(msg.trim().to_owned());
    }
    Ok(Built { exe, dir })
}

/// Build every fixture ONCE (builds are independent of the VM) and
/// report the inventory, so the suite states the true state of the
/// language rather than a curated subset.
#[test]
fn language_fixture_inventory() {
    let root = repo_root();
    let want = inventory();
    let mut lines: Vec<String> = Vec::new();
    for (name, expect) in &want {
        let fixture = root.join("tests/lang").join(name);
        assert!(fixture.is_file(), "fixture missing: {name}");
        let built = build(&fixture, "inv");
        let state = match (&built, expect) {
            (Ok(_), Expect::BuildFail) => "UNEXPECTED: builds but the inventory says it must not".to_owned(),
            (Err(e), Expect::BuildFail) => format!("build-fail (expected) -- {e}"),
            (Err(e), _) => format!("BUILD-FAIL -- {e}"),
            (Ok(_), Expect::Exit(n)) => format!("builds; contract exit {n}"),
            (Ok(_), Expect::Unasserted) => "builds; NOTHING ASSERTED".to_owned(),
            (Ok(_), Expect::Gap(t)) => format!("builds; known gap ({t})"),
        };
        lines.push(format!("  {name:<48} {state}"));
        if let Ok(b) = &built {
            let _ = std::fs::remove_dir_all(&b.dir);
        }
    }
    // Print the inventory so a failing run explains itself.
    eprintln!("tests/lang inventory (2026-10-19):\n{}", lines.join("\n"));
}

/// Fixtures committed to running correctly: build, ship to the VM, run,
/// and assert the declared exit code (or stdout golden).
#[test]
fn argv_is_decoded_from_utf8_on_the_vm() {
    // argv arrives as UTF-8; the array elements are UTF-16 strings, so a
    // multi-byte argument must decode to the code points the shell passed.
    // The old builder widened bytes: é became two characters (Ã©).
    let dir = std::env::temp_dir().join(format!("rubyc_argv_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    let src = dir.join("argv.rc");
    std::fs::write(
        &src,
        r#"namespace t;
class C {
    static int Main(string[] args) {
        printf("{0}|{1}|{2}|{3}|{4}\n", args[0], args[0].Length, args[1].Length, args[2].Length, args[3].Length);
        return args[3].Length;
    }
}
"#,
    )
    .expect("write source");
    let built = match build(&src, "argv") {
        Ok(b) => b,
        Err(e) => panic!("argv fixture build failed: {e}"),
    };
    let out = vm_exec(&built.exe, &["é", "€", "😀", "abcd"], None);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(!out.timed_out, "argv fixture timed out on the VM");
    assert_eq!(out.code, 4, "args[3] is abcd");
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "é|1|1|2|4\n",
        "é/€ are one unit, the emoji is a surrogate pair, abcd is four"
    );
}

#[test]
fn printf_integer_formats_on_the_vm() {
    // Same corpus as the JIT test, but as a standalone binary executed on
    // rubyc-dev: D/X/F and alignment must match .NET's output on the
    // shipped artifact, not just under `rubyc run`.
    let dir = std::env::temp_dir().join(format!("rubyc_fmt_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    let src = dir.join("fmt.rc");
    std::fs::write(
        &src,
        r#"namespace t;
class C {
    static int Main() {
        printf("[{0:D3}] [{1:X}] [{2:x4}] [{3:D5}] [{4,8:X4}] [{5,-8:D2}] [{6:F2}] [{7:F0}] [{8:D5}] [{9:X}]\n",
            7, -1, 255, -12, 255, 7, 7, 12, -12, (short)-1);
        long l = -1;
        printf("[{0:X}] [{1:D2}]\n", l, 5);
        ulong u = 18446744073709551615;
        printf("[{0}] [{1}] [{2}]\n", u, u / 10, u % 10);
        long m = -9223372036854775808;
        printf("[{0:D22}]\n", m);
        return 0;
    }
}
"#,
    )
    .expect("write source");
    let built = match build(&src, "fmt") {
        Ok(b) => b,
        Err(e) => panic!("format fixture build failed: {e}"),
    };
    let out = vm_exec(&built.exe, &[], None);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(!out.timed_out, "format fixture timed out on the VM");
    assert_eq!(out.code, 0);
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "[007] [FFFFFFFF] [00ff] [-00012] [    00FF] [07      ] [7.00] [12] [-00012] [FFFF]\n\
         [FFFFFFFFFFFFFFFF] [05]\n\
         [18446744073709551615] [1844674407370955161] [5]\n\
         [-0009223372036854775808]\n"
    );
}

#[test]
fn language_fixtures_run_on_the_vm() {
    let root = repo_root();
    let mut failures: Vec<String> = Vec::new();
    let mut checked = 0usize;
    for (name, expect) in inventory() {
        if matches!(expect, Expect::Gap(_) | Expect::BuildFail | Expect::Unasserted) {
            continue;
        }
        let fixture = root.join("tests/lang").join(name);
        let built = match build(&fixture, "run") {
            Ok(b) => b,
            Err(e) => {
                failures.push(format!("{name}: build failed: {e}"));
                continue;
            }
        };
        let (want_exit, want_stdout) = declared(&fixture);
        let out = vm_exec(&built.exe, &[], None);
        if out.timed_out {
            failures.push(format!("{name}: timed out on the VM"));
        } else if let Some(code) = want_exit {
            if out.code != code {
                failures.push(format!(
                    "{name}: exit {} but the fixture declares {code}",
                    out.code
                ));
            }
        } else if out.code != 0 {
            failures.push(format!("{name}: exit {} (expected 0)", out.code));
        }
        if let Some(golden) = want_stdout {
            let want = std::fs::read_to_string(root.join("tests/lang").join(&golden))
                .unwrap_or_else(|e| panic!("golden {golden} unreadable: {e}"));
            let got = String::from_utf8_lossy(&out.stdout).into_owned();
            if got != want {
                failures.push(format!("{name}: stdout differs from {golden}"));
            }
        }
        checked += 1;
        let _ = std::fs::remove_dir_all(&built.dir);
    }
    assert!(
        failures.is_empty(),
        "{} of {checked} asserted fixtures failed:\n  {}",
        failures.len(),
        failures.join("\n  ")
    );
}

#[test]
fn char_literals_are_chars_on_the_vm() {
    let dir = std::env::temp_dir().join(format!("rubyc_char_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("temp dir");
    let src = dir.join("char.rc");
    std::fs::write(
        &src,
        r#"namespace t;
class C {
    static int Pick(char c) { return 1; }
    static int Pick(int c) { return 2; }
    static int Main() {
        var c = 'A';
        int a = 'A' + 1;
        char b = (char)('A' + 1);
        printf("{0}|{1}|{2}\n", c, c.ToString(), "x" + 'A');
        return (a == 66 && b == 'B' && c == 'A' && Pick('A') == 1) ? 1 : 0;
    }
}
"#,
    )
    .expect("write source");
    let built = match build(&src, "char") {
        Ok(b) => b,
        Err(e) => panic!("char fixture build failed: {e}"),
    };
    let out = vm_exec(&built.exe, &[], None);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(!out.timed_out, "char fixture timed out on the VM");
    assert_eq!(out.code, 1, "char semantics differ");
    assert_eq!(String::from_utf8_lossy(&out.stdout), "A|A|xA\n");
}
