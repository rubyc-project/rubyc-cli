# RubyC IR — Overview

The IR (Intermediate Representation) is the contract between language frontends
and target backends. It is language-agnostic: any language (RubyC, Python,
Rust, C#, or a custom DSL) can lower to it, and any target (x86_64, ARM64,
WASM, or a custom ISA) can consume it.

## Design Goals

1. **Language-agnostic** — No OOP-specific assumptions. Vtables, `this`
   receivers, and refcount GC are *optional* features, not requirements.
   A language without classes emits no `CallVirt`/`VTableAddr`/`Retain`/
   `Release` instructions.

2. **Backend-agnostic** — The IR describes *what* to compute, not *how*.
   Register allocation, calling conventions, and instruction scheduling are
   the backend's responsibility.

3. **Three-address form** — Every computation writes to a named virtual
   register (vreg). No in-place updates, no implicit stack.

4. **Basic blocks** — Code is organized into basic blocks with explicit
   terminators. No hidden control flow.

5. **Single 64-bit value model** — Everything is a 64-bit word in a vreg.
   Integers (i64) and floats (f64) share the same register space.
   Objects are pointers. Strings are pointers into a data blob.

## The Big Picture

```
Source ──► AST ──► TypeTable ──► IR Program ──► MachineCode ──► ELF
                      │                │
                      │                └──► (serialized) ──► Plugin
                      │
                      └── semantic analysis, type checking
```

- **Frontend** (language-specific): parses source into an AST, performs
  semantic analysis to build a `TypeTable`, and lowers the AST to an
  `IR Program`.
- **IR** (language-agnostic): a `Program` is a vector of `IrFunction`s,
  each a vector of `Block`s, each a vector of `Instr`s plus a `Terminator`.
  The `Program` also carries vtables, imports, exports, strings, and
  metadata.
- **Serialization** (plugin ABI): the `Program` is serialized to a versioned
  byte container. This is the boundary between the compiler core and
  target/language plugins.
- **Backend** (target-specific): consumes a `Program`, performs register
  allocation, emits machine code, and produces `MachineCode` (code + data
  + relocations).
- **Image writer** (target-specific): turns `MachineCode` into a loadable
  ELF image.

## Key Concepts

| Concept | Description |
|---|---|
| **VReg** | A 32-bit virtual register index. Vreg 0 in a function with a receiver is the receiver. |
| **Block** | A basic block: a sequence of instructions terminated by exactly one `Terminator`. |
| **Instr** | A three-address instruction. See [instructions.md](instructions.md). |
| **Terminator** | How a block ends: `Ret`, `Exit`, `Branch`, or `ExitWith`. |
| **Program** | The top-level IR unit: functions, vtables, imports, exports, strings, names, entry point. |
| **ReceiverKind** | How a function receives its object: `None` (free function), `This` (implicit receiver), `Explicit` (receiver is a regular param). |

## Related Pages

- [instructions.md](instructions.md) — every instruction, its operands, and semantics
- [types-and-values.md](types-and-values.md) — the 64-bit value model
- [object-model.md](object-model.md) — object layout, heap, GC
- [control-flow.md](control-flow.md) — blocks, terminators, loops, exceptions
- [calling-convention.md](calling-convention.md) — how calls are encoded
- [serialization.md](serialization.md) — the binary format
- [plugin-abi.md](plugin-abi.md) — how to build a target plugin
- [writing-a-target.md](writing-a-target.md) — step-by-step: build a new backend
- [extension-points.md](extension-points.md) — how to extend the IR
