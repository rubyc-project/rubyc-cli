# Control Flow

## Basic Blocks

Code is organized into basic blocks. Each block is a sequence of
instructions terminated by exactly one terminator. There are no branches
*within* a block — all conditional jumps are at the end.

```
Block 0:
  Const { dst: v1, imm: 0 }
  Branch { cond: None, if_true: 1, if_false: 1 }  // unconditional

Block 1:
  ...
  Branch { cond: Some(v2), if_true: 2, if_false: 3 }

Block 2:
  ...
  Ret { src: Some(v4) }

Block 3:
  ...
  Ret { src: None }
```

## Terminators

Every block ends with exactly one terminator:

| Terminator | Semantics |
|---|---|
| `Ret { src }` | Return from the function. `src` is the return value (or `None` for void). |
| `Exit` | Exit the process with code 0. |
| `ExitWith { code }` | Exit the process with code `code`. |
| `Branch { cond, if_true, if_false }` | Conditional or unconditional branch. |

### Branch Details

- `Branch { cond: None, ... }` — unconditional jump to `if_true`.
  `if_false` is ignored.
- `Branch { cond: Some(v), if_true: t, if_false: f }` — if `v != 0`,
  go to block `t`; else go to block `f`.

## Loops

Loops are encoded as branch-based control flow. There are no `while` or
`for` instructions — the frontend desugars loops into branches.

### While Loop

```
// while (cond) { body }

Block 0 (header):
  ... compute cond into v1
  Branch { cond: Some(v1), if_true: 1, if_false: 2 }  // body / exit

Block 1 (body):
  ... body instructions
  Branch { cond: None, if_true: 0, if_false: 0 }      // back to header

Block 2 (exit):
  ...
```

### For Loop

A `for` loop is desugared into a while loop with an init block:

```
// for (init; cond; step) { body }

Block 0 (init):
  ... init instructions
  Branch { cond: None, if_true: 1, if_false: 1 }      // to header

Block 1 (header):
  ... compute cond into v1
  Branch { cond: Some(v1), if_true: 2, if_false: 3 }  // body / exit

Block 2 (body):
  ... body instructions
  ... step instructions
  Branch { cond: None, if_true: 1, if_false: 1 }      // back to header

Block 3 (exit):
  ...
```

### Break / Continue

`break` and `continue` are desugared into branches:
- `continue` → branch to the loop header
- `break` → branch to the block after the loop

## Entry Point

`Program::entry` is the index of the function to call when the program
starts. For executables, this is typically the `@start` function (which
calls `main`). For shared libraries, `entry` is `None` — the library
exports functions that are called by the host.

```rust
// For an executable:
Program {
    entry: Some(0),  // function 0 is @start
    functions: vec![
        IrFunction { name: "@start", ... },
        IrFunction { name: "Main.main", ... },
    ],
    ...
}

// For a shared library:
Program {
    entry: None,
    functions: vec![
        IrFunction { name: "MyClass.method", ... },
    ],
    ...
}
```

## Exceptions (Optional)

Languages with exceptions use `Unwind` and `Resume`:

```
// try { body } catch (e) { handler }

Block 0 (body):
  ... body instructions
  // If an exception occurs, Unwind raises it
  Ret { src: None }

Block 1 (handler):
  // Resume sets up the catch parameter
  ... handler instructions
  Ret { src: None }
```

The backend is responsible for implementing the actual exception
mechanism (zero-cost, setjmp/longjmp, table-based, etc.). The IR only
says *where* the exception is raised and *where* it's caught.

Languages without exceptions (C, Rust without `?`) simply don't emit
`Unwind`/`Resume`.
