# CLI reference

`rubyc` reads RubyC source or bytecode from a file **or stdin**, and
writes its output to a file **or stdout**. Every command follows the same
I/O contract, which is what makes shell pipes work naturally.

```text
rubyc <command> [flags]
```

## Commands

| Command | Purpose | Output default |
|---|---|---|
| `compile` | source → chosen `-O` format (bytecode by default) | stdout |
| `build` | source → native executable (`-O exe` fixed); auto-discovers `.rcs`/`.rcp` when no file given | `out` (or `out.exe` on Windows) |
| `run` | compile in-memory + execute immediately; accepts source *or* bytecode; auto-discovers `.rcs`/`.rcp` | program's stdout |
| `lint` | check source for lint diagnostics without compiling; auto-discovers `.rcs`/`.rcp` | diagnostics on stderr |
| `dump` | human-readable dump of bytecode input | stdout |
| `check-roundtrip` | parse → encode → decode → structural compare; CI guard | report on stderr |
| `list` | list installed plugins (`--remote` for registry listing) | stdout |
| `install` | install a target plugin from the registry (stub) | — |
| `download` | download a plugin from the registry (stub) | — |
| `remove` | remove an installed plugin (stub) | — |
| `update` | update installed plugins (stub) | — |

## Flags

| Flag | Long form | Values | Meaning |
|---|---|---|---|
| `-i FILE` | `--in` / `--file` | path or `-` | input; omitted **or `-`** = stdin |
| `-o FILE` | `--out` | path or `-` | output; `-` = stdout |
| `-I FMT` | `--in-format` | `auto` · `source` · `bytecode` | force input interpretation |
| `-O FMT` | `--out-format` | `bytecode` · `bytecode-shebang` · `base64-bytecode` · `exe` · `shared` | output format |
| `-B` | `--base64` | — | base64-encode/decode bytecode (text-pipe safe) |
| | `--shebang[=INTERP]` | interpreter line | prepend `#!/usr/bin/env rubyc` (default) to bytecode |
| | `--lib PATH` | `.so` path, repeatable | link `[LibraryImport]` stubs against these objects |
| | `--error-format FMT` | `text` · `json` · `json-pretty` | diagnostic rendering (see below) |
| | `--no-color` | — | disable ANSI colors (auto-off when piped or `NO_COLOR` set) |
| | `--define SYM` | repeatable | seed preprocessor symbols for `#If` |
| | `--warn RULE` / `--deny RULE` / `--allow RULE` | rule id or name | lint level overrides, applied in CLI order |
| `-v` / `-q` | `--verbose` / `--quiet` | — | more / less output |
| | `--kind KIND` | `target` | filter `list` output by plugin kind |
| | `--remote` | — | `list` shows registry plugins instead of installed |
| | `--url URL` | direct URL | `download` fetches a plugin from a URL |

Flags accept both `--flag value` and `--flag=value`.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | success (or a RubyC program returning zero) |
| program's value | an `int main()`/`Main()` return becomes the process exit code |
| `1` | compilation failed, or a denied lint fired |
| `2` | bad command line (unknown command/flag, invalid value) — check stderr for usage hint |

---

## Piping: the four directions

### 1. Source in via pipe → bytecode out

```bash
cat app.rc | rubyc compile -O bytecode > app.rbc
# or without cat:
rubyc compile -O bytecode < app.rc > app.rbc
```

The preamble `[EB 08]…🐕🇨…` on stdout identifies RubyC bytecode.

### 2. Bytecode between two rubyc invocations

```bash
rubyc compile -O bytecode app.rc | rubyc run
```

