# Library format: images, type tables, and discovery

This page is the contract for RubyC library artifacts: what bytes live
where, how consumers read types without sources, and how the install
tree stays consistent. Code references point at the implementation;
this page states the invariants that must hold across changes.

## Artifacts

A library build produces an **image** plus its **type fragment**:

| Output | Image | Fragment location (default) |
|---|---|---|
| `shared` | `<lib>.so` (ELF ET_DYN) | Post-magic table (below) |
| `static` | `<lib>.a` (GNU ar) | `<lib>.rci` archive member |

With `--no-embed-types`, no fragment is emitted anywhere (pure-ABI
image for plain C/C++ hosts).

Legacy form (pre-table backfills): a `.rubyc_types` ELF section in
`.so`, or a sidecar `<lib>.rci` file next to the image. Readers still
accept both; nothing new emits them.

## Preamble and post-magic table (`.so`)

Every image starts with the 13-byte container preamble (`native::container`):

```
offset 64: EB 08                       jump over magic
offset 66: F0 9F 90 95 F0 9F 87 A8     dog magic
offset 74: kind byte                   0x03 shared, 0x05 shared-with-types
offset 75: version u16 LE              bytecode FORMAT_VERSION
```

When kind is `0x05`, offset 77 holds a u64 LE table length followed by
exactly that many fragment bytes (`rci::encode` output, `RCI1` magic);
the payload (program headers) follows. Unknown kind bytes are a hard
error, never a misparse. Truncated tables (length past EOF) read as
absent, never panic. Old readers see kind `0x05`, reject it cleanly,
and keep loading `0x03` images untouched — no format-version bump was
needed for the migration.

The table is *not* covered by any `LOAD` segment and carries no `ALLOC`
flag wherever a section view exists: loaders skip it, `dlopen` never
sees it.

## Archive members (`.a`)

GNU ar, no symbol index (our linker resolves members itself; plain
`ar t`/`x` interoperate). Member-name convention:

- bare `<lib>` — machine code (`MCM1`).
- `<lib>.rci` — type fragment (only with embedding on).
- Linkers merge code members and skip `.rci` members; readers do the
  inverse.

## Fragment contents and trust

A fragment is the library's own post-resolution table entries
(classes, interfaces, delegates) plus the source hash of the inputs
that produced them (`rci_source_hash`: sorted sources + defines +
compiler identity).

- **Dependency consume** (`dynamic` projects): embedded-or-file bytes
  decode, the lib name must match, and the hash must equal a fresh
  hash — anything else falls back to compiling from sources. Stale
  fragments are never trusted.
- **Discovery** (`using` with no `ruby_c` entry): no sources exist to
  hash against, so installed fragments are trusted as-written; a
  missing image (`.so` or `.a`) vetoes. Single owner auto-links;
  two or more fail loudly naming every owner. Shared framework
  namespaces (`System.Net` spans five libraries) are inherently
  multi-owner at `using` granularity — those consumers declare an
  explicit `ruby_c` entry instead (backfill always does; discovery is
  a hand-written-code convenience, not a backfill path).
- **Static link**: `.a` members merge by body (functions by entry,
  vtable rows and static slots by object symbol). Merged state
  duplicates per image — unlike dynamic linking, there is no shared
  home for statics.

## Install layout (`~/.local/share/rubyc/libraries/`)

```
libraries/
  <lib>.so          shared image (table inside when embedded)
  <lib>.rci         sidecar fragment (legacy only; absent when embedded)
  static/<lib>.a    static archive (code + `.rci` member when embedded)
  index.tsv         type path → owning lib (regenerated when any
                    artifact is newer; atomic temp+rename write)
  dynamic/          dev symlink to `.` (flattened layout remnant)
```

`index.tsv` rows are `type_path<TAB>kind<TAB>lib`, sorted; `kind` is
`class`, `interface`, or `delegate`. Lookup matches a namespace
exactly or as an ancestor (`System.Text` owns `System.Text.Decoder`).

## Commands

- `install libs [--format shared|static|both] [--jobs N]`: operate on the
  nearest `.rcs`/`.rcp` walking up from the working directory (the same
  discovery `build` uses); rebuild only stale libraries (installed image vs
  hash inputs), install via post-build hooks. `--format both` runs a shared
  pass then a static pass (dynamic projects skip static: no home image to
  copy-link from).
- `build <solution> --jobs N`: concurrent project builds.
- `build <project> --rci-only`: fragment sidecar only, no codegen.
