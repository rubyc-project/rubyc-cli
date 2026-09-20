# Getting started

## Hello world

```rubyc
namespace app {
    class hello {
        int main() {
            printf("Hello from RubyC!");
        }
    }
}
```

```bash
rubyc run hello.rc        # JIT-execute immediately
rubyc build hello.rc -o hello   # fully static native executable (~4 KB)
./hello
```

No runtime install, no libc dependency, no dynamic loader — the executable
talks to the kernel directly.

## Variables & arithmetic

```rubyc
namespace t {
    class calc {
        int main() {
            int x = 20;
            int y = 22;
            int sum = x + y;
            printf("{0}", sum);      // 42
        }
    }
}
```

Supported today: `int` arithmetic (`+ - * /`), unary minus, parentheses,
locals with initializers, assignment.

## Classes & virtual dispatch

```rubyc
namespace shapes {
    class base { public int Area() { return 0; } }
    class square : base {
        public int side = 4;
        public override-free int Area() { return this.side * this.side; }
    }
}
```

Method calls dispatch through per-class vtables; the nearest override wins.

## Native interop

Call any C library:

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
rubyc build --lib libm.so host.rc -o host && ./host   # → 5
```

The imported machine code is embedded into your executable at link time —
the result stays fully static. See [interop.md](interop.md) for both
directions (C hosts can also `dlopen` RubyC-built libraries).

## Lints

```bash
rubyc build --deny unused-local app.rc
rubyc run  --allow W0002 script.rc
```

Warnings never fail builds unless upgraded with `--deny`. Stable ids
(`W0001`, …) or rule names both work.
