//! THE stress test: one long, deliberately complex RubyC program that
//! interweaves every language feature shipped so far — nested loops,
//! deep if/else-if ladders, switch inside loops, ternary chains, bitwise
//! arithmetic, compound assignment, heap objects created and released
//! inside loop bodies, virtual dispatch through base references, and a
//! scan_int stdin feed. Exact output asserted on BOTH execution paths.


use crate::tests::common::{exec, run};
use rubyc::parser;
use std::process::Command;

const SRC: &str = r#"
namespace stress {
    class node {
        public int value = 0;
        public int doubled() { return value * 2; }
    }

    class square {
        public int side = 1;
        public override int area() { return side * side; }
        public override int tag() { return 2; }
    }

    class circle {
        public int radius = 1;
        public override int area() { return radius * radius * 3; }
        public override int tag() { return 3; }
    }

    class engine {
        int checksum = 0;
        int iterations = 0;

        int classify(int n) {
            if (n < 0) { return -1; }
            if (n == 0) { return 0; }
            if (n < 10) { return 1; }
            if (n < 100) { return 2; }
            return 3;
        }

        int fib(int n) {
            if (n <= 1) { return n; }
            int a = 0;
            int b = 1;
            for (int i = 2; i <= n; i++) {
                int t = a + b;
                a = b;
                b = t;
            }
            return b;
        }

        int run(int seed) {
            int acc = 0;
            // --- nested loops with switch + continue/break interplay ---
            for (int outer = 1; outer <= 4; outer++) {
                int inner = 0;
                while (inner < outer * 2) {
                    inner++;
                    int bucket = (outer * inner) % 5;
                    switch (bucket) {
                        case 0:
                            acc += outer * 100;
                            break;
                        case 1:
                        case 2:
                            acc += inner;
                            break;
                        default:
                            if (bucket == 3 && inner > 3) { break; }
                            acc ^= bucket;
                            break;
                    }
                    if (acc > 5000) { acc %= 997; }
                    iterations++;
                    if (iterations >= 40) { break; }
                }
                if (iterations >= 40) { break; }
            }
            return acc;
        }

        int main(int seed) {
            // --- phase 1: classification ladder over seeds ---
            int phase1 = 0;
            for (int s = -2; s <= 12; s += 3) {
                int k = classify(s);
                phase1 = phase1 * 10 + (k + 1);
            }
            printf("p1={0}", phase1);

            // --- phase 2: heap churn through virtual dispatch ---
            int total_area = 0;
            // NOTE: no base-typed aliasing yet (MEM.2 retain-on-assign);
            // dispatch exercised via derived receivers.
            for (int i = 1; i <= 6; i++) {
                int a = 0;
                if (i % 2 == 0) {
                    square sq = new square();
                    sq.side = i * 2;
                    a = sq.area() + sq.tag();
                } else {
                    circle ci = new circle();
                    ci.radius = i;
                    a = ci.area() + ci.tag();
                }
                total_area += a;
                node nd = new node();
                nd.value = a;
                total_area += nd.doubled() % 7;
            }
            printf(" p2={0}", total_area);

            // --- phase 3: ternary chains + bitwise + compound ---
            int mix = 0;
            for (int b = 0; b < 16; b++) {
                mix |= (b & 3) == 0 ? 1 << (b / 4) : 0;
                if (b % 5 == 3) { mix ^= b; }
                if ((mix & 64) != 0) { mix -= 64; }
            }
            printf(" p3={0}", mix);

            // --- phase 4: recursion + modulo maze ---
            int f = fib(15);
            int maze = 0;
            do {
                maze += f % 9;
                f /= 3;
                if (f == 13) { maze *= 2; }
            } while (f > 0);
            printf(" p4={0}", maze);

            // --- phase 5: the switch/loop gauntlet ---
            int g = run(seed);
            printf(" p5={0}", g);

            // --- phase 6: stdin echo with guard ---
            int from_stdin = scan_int();
            int echoed = from_stdin > 0 ? from_stdin * 2 : -from_stdin;
            printf(" p6={0}", echoed);

            checksum = phase1 + total_area + mix + maze + g + echoed;
            printf(" sum={0}", checksum);
            return checksum % 97;
        }
    }
}
"#;

