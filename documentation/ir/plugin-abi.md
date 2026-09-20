# Plugin ABI

Plugins are the mechanism for adding new targets without modifying the
compiler core. The RubyC and RubyC-Assembly language frontends are built
directly into `rubyc-core`; only targets are dynamic plugins.

## Plugin Discovery

Plugins are discovered via `.toml` manifest files in the targets directory:

```
~/.local/share/rubyc/
└── targets/
    └── my_target/
        ├── my_target.toml    # manifest
        └── libmy_target.so   # ELF plugin
```

The manifest file describes the plugin:

```toml
name = "my_target"
version = "0.1.0"
triple = "aarch64-unknown-linux-gnu"
kind = "target"
entry = "my_target_lower"
```

## ELF Plugin Format

Plugins are x86_64 ELF shared objects, loaded via `dlopen`. They must
export specific symbols:

### Target Plugin Symbols

| Symbol | Type | Purpose |
|---|---|---|
| `rbxt_name` | `&'static str` | Plugin name |
| `rbxt_triple` | `&'static str` | Target triple |
| `rbxt_base_addr` | `fn() -> usize` | Base address of the loaded plugin |
| `rbxt_lower` | `fn(&[u8]) -> Result<Vec<u8>, String>` | Lower IR bytes to MachineCode |
| `rbxt_write_image` | `fn(&[u8], bool) -> Result<Vec<u8>, String>` | Emit native image bytes |
| `rbxt_write_shared` | `fn(&[u8]) -> Result<Vec<u8>, String>` | Emit shared library bytes |
| `rbxt_run` | `fn(&[u8], ...) -> Result<i32, String>` | JIT-run the produced code |

## Building a Target Plugin

1. Create a new crate (or add to an existing one) that depends on
   `rubyc-core`.
2. Implement `CodeGenBackend`:
   ```rust
   impl CodeGenBackend for MyBackend {
       fn lower(&self, program: &Program) -> Result<MachineCode, TargetError> {
           // ... register allocation, code emission ...
       }
   }
   ```
3. Export the required symbols:
   ```rust
   #[no_mangle]
   pub extern "C" fn rbxt_name() -> &'static str { "my_target" }

   #[no_mangle]
   pub extern "C" fn rbxt_triple() -> &'static str { "aarch64-unknown-linux-gnu" }

   #[no_mangle]
   pub extern "C" fn rbxt_lower(ir_bytes: &[u8]) -> Result<Vec<u8>, String> {
       let program = decode_program(ir_bytes).map_err(|e| e.to_string())?;
       let backend = MyBackend;
       let mc = backend.lower(&program).map_err(|e| e.to_string())?;
       Ok(serialize_machine_code(&mc))
   }
   ```
4. Build as a shared library:
   ```
   cargo build --release --target x86_64-unknown-linux-gnu
   ```
5. Place the `.so` and `.toml` in `~/.local/share/rubyc/targets/my_target/`.

## The Byte Boundary

The key insight: plugins communicate through **bytes**, not Rust types.

- The compiler core serializes the IR `Program` to bytes.
- The target plugin deserializes the bytes into a `Program` and lowers it.

This means plugins can be written in any language that can produce a
compatible byte format (C, Rust, etc.). The byte format is the ABI.

## Version Compatibility

- IR containers carry a version number. A target plugin must handle the
  IR version it was built against.
- If the IR format changes, old plugins may fail to decode new containers
  (or vice versa). The version number allows the compiler to reject
  incompatible plugins.