`run` sniffs the magic bytes, so `-I` is only needed when input format is
ambiguous (see [garbage rejection](#safety)).

### 3. Build an executable from piped source

```bash
rubyc build -o app < app.rc && ./app
```

### 4. Text-safe bytecode through base64

Binary bytecode survives most pipes, but not tools that mangle bytes
(chat, JSON payloads, old mail). Base64 makes it text:

```bash
rubyc compile -O bytecode -B app.rc > app.b64      # pure ASCII out
rubyc run -B < app.b64                              # decode + execute
```

## Heredocs and quick scripts

```bash
rubyc run <<'EOF'
namespace t; class c { int main() { printf("hi\n"); } }
EOF
```

No temp files needed for one-shot experiments.

## Shebang scripts

Make a directly executable script:

```bash
rubyc compile -O bytecode-shebang app.rc > app && chmod +x app
./app                       # kernel hands the file to `rubyc` via env
```

Custom interpreter path:

```bash
rubyc compile --shebang=/opt/rubyc/bin/rubyc -O bytecode-shebang app.rc > app
```

## Diagnostics

Text mode (default, colored on terminals):

```text
error[E0001]: expected an expression, found Semicolon
    in t::c::main
--> bad.rc:1
   |
 1 | namespace t { class c { int main() { int x = ; } } }
   |                                              ^
```

JSON mode for editors/CI:

```bash
rubyc build bad.rc --error-format json
```

```json
[{"code":"E0001","severity":"error","message":"expected an expression, found Semicolon",
  "help":null,"file":"bad.rc","line":1,"column":46,"endColumn":47,
  "namespace":"t","class":"c","function":"main",
  "sourceLine":"namespace t { class c { int main() { int x = ; } } }"}]
```

Fields are stable; new fields may be appended (additive versioning).
`json-pretty` is identical content, indented.

## Shared libraries

```bash
rubyc build -O shared lib.rc -o librubyclib.so     # exports public methods
rubyc build app.rc --lib libm.so -o app             # bind [LibraryImport] stubs
```

See [attributes](../language/attributes.md) for the source side.

## Safety

Input that is neither valid source nor valid bytecode is rejected with a
non-zero exit — never misinterpreted:

```bash
$ echo "definitely not bytecode" | rubyc run -I bytecode
error: … ; exit 1
```

## Assembly input

`build` accepts NASM-flavored `.asm` files through the built-in assembler
(same 🐕🇨 container, same flags):

```bash
rubyc build hello.asm -o hello     # text → machine code → static ELF
```

The implementation and its encoding tests live under
`compiler/rubyc-core/src/parser/assembly/` and
`tests/rubyc-core/parser/assembly_encoding.rs`.

## Target discovery

```bash
rubyc --list-targets       # standard metadata for every discovered target
```

Discovery directory:
| OS | Targets |
|---|---|
| Linux | `~/.local/share/rubyc/targets/` |
| macOS | `~/Library/Application Support/rubyc/targets/` |
| Windows | `%LOCALAPPDATA%\rubyc\targets\` |

Override with `--targets-dir PATH` or env var `RUBYC_TARGETS_DIR`.
Discovery is metadata-only: executing an external target backend still
requires a future stable plugin ABI.

## Project system (.rcp / .rcs)

```bash
rubyc build MyApp.rcp          # build from project file (hooks fire)
rubyc build                    # auto-detect nearest .rcp/.rcs upward
```

Project files use TOML:
```toml
# MyApp.rcp
[meta]
name = "MyApp"
version = "0.1.0"

[target]
platform = "linux_x86_64"
output = "exe"

[build]
sources = ["main.rc"]

[hooks]
pre-build = "echo starting"
post-build = "strip ./MyApp"
```

Solutions group projects in `.rcs` files with `[[project]] path = "…"`.
See [project-system](project-system.md) for full reference.

## Per-command help

Every command has its own help with flags and examples:

```bash
rubyc compile --help    # compile-specific flags + examples
rubyc build --help      # build-specific flags + examples
rubyc lint --help       # lint-specific flags + examples
rubyc list --help       # list-specific flags + examples
```

## Plugin management

```bash
rubyc list                           # all installed targets
rubyc list --kind target             # installed targets only
rubyc list --remote                  # plugins available in the registry
rubyc install aarch64 --type target  # install a target plugin (stub)
rubyc download myplugin              # download a plugin (stub)
rubyc download --url https://example.com/p.so myplugin
rubyc remove mytarget                # remove an installed plugin (stub)
rubyc update                         # update installed plugins (stub)
```

## Diagnostics: JSON schema

`--error-format json` emits a JSON array on stderr:

| Field | Type | Always present |
|---|---|---|
| `code` | string (`E0001`, `E0002`, …) | ✓ |
| `severity` | `"error"` · `"warning"` | ✓ |
| `message` | human-readable text | ✓ |
| `help` | string or null | ✓ |
| `file` | string path | ✓ |
| `line` | 1-based integer | ✓ |
| `column` | 1-based integer | ✓ |
| `endColumn` | 1-based integer | ✓ |
| `namespace` / `class` / `function` | scope context strings or null | ✓ |
| `sourceLine` | raw source text of the line | ✓ |

Fields are stable; new fields may be appended (additive versioning).
`json-pretty` is identical content, indented for readability.

## Linting

```bash
rubyc lint hello.rc                # check a single file
rubyc lint                         # auto-discover nearest project
rubyc lint proj.rcs                # check every project in a solution
rubyc lint bad.rc --deny W0001     # fail on a specific lint
```

`lint` runs the parser, semantic analysis, and lint rules without producing
any output artifact. It exits `1` when a denied lint fires or a compile error
occurs.

## Introspection

```bash
rubyc dump app.rbc                 # instruction-level listing
rubyc check-roundtrip app.rc       # AST survives bytecode encode/decode?
rubyc list                         # list installed plugins
rubyc --help                       # command list + flags (spec-driven)
```
