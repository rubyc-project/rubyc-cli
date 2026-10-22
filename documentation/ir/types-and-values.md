# Types and Values

## The 64-Bit Value Model

Every value in the IR is a 64-bit word stored in a virtual register (vreg).
There is no distinction between "integer vreg" and "float vreg" — they
share the same register space.

### Integers (i64)

Signed 64-bit two's-complement integers. The full range is
`i64::MIN ..= i64::MAX`. All integer arithmetic wraps silently (no
overflow traps except for `Div` by zero, which traps).

### Floats (f64)

IEEE 754 double-precision floats. The bit pattern is stored directly in
the vreg (no tagging). `FConst` stores the `u64` bit pattern; the backend
reinterprets it as an `f64` when emitting an FPU instruction.

**Tagging:** The IR is currently *untagged* — a vreg holds either an i64
or an f64, and the frontend is responsible for never mixing them in a way
that would produce a wrong result. A future version may add a type tag to
enable mixed arithmetic and better error messages.

### Objects

An object is a pointer to a heap block. The block layout is:

```
┌─────────────────────────┐
│ vtable ptr (8 bytes)    │  ← only for classes with vtables
│ refcount (8 bytes)      │  ← only for refcounted GC
│ runtime type (8 bytes)  │  ← only for classes that carry one
├─────────────────────────┤
│ field 0 (8 bytes)       │  ← byte offset 24 (or less if no header)
│ field 1 (8 bytes)       │
│ ...                     │
└─────────────────────────┘
```

The exact header size depends on the language. A language without vtables
or refcounting has a header size of 0.

RubyC's class instances carry all three: the runtime type is the
descriptor of the instance's instantiation, and it is what carries a
generic class's type arguments (see
[object-model.md](object-model.md)).

### Strings

A string is a pointer into the `Program::strings` blob. The `WriteStr`
instruction takes an offset and length into this blob. The string is
NUL-terminated within the blob.

### Function Values

A function value is a pointer to code. `CallFn` uses a function index
(compile-time); `CallImport` uses an import index; `CallByName` uses a
name (runtime lookup).

### Booleans

There is no distinct boolean type. A boolean is an i64: 0 = false,
non-zero = true. `Cmp` and `FCmp` produce 0 or 1.

### Null / Undefined

There is no distinct null or undefined value. A null pointer is 0. A
language that needs null checks must emit an explicit comparison.
