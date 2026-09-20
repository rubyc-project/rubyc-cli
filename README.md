<div align="center">
<img src="images/logo.png" alt="RubyC" width="400"/>
</div>
<div align="center">
    <span align="center" style="font-size: 128px;">🐕🇨</span>

```text
ooooooooo.                .o8                     .oooooo.  
`888   `Y88.             "888                    d8P'  `Y8b 
 888   .d88' oooo  oooo   888oooo.  oooo    ooo 888         
 888ooo88P'  `888  `888   d88' `88b  `88.  .8'  888         
 888`88b.     888   888   888   888   `88..8'   888         
 888  `88b.   888   888   888   888    `888'    `88b    ooo 
o888o  o888o  `V88V"V8P'  `Y8bod8P'     .8'      `Y8bood8P' 
                                     .o..P'                  
                                     `Y8P'                   
```
**A C# compiler that emits native binaries · hand-written frontend + x86-64 backend · zero dependencies, zero runtime**
</div>

# RubyC

RubyC compiles a large, growing subset of C# — classes, generics-shaped code,
LINQ-style helpers, real-world library patterns — straight to native executables
and shared libraries. No .NET runtime, no VM, no GC: the binary stands alone.

> **Status (September 2026): in active development.** The frontend (lexer →
> parser → semantic → lowering) compiles thousands of real `dotnet/runtime`
> files; the current campaign is driving `System.Private.CoreLib` through
> lowering to zero errors, then the remaining 180 pulled libraries. See
> [Where the project stands](#where-the-project-stands) and the
> [full todo](#todo-complete-vs-remaining) below.

This repository is a **git workspace**: `rubyc-core` is a submodule, and the
plugin crates are nested submodules under it. Clone with:

```bash
git clone --recurse-submodules <this-repo-url>
# or, if you already cloned:
git submodule update --init --recursive
```

## 📦 What this repo contains

| Path | Contents |
|---|---|
| `src/` | the `rubyc` binary (CLI argument parsing + driver invocation) |
| `build.rs` | platform `target_os_*` cfgs **and** the plugin auto-install step |
| `tests/` | end-to-end CLI tests (spawn the `rubyc` binary), C↔RubyC interop |
| [rubyc-core/](rubyc-core/) | **submodule** — compiler stages, plugin ABI, project system |
| [documentation/](documentation/README.md) | language reference, stdlib, compiler internals, guides |
| `docs/`, `examples/`, `images/`, `scripts/` | working docs, interop examples, assets, build scripts |
| `dotnet-runtime/` | pulled `dotnet/runtime` sources + pull/build scripts |
| `tools/pull-dotnet.sh` | sparse-checkout pull script emitting `.rcp`/`.rcs` per library |

## 🔨 Build & run

```bash
cargo build --release        # builds every crate in the workspace
./target/release/rubyc       # the compiler driver
```

**Auto-install:** building the `rubyc` CLI copies each plugin `.so` (plus its
discovery manifest) into the per-OS plugin directory on **every build**, so
`rubyc list` / plugin discovery works immediately — no separate `cargo install`
step. On Linux the default install root is `~/.local/share/rubyc/`
(override with `$RUBYC_BASE` or `$RUBYC_HOME`):

```
~/.local/share/rubyc/
  targets/linux_x86_64/       {linux_x86_64.toml, librubyc_target_linux_x86_64.so}
  targets/windows_x86_64/   {windows_x86_64.toml, librubyc_target_windows_x86_64.so}
```

`windows_x86_64` is a registered stub (name, triple, loud refusal) ahead of
any backend work: `rubyc --list-targets` shows it, and selecting it fails
with `target 'windows_x86_64' selected but this binary compiles for
'linux_x86_64'` instead of silently building the wrong platform.

## ✅ Examples that compile today

Hello world:

```csharp
namespace app;
class hello
{
    int main()
    {
        printf("hello");
        return 0;
    }
}
```

```bash
rubyc build hello.rc --target linux_x86_64 -o hello
```

Recursion, arrays, properties, virtual dispatch, switches — all working:

```csharp
namespace app;

class Fib
{
    public int Run(int n)
    {
        if (n < 2) { return n; }
        return Run(n - 1) + Run(n - 2);
    }
}

class Shape
{
    public virtual string Name() { return "shape"; }
}

