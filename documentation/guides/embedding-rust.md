# Embedding RubyC from Rust

The `rubyc-core` crate is a normal Rust dependency. Use the driver API for
high-level operations, or drop to the individual stages.

## Cargo

```toml
[dependencies]
rubyc-core = { path = "../rubyc-core" }
```

The package is `rubyc-core`; the library it exports is `rubyc` (the
`[lib] name` in its `Cargo.toml`), so the import paths are
`rubyc::…`, not `rubyc_core::…`.

## Compile

`compile_source` takes the source, the `--lib` files for
`[LibraryImport]`, and the pipeline (the backend and the image writer
the CLI would have picked).

```rust
use rubyc::driver::compile_source;
use rubyc::native::NativePipeline;
use std::path::PathBuf;

let pipeline = /* NativePipeline::new(target, backend, image_writer,
                   loader, memory) — the ports the CLI builds for the
                   selected target */;
let image = compile_source(
    "namespace t; class h { int main() { printf(\"ok\"); } }",
    &[],                                    // --lib files for [LibraryImport]
    &pipeline,
)?;
std::fs::write("app", image)?;
```

The pipeline is a bundle of the four ports a target provides
(`CodeGenBackend`, `ImageWriter`, `BytecodeLoader`, `MemoryManager`);
`NativePipeline::new` takes them in that order, and `with_target`
routes the same work through a target plugin's FFI boundary instead.

## Run (JIT)

```rust
let status = rubyc::driver::run_source(
    "namespace t; class h { int main() { printf(\"ok\"); } }",
    &pipeline,
)?;
assert_eq!(status, 0);
```

## Parse only

```rust
let unit = rubyc::driver::parse_source(src)?;   // Vec<Diagnostic> on error
for d in rubyc::lints::analyze(&unit, &Default::default()) {
    println!("{} {}", d.severity, d.message);
}
```

See `src/driver.rs` for the full surface (`cli_main` mirrors the CLI).
