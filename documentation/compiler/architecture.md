# Compiler architecture

The pipeline is: **lexer → parser → (bytecode) → semantic → IR lowering →
register allocation → x86-64 emission → image writer / JIT**.

## Module placement rules

Every source file lives beside its pipeline stage. The rules are enforced
by review and kept this way deliberately — a file that outgrows its home is
split or moved **in the same commit** that pushes it over the line:

| Directory | Contains | May depend on |
|---|---|---|
| `src/models/` | Pure data: AST nodes, tokens, spans, symbol tables, diagnostics. Includes `compare.rs` (structural equality) and `visitor.rs` (AST traversal) | nothing upward |
| `src/parser/` | Lexer + recursive-descent parser (`rd/` split by grammar area) | models |
| `src/bytecode/` | Container format, tag-tree encode/decode, serialization traits | models |
| `src/native/semantic/` | Type tables + access/conformance checks | models |
| `src/native/ir/` | Virtual-register IR + lowering | semantic, models |
| `src/native/codegen/` | Target-neutral backend machinery (register allocation) | ir |
| `src/native/target_os/<os>/` | Current in-core platform encoders, image writers, loaders, memory | codegen, ir |
| `src/parser/assembly/` | NASM-flavored assembly parser and x86-64 byte encoder | parser models |
| `src/cli/`, `src/driver.rs` | Arg tables/parsing/rendering; embedding facade | everything below |
| `src/utilities/` | Self-contained helpers with no compiler knowledge (base64, `{}` format engine) | std only |
| `src/lints.rs` | Rule engine over AST + semantic data | parser, models |

Hard constraints: no dependency cycles; `models/` never imports logic
layers; native platform specifics exist inside `target_os/<name>/`; shared
helpers live in `utilities/`, never duplicated.

## Module map

| Stage | Path | Output |
|---|---|---|
| Lexer | `src/parser/lexer.rs` | `Vec<Token>` |
| Parser | `src/parser/rd/` | `CompilationUnit` AST |
| Bytecode | `src/bytecode/` | 🐕🇨 container, tag-tree encode/decode |
| Semantic | `src/native/semantic/` | `TypeTable` + diagnostics |
| IR lowering | `src/native/ir/lower.rs` | virtual-register three-address code |
| Regalloc | `src/native/codegen/regalloc.rs` | vreg → callee-saved reg or slot |
| Emission | `src/native/target_os/linux_x86_64/encode_x64.rs` | machine code + relocations |
| Images | `elf.rs` (exec) · `shlib.rs` (shared) | static ELF / ET_DYN `.so` |
| Assembly | `src/parser/assembly/` | parsed NASM-flavored subset → x86-64 bytes |

## Target crates versus platform code

`modules/targets/rubyc-target` defines the polymorphic `TargetBackend` trait,
the registry, and discovery paths. `modules/targets/rubyc-target-linux-x86_64`
is the intended platform adapter, but its lowering and image-writing methods
currently report that work is delegated to the core pipeline. The production
Linux x86-64 implementation therefore remains in
`compiler/rubyc-core/src/native/target_os/linux_x86_64/`.

The registry currently exposes the built-in `linux_x86_64` name. Language
metadata files can augment language discovery; loading executable target
backends from external manifests is future work.

## Assembly paths

The built-in assembler is not a target crate. It lives in
`compiler/rubyc-core/src/parser/assembly/` and provides a two-pass parser and
encoder. Standalone `.asm` input is assembled by the driver and wrapped by
the Linux x86-64 ELF image writer. Preprocessing also captures `#Assembly`
blocks on the compilation unit, but lowering those blocks to raw native IR is
not complete yet.

## Register model

- Allocation pool: **callee-saved only** (`rbx`, `r12`–`r15`) — a call can
  never invalidate an allocated value, so emission needs no liveness
  reasoning.
- Caller-saved registers are reserved for arguments (`rdi`=receiver,
  `rsi`…=params), syscall ABI, division operands, and scratch (`rax`
  result path, `r10`/`r11` addressing).
- Functions save/restore exactly the subset of callee-saved registers they
  use; spill slots live in the frame at `[rbp - 8*(k+1)]`.

## Calling convention

Receiver `rdi`, parameters `rsi, rdx, rcx, r8, r9`, tail on the stack,
return `rax`. SysV-shaped so `[LibraryImport]` externals share the path;
shared-library export trampolines shift the C register window one slot to
insert a NULL receiver.

## Relocations

Every unknown address is emitted as a zero placeholder plus a relocation:

- `Rel32Call` — internal calls and runtime helpers.
- `Abs64` — string pointers, vtable rows, heap base, import targets.

The ELF writer resolves them against final addresses; the shared-library
writer instead redirects them through a GOT with `R_X86_64_RELATIVE`
records so the dynamic loader fixes them up at any load base.

## Constant folding

Literal-only arithmetic collapses during lowering (wrapping semantics;
division by zero and `i64::MIN / -1` stay runtime traps). Values propagate
through moves, so chained locals fold too. See `tests/const_fold.rs`.
