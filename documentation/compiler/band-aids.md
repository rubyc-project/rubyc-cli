# Compiler band-aid ledger

Live inventory of every place the compiler branches on a **library type or
member name** while lowering. The compiler is supposed to know only its own
language; these are the places where it also knows things about the .NET
libraries it compiles from source.

- Machine-checked by `rubyc-core/tests/hygiene/band_aid_guard.rs`. It no
  longer claims to catch *every* new key — a broad "any uppercase string in
  a comparison" scan is useless, because the compiler's own vocabulary
  (`Length`, `Add`, `Compare`) is also the libraries'. What it does check,
  precisely, is that the folds this campaign **removed** have not come back,
  and that every one of them is still recorded below. Add a row here when
  you remove one, or the test fails.
- **Target: zero.** Every row ends in one of three dispositions:
  `INTRINSIC` (kept, documented), `RELOCATE` (move out of shared lowering),
  `DELETE` (remove once the general path replaces it).

The original count ("322 raw sites, 149 distinct names", 2026-10-17) was taken
before the 1B/1C cleanup. Re-counted against the current sources: **6 name-keyed folds
remain** — `Memmove`/`Buffer`, the `NativeMemory` allocator family,
`SafeFileHandle`, `NarrowUtf16ToAscii` (a provably equivalent fast path),
and the two language-level array shape queries — plus the intrinsic and
language surfaces listed in the tables below.

**Progress.** The 1C/1B cleanup removed the folds listed under
"removed by the campaign" below, and a later pass removed the 45 debug gates
whose *conditions* named library types (`RUBYC_CHAINDBG` keyed on
`Latin1Encoding`, `RUBYC_FIELDDBG` on `CACHE_LINE_SIZE`/`Invariant`,
`RUBYC_GBSLOT` on the encoding types, and so on) — diagnostic-only code, but
name-keyed code in the lowering layer all the same. The remaining rows are the
ones decision (C) keeps for now: folds that cannot be deleted until a general
address-of / interior-pointer lowering exists.

## Removed by the campaign (guarded — these must not return)

| Fold | What it fabricated |
|---|---|
| `ArgumentOutOfRangeException.ThrowIf*` family | bypassed the throwing validator bodies |
| `IsSupported` / `IsHardwareAccelerated` | blanket true for hardware/feature queries |
| `GetMethodTable` | read an object header word this runtime does not have |
| `IsBitwiseEquatable` | closed-primitive bitwise-equality check |
| `GenericCache.CreateCacheTable` | answered 0, so the cache could never fill |
| `Thread.FastPollGC` | answered 0 |
| `Interlocked.MemoryBarrier` | answered 0 |
| `RuntimeHelpers.AllocateTypeAssociatedMemory` | intercepted allocation, ignored alignment |
| `GlobalizationMode.Invariant` | answered the host's ICU availability |
| `ExceptionFallback` / `SetDefaultFallbacks` | neutralized encoding-fallback setup |
| `IsReferenceOrContainsReferences` | answered a GC layout query with a constant |
| `SearchValues.Create` | answered with a searcher that matches nothing (result-changing) |
| `typeof(T).<member>` fast path | answered Type members from the spelling instead of the descriptor |
| `obj.GetType()` static-type fold | answered the STATIC type where the runtime type is known |

## Why the cross-platform IR matters here

`native/ir/` is the portable layer — the JIT and the bytecode path both consume
it, and it is shared by every target backend. A fold that encodes *this host's*
configuration (no ETW installed, invariant globalization, single-threaded) does
not belong there: it bakes a deployment decision into portable IR that every
other target inherits. That is the `RELOCATE` class.

A fold that encodes *language* semantics (`Unsafe.As` is the identity in this
ref model, because references are erased) is fine anywhere. That is the
`INTRINSIC` class.

## INTRINSIC — documented JIT intrinsics (keep)

Modeled on CoreCLR's `src/coreclr/jit/namedintrinsiclist.h`, which keeps all of
its library knowledge in one enum. Each needs a spec citation and a test that
proves it generalizes across the whole type family, not one call site.

