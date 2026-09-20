# Preprocessor

Line-based directives handled before tokenization. Excluded lines keep
their line numbers, so diagnostics stay exact. No textual macros —
compile-time constants belong in the language; inline code belongs to
[`#macro`](../language/macros.md) and [`[Inline]` methods](../language/attributes.md).

## Directives

| Directive | Effect |
|---|---|
| `#define SYM` | define symbol for `#If` |
| `#undef SYM` | remove symbol |
| `--define SYM` (CLI) | seed a symbol from the command line |
| `#If A B …` / `#Else` / `#EndIf` | conditional block: all listed symbols must be defined |
| `#Error "msg"` | abort compilation |
| `#Warning "msg"` | emit warning, continue |
| `#Assembly` … `#EndAssembly` | assemble and emit raw native instructions at program entry |

`#Assmbly` and `#EndAssmbly` are accepted as compatibility spellings. The
surface is owned by the same-version `rubyc-assembly` language dependency and
catalogued as `RubyC.Assembly.Block`, `.Inline`, `.AtAddress`, and source-file
assembly. Block and source-file execution are implemented; Inline and
AtAddress remain reserved package APIs.

Built-in symbols: `DEBUG`, target triple (`LINUX_X86_64`).

## Example

```rubyc
#define VERBOSE

namespace app { class hello {
    int main() {
        #If DEBUG
        printf("debug build\n");
        #EndIf
        printf("hello\n");
    }
} }
```

## Assembly blocks

```rubyc
#Assembly
    ; raw NASM-flavored x86-64, captured verbatim
#EndAssembly
```

Blocks are collected verbatim by preprocessing. They are not yet attached to
the parsed unit or lowered into native code; use standalone `.asm` input when
assembly must be emitted today. Nesting is an error; unclosed blocks are
errors.

## Diagnostics

E0004 covers every malformed directive (unknown directive, missing
`#EndIf`, nested/unclosed `#Assembly`) with the line number preserved.
