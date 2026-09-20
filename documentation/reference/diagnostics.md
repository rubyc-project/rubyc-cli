# Diagnostic reference

Every diagnostic carries a stable code, severity (`error`/`warning`),
source span, and scope context (`namespace :: class :: function`).
Memory-related and style diagnostics additionally print a `help:` line
with a suggested fix.

Codes are grouped by subsystem:

- `E000x` — lexical / parse errors
- `E0011`–`E0016` — semantic checks (bases, interfaces, inheritance)
- `E0017`–`E0018` — access modifier enforcement
- `E0019`–`E0023` — declaration validity (bodyless stubs, export limits)
- `W0001`–`W0002` — lint warnings (adjust with `--warn/--deny/--allow`,
  see [cli.md](cli.md))

## Parse errors

### E0001 — syntax error

The parser found a token it did not expect at this position.

```text
error[E0001]: expected Semicolon after statement, found identifier 'x'
  → namespace t; class c { int main() { int a = 1 x = 2; } }
                                              ^^
```

**help**: check for a missing `;`, comma or closing brace just before the
highlighted token.

### E0005 — unterminated block comment

A `/*` was opened but never closed before end of file.

```text
error[E0005]: unterminated block comment
```

**help**: close the comment with `*/`, or use `//` for a line comment.

## Semantic checks

| Code | Meaning | Fix |
|---|---|---|
| E0011 | base class/interface name not declared | check spelling; declare the base first |
| E0012 | interface name declared more than once | remove the duplicate declaration |
| E0013 | two classes share the same name | rename one class |
| E0014 | inheritance cycle (class derives, directly or indirectly, from itself) | break the cycle |
| E0015 | class does not implement a method required by an interface it declares | add the missing method |
| E0016 | implemented method's signature doesn't match the interface declaration | align parameter types with the interface |

## Access modifiers

### E0017 — private member accessed outside its class

```rubyc
class lib { int secret = 1; }
class app {
    int main() { lib l = new lib(); printf("{0}", l.secret); }
}
```

```text
error[E0017]: 'lib.secret' is private and is only accessible within 'lib'
```

**help**: declare the member `public`, or add a public method inside
`lib` that exposes it.

### E0018 — protected member accessed outside the hierarchy

Same as E0017 but for `protected`: visible only inside the declaring
class and classes derived from it.

## Declaration validity

| Code | Meaning | Fix |
|---|---|---|
| E0019 | method has no body and is not an import stub | give it a body, or mark `[LibraryImport]` |
| E0020 | `[LibraryImport]` method declares a body | remove the body |
| E0021 | `[LibraryImport]` missing library name | add the positional library string |
| E0022 | class exceeds 15 fields | split the class or group fields |
| E0023 | two public methods export the same symbol | rename one; namespaces never reach the ABI |

Note: E0023 is raised during IR lowering when building shared-library
exports, so it surfaces after the semantic pass.

## Lints (warnings by default)

| Code | Default | Fires when |
|---|---|---|
| W0001 unused-local | warn | local declared but never read |
| W0002 unused-private-member | warn | private field/method unreferenced anywhere |

Upgrade or silence per rule: `--deny unused-local`, `--allow W0001`.
Denied lints become hard errors and fail the build.