| Name | Sites | Semantics | Disposition |
|---|---|---|---|
| `Unsafe` (`As`, `AsRef`, `AsPointer`, `BitCast`) | eval.rs:10379, 10476, 10803 | identity in the erased-ref model | INTRINSIC |
| `Unsafe` (`Add`, `AddByteOffset`) | eval.rs:6103, 10477 | address arithmetic | INTRINSIC |
| `Unsafe` (`ReadUnaligned`/`Read`) | eval.rs:10541, 10551 | load primitive | INTRINSIC |
| `BitOperations` (`Log2`) | eval.rs:9792, 10246, 10329 | exact halving loop | INTRINSIC |
| `MemoryMarshal` (`Cast`) | eval.rs:10878 | reinterpret + width rescale | INTRINSIC |
| `MemoryMarshal` (`GetReference`, `GetArrayDataReference`, `GetNonNullPinnableReference`) | eval.rs:10350, 10375, 10388 | address of element 0 / data block | INTRINSIC |
| `Volatile` (`Read`, `Write`) | eval.rs:10844, 10850 | erased refs make these the plain value | INTRINSIC |
| `Interlocked` (`CompareExchange`, `Exchange`, `ExchangeAdd`, `MemoryBarrier`) | eval.rs:10745, 10869; calls.rs:295 | single-threaded compare-and-store | INTRINSIC |
| `IsNullRef` | eval.rs:10716 | null test | INTRINSIC |
| `NarrowUtf16ToAscii` | eval.rs:10335, 10345 | lossy scalar lowering | INTRINSIC |
| `Span` / `ReadOnlySpan` shape gates (`Slice`, `Fill`, `AsSpan`, `Length`, `HasValue`/`Value`) | eval.rs:9327, 9662, 9880 | element-width-closed fast paths | INTRINSIC |
| `Object` | select.rs:1535 | language-level universal base | INTRINSIC (language, not library) |
| `System.Type` member surface (`Name`, `FullName`, `Namespace`, `Is*`, `TypeHandle`) | typeinfo.rs; eval.rs | the compiler implements the `Type` facade over native descriptors | INTRINSIC (language) |
| `System.InvalidCastException` | body.rs | the type C# REQUIRES a failed unboxing cast to throw; constructed through the ordinary `new` path | INTRINSIC (language, like `OverflowException`) |
| `GetType` | eval.rs | answered from the receiver's RUNTIME type (vtable row, box descriptor, or array element type) | INTRINSIC (language) |
| `scan_int`, `printf`, `print`, `put_str`, `eprintf` | types.rs | compiler builtins with known result types | INTRINSIC (compiler) |
| framework primitive spellings (`Int32`, `Boolean`, ...) | typeinfo.rs | the language's primitive type names, used to give one descriptor per primitive | INTRINSIC (language) |
| `System.OverflowException` | body.rs (`throw_overflow_if`) | the type C# REQUIRES `checked` to throw; constructed through the ordinary `new` path, so a program that declares it gets its real type/row | INTRINSIC (language, like `Object`) |

Note on vector `IsHardwareAccelerated`: CoreCLR models this as a *support query*
resolved per target (`NI_IsSupported` / `NI_IsHardwareAccelerated` in its
intrinsic table). The fold that answered it with a constant is gone (see
"Removed by the campaign" above): the property's getter reads the property
itself, so only the JIT's intrinsic can answer it, and the compiler reports
the gap instead of inventing a capability answer. A target-capability layer
would own the answer when it lands.

## Remaining name-keyed folds — verified present

The tables that used to sit here listed **49 names**. Re-checking each one
against the current sources found **41 of them gone**. Keeping stale rows is
how a ledger stops being read, so the removed ones now live in the
"Removed by the campaign" table above, each with what it fabricated.

What genuinely remains is this. Each row's disposition is decided by one test:
**delete the fold and compile the library's own body — what happens?**

| Name | Sites | Disposition | Why |
|---|---|---|---|
| `Memmove`, `Buffer` | eval.rs:10596, 10694 | INTRINSIC | `Buffer.Memmove`'s body calls `Memmove` on `ref byte`; the general form is a memory copy the IR already has (`MemCpy`), but the reachable path needs interior addresses of arbitrary refs. Deleting it now regresses working copies; the general lowering needs interior addresses of arbitrary refs. |
| `NativeMemory.AlignedAlloc` / `AlignedRealloc` / `Alloc` / `Realloc` | eval.rs:10764, 10797 | INTRINSIC | These ARE the native allocator. Their bodies bottom out in `Interop.Sys.*` P/Invoke, which needs the interop plumbing to land; the fold is the allocator, not a fake answer. |
| `SafeFileHandle` | eval.rs:3149, 3804 | INTRINSIC (marshal boundary) | Handle marshalling at a P/Invoke boundary: the value crossing the ABI is the raw handle, which is what `SafeHandle` is by the time it reaches native code. Same class as any other marshalling rule. |
| `NarrowUtf16ToAscii` | eval.rs:10335, 10345 | INTRINSIC (ASCII fast path) | The body is a vectorised ASCII narrowing loop over spans. The fold is a semantic shortcut for a case the general loop also handles, kept for speed; generality is provable but the replacement is a performance task, not a correctness one. |
| `Length`, `Rank` | types.rs:495 | LANGUAGE, not library | Array shape queries on the built-in array type — part of the language, listed for completeness rather than counted against the total. |

