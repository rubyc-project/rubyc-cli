# Types and variables

## Numeric types

All integer types share 64-bit storage today; the type name controls
compile-time checks. C#-compatible names:

| Type | Range |
|---|---|
| `sbyte` / `byte` | −128..127 / 0..255 (checked at compile time) |
| `short` / `ushort` | ±32 K ranges |
| `int` (default) / `uint` | ±2.1 G / 0..4.2 G |
| `long` / `ulong` | full ±9.2 E / 0..18 E |
| `nint` / `nuint` | same as long today |
| `bool` | `true` / `false`; stored as 0/1 |

Arithmetic wraps silently (C# *unchecked* default); `checked { }` blocks
arrive with the A2 phase.

## Operators

Comparison operators (`==`, `!=`, `<`, `>`, `<=`, `>=`) produce `bool`.
Logical `&&` / `||` and unary `!` operate on bools (non-zero integers
normalize to `true`). Precedence, loosest to tightest:
`||` · `&&` · comparisons · `+ -` · `* /`.

```rubyc
bool ready = x > 0 && !(y == 0);
printf("{0}", ready);        // → 1 or 0
```

## Variables

```rubyc
namespace t {
    class demo {
        int main() {
            int x = 10;          // declaration + initializer
            x = x * 2;           // assignment
            int y = x + 5;       // read into a new local
            printf("{0}", y);    // → 25
        }
    }
}
```

- Locals must be initialized before use; reading an uninitialized local is
  a compile error.
- Names are case-sensitive, UTF-8 identifiers allowed.
- Shadowing in nested scopes is not yet supported (one home per name per
  method today).

## Fields

Class fields are declared inside classes with optional initializers; the
initializer runs when the object is constructed:

```rubyc
class player {
    public int hp = 100;
}
```

Field writes through object references: `obj.f = expr;`

```rubyc
p x = new p();
x.hp = 77;
printf("{0}", x.hp);   // → 77
```

Field access respects access modifiers (see [classes.md](classes.md)).

## Arrays

One-dimensional and jagged (array-of-array) arrays are supported. Allocate
with `new T[n]`, index with `arr[i]`, and read the element count with
`arr.Length`. Elements are 8-byte slots at a fixed stride; the length
lives in the first slot of the allocation.

```rubyc
int[] a = new int[4];
a[0] = 10;
a[1] = 20;
printf("{0} {1} {2}", a[0], a[1], a.Length);  // → 10 20 4
```

Jagged arrays nest naturally — each inner array is its own allocation:

```rubyc
int[][] grid = new int[2];
grid[0] = new int[3];
grid[1] = new int[3];
grid[0][1] = 7;
printf("{0}", grid[0][1]);   // → 7
```

Object arrays store references, so field access through an element works:

```rubyc
Point[] ps = new Point[3];
ps[0] = new Point(1, 2);
printf("{0}", ps[0].x);      // → 1
```

Bounds are not yet trapped at runtime; an out-of-range index reads or
writes past the allocation. Array initializers, `Rank`, ranges, and
`foreach` arrive in the array-completion phase (see [README](../../README.md)).

## What's next

Control flow is in; remaining array work (initializers, bounds traps,
`foreach`), then strings and structs.
