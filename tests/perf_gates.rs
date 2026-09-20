//! Performance harness and acceptance gates.
//!
//! Run explicitly (never part of `cargo test`):
//!
//! ```text
//! cargo test --release --test perf_gates -- --ignored            # report
//! RUBYC_RUN_GATES=1 cargo test --release --test perf_gates -- --ignored
//! ```
//!
//! Report-only by default; setting `RUBYC_RUN_GATES=1` turns the checks at
//! the bottom into hard assertions. C comparisons require a `cc` on PATH
//! and are skipped otherwise. Numbers land in
//! `documentation/compiler/benchmarks.md`.

use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

struct Cc {
    path: String,
}

fn find_cc() -> Option<Cc> {
    for c in ["cc", "gcc", "clang"] {
        if Command::new(c)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Some(Cc {
                path: c.to_string(),
            });
        }
    }
    None
}

fn workdir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("rubyc_perf_{tag}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn p(dir: &std::path::Path, file: &str) -> PathBuf {
    dir.join(file)
}

/// `n` dependent integer statements through one local: every operation waits
/// on the previous result, so neither compiler can skip work.
fn arith_source(n: usize) -> String {
    let mut s = String::from("namespace bench { class run {\n int main() {\n");
    s.push_str("  int v = 3;\n");
    for _ in 0..n {
        s.push_str("  v = v * 1103515245 + 12345;\n");
    }
    s.push_str("  printf(\"{0}\", v);\n } } }\n");
    s
}

const ARITH_STMTS: usize = 100_000;

fn arith_c_source(n: usize) -> String {
    format!(
        r#"#include <stdio.h>
int main(void) {{
    /* volatile keeps the trip count opaque so -O2 cannot fold the loop */
    volatile long gate = 1;
    long n = {n}L * gate;
    long v = 3L * gate;
    for (long i = 0; i < n; i++) {{
        v = v * 1103515245L + 12345L;
    }}
    printf("%ld", v);
    return 0;
}}
"#
    )
}

fn hello_source() -> &'static str {
    r#"namespace bench { class hello { int main() { printf("hello"); } } }"#
}

fn hello_c_source() -> &'static str {
    r#"#include <stdio.h>
int main(void) { fputs("hello", stdout); return 0; }
"#
}

fn run_cmd(cmd: &mut Command) -> (bool, Vec<u8>) {
    match cmd.output() {
        Ok(o) => (o.status.success(), o.stdout),
        Err(e) => panic!("{} failed: {e}", cmd.get_program().to_string_lossy()),
    }
}

fn timed<F: FnMut()>(mut f: F) -> Duration {
    let t = Instant::now();
    f();
    t.elapsed()
}

fn median(mut xs: Vec<Duration>) -> Duration {
    xs.sort();
    xs[xs.len() / 2]
}

fn ms(d: Duration) -> String {
    format!("{:.2}ms", d.as_secs_f64() * 1e3)
}

fn us(d: Duration) -> String {
    format!("{:.0}µs", d.as_secs_f64() * 1e6)
}