### Why the four "folds" that look like band-aids are intrinsics

`Unsafe.*`, `Interlocked.*`, `Volatile.*` and
`MemoryMarshal.GetArrayDataReference` were listed here as DELETE with the note
"needs address-of / interior-pointer lowering first". That note was half right
and half wrong, and the correction matters:

- **Half right:** the address-of lowering WAS broken, and is now fixed. `&local`
  used to hand out the local's VALUE, so `int* p = &x; return *p;` dereferenced
  the integer 41 and segfaulted; writes through a pointer were dropped
  entirely. Both directions of aliasing now work (measured: 8, 8, 10).
- **Half wrong:** fixing that does NOT make these deletable, because they are
  `[Intrinsic]` methods. The library's own bodies prove it:
  - `Unsafe.As<TFrom,TTo>` — `throw new PlatformNotSupportedException();`
    with the real semantics left as IL in a comment (`ldarg.0; ret`).
  - `Interlocked.Exchange(ref byte, byte)` — recurses into itself under
    `// Must expand intrinsic`.
  - `Volatile.Write(ref int, int)` — `[Intrinsic]`, routed through
    `Unsafe.As<int, VolatileInt32>`.
  - `MemoryMarshal.GetArrayDataReference(Array)` — needs
    `RuntimeHelpers.GetMethodTable(array)->BaseSize`, i.e. a method table
    this runtime deliberately does not have (the `GetMethodTable` fold that
    answered it was removed).

  So compiling the body instead of folding gives `PlatformNotSupportedException`
  or infinite recursion. An `[Intrinsic]` method is one whose implementation
  MUST come from the compiler — that is what the attribute means. Replacing it
  is the compiler doing its job, not knowing the libraries.

This is the distinction the ledger exists to draw, so it is worth stating
sharply: a **band-aid** invents behaviour for a type the compiler has no
business knowing; an **intrinsic** supplies behaviour the source cannot express
at all.

## Remaining approximations (documented, not folds)

These answer a question the compiler cannot yet answer exactly. They are
listed so the record is honest about what is still approximate; each one
is a real gap, not a shortcut around a correct implementation.

| Area | Where | What is approximate | What would make it exact |
|---|---|---|---|
| Reflection members | eval.rs | `GetMethods`, `GetConstructors`, ... answer null/empty | a real reflection surface over the type descriptors |
| `Array`-typed `GetType()` | eval.rs | answers `System.Array`; the element type is not in the static type | storing the element descriptor in the array header |
| Box virtual dispatch | typeinfo.rs | a boxed primitive without a wrapper class has a sentinel first word, so `ToString`/`Equals` on it have no target | synthesized wrapper rows per primitive |
| `RuntimeTypeHandle` | eval.rs | the handle is the descriptor pointer, but the library's struct accessors are not wired to it | mapping the handle struct onto the descriptor |
| Enum and struct boxing | body.rs | only primitives box at object boundaries today | the same boundary rule with the enum/struct descriptor |

## Not band-aids

- `base`, `void` — local syntax markers, not library names.
- The library-keyed debug gates that used to live here (`Foo`,
  `CompletionSentinel` in `select.rs`, and the 45 gates listed under
  "Progress" above) are gone as of the debug-gate pass.

## Verification

```bash
cargo test -p rubyc-core --lib hygiene_band_aid
```

The guard checks two things, both precisely:

1. **No removal has been undone.** Each fold in the "Removed by the campaign"
   table is matched against comparisons and `matches!` arms in the lowering
   sources; a hit fails with `file:line` and the fold's name.
2. **The record is complete.** Every removed fold must still appear in this
   file, so the ledger cannot quietly drop a row.

It deliberately does *not* try to catch every new library key: a scan loose
enough to catch `IsSupported` also catches `Length` and `Add`, and a test that
cries wolf gets ignored. The narrow check is the one that holds.