class Circle : Shape
{
    public override string Name() { return "circle"; }
}

class Accumulator
{
    private int _total;
    public int Total { get { return _total; } }
    public void Add(int x) { _total += x; }
}

class Program
{
    int main()
    {
        Shape s = new Circle();      // virtual dispatch
        string n = s.Name();

        Accumulator a = new Accumulator();
        a.Add(5);
        switch (a.Total)             // switches, properties
        {
            case 5:
                break;
            default:
                break;
        }

        int[] ns = new int[3];       // arrays
        ns[0] = new Fib().Run(10);
        return 0;
    }
}
```

Beyond the basics, the compiler now handles real-world library patterns:
optional parameters, overload resolution with implicit conversions, extension
methods (`span.StartsWith(...)`, `s.AsSpan()`), nested types, generics-shaped
code, switch patterns, out-vars, discards, `is`-patterns, local functions,
`using` aliases, string interpolation, indexers, and explicit interface
implementations.

## 🖥️ CLI tour

```bash
rubyc run program.rc              # JIT-execute source or bytecode
rubyc build program.rc -o app     # fully static native executable
rubyc build --debug program.rc    # emit .symtab for GDB/LLDB
rubyc compile -O shared lib.rc -o librc.so    # C hosts: dlopen/dlsym
rubyc compile -O static lib.rc -o librc.a     # static archive for --lib linking
rubyc build solution.rcs --jobs 6             # concurrent project builds
rubyc build lib.rcp --rci-only                # just the .rci fragment, no codegen
rubyc lint program.rc             # lint diagnostics without compiling
rubyc dump program.rc             # AST inspection
rubyc dump program.rc -v          # AST + IR + machine code
rubyc list                        # list installed plugins
rubyc list --remote               # plugins available in the registry
```

Native interop, both directions:

```csharp
namespace app {
    class host {
        [LibraryImport("m", EntryPoint = "add")]
        static partial int add(int a, int b);

        int main() { printf("{0}", add(2, 3)); return 0; }
    }
}
```

```bash
rubyc build --lib libm.so host.rc -o host && ./host   # imports statically embedded
rubyc build --lib libm.a host.rc -o host && ./host    # same from a static archive
cc consumer.c -ldl -o consumer && ./consumer          # C calls librc.so's exports
```

Libraries install to `~/.local/share/rubyc/libraries/` (`.so` plus `.rci`
sidecars; `static/` holds `.a` archives; `index.tsv` maps types to owners).
A dynamic project needs no `ruby_c` entry for a `using` the index owns
alone — the fragment auto-links; two owners fail loudly instead.

## 📖 Help output

`rubyc --help`:

```text
RubyC Compiler v0.1.0

Usage: rubyc <COMMAND> [OPTIONS]

Commands:
  compile          compile source (alias for build)
  build            build a native executable, shared library, or bytecode
  run              compile and execute immediately
  lint             Check source for lint diagnostics without compiling
  dump             print AST, IR, and machine code structure
  list             List installed or remote target plugins
  install          Install a target plugin from the registry
  download         Download a target plugin from the registry
  remove           Remove an installed plugin, or a project from the nearest solution
  update           Update installed plugins to the latest version
  check-roundtrip  verify AST survives bytecode encode/decode
  new              scaffold a new project or solution
  create           alias for new
  add              add a project to the nearest solution

Global Options:
  -D, --define <SYM>        Seed a #If symbol (repeatable)
  -v, --verbose             Print stage-by-stage progress to stderr
  -q, --quiet               Suppress informational output
      --base <DIR>          Target installation root; its targets/ is scanned (env: RUBYC_BASE)
      --targets-dir <DIR>   Target discovery directory (beats --base; env: RUBYC_TARGETS_DIR)
  -t, --target <NAME>       Select compilation target by name (default: host)
      --no-color            Disable colored output
      --error-format <FMT>  Diagnostic style: text (default), json, json-pretty
      --warn <LINT>         Raise a lint to warning (e.g. --warn W0001)
      --deny <LINT>         Fail the build when a lint fires
      --allow <LINT>        Silence a lint entirely
  -h, --help       Print help
  -V, --version    Print version

QUICK START
rubyc run prog.rc                      compile + execute source (or bytecode)
rubyc build prog.rc -o app             static native executable
rubyc compile -O bytecode p.rc > p.rbc bytecode to stdout; run it: rubyc run < p.rbc
rubyc build bad.rc --error-format json machine-readable diagnostics (code/line/column)

