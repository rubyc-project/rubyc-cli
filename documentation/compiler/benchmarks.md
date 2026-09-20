# Benchmarks

Every number in this file is produced by the checked-in harness — never
hand-written. Re-run it after any backend change; regressions show up
immediately.

## Running

```bash
# full report (release compiler, C comparison when cc exists)
cargo test --release --test perf_gates -- --ignored --nocapture

# enforce acceptance gates (CI / pre-merge)
RUBYC_RUN_GATES=1 cargo test --release --test perf_gates -- --ignored
```

## What is measured

| Metric | Method |
|---|---|
| Binary size | bytes of a `printf("hello")` executable vs `cc` dynamic / `-static` |
| Startup | median spawn-to-exit wall time over 51 runs of that binary |
| Compile speed | wall time to compile a generated 100 000-statement dependency chain (~2.9 MB source), median of 3 |
| Execution | the same chain executed: every operation depends on the previous result, so no compiler can fold or skip work. The C twin reads its trip count through a `volatile`, keeping `-O2` honest |
| Ratios | rubyc execution ÷ gcc `-O0` and ÷ gcc `-O2`; size ratio vs static C |

The workload is deliberately latency-bound (one dependent multiply-add per
step). Throughput-bound workloads (independent operations) become measurable
once control flow lands and will be added to the same harness.

## Baseline — 2026-08-25, Linux x86-64, GCC 16.2, release build

### Naive backend (stack-machine lowering, pre register allocation)

| Metric | Value |
|---|---|
| exec ratio | ~2.5–3.8× of `-O0` |

### Current backend (virtual registers + linear-scan allocator)

| Metric | RubyC | C cc |
|---|---|---|
| hello binary | **4,109 B** | 12,616 B dynamic (`-static` skipped: glibc.a absent) |
| startup | **~80 µs** | — |
| compile 100k stmts | ~280 ms | — |
| exec 100k-op chain | **~0.52 ms** | ~0.5 ms `-O0` · ~0.40 ms `-O2` |
| exec ratio | **~1.0–1.4× of `-O0`** | ~1.3× of `-O2` |

Notes:

- The 4 KB binary floor comes from W^X page granularity: executable and
  writable segments cannot share a page, so two pages is the minimum for any
  conforming static ELF. Still 3× smaller than dynamic C.
- The workload is latency-bound (dependent chain), which caps how far ahead
  of `-O0` any compiler can get; throughput benchmarks land with control flow.
- Register allocation removed the per-operation memory round-trips that made
  the naive backend ~3× slower.

## Gates (CI)

Enforced with `RUBYC_RUN_GATES=1`:

- hello binary < 4 KiB
- startup median < 5 ms
- compile 100k statements < 1.5 s
- execution ≤ 2× gcc `-O0`

Tighten gates as the backend improves; never loosen without a written
reason in this file.
