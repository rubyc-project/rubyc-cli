# Macros — `#macro` (compile inline, zero overhead)

RubyC macros expand **at compile time, inline at the call site**: no call
instruction, no stack frame, zero runtime overhead. They are AST-level
(hygienic) — not C-style text pasting — so errors inside an expansion
point at *your* call site with full carets.

## Defining

```rubyc
#macro sq(x) => (x)*(x)
#macro maxab(a,b) => a > b ? a : b
```

Single-line expression macros only (v1). The body is parsed once and
stored on the unit; every occurrence of `name(args…)` in expression
position is replaced by the body with parameters substituted.

## Using

```rubyc
printf("{0} {1}", sq(6), maxab(3, 9));   // → 36 9
int y = sq(sq(3));                        // nesting works: 81
```

Expansion happens after parsing and before the unknown-name check, so a
typo inside a macro body is reported at the macro definition, while bad
*arguments* are reported at the call site.

## Rules & limits

| Rule | Detail |
|---|---|
| Scope | file-local (until multi-file projects land, A12) |
| Form | single-line `=> expression` |
| Recursion | allowed up to depth 32, then compile error |
| Arity mismatch | compile error |
| Redefinition | compile error |
| Side effects | arguments are substituted textually into the body — prefer pure expressions (e.g. don't pass `i++`) |

## Why not C-style `#define`?

C's textual macros break IDEs, spans and carets; RubyC keeps the
zero-overhead benefit and drops the footguns. For anything larger than an
expression, use `[Inline]` methods instead.

## Related

- [`[Inline]` methods](attributes.md) — same zero-call-overhead goal for
  whole-function bodies.
- [Preprocessor](preprocessing.md) — `#define/#If/#Error/#Assembly`.
