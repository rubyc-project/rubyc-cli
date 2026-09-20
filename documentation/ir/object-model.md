# Object Model

## Heap Layout

The IR assumes a bump allocator + freelist for the object heap. The
`Program::heap_size` field tells the backend how many bytes of heap are
required at runtime.

```
Heap:
┌──────────────────────────────────────────────────┐
│ Object 0                                         │
│   [vtable ptr] [refcount] [field0] [field1] ... │
├──────────────────────────────────────────────────┤
│ Object 1                                         │
│   [vtable ptr] [refcount] [field0] [field1] ... │
├──────────────────────────────────────────────────┤
│ ...                                              │
└──────────────────────────────────────────────────┘
```

The heap is a contiguous region. `Alloc` bumps the allocation pointer;
`Release` (when refcount hits 0) pushes the block onto a freelist.

## Object Header

The header size depends on the language:

| Language | Vtable | Refcount | Header Size |
|---|---|---|---|
| RubyC | Yes | Yes | 16 bytes |
| C# (typical) | Yes | No | 8 bytes |
| Python | Yes | Yes | 16+ bytes |
| Rust | No | No | 0 bytes |
| C struct | No | No | 0 bytes |

The frontend sets the header size implicitly by the byte offsets it emits
in `LoadField`/`StoreField`. The backend doesn't need to know the header
size — it just reads/writes at the given byte offset.

## Fields

Fields are accessed by byte offset from the object pointer. The frontend
computes the byte offset: `header_size + field_index * 8` (for 8-byte
fields). The `LoadField`/`StoreField` instructions take the absolute byte
offset.

```
// For a class with vtable + refcount (16-byte header):
//   field 0 is at offset 16
//   field 1 is at offset 24
//   field 2 is at offset 32

LoadField { dst: v4, base: v0, off: 16 }  // v4 = v0.field0
StoreField { base: v0, off: 24, src: v5 }  // v0.field1 = v5
```

## Reference Counting (Optional)

Languages with refcounted GC emit `Retain`/`Release` instructions:

```
Retain { src: v0 }       // refcount(v0) += 1
Release { src: v0, bytes: 48 }  // refcount(v0) -= 1; free if zero
```

Languages without refcounting (Rust, C, Python with tracing GC) simply
don't emit these instructions. The backend can optimize them away.

## Drop (Optional)

Languages with explicit destructors (Rust, C++) emit `Drop`:

```
Drop { src: v0 }  // call v0's destructor
```

This is distinct from `Release`. In Rust, `Drop` is called when a value
goes out of scope, even if it's a stack-allocated struct. In RubyC,
`Release` handles the refcount decrement and potential deallocation.

## Alternative Memory Models

A new language can use a different memory model by:

1. **Stack-only:** Use `StackAlloc` for all allocations. No `Alloc`, no
   `Retain`/`Release`, no `Drop` (or `Drop` for cleanup).
2. **Tracing GC (Python, Java):** Use `Alloc` for heap objects, but no
   `Retain`/`Release`. The GC is handled by the runtime, not the IR.
3. **Manual memory (C):** Use `Alloc` + explicit `CallImport` for
   `free()`. No `Retain`/`Release`/`Drop`.
4. **Region-based (Swift, Rust NLL):** Use `Alloc` with region tracking
   done by the frontend. No `Retain`/`Release`.

The IR doesn't mandate any specific memory model. It provides the
*instructions* (`Alloc`, `StackAlloc`, `Retain`, `Release`, `Drop`) and
the language frontend decides which to emit.
