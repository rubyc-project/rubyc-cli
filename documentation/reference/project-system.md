# Project system

RubyC uses TOML-based project files for multi-file builds, dependency
management, and build hooks — similar to `.csproj` in .NET or `Cargo.toml`
in Rust.

## Files

| Extension | Purpose |
|---|---|
| `.rcp` | single project (sources, target, hooks) |
| `.rcs` | solution grouping multiple projects |

## Finding projects

When you run `rubyc build` without a file argument, RubyC walks parent
directories looking for the nearest `.rcp` or `.rcs` file.

## .rcp reference

```toml
[meta]
name = "MyApp"
version = "0.1.0"
author = "you@example.com"
description = "Does something useful"

[target]
platform = "linux_x86_64"   # from --list-targets
output = "exe"               # exe | shared | bytecode

[libs]
external = ["libm.so"]       # linked via [LibraryImport]
ruby_c = []                  # future: pre-built RubyC libraries

[build]
sources = ["main.rc", "lib/*.rc"]
entry-point = "Main"

[hooks]
pre-build = "./scripts/gen-version.sh"
post-build = "strip ./target/release/MyApp"
```

### Hooks

Hooks are shell commands executed at specific points. They receive:

| Env var | Value |
|---|---|
| `$RUBYC_PROJECT_NAME` | the `[meta] name` value |
| `$RUBYC_OUTPUT_PATH` | where the binary was written |
| `$RUBYC_CONFIG` | active profile name (`debug` or `release`) |

Non-zero exit from a hook fails the build.

## .rcs reference

```toml
solution "MySolution"
    version = "1.0.0"

    [[project]]
    path = "src/App.rcp"

    [[project]]
    path = "lib/MathLib.rcp"

    [hooks]
    pre-solution = "echo building all..."

    [profiles]
    debug = { defines = ["DEBUG"], optimize = false }
    release = { defines = ["RELEASE"], optimize = true, strip = true }
```

## CLI

```bash
rubyc new solution MySolution         # scaffold solution + default project
rubyc new project MyLib --into .     # add project to existing directory
rubyc build MyApp.rcs                 # build all projects in order
rubyc build                           # auto-detect nearest project file
```
