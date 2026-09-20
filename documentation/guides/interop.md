# Native interop

RubyC consumes foreign libraries, and foreign hosts consume RubyC
libraries. Both directions are exercised end-to-end by
`tests/native_interop.rs`; the live sources live in `examples/interop/`.

## RubyC → C: importing

Declare an import stub with `[LibraryImport]`. The method has no body;
the attribute names the library (used to pick among `--lib` files) and
the exported symbol to bind.

```rubyc
namespace app {
    class host {
        [LibraryImport("native_add", EntryPoint = "add")]
        int add(int a, int b);

        int main() { printf("{0}", this.add(2, 3)); }
    }
}
```

```bash
cc -shared -fPIC native_add.c -o libnative_add.so
rubyc build --lib libnative_add.so host.rc -o host
./host          # → 3
```

**How it links:** executables are fully static, so at build time the
compiler opens each `--lib` ELF, finds the symbol in `.dynsym`/`.symtab`,
and copies the machine code into the output image, patching call sites.
No dynamic loader, no runtime dependency. The imported body must be
self-contained (no references into its library's data or other symbols).

Constraints: at most 6 integer arguments (SysV register window), 32-bit
integer parameters/returns.

## C → RubyC: consuming shared libraries

Build any public methods into an ET_DYN object; they are exported as
plain symbols — namespaces and classes never reach the ABI.

```bash
rubyc compile -O shared testlib.rc -o libtestns.so
```

```c
void *h = dlopen("./libtestns.so", RTLD_NOW);
int (*add)(int, int) = dlsym(h, "add");
printf("%d\n", add(40, 2));   // 42
```

The `.so` is a real ELF: GOT-based position-independent code,
`R_X86_64_RELATIVE` relocations applied by the loader, SysV hash table
for `dlsym`. The 🐕🇨 signature sits at byte 64, immediately after the
ELF header (ELF owns offset 0).

Exported methods must not touch instance state (they receive a null
receiver); once `static` members land, exports will be required static.

## Rules of thumb

- One `--lib` flag per candidate library; the import's library name is
  matched against file names (`"native_add"` matches `libnative_add.so`)
  before falling back to search order.
- Missing symbols fail the build with the exact name and searched paths.
- All four combinations (RubyC/C host × C/RubyC library) are verified in
  CI by `cargo test --test native_interop`.
