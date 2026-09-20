# Native built-ins

Functions available in every program without imports. They compile to raw
syscalls plus small hand-written assembly snippets — there is no libc, no
runtime library, and no buffering layer between your program and the
kernel.

| Function | Writes | Reads | Returns |
|---|---|---|---|
| `printf(fmt, args…)` | stdout (fd 1) | — | `int`, always `0` |
| `print(…)` / `put_str(…)` | stdout (fd 1) | — | `int`, always `0` |
| `eprintf(fmt, args…)` | stderr (fd 2) | — | `int`, always `0` |
| `scan_int()` | — | stdin (fd 0) | the parsed integer |

All five are ordinary expressions: their values can be assigned, compared,
or ignored.

---

## printf

```rubyc
printf("hello\n");
printf("{0} + {1} = {2}\n", 2, 3, 5);   // → 2 + 3 = 5
```

### Parameters

| # | Name | Type | Notes |
|---|---|---|---|
| 1 | `format` | string **literal** | literal text + `{N}` placeholders |
| 2+ | `args…` | integer expressions | consumed left-to-right by `{N}` |

### Format-string rules

- Placeholders are **zero-based** (`{0}` is the first argument) and may
  repeat or appear in any order:
  ```rubyc
  printf("{1} before {0}", a, b);
  printf("{0} and {0}", x);
  ```
- Integers print as **signed decimal**, including a `-` for negatives.
- Segments evaluate arguments **once each**, before any output is written,
  in source order:
  ```rubyc
  printf("{0} then {1}", first(), second());   // first() completes before second() starts
  ```
- `{{` prints a literal `{`; `}}` prints a literal `}`:
  ```rubyc
  printf("{{literal}}");                  // → {literal}
  ```

### Compile errors (caught before anything runs)

| Error | Example | Message shape |
|---|---|---|
| Unclosed brace | `printf("{0", x);` | `'{' without matching '}'` |
| Non-digit placeholder | `printf("{x}", y);` | invalid placeholder |
| Index past last argument | `printf("{3}", a);` | placeholder {3} but only 1 argument(s) given |

There is deliberately **no** width/precision/alignment syntax (`%5d`,
`%-08x` …). Formatting beyond signed decimal arrives with strings and
`RubyC.Console`.

### Behavior notes

- Output goes to fd 1 via one `write(2)` syscall per call — unbuffered;
  interleaving with `scan_int()` is predictable.
- A trailing `\n` is never added implicitly.

## print / put_str

Exact aliases of `printf` (same parameters, same rules, same return).

## eprintf

Identical to `printf` except output goes to **fd 2 (stderr)**:

```rubyc
if (divisor == 0) {
    eprintf("error: divisor is 0\n");
}
```

Use it for diagnostics so piping stdout to another program stays clean:

```bash
./app 2>errors.log | consumer
```

---

## scan_int

```rubyc
int n = scan_int();
printf("{0}\n", n * 2);
```

```bash
echo 21 | ./app        # → 42
```

### Parameters

None. It takes no arguments; passing one is a compile error.

### Return value

The parsed integer. Parsing rules, in order:

1. Up to **80 bytes** are fetched from stdin with a single `read(2)`
   syscall (no retry loop, no buffering).
2. An optional leading `-` makes the result negative (`--` also accepted,
   matching the current snippet's behavior).
3. Digits accumulate in base 10 until the first non-digit byte or end of
   buffer.
4. Returns `0` on EOF, read error, or when the first byte is not a sign or
   digit — there is no error channel yet (planned: exceptions, A9).

### Behavior notes

- One call consumes up to one "buffer" of input per read; a second call
  issues a fresh syscall (the kernel may still have buffered data from a
  pipe).
- Overflow wraps silently, consistent with integer arithmetic semantics.
- For line-based input (`name = scan_int()` style prompts), print the
  prompt with `printf` first — there is no automatic flush because there
  is no buffer.

---

## What is *not* here yet

`scanf`, string reading, floats, char I/O, file access — all arrive with
their enabling features (strings A7, floats A7.5, files post-A9). The
built-in surface grows only with the language; nothing here is a promise.
