# IR Instructions

Every instruction is a three-address form: it reads from source vregs and
writes to a destination vreg. Exceptions are memory-side-effect instructions
(`WriteStr`, `StoreField`, `Retain`, etc.) and terminators.

## Constants and Moves

### `Const { dst, imm }`
- **Operands:** `dst: VReg`, `imm: i64`
- **Semantics:** `dst = imm`
- **Languages:** All
- **Backends:** All must implement

### `FConst { dst, bits }`
- **Operands:** `dst: VReg`, `bits: u64` (IEEE 754 double bit pattern)
- **Semantics:** `dst = f64::from_bits(bits)`
- **Languages:** Any language with float literals
- **Backends:** All must implement (may trap if no FPU)

### `Mov { dst, a }`
- **Operands:** `dst: VReg`, `a: VReg`
- **Semantics:** `dst = a`
- **Languages:** All
- **Backends:** All must implement (may be a no-op if same home)

### `Raw { bytes }`
- **Operands:** `bytes: Vec<u8>` — raw machine code
- **Semantics:** Inline raw bytes at this point in the function
- **Languages:** Rarely emitted by frontends
- **Backends:** All must implement (append bytes verbatim)

## Integer Arithmetic

### `Bin { dst, op, a, b }`
- **Operands:** `dst: VReg`, `op: BinOp`, `a: VReg`, `b: VReg`
- **Semantics:** `dst = a op b` (i64)
- **BinOps:**
  - `Add`, `Sub`, `Mul` — standard arithmetic
  - `Div` — signed integer division (traps on divide-by-zero)
  - `Rem` — signed integer remainder
  - `BitAnd`, `BitOr`, `BitXor` — bitwise
  - `Shl`, `Shr` — shifts (shamt masked to 0..=63)
  - `And`, `Or` — short-circuit logical (result is 0 or 1)
- **Languages:** All
- **Backends:** All must implement

### `Neg { dst, a }`
- **Operands:** `dst: VReg`, `a: VReg`
- **Semantics:** `dst = -a` (i64)
- **Languages:** All
- **Backends:** All must implement

## Float Arithmetic

### `FBin { dst, op, a, b }`
- **Operands:** `dst: VReg`, `op: FBinOp`, `a: VReg`, `b: VReg`
- **Semantics:** `dst = f64(a) op f64(b)`
- **FBinOps:** `Add`, `Sub`, `Mul`, `Div`
- **Languages:** Any language with float arithmetic
- **Backends:** All must implement (may trap if no FPU)

### `FNeg { dst, a }`
- **Operands:** `dst: VReg`, `a: VReg`
- **Semantics:** `dst = -f64(a)`
- **Languages:** Any language with float arithmetic
- **Backends:** All must implement

## Comparisons

### `Cmp { dst, op, a, b }`
- **Operands:** `dst: VReg`, `op: CmpOp`, `a: VReg`, `b: VReg`
- **Semantics:** `dst = i64(a) op i64(b) ? 1 : 0`
- **CmpOps:** `Eq`, `Ne`, `Lt`, `Gt`, `Le`, `Ge`
- **Languages:** All
- **Backends:** All must implement

### `FCmp { dst, op, a, b }`
- **Operands:** `dst: VReg`, `op: FCmpOp`, `a: VReg`, `b: VReg`
- **Semantics:** `dst = f64(a) op f64(b) ? 1 : 0`
- **FCmpOps:** `Eq`, `Ne`, `Lt`, `Gt`, `Le`, `Ge`
- **Languages:** Any language with float comparison
- **Backends:** All must implement

## Calls

### `CallFn { dst, fidx, args }`
- **Operands:** `dst: VReg`, `fidx: u32` (function index in `Program::functions`), `args: Vec<VReg>`
- **Semantics:** `dst = functions[fidx](args...)`
- **Languages:** All
- **Backends:** All must implement

