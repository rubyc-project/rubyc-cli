# Embedding RubyC from C

`librubyc.so` (dynamic) / `librubyc.a` (static) expose a C ABI so any
language can compile and run RubyC code in-process.

## Build

```bash
cargo build --release -p rubyc-core
# → target/release/librubyc.so  (or .a)
```

Header: `compiler/rubyc-core/include/rubyc.h`.

## Minimal program

```c
#include "rubyc.h"

int main(void) {
    const char *src =
        "namespace t { class t { int main() { printf(\"hi\"); } } }";

    if (rubyc_run_source(src) != 0) {
        const char *err = rubyc_last_error();
        fprintf(stderr, "failed: %s\n", err ? err : "?");
        return 1;
    }
    return 0;
}
```

```bash
cc app.c -I compiler/rubyc-core/include \
   -L target/release -lrubyc \
   -Wl,-rpath,'$ORIGIN/../target/release' -o app
./app        # → hi
```

## API

| Function | Purpose |
|---|---|
| `rubyc_compile_file(path, &bytes, &len)` | compile to a static native image |
| `rubyc_run_source(src)` | JIT-execute; returns exit status |
| `rubyc_last_error()` | last error on this thread, or NULL |
| `rubyc_free_string(p)` / `rubyc_free_buffer(p, len)` | release returned data |

## Notes

- Strings are UTF-8, NUL-terminated.
- Error strings are thread-local and valid until the next call.
- Compiled *programs* remain fully static; only the compiler library is a
  shared object.
