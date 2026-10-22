# Embedding RubyC

The compiler core is a Rust library crate (`rubyc-core`, published to the
workspace as `rubyc`). A host program drives it directly; there is no
C ABI in the tree today.

## Embedding from Rust

```rust
use rubyc::prelude::{parse, encode, ToBytecode};

let unit = parse("namespace t; class c { int main() { printf(\"hi\\n\"); } }")
    .map_err(|diags| format!("{diags:?}"))?;
let bytes = encode(&unit);
```

`parse` returns the AST; `encode` turns it into the 🐕🇨 bytecode
container. The full surface is in the crate's `prelude` module, and
`src/driver.rs` shows the whole pipeline (parse → preprocess → lower →
emit) if you want one call instead of four.

## Embedding from C

There is no C ABI at the moment: `rubyc-core` builds as an `rlib`, with
no `cdylib` and no C header. The `examples/interop/` tree shows the
direction that works today — a RubyC program calls into a C library
through `[LibraryImport]` — and a C host that drives a compiled
RubyC *shared library* through its own exported symbols.

For a C host that wants to compile RubyC source in-process, the
supported route is to call the `rubyc` binary: it takes a file or stdin
and writes an executable, a shared library, or bytecode to a file or
stdout, so a shell or a C program can drive it with `popen`/`exec`.

## Calling into a compiled RubyC library from C

A RubyC shared library exports its public methods as ordinary C symbols
and carries its type fragment in the image, so a C host can call them
without the compiler at run time. See
[interop.md](interop.md) for the full round trip, and
`examples/interop/` for the sources.

## Notes

- The compiler is a Rust library; embedding it from another language
  means either a Rust host or driving the CLI.
- Compiled *programs* are fully static and standalone. Only the compiler
  is a library that a host links against.
