# Embedding RubyC from Rust

The `rubyc-core` crate is a normal Rust dependency. Use the driver API for
high-level operations, or drop to the individual stages.

## Cargo

```toml
[dependencies]
rubyc-core = { path = "../rubyc-core" }
```

## Compile

```rust
use rubyc_core::driver::compile_source;
use std::path::PathBuf;

let image = compile_source(
    "namespace t; class h { int main() { printf(\"ok\"); } }",
    &[],                                    // --lib files for [LibraryImport]
)?;
std::fs::write("app", image)?;
```

## Run (JIT)

```rust
let status = rubyc_core::driver::run_source(
    "namespace t; class h { int main() { printf(\"ok\"); } }",
)?;
assert_eq!(status, 0);
```

## Parse only

```rust
let unit = rubyc_core::driver::parse_source(src)?;   // Vec<Diagnostic> on error
for d in rubyc_core::lints::analyze(&unit, &Default::default()) {
    println!("{} {}", d.severity, d.message);
}
```

See `src/driver.rs` for the full surface (`cli_main` mirrors the CLI).
