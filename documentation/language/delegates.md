# Delegates

A **delegate** declares a named callable type — a signature (return type plus
parameters) that a method, lambda, or method group can later be bound to.

```rubyc
namespace program {
    public delegate int IntBinary(int left, int right);

    class app {
        public static int Add(int a, int b) {
            return a + b;
        }

        int main() {
            IntBinary op = Add;   // method group -> delegate
            printf("{0}", op(2, 3)); // → 5
        }
    }
}
```

## Generic delegates

A delegate may carry type parameters, used to express a family of related
callable signatures. Generic *variance* qualifiers (`in` / `out`) are accepted
on the parameters.

```rubyc
public delegate TResult GenericConverter<TInput, TResult>(TInput value);
public delegate TOutput Transformer<in TInput, out TOutput>(TInput value);
```

## Qualified and generic return types

The return type may be a fully-qualified name and may itself be generic; the
base name is retained while the generic argument list is consumed.

```rubyc
public delegate System.Threading.Tasks.Task<int> Fetcher<T>(string url);
```

## What is implemented

Delegates are first-class **declarations**: they parse, round-trip through
bytecode, and are accepted as namespace-level members. Binding a value to a
delegate (method groups, `new Delegate(...)`, lambda expressions) and invoking
a delegate are separate roadmap items — see
[TODO.md](../../TODO.md) for "Lambdas, closures, anonymous methods, method
groups, and delegate invocation" and "Delegates and generic delegates".

## Diagnostics

Malformed declarations report stable `E0001` syntax errors, e.g.:

```text
error[E0001]: expected identifier as delegate name, found 'void'
error[E0001]: expected '(' to open delegate parameter list, found 'void'
```

The phrase after "expected identifier" is localized from
`locales/en.json` (keys `rubyc.parser.rd.Parser.ctx.*`).
