# Calling Convention

## IR is ABI-Agnostic

The IR describes *what* to call, not *how* to call it. The mapping from
vregs to physical registers, the order of arguments, and the return value
register are all backend decisions.

## Call Instructions

### `CallFn { dst, fidx, args }`

Call a known function by index. The frontend knows the function's
signature (arity, receiver kind) at compile time.

```
// f(a, b) where f takes 2 params, no receiver
CallFn { dst: v10, fidx: 3, args: [v1, v2] }
```

### `CallVirt { dst, slot, recv, args }`

Call a virtual method by vtable slot. The receiver is passed explicitly
and is also the `this` for the callee.

```
// obj.method(a) where method is at vtable slot 2
CallVirt { dst: v10, slot: 2, recv: v0, args: [v1] }
```

The callee sees `recv` as its receiver (vreg 0 if `ReceiverKind::This`).

### `CallByName { dst, recv, name, args }`

Call a method by name (dynamic dispatch). `name` indexes
`Program::names`. The backend resolves the name at runtime (e.g., via a
lookup table).

```
// obj.method(a) where "method" is names[5]
CallByName { dst: v10, recv: v0, name: 5, args: [v1] }
```

### `CallImport { dst, import, args }`

Call an external (C) function. `import` indexes `Program::imports`,
which is a list of `(name, library_hint)` pairs.

```
// printf("hello") where printf is imports[0]
CallImport { dst: v10, import: 0, args: [v1] }
```

## ReceiverKind

`IrFunction::receiver` determines how the function receives its object:

| Kind | Meaning | Vreg 0 | Example |
|---|---|---|---|
| `None` | Free function / static | First param | `add(a, b)` |
| `This` | Implicit receiver | `this` pointer | `obj.method()` in C#/RubyC |
| `Explicit` | Receiver is a regular param | First param | `fn(self, a)` in Rust |

### `ReceiverKind::None`

The function has no receiver. All params are regular arguments.

```
// void add(int a, int b)
IrFunction { receiver: None, arity: 2, ... }
// vreg 0 = a, vreg 1 = b
```

### `ReceiverKind::This`

The function has an implicit receiver. Vreg 0 is the `this` pointer.
Params start at vreg 1.

```
// void method(int a, int b)
IrFunction { receiver: This, arity: 2, ... }
// vreg 0 = this, vreg 1 = a, vreg 2 = b
```

### `ReceiverKind::Explicit`

The receiver is a regular parameter, not a special vreg. Used by Rust
where `self` is just the first parameter.

```
// fn method(self, int a)
IrFunction { receiver: Explicit, arity: 2, ... }
// vreg 0 = self, vreg 1 = a
// (same layout as This, but the frontend treats self as a value, not a reference)
```

The difference between `This` and `Explicit` is semantic: `This` implies
the receiver is a reference/pointer to an object, while `Explicit` means
the receiver could be a value (moved into the function). The backend may
treat them identically.

## x86_64 Linux Convention (Example)

The x86_64 backend uses the System V AMD64 ABI:

| Role | Register |
|---|---|
| Receiver (`this`) | `rdi` |
| Param 0 | `rsi` (if no receiver) or `rdx` (if receiver) |
| Param 1 | `rdx` (if no receiver) or `rcx` (if receiver) |
| Param 2 | `rcx` (if no receiver) or `r8` (if receiver) |
| Param 3+ | Stack |
| Return value | `rax` |
| Return address | Stack (implicit) |

The backend maps vregs to registers via linear-scan register allocation.
The prologue materializes incoming arguments from the ABI registers into
their allocated homes.

## Defining a New Convention

A new backend defines its own convention:

1. **Register allocation:** Decide which physical registers are
   available and how to map vregs to them.
2. **Argument passing:** Decide which registers hold arguments and in
   what order.
3. **Return value:** Decide which register holds the return value.
4. **Callee-saved vs caller-saved:** Decide which registers the callee
   must preserve.
5. **Stack frame:** Decide how to handle values that don't fit in
   registers (stack slots).

The IR doesn't prescribe any of this. The backend is free to use any
convention it wants, as long as it's consistent within the target.