Exit codes: 0 success · nonzero compilation failure or program exit · 2 bad usage

  --list-targets       enumerate installed compilation targets
Per-command flags and examples: rubyc <command> --help   ·   Full docs: documentation/reference/cli.md
```

`rubyc build --help`:

```text
build a native executable, shared library, or bytecode

Usage: rubyc build [OPTIONS] [FILE]

Options:
  -i, --in <FILE>              Input path ('-' or omitted reads stdin)
  -o, --out <FILE>             Output path ('-' means stdout)
  -O, --out-format <FMT>       bytecode, bytecode-shebang, base64-bytecode, exe (default), shared, static
  -B, --base64                 I/O bytes are base64 text
  -b, --bytecode               Shorthand for --out-format bytecode
      --shebang <INTERPRETER>  Wrap bytecode output in a '#!' interpreter line
  -g, --debug                  Emit debug symbols (.symtab) in ELF output
  -m, --no-magic               Omit the 🐕🇨 RubyC preamble from all emitted artifacts (AST, IR, ELF)
  -l, --lib <FILE>             Link a library file (.so body extraction, .a static merge); repeatable
      --rci-only               Emit only the .rci sidecar, skipping codegen and image writes
      --jobs <N>               Solution build parallelism (default 1)
  -P, --skip-pre-hook          Skip pre-build hooks (dev-loop only: assumes transient sources already materialized)
  -Q, --skip-post-hook         Skip post-build hooks (dev-loop only: leaves transient sources in place)

Global Options:  -D, --define <SYM>       Seed a #If symbol (repeatable)
  -v, --verbose            Print stage-by-stage progress to stderr
  -q, --quiet              Suppress informational output
      --base <DIR>         Target installation root; its targets/ is scanned (env: RUBYC_BASE)
      --targets-dir <DIR>  Target discovery directory (beats --base; env: RUBYC_TARGETS_DIR)
  -t, --target <NAME>      Select compilation target by name (default: host)

EXAMPLES:
  rubyc build hello.rc                    writes ./hello (exe default)
  rubyc build --bytecode hello.rc         raw bytecode to stdout
  rubyc build -O shared -o lib.so hello.rc  shared library
  cat hello.rc | rubyc build -o hello     piped in
```

## ⚡ Benchmarks: hello world vs C / C++

Same program (`hello`, no newline) in [`examples/benchmarks/`](examples/benchmarks/).
Runtimes are 200-run averages on the x86_64 dev VM (includes process spawn);
single run, expect noise. The RubyC binary is fully static with zero
dependencies; the C/C++ binaries are default dynamically-linked `gcc -O2` builds.

|                        | C (`gcc -O2`) | C++ (`g++ -O2`) | RubyC |
|------------------------|---------------|-----------------|-------|
| Binary size            | 12,568 B      | 12,576 B        | **4,688 B** |
| Runtime (avg)          | 516 µs        | 861 µs          | **226 µs** |
| Compile time           | 23 ms         | —               | 2 ms  |

```bash
gcc -O2 hello.c -o hello_c
g++ -O2 hello.cpp -o hello_cpp
rubyc build hello.rc --target linux_x86_64 -o hello_rc
```

## 🗂️ Workspace layout

**Targets** are dynamic plugins: no OS target is compiled into the `rubyc`
binary. Targets are separate `.so` crates, loaded at runtime across the plugin
ABI (IR bytes in, machine-code bytes out). The RubyC and RubyC-Assembly
languages are built directly into `rubyc-core`.

```
.                                   rubyc-cli (this repo, workspace root)
├── src/                            the `rubyc` binary
├── build.rs                        platform cfgs + plugin auto-install
├── rubyc-core/                     submodule: compiler core
│   └── plugins/
│       └── targets/
│           ├── rubyc-target/       target trait + discovery registry
│           ├── rubyc-target-linux-x86_64/  x86-64 backend (rbxt_*)
│           └── rubyc-target-windows-x86_64/  windows stub (name/triple/loud refusal)
└── examples/, documentation/       interop examples & docs
```

- **`rubyc-core`** — compiler stages (lexer → parser → AST → bytecode →
  semantic → IR → regalloc → x86-64), the plugin FFI/loader, the project
  system, i18n, and the C embedding ABI. The RubyC and RubyC-Assembly
  language frontends are built in. See its
  [README](rubyc-core/README.md) for every exposed function.
- **`rubyc-target`** — the polymorphic `TargetBackend` trait, the registry, and
  discovery. See [README](rubyc-core/plugins/targets/rubyc-target/README.md).
- **`rubyc-target-linux-x86_64`** — the x86-64 backend (encoder, regalloc,
  ELF writer, JIT). See
  [README](rubyc-core/plugins/targets/rubyc-target-linux-x86_64/README.md).
- **`rubyc-target-windows-x86_64`** — registered Windows stub (triple
  `x86_64-pc-windows-msvc`; every method refuses loudly). No backend yet.

## 🛠️ Development

```bash
cargo build                     # debug; auto-installs plugins
cargo build --release           # optimized
cargo test --workspace          # full suite (all crates)
cargo clippy --all-targets
RUBYC_RUN_GATES=1 cargo test --release --test perf_gates -- --ignored
```

See [documentation/compiler/benchmarks.md](documentation/compiler/benchmarks.md)
for the performance gates every backend change must pass.

## 🏛️ Architecture

The pipeline:

```text
Source
  ↓
