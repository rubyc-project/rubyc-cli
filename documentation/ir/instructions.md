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

### `F2I { dst, src }`
- **Operands:** `dst: VReg`, `src: VReg` (f64 bits)
- **Semantics:** `dst = (i64)src`, truncating toward zero (`cvttsd2si`).
  This is C#'s unchecked conversion from `float`/`double` to an integral
  type; an out-of-range value yields `i64::MIN`.
- **Languages:** Any language with float arithmetic
- **Backends:** All must implement

### `I2F { dst, src }`
- **Operands:** `dst: VReg`, `src: VReg`
- **Semantics:** `dst = (f64)src` (`cvtsi2sd`) — C#'s conversion from an
  integral type to `float`/`double`, exact for `|src| < 2^53`
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

### `CallStatic { dst, fidx, args }`
- **Operands:** `dst: VReg`, `fidx: u32` (function index in `Program::functions`), `args: Vec<VReg>`
- **Semantics:** `dst = functions[fidx](args...)` — a direct call to a static
  method, which has no receiver. The backend passes 0 in the receiver
  register.
- **Languages:** All
- **Backends:** All must implement

### `CallFnAddr { dst, target, args }`
- **Operands:** `dst: VReg`, `target: VReg` (the address of the callee), `args: Vec<VReg>`
- **Semantics:** `dst = target(args...)` — call through a function pointer
  held in a register. A delegate is a value, so the callee's address is
  only known at run time; this is the call that makes a delegate object
  callable.
- **Languages:** Languages with function values
- **Backends:** All must implement

### `FnAddr { dst, fidx }`
- **Operands:** `dst: VReg`, `fidx: u32` (function index in `Program::functions`)
- **Semantics:** `dst = &functions[fidx]` — the address of a function, for
  a function pointer or a delegate
- **Languages:** Languages with function values
- **Backends:** All must implement

## Allocation and Memory

### `Alloc { dst, bytes }`
- **Operands:** `dst: VReg`, `bytes: u64`
- **Semantics:** `dst = heap_alloc(bytes)` — allocate a heap object
- **Languages:** OOP languages, any language with heap objects
- **Backends:** All must implement

### `AllocDyn { dst, bytes }`
- **Operands:** `dst: VReg`, `bytes: VReg` (the size is a runtime value)
- **Semantics:** `dst = heap_alloc(*bytes)` — allocate a heap object whose
  size is only known at run time (a concatenated string, a struct with a
  computed length)
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

### `LoadStaticAddr { dst, idx }`
- **Operands:** `dst: VReg`, `idx: u32` (index into `Program::statics`)
- **Semantics:** `dst = &statics[idx]` — the address of a static slot
- **Languages:** OOP languages with static storage
- **Backends:** All must implement

### `LoadStrAddr { dst, off, len }`
- **Operands:** `dst: VReg`, `off: u32` (offset into `Program::strings`), `len: u32`
- **Semantics:** `dst = &strings[off]` — the address of a string literal
- **Languages:** Any language with string literals
- **Backends:** All must implement

### `LoadImport { dst, import }`
- **Operands:** `dst: VReg`, `import: u32` (index into `Program::imports`)
- **Semantics:** `dst = &imports[import]` — the address of an imported
  symbol's slot
- **Languages:** Languages with FFI
- **Backends:** All must implement

### `MemCpy { dst, src, len }`
- **Operands:** `dst: VReg` (destination), `src: VReg` (source), `len: VReg`
  (byte count)
- **Semantics:** copy `len` bytes from `src` to `dst`
- **Languages:** Any language with a copy operation
- **Backends:** All must implement

### `MemCmp { dst, a, b, len }`
- **Operands:** `dst: VReg`, `a: VReg`, `b: VReg`, `len: VReg`
- **Semantics:** `dst = memcmp(a, b, len)` — 0 when the ranges are equal
- **Languages:** Any language that compares memory ranges
- **Backends:** All must implement

### `LoadByte { dst, base, off }`
- **Operands:** `dst: VReg`, `base: VReg`, `off: u32` (byte offset)
- **Semantics:** `dst = *(u8*)(base + off)` — a single byte, zero-extended
- **Languages:** Any language with byte-addressed data
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

### `WriteString { src }`
- **Operands:** `src: VReg` (a first-class string: length at +0, refcount
  at +8, data at +16)
- **Semantics:** `write(stdout, [src+16..])` — print a string value
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

## Coroutines and Tasks

### `TaskAlloc { dst, bytes }`
- **Operands:** `dst: VReg`, `bytes: u64`
- **Semantics:** `dst = task_alloc(bytes)` — allocate a coroutine frame on
  the heap. The frame layout belongs to the runtime, not the compiler:
  `[0]` state, `[1]` return value, `[2]` saved instruction pointer,
  `[3..]` saved locals.
- **Languages:** Languages with coroutines or async functions
- **Backends:** All must implement

### `Suspend { state }`
- **Operands:** `state: VReg` (the state-machine state to resume at)
- **Semantics:** Save the current state and locals into the task frame and
  return to the scheduler. A resumed task continues at the next state
  block.
- **Languages:** Languages with coroutines or async functions
- **Backends:** All must implement

### `CoroutineResume { task, dst }`
- **Operands:** `task: VReg` (the task frame), `dst: VReg`
- **Semantics:** Resume `task`; `dst` receives its result once the
  coroutine completes.
- **Languages:** Languages with coroutines or async functions
- **Backends:** All must implement

### `HandlerEnter { dst, handler }`
- **Operands:** `dst: VReg`, `handler: u32` (block index)
- **Semantics:** Push a catch handler onto the runtime's handler stack and
  jump to `handler` when an exception unwinds past it. The runtime's
  `setjmp` lands here on the first entry (`dst` = 0) and again after a
  `longjmp` (`dst` = 1).
- **Languages:** Languages with exceptions
- **Backends:** Optional

### `HandlerPop`
- **Semantics:** Remove the innermost active handler — the normal
  completion of a `try`.
- **Languages:** Languages with exceptions
- **Backends:** Optional

### `LoadExc { dst }`
- **Operands:** `dst: VReg`
- **Semantics:** `dst` = the in-flight exception payload.
- **Languages:** Languages with exceptions
- **Backends:** Optional

### `BuildArgv { dst }`
- **Operands:** `dst: VReg`
- **Semantics:** `dst` = a string array built from the process argv. A host
  without an argv slot (the JIT) supplies its own; a zero slot yields an
  empty array rather than a fault.
- **Languages:** Languages with a process entry point
- **Backends:** All must implement

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

### `Rethrow`
- **Semantics:** Re-throw the in-flight exception to an enclosing handler.
  Never returns.

### `Branch { cond, if_true, if_false }`
- **Operands:** `cond: Option<VReg>`, `if_true: u32`, `if_false: u32`
- **Semantics:** If `cond` is `None`, branch to `if_true` unconditionally
  (fall-through). If `cond` is `Some(v)`, branch to `if_true` if `v != 0`,
  else `if_false`.
