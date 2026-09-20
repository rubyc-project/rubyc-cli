# Properties, Const Fields, Indexers, and Events

These features extend class members beyond plain fields and methods.

## Const fields

A `const` field is a compile-time constant. It has no object-layout slot; the
value is inlined at every use site.

```rubyc
public class Config {
    public const int MaxRetries = 3;
    public const string Name = "my-app";
}
```

## Properties

A **property** is an accessor-backed member: one or more blocks (`get`, `set`,
`init`) or a single expression body. Storage is not implicit — a property with
an explicit accessor body reads from and writes to an explicit backing field
(e.g. `_value`). Auto-properties (`{ get; set; }`) are declarations only;
lowering is a separate roadmap item.

```rubyc
public class Widget {
    private int _value;

    // Explicit accessors.
    public int Value {
        get { return _value; }
        set { _value = value; }
    }

    // Auto-property with initializer (declaration only).
    public int Auto { get; set; } = 11;

    // Getter-only.
    public int GetterOnly { get; } = 22;

    // Init-only.
    public string InitOnly { get; init; } = "INIT";

    // Expression-bodied.
    public int Double => _value * 2;
}
```

### Accessor forms

| Form | Meaning |
|------|---------|
| `get { … }` | Explicit getter body |
| `get;` | Auto getter (lowering pending) |
| `get => expr;` | Expression getter |
| `set { … }` | Explicit setter body |
| `set;` | Auto setter (lowering pending) |
| `init;` | Init-only accessor (lowering pending) |
| `=> expr;` | Expression body (shorthand for `get => expr;`) |

## Indexers

An **indexer** is a parameterized property — a `this[params]` member with the
same accessor forms as a property.

```rubyc
public class IntBag {
    private int[] _values = new int[64];

    public int this[int index] {
        get { return _values[index]; }
        set { _values[index] = value; }
    }

    // Expression-bodied.
    public int this[int i] => _values[i];
}
```

## Events

An **event** declares a named notification hook. It is a declaration only —
`+=` / `-=` subscription and `invoke` are separate roadmap items.

```rubyc
public class EventSource {
    public event System.Action Changed;

    public int Raise() {
        if (Changed is null) { return -1; }
        // invoke Changed; (lowering pending)
        return 0;
    }
}
```

## What is implemented

Const fields, properties, indexers, and events are first-class **declarations**:
they parse, round-trip through bytecode, and are accepted as class members.
Property/indexer accessor lowering (auto-property storage, `init` semantics),
event subscription (`+=` / `-=`), and event invocation are separate roadmap
items — see [TODO.md](../../TODO.md).

## Diagnostics

Malformed declarations report stable `E0001` syntax errors, e.g.:

```text
error[E0001]: expected identifier as event name, found 'int'
error[E0001]: expected '[' to open indexer parameter list, found 'int'
error[E0001]: expected ']' to close indexer parameter list, found 'int'
```
