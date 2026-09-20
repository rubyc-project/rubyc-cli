# Control flow

RubyC's control flow mirrors C#: brace-delimited blocks, C-style loops,
and short-circuit boolean operators. Every construct lowers to explicit
basic blocks — there is no hidden runtime magic.

## if / else

```rubyc
if (x > 0) {
    printf("positive");
} else if (x == 0) {
    printf("zero");
} else {
    printf("negative");
}
```

`else if` chains are ordinary nesting; depth is unlimited.

## while / do-while

```rubyc
int i = 0;
while (i < 10) { i++; }          // may run zero times

do { printf("once"); } while (false);   // body runs at least once
```

## for

```rubyc
for (int i = 0; i < 5; i++) {
    printf("{0}", i);
}
```

All three header clauses are optional: `for (;;)` is an infinite loop.
The init clause may declare a variable whose scope is the loop only.

## break / continue

- `break;` exits the innermost loop **or switch**.
- `continue;` jumps to the innermost loop's next iteration (skipping any
  enclosing switch).

```rubyc
for (int i = 0; i < 6; i++) {
    if (i % 2 == 0) { continue; }
    if (i > 4) { break; }
    printf("{0}", i);
}
```

## switch / case / default

Integer-constant dispatch with C#-style **no fallthrough** — every arm
must end in `break;`, `continue;`, or `return`.

```rubyc
switch (code) {
    case 200:
    case 201:
        printf("ok");       // multi-label arm
        break;
    case 404:
        printf("missing");
        break;
    default:
        printf("other");
        break;
}
```

Multiple labels may share one body. Exactly one `default` is allowed.
Lowered as a compare-chain over basic blocks; a constant subject folds to
just the matching arm.

## Ternary `?:`

```rubyc
int bigger = a > b ? a : b;
string label = ready ? "yes" : "no";   // when strings land (A7)
```

## Operators quick reference

| Class | Operators |
|---|---|
| arithmetic | `+ - * / %` |
| bitwise | `& \| ^ ~ << >>` |
| comparison | `== != < > <= >=` |
| logical | `&& \|\| !` |
| ternary | `? :` |
| compound | `+= -= *= /= %= &= \|= ^= <<= >>=` |
| inc/dec (statement) | `++ --` |

Precedence, loosest → tightest: `||` · `&&` · `|` · `^` · `&` · comparisons · shifts · `+ -` · `* / %`.
