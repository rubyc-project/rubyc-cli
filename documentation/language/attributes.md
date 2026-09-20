# Attributes

Attributes attach compiler-facing metadata to declarations:

```rubyc
[Name(positional, Key = value)]
```

Syntax: an attribute list sits directly above a method declaration.
Arguments are positional first, then `Key = value` pairs; values are
string or integer literals today.

They are parsed into the AST, survive bytecode round-trips, and are
consumed by named compiler passes.

## LibraryImport — native interop

Declares an external function provided by a shared library. The method has
no body; the linker binds it at build time by extracting the symbol's
machine code from the referenced ELF.

```rubyc
namespace app {
    class host {
        [LibraryImport("m", EntryPoint = "add")]
        int add(int a, int b);

        int main() { printf("{0}", this.add(2, 3)); }
    }
}
```

```bash
cc -shared -fPIC m.c -o libm.so
rubyc build --lib libm.so host.rc -o host
./host        # → 5
```

Rules:

- Positional argument 0 (or `Library = "…"` key) names the library hint.
- `EntryPoint = "sym"` overrides the exported symbol name.
- At most 6 integer parameters; 32-bit ints today.
- The executable stays fully static: the imported code is *embedded*, not
  dynamically loaded.
- Libraries may declare their own imports if `--lib` files are supplied at
  `-O shared` time.

## [Inline]

Marks a method whose single-`return` body is spliced at every call site —
zero call overhead:

```rubyc
[Inline]
public int square(int x) { return x * x; }
```

Expansion happens before semantic checks; arguments must be side-effect-free
(literals/identifiers). Larger bodies fall back to normal calls.

## Where attributes can appear today

| Target | Supported |
|---|---|
| methods (bodyless stubs) | yes — `LibraryImport` |
| methods with bodies, classes, fields, parameters | parsed and stored, but no pass consumes them yet |

## Unknown attributes

An attribute no pass recognizes is **accepted silently**. It round-trips
through bytecode and is preserved in the AST — nothing errors, nothing
changes codegen:

```rubyc
[MyCustom(1, Flag = "yes")]     // parses fine, currently inert
int helper();
```

This is deliberate staging for custom attribute definitions: the syntax
is stable ahead of the semantics.

## Defining your own attributes

Not yet. Today exactly one built-in (`LibraryImport`) has meaning; there
is no syntax to declare a new attribute kind. Custom attribute
declarations are on the roadmap as a core feature (declaration surface,
validation rules, compile-time query API) and will land with their own
diagnostics.

## Future attributes

The attribute surface grows with the language: `[NoBoxing]`, `atomic`
fields, `likely`/`inline` hints, `thread_bound` classes and compile-time
`Embed("file")` are planned extensions and land with their phases
(see the roadmap board in the repository README). Caller-info attributes
(`[CallerMemberName]`, `[CallerArgumentExpression]`) arrive with custom
attribute infrastructure.