### `CallVirt { dst, slot, recv, args }`
- **Operands:** `dst: VReg`, `slot: u32` (vtable slot index), `recv: VReg`, `args: Vec<VReg>`
- **Semantics:** `dst = vtable[recv][slot](recv, args...)`
- **Languages:** OOP languages with virtual dispatch (RubyC, C#, Java)
- **Backends:** All must implement (may trap if `recv` is null)

### `CallByName { dst, recv, name, args }`
- **Operands:** `dst: VReg`, `recv: VReg`, `name: u32` (index into `Program::names`), `args: Vec<VReg>`
- **Semantics:** `dst = recv.name(args...)` — dynamic dispatch by name
- **Languages:** Duck-typing languages (Python, Ruby), reflection-based dispatch
- **Backends:** All must implement (may fall back to a runtime lookup table)

### `CallImport { dst, import, args }`
- **Operands:** `dst: VReg`, `import: u32` (index into `Program::imports`), `args: Vec<VReg>`
- **Semantics:** `dst = import(import[idx], args...)` — call an external (C) function
- **Languages:** All (for FFI)
- **Backends:** All must implement

## Allocation and Memory

### `Alloc { dst, bytes }`
- **Operands:** `dst: VReg`, `bytes: u64`
- **Semantics:** `dst = heap_alloc(bytes)` — allocate a heap object
- **Languages:** OOP languages, any language with heap objects
- **Backends:** All must implement

### `StackAlloc { dst, bytes }`
- **Operands:** `dst: VReg`, `bytes: u64`
- **Semantics:** `dst = stack_alloc(bytes)` — allocate on the stack frame
- **Languages:** Rust (stack-allocated structs), C (VLA)
- **Backends:** All must implement

### `VTableAddr { dst, class }`
- **Operands:** `dst: VReg`, `class: u32` (index into `Program::vtables`)
- **Semantics:** `dst = &vtables[class]` — load the vtable pointer for a class
- **Languages:** OOP languages with vtables
- **Backends:** All must implement

## Object Fields

### `LoadField { dst, base, off }`
- **Operands:** `dst: VReg`, `base: VReg` (object pointer), `off: u32` (byte offset)
- **Semantics:** `dst = *reinterpret_cast<i64*>(base + off)`
- **Languages:** OOP languages, any language with structs
- **Backends:** All must implement

### `StoreField { base, off, src }`
- **Operands:** `base: VReg` (object pointer), `off: u32` (byte offset), `src: VReg`
- **Semantics:** `*reinterpret_cast<i64*>(base + off) = src`
- **Languages:** OOP languages, any language with structs
- **Backends:** All must implement

## I/O

### `WriteStr { off, len }`
- **Operands:** `off: u32` (offset into `Program::strings`), `len: u32`
- **Semantics:** `write(stdout, strings[off..off+len])`
- **Languages:** All
- **Backends:** All must implement

### `EWriteStr { off, len }`
- **Operands:** `off: u32`, `len: u32`
- **Semantics:** `write(stderr, strings[off..off+len])`
- **Languages:** All
- **Backends:** All must implement

### `WriteInt { src }`
- **Operands:** `src: VReg`
- **Semantics:** `write(stdout, itoa(i64(src)))`
- **Languages:** All
- **Backends:** All must implement

### `EWriteInt { src }`
- **Operands:** `src: VReg`
- **Semantics:** `write(stderr, itoa(i64(src)))`
- **Languages:** All
- **Backends:** All must implement

### `ScanInt { dst }`
- **Operands:** `dst: VReg`
- **Semantics:** `dst = read_int(stdin)`
- **Languages:** All
- **Backends:** All must implement

## Reference Counting (Optional)

### `Retain { src }`
- **Operands:** `src: VReg` (object pointer)
- **Semantics:** `refcount(src) += 1` (atomic)
- **Languages:** Refcounted GC languages (RubyC, C#)
- **Backends:** Optional (no-op if the language has no refcounting)

### `Release { src, bytes }`
- **Operands:** `src: VReg` (object pointer), `bytes: u64` (allocation size)
- **Semantics:** `refcount(src) -= 1; if (refcount == 0) free(src, bytes)`
- **Languages:** Refcounted GC languages
- **Backends:** Optional

### `Drop { src }`
- **Operands:** `src: VReg`
- **Semantics:** Call the destructor for `src`. Distinct from `Release`
  (refcount decrement). Used by Rust at scope exit.
- **Languages:** Rust, C++
- **Backends:** Optional

## Exceptions (Optional)

### `Unwind { src }`
- **Operands:** `src: VReg` (exception value / error object)
- **Semantics:** Raise an exception / panic with value `src`.
- **Languages:** Rust (panic), Python (raise), C# (throw), C++ (throw)
- **Backends:** Optional

### `Resume { dst, args }`
- **Operands:** `dst: VReg`, `args: Vec<VReg>`
- **Semantics:** Resume execution after a catch. `dst` receives the
  exception value; `args` are the catch parameters.
- **Languages:** Python (except), Rust (catch), C# (catch)
- **Backends:** Optional

## Terminators

Terminators end a basic block. Every block has exactly one.

### `Ret { src }`
- **Operands:** `src: Option<VReg>` (return value)
- **Semantics:** Return from the function. If `src` is `Some(v)`, return `v`;
  otherwise return nothing (void).

### `Exit`
- **Semantics:** Exit the process with code 0.

### `ExitWith { code }`
- **Operands:** `code: VReg`
- **Semantics:** Exit the process with the code in `code`.

### `Branch { cond, if_true, if_false }`
- **Operands:** `cond: Option<VReg>`, `if_true: u32`, `if_false: u32`
- **Semantics:** If `cond` is `None`, branch to `if_true` unconditionally
  (fall-through). If `cond` is `Some(v)`, branch to `if_true` if `v != 0`,
  else `if_false`.