Lexer / Parser
  ↓
AST
  ↓
Semantic analysis / type checking
  ↓
Lowering
  ↓
IR
  ↓
Optimisation
  ↓
Code generation
```

Core stages (relative to `rubyc-core/src/`):

| Stage | Path | Notes |
|---|---|---|
| Lexer | `parser/lexer.rs` | hand-written; block/line comments, string/int literals |
| Parser | `parser/rd/{unit,declaration,statement,expression}` | recursive descent → span-tagged AST |
| Bytecode | `bytecode/` | 🐕🇨 container v1, tag-tree encode/decode, round-trip verified |
| Semantic | `native/semantic/{mod,table}` | type tables, inheritance cycles, C# access rules |
| IR | `ir/{instructions,lower}` | virtual-register three-address lowering; vtables, imports, exports |
| Regalloc | `plugins/targets/rubyc-target-linux-x86_64/src/x86/regalloc.rs` | linear scan over callee-saved pool |
| Encoder | `plugins/targets/rubyc-target-linux-x86_64/src/x86/encode_x64.rs` | hand-encoded x86-64 on allocated vregs |
| Images | `…/elf.rs` / `…/shlib.rs` | static ET_EXEC · ET_DYN shared objects, GOT-based PIC |
| JIT | `native/jit.rs` | W^X: mmap W → copy → mprotect RX |
| Linter | `lints.rs` | stable ids (`W0001`…), `--warn/--deny/--allow` |

## 📍 Where the project stands

**Done — the compiler proper (1100+ tests green):**
full C# parsing surface (nullable, patterns, LINQ shapes, attributes,
preprocessor with `#if` logic, `using` aliases/statics, file-scoped
namespaces); name/scope resolution; overload resolution (arity, defaults,
implicit conversions, generics-tolerant); extension methods; nested types;
inheritance with C# hiding/accessibility (`private`/`protected`/`internal`,
nesting families); interface conformance incl. bases and statics; enums as
const-holders with `Object` members; `decimal` as 64-bit scalar; properties
(typed, assigned, placeholders for explicit accessors); indexers; switch
scoping/patterns; local functions (forward refs); out-vars; discards;
lambdas (parsed); primary constructors; ` `fixed`, `stackalloc`-shapes,
function pointers, `ref` returns; `default`/`typeof`/`sizeof`/`nameof`;
`checked`/`unchecked`; delegates (parsed, invoked as placeholder);
`[LibraryImport]`/`[DllImport]` externs; multi-arity generic instantiation
tolerance; same-name disambiguation (nesting, arity, method-anchored entries);
bytecode v1 round-trips; CLI (`build/run/lint/dump/list/...`, JSON errors,
i18n); `--skip-pre-hook`/`--skip-post-hook` dev flags.

**In progress — the `dotnet/runtime` campaign:**
`tools/pull-dotnet.sh` sparse-pulls 181 libraries and emits `.rcp`/`.rcs`
per library; ~50 `src/coreclr` partials are staged for `System.Private.CoreLib`.
Parse/scan/semantic gates are at zero; lowering is driven one blocking error
at a time (currently deep into `System.Private.CoreLib`, ~2.5 min per full
measurement with the release binary).

## ✅ Todo: complete vs remaining

### ✅ Done
- [x] Lexer/parser for the full C# surface above (1100+ tests and counting:
      790 core, 93 x86-64 backend, 6 windows stub, 254 CLI end-to-end)
- [x] Semantic analysis: scopes, type tables, inheritance, access, const folding
- [x] Lowering: calls (static/virtual/import), ctors, fields, properties, switches
- [x] Overload + extension + generic-tolerant resolution
- [x] Bytecode container v1 with round-trip tests
- [x] CLI with projects/solutions/hooks, JSON diagnostics, i18n
- [x] x86-64 backend: encoder, regalloc, ELF images, JIT loader
- [x] Pull script + per-library `.rcp`/`.rcs` generation (181 libs)
- [x] Static archives (`.a`) + static link step; `--lib` works for `.so` and `.a`
- [x] `using`-driven discovery via `libraries/index.tsv` (single-owner auto-link)
- [x] Backfill speed: `--rci-only` fragments, `--jobs` parallel solution builds
- [x] Flattened install layout (`libraries/`, `static/` subdir for archives)
- [x] CoreCLR partial staging for System.Private.CoreLib
- [x] Dev-loop flags (`--skip-pre-hook`, `--skip-post-hook`)

### ⬜ Remaining — to compile the dotnet runtime
- [ ] Drive `System.Private.CoreLib` lowering to zero errors (in progress —
      one blocking construct at a time; parse/scan/semantic already zero)
- [ ] Backend validation for CoreLib scale (vtable layout, field offsets,
      image emission for ~1800 classes — untested at this size)
- [ ] Implicit CoreLib reference for dependent libraries (`Debug`,
      `ArgumentNullException`, etc. need no explicit `ruby_c` entry —
      `using`-driven discovery covers installed libraries today)
- [ ] Same measure-fix loop for the other 180 pulled libraries
- [ ] Execute + verify built artifacts on the VM (`scp`/`ssh rubyc-dev`
      workflow: build locally, run natively, compare output)
- [ ] Runtime-correctness workstream (current placeholders — delegates,
      explicit accessors, P/Invoke bodies, decimal precision — need real
      implementations before programs run *correctly*, not just compile)
- [ ] Full generic preservation (monomorphization; today: arity-tolerant
      approximations) and qualified type identity (same-named nesteds)
- [ ] Lambdas/delegates/closures, async state machines, LINQ lowering
- [ ] Exceptions (EH tables, filters), attributes emission, strings workstream

## 📁 Project system

```bash
rubyc new solution MyApp        # scaffold .rcs + default .rcp
rubyc build                     # auto-detect nearest .rcp/.rcs upward
rubyc run                       # auto-detect and run nearest project
rubyc lint                      # auto-detect and lint nearest project
```

Project files are TOML (`.rcp`); solutions group projects in `.rcs` files.
See [documentation/](documentation/README.md) for the full schema.

## 🎯 Target discovery

```bash
rubyc list                      # installed targets
rubyc list --kind target        # installed targets only
rubyc list --remote             # plugins available in the registry
rubyc --list-targets            # enumerate installed compilation targets
```

The discovery directory can be overridden with `--targets-dir` (or its
environment-variable equivalent).

## 🌐 Diagnostics & i18n

Errors carry codes (`E00xx` errors, `W00xx` lints), spans, scope context
(`ns::class::fn`), colored text or JSON output (`--error-format json`).
Diagnostics text lives in `rubyc-core/locales/en.json` (English during
development; more locales later). Lowering errors name their method
(`[in Class.Method]`).

## 🧪 Testing

1100+ tests (790 core lib, 93 x86-64 backend, 6 windows stub, 254 CLI
end-to-end) run via `cargo test -p rubyc-core --lib` (seconds) plus the
backend and CLI suites. The suite
covers parser round-trips, bytecode identity, access modifiers, lint policy,
CLI pipes, i18n, target discovery, and C↔RubyC interop. End-to-end tests that
spawn the `rubyc` binary live in `rubyc-cli`; executable tests run on the VM
(`ssh rubyc-dev`) since the host can't run x86-64 targets natively in this
setup.