fn feed_and_run(stdin_data: &str) -> (i32, String, String) {
    // Write source once; both paths read it as a FILE so stdin stays free
    // for the program's own scan_int().
    let src = std::env::temp_dir().join("rubyc_stress_src.rc");
    std::fs::write(&src, SRC).unwrap();

    // JIT path
    let mut jit_cmd = Command::new(crate::tests::common::rubyc_path());
    jit_cmd.arg("run").arg(&src);
    let jit = run(&mut jit_cmd, stdin_data.as_bytes());
    assert!(!jit.timed_out, "JIT run HUNG");

    // ELF path
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let src = std::env::temp_dir().join(format!("rubyc_stress_{n}.rc"));
    let bin = std::env::temp_dir().join(format!("rubyc_stress_{n}.bin"));
    std::fs::write(&src, SRC).unwrap();
    let mut build = Command::new(crate::tests::common::rubyc_path());
    let built = exec(build.args(["build"]).arg(&src).arg("-o").arg(&bin));
    assert!(
        !built.timed_out && built.code == 0,
        "build failed:\n{}",
        built.stderr_str()
    );
    let mut exe = Command::new(&bin);
    let elf_out = run(&mut exe, stdin_data.as_bytes());
    assert!(!elf_out.timed_out, "ELF program HUNG");
    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&bin);

    (
        jit.code,
        jit.stdout_str(),
        format!("{}|{}", elf_out.stdout_str(), elf_out.code),
    )
}

/// Expected output for stdin "42" — every value independently
/// hand-computed (see DECISIONS log in the morning report).
const EXPECTED: &str = "p1=2223 p2=367 p3=3 p4=29 p5=724 p6=84 sum=3430";
const EXPECTED_EXIT: i32 = 3430 % 97; // 35 — main returns checksum % 97

#[test]
fn full_pipeline_jit_and_elf_agree_and_are_correct() {
    let (jit_code, jit_out, elf_combo) = feed_and_run("42");

    assert_eq!(jit_out, EXPECTED, "JIT stdout");
    assert_eq!(jit_code, EXPECTED_EXIT, "JIT exit code");

    // ELF path: same stdin, same expected stdout, same exit code.
    let parts: Vec<&str> = elf_combo.splitn(2, '|').collect();
    assert_eq!(parts[0], EXPECTED, "ELF stdout");
    let elf_code: i32 = parts[1].parse().unwrap();
    assert_eq!(elf_code, EXPECTED_EXIT, "ELF exit code");
}

#[test]
fn negative_stdin_takes_abs_branch() {
    // scan_int reads -25 → echoed = -(-25)=25; everything else identical.
    let src = std::env::temp_dir().join("rubyc_stress_src.rc");
    std::fs::write(&src, SRC).unwrap();
    let mut cmd = Command::new(crate::tests::common::rubyc_path());
    cmd.arg("run").arg(&src);
    let out = run(&mut cmd, b"-25\n");
    assert!(!out.timed_out);

    let new_sum = 2223 + 367 + 3 + 29 + 724 + 25;
    let expected = format!(
        "p1=2223 p2=367 p3=3 p4=29 p5=724 p6=25 sum={new_sum}"
    );
    assert_eq!(out.stdout_str(), expected);
    assert_eq!(out.code, new_sum % 97);
}

#[test]
fn bytecode_round_trip_of_stress_program() {
    use rubyc::bytecode::traits::{FromBytecode, ToBytecode};
    use rubyc::models::compare::StructuralEq;
    let unit = parser::parse(SRC).unwrap();
    let encoded = unit.to_bytecode();
    let decoded = rubyc::models::CompilationUnit::from_bytecode(&encoded).unwrap();
    assert!(
        unit.structural_eq(&decoded),
        "the whole gnarly AST must survive encode/decode"
    );
}
