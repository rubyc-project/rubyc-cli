# RubyC Documentation

| Folder | Contents |
|---|---|
| [language/](language/) | the RubyC language: syntax, types, semantics |
| [stdlib/](stdlib/) | built-in functions and runtime surface (`printf`, …) |
| [compiler/](compiler/) | architecture, bytecode format, benchmarks, lints |
| [guides/](guides/) | task-oriented: getting started, interop, embedding |
| [reference/](reference/) | diagnostic codes, CLI reference |
| [ir/](ir/) | the intermediate representation: instructions, value model, plugin ABI |

## Current pages

**Guides**
- [guides/getting-started.md](guides/getting-started.md) — hello world,
  variables, classes, interop, lints
- [guides/interop.md](guides/interop.md) — consuming C libraries from RubyC
  and RubyC libraries from C
- [guides/embedding-c.md](guides/embedding-c.md) — link librubyc.so from C
- [guides/embedding-rust.md](guides/embedding-rust.md) — driver API from Rust

**Compiler**
- [compiler/benchmarks.md](compiler/benchmarks.md) — performance harness,
  methodology, baseline numbers, acceptance gates
- [compiler/architecture.md](compiler/architecture.md) — pipeline, module
  map, register model, calling convention, relocations, folding

**Language**
- [language/types-and-variables.md · language/control-flow.md · language/macros.md](language/types-and-variables.md)
- [language/classes.md](language/classes.md)
- [language/delegates.md](language/delegates.md) — named callable types and
  generic delegates
- [language/attributes.md](language/attributes.md)

**Stdlib**
- [stdlib/native-builtins.md](stdlib/native-builtins.md)

**IR**
- [ir/README.md](ir/README.md) — overview: design goals, big picture, key concepts
- [ir/instructions.md](ir/instructions.md) — every instruction with operands and semantics
- [ir/types-and-values.md](ir/types-and-values.md) — the 64-bit value model
- [ir/object-model.md](ir/object-model.md) — object layout, heap, GC
- [ir/control-flow.md](ir/control-flow.md) — blocks, terminators, loops, exceptions
- [ir/calling-convention.md](ir/calling-convention.md) — how calls are encoded
- [ir/serialization.md](ir/serialization.md) — the binary format
- [ir/plugin-abi.md](ir/plugin-abi.md) — how to build a target plugin
- [ir/writing-a-target.md](ir/writing-a-target.md) — step-by-step: build a new backend
- [ir/extension-points.md](ir/extension-points.md) — how to extend the IR

**Reference**
- [reference/cli.md](reference/cli.md) — commands and flags
- [reference/diagnostics.md](reference/diagnostics.md) — every E/W code with fixes

Pages land in the same milestone as the feature they document; this index
is updated every phase.
- [reference/project-system.md](reference/project-system.md) — project system (.rcs/.rcp)
- [guides/bare-metal.md](guides/bare-metal.md) — bare-metal target guide
- [reference/preprocessing.md](reference/preprocessing.md) — preprocessor directives