#[test]
#[ignore]
fn performance_report_and_gates() {
    let rubyc = crate::tests::common::rubyc_path();
    let rubyc = &rubyc;
    let cc = find_cc();
    let ws = workdir("main");

    println!(
        "\n=== RubyC performance report ({}) ===",
        if std::env::var("RUBYC_RUN_GATES").is_ok() {
            "gates ON"
        } else {
            "report only"
        }
    );

    // ---------- binary size ----------
    let hello_rc = p(&ws, "hello.rc");
    std::fs::write(&hello_rc, hello_source()).unwrap();
    let (ok, _) = run_cmd(
        Command::new(rubyc)
            .args(["build"])
            .arg(&hello_rc)
            .arg("-o")
            .arg(p(&ws, "hello")),
    );
    assert!(ok, "rubyc build failed");
    let rubyc_hello_size = std::fs::metadata(p(&ws, "hello")).unwrap().len();

    println!("\n[binary size: prints \"hello\"]");
    print_row("rubyc (static ELF)", &format!("{} B", rubyc_hello_size));
    let mut c_static = None;
    if let Some(cc) = &cc {
        let c_src = p(&ws, "hello.c");
        std::fs::write(&c_src, hello_c_source()).unwrap();
        let out = p(&ws, "hello_c_dyn");
        let (ok, _) = run_cmd(Command::new(&cc.path).arg(&c_src).arg("-o").arg(&out));
        if ok {
            let bytes = std::fs::metadata(&out).unwrap().len();
            print_row("C cc (dynamic)", &format!("{bytes} B"));
        }
        let out = p(&ws, "hello_c_st");
        let (ok, _) = run_cmd(
            Command::new(&cc.path)
                .arg("-static")
                .arg(&c_src)
                .arg("-o")
                .arg(&out),
        );
        if ok {
            c_static = Some(std::fs::metadata(&out).unwrap().len());
            print_row("C cc (-static)", &format!("{} B", c_static.unwrap()));
        }
    } else {
        println!("  (cc not found — C columns skipped)");
    }

    // ---------- startup latency ----------
    let mut starts = Vec::new();
    for _ in 0..51 {
        let exe = p(&ws, "hello");
        starts.push(timed(|| {
            run_cmd(&mut Command::new(&exe));
        }));
    }
    let startup = median(starts);
    println!("\n[startup: spawn+exit, median of 51]");
    print_row("rubyc", &us(startup));

    // ---------- compile speed ----------
    let big_rc = p(&ws, &format!("arith_{ARITH_STMTS}.rc"));
    std::fs::write(&big_rc, arith_source(ARITH_STMTS)).unwrap();
    let src_len = std::fs::metadata(&big_rc).unwrap().len();
    let mut compiles = Vec::new();
    for _ in 0..3 {
        compiles.push(timed(|| {
            run_cmd(
                Command::new(rubyc)
                    .args(["build"])
                    .arg(&big_rc)
                    .arg("-o")
                    .arg(p(&ws, "arith")),
            );
        }));
    }
    let compile_t = median(compiles);
    println!(
        "\n[compile speed: {ARITH_STMTS} statements, {:.1} KB source, median of 3]",
        src_len as f64 / 1024.0
    );
    print_row("rubyc build", &ms(compile_t));

    // ---------- straight-line arithmetic execution ----------
    let (ok, out_rubyc) = run_cmd(&mut Command::new(p(&ws, "arith")));
    assert!(ok, "arith run failed");
    let mut execs = Vec::new();
    for _ in 0..15 {
        let exe = p(&ws, "arith");
        execs.push(timed(|| {
            run_cmd(&mut Command::new(&exe));
        }));
    }
    let rubyc_exec = median(execs);

    println!("\n[execution: {ARITH_STMTS}-op dependency chain, median of 15]");
    print_row("rubyc", &ms(rubyc_exec));

    let mut c_o0 = None;
    let mut c_o2 = None;
    if let Some(cc) = &cc {
        let c_src = p(&ws, "arith.c");
        std::fs::write(&c_src, arith_c_source(ARITH_STMTS)).unwrap();
        for (opt, slot, name) in [
            ("-O0", &mut c_o0, "C cc -O0"),
            ("-O2", &mut c_o2, "C cc -O2"),
        ] {
            let bin = p(&ws, &format!("arith_{}", opt.trim_start_matches('-')));
            let mut out = Command::new(&cc.path)
                .arg(opt)
                .arg(&c_src)
                .arg("-o")
                .arg(&bin)
                .output()
                .expect("cc spawn");
            if !out.status.success() {
                // Transient exec flakes happen under parallel test load; try
                // once more before reporting the compiler as the failure.
                out = Command::new(&cc.path)
                    .arg(opt)
                    .arg(&c_src)
                    .arg("-o")
                    .arg(&bin)
                    .output()
                    .expect("cc spawn");
            }
            let ok = out.status.success();
            if !ok {
                println!(
                    "  [skip] {} build failed: {}",
                    opt,
                    String::from_utf8_lossy(&out.stderr)
                );
                continue;
            }
            let want = out_rubyc.clone();
            let (ok2, got) = run_cmd(&mut Command::new(&bin));
            assert!(ok2 && got == want, "C {opt} produced different output");
            let mut times = Vec::new();
            for _ in 0..15 {
                times.push(timed(|| {
                    run_cmd(&mut Command::new(&bin));
                }));
            }
            let t = median(times);
            *slot = Some(t);
            print_row(name, &ms(t));
        }
    }

    // ---------- ratios ----------
    println!("\n[ratios]");
    if let Some(o0) = c_o0 {
        let r = rubyc_exec.as_secs_f64() / o0.as_secs_f64();
        print_row("rubyc / C -O0", &format!("{r:.2}x"));
    }
    if let Some(o2) = c_o2 {
        let r = rubyc_exec.as_secs_f64() / o2.as_secs_f64();
        print_row("rubyc / C -O2", &format!("{r:.2}x"));
    }
    if let Some(st) = c_static {
        print_row(
            "size vs C -static",
            &format!("{:.0}x smaller", st as f64 / rubyc_hello_size as f64),
        );
    }

    // ---------- gates ----------
    if std::env::var("RUBYC_RUN_GATES").is_ok() {
        println!("\n[gates]");
        // W^X needs separate R-X and RW pages, so 2 pages is the physical floor
        // for any conforming static ELF; the gate reflects that floor.
        gate(
            rubyc_hello_size <= 6144,
            "hello binary within two pages (6 KiB)",
        );
        gate(startup < Duration::from_millis(5), "startup median < 5 ms");
        gate(
            compile_t < Duration::from_millis(1500),
            "compile 100k statements < 1.5 s",
        );
        if let Some(o0) = c_o0 {
            gate(
                rubyc_exec <= o0 * 2,
                "exec ≤ 2× gcc -O0 on dependency chain",
            );
        }
    } else {
        println!("\n(gates skipped — set RUBYC_RUN_GATES=1)");
    }
}

fn print_row(label: &str, value: &str) {
    println!("  {label:<28} {value}");
}

fn gate(ok: bool, what: &str) {
    println!("  {} {what}", if ok { "PASS" } else { "FAIL" });
    assert!(ok, "gate failed: {what}");
}
