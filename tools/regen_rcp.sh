#!/usr/bin/env bash
# Regenerate transient .rcp per library + dotnet-runtime.rcs solution.
# Same project enumeration as pull-dotnet.sh section 4-5, but keeps the
# existing description line (stable, no git churn) instead of the pull
# provenance. Run from the repo root; safe to re-run any time (a build
# deletes .rcp/.rcs via post-build hooks).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
LIBS_DIR="$ROOT/dotnet-runtime/src/libraries"
SOLUTION="$LIBS_DIR/dotnet-runtime.rcs"

libdirs=()
while IFS= read -r -d '' d; do
  libdirs+=("$d")
done < <(
  for d in "$LIBS_DIR"/*/; do
    d="${d%/}"
    [ -d "$d/src" ] || continue
    [ -n "$(find "$d/src" -name '*.cs' -print -quit 2>/dev/null)" ] || continue
    printf '%s\0' "$d"
  done | sort -z
)
[ ${#libdirs[@]} -gt 0 ] || { echo "no libraries found under $LIBS_DIR" >&2; exit 1; }

: > "$SOLUTION"
printf 'solution "dotnet-runtime"\n\n' >> "$SOLUTION"

declare -A known_libs=()
for d in "${libdirs[@]}"; do
  known_libs[$(basename "$d")]=1
done

project_refs() {
  local dir="$1" inc norm cand ref
  grep -rhoE '<ProjectReference Include="[^"]+"' "$dir/src" --include='*.csproj' 2>/dev/null \
    | sed -E 's/.*Include="//; s/"$//' \
    | while IFS= read -r inc; do
      norm="${inc//\\//}"
      cand="$norm"
      case "$norm" in
        *'$(LibrariesProjectRoot)'*) cand="${norm#*'$(LibrariesProjectRoot)'}"; cand="${cand#/}" ;;
        *)
          case "$norm" in
            */src/*)
              cand="${norm%%/src/*}"
              cand="${cand##*/}"
              ;;
            *) continue ;;
          esac
      esac
      ref="${cand%%/*}"
      [ -n "$ref" ] && printf '%s\n' "$ref"
    done | sort -u
}

# PackageReferences that map to pulled libraries (e.g. System.Memory):
# `<PackageReference Include="X" ...>` in any src csproj, X taken as the
# lib dir name (dangling names dropped at use).
package_refs() {
  local dir="$1"
  grep -rhoE '<PackageReference Include="[^"]+"' "$dir/src" --include='*.csproj' 2>/dev/null \
    | sed -E 's/.*Include="//; s/"$//' \
    | sort -u
}

# Curated framework-implicit deps (tools/framework-implicit-deps.tsv):
# types used with no csproj trace at all (E0025 discoveries).
implicit_tsv_refs() {
  local lib="$1" tsv="$SCRIPT_DIR/framework-implicit-deps.tsv"
  [ -f "$tsv" ] || return 0
  awk -F'\t' -v lib="$lib" '$1 == lib { print $2 }' "$tsv" | sort -u
}

for d in "${libdirs[@]}"; do
  lib="$(basename "$d")"
  rcp="$d/$lib.rcp"
  refs=()
  while IFS= read -r ref; do
    [ -n "$ref" ] || continue
    [ "$ref" = "$lib" ] && continue
    [ -n "${known_libs[$ref]:-}" ] || continue
    refs+=("../$ref/$ref.rcp")
  done < <(project_refs "$d")
  while IFS= read -r ref; do
    [ -n "$ref" ] || continue
    [ "$ref" = "$lib" ] && continue
    [ -n "${known_libs[$ref]:-}" ] || continue
    refs+=("../$ref/$ref.rcp")
  done < <(package_refs "$d")
  while IFS= read -r ref; do
    [ -n "$ref" ] || continue
    [ "$ref" = "$lib" ] && continue
    [ -n "${known_libs[$ref]:-}" ] || continue
    refs+=("../$ref/$ref.rcp")
  done < <(implicit_tsv_refs "$lib")
  if [ "$lib" != "System.Private.CoreLib" ] && [ -n "${known_libs[System.Private.CoreLib]:-}" ]; then
    refs+=("../System.Private.CoreLib/System.Private.CoreLib.rcp")
  fi
  if [ "$lib" != "System.Private.CoreLib" ] && [ "$lib" != "System.Private.Uri" ] && [ -n "${known_libs[System.Private.Uri]:-}" ]; then
    refs+=("../System.Private.Uri/System.Private.Uri.rcp")
  fi
  if [ "$lib" != "System.Private.CoreLib" ] && [ "$lib" != "System.Runtime.Numerics" ] && [ -n "${known_libs[System.Runtime.Numerics]:-}" ]; then
    refs+=("../System.Runtime.Numerics/System.Runtime.Numerics.rcp")
  fi
  libslines=""
  if [ ${#refs[@]} -gt 0 ]; then
    sorted=()
    while IFS= read -r r; do sorted+=("$r"); done < <(printf '%s\n' "${refs[@]}" | sort -u)
    libslist=$(printf '"%s", ' "${sorted[@]}")
    libslist="[${libslist%, }]"
    libslines="ruby_c = $libslist"
  fi
  defines_line='defines = ["TARGET_LINUX", "TARGET_UNIX", "TARGET_64BIT", "TARGET_AMD64", "TARGET_LITTLE_ENDIAN", "CORECLR", "NET", "FEATURE_PERFTRACING"]'
  if [ "$lib" = "System.Private.CoreLib" ]; then
    defines_line='defines = ["TARGET_LINUX", "TARGET_UNIX", "TARGET_64BIT", "TARGET_AMD64", "TARGET_LITTLE_ENDIAN", "CORECLR", "NET", "FEATURE_PERFTRACING", "SYSTEM_PRIVATE_CORELIB"]'
  fi
  # Harvest per-library <DefineConstants> additions (e.g. Asn1's
  # CP_NO_ZEROMEMORY, Crypto's INTERNAL_ASYMMETRIC_IMPLEMENTATIONS):
  # unconditional single-line elements in src/*.csproj only.
  extra=""
  while IFS= read -r line; do
    case "$line" in *Condition=*) continue;; esac
    content="${line#*<DefineConstants>}"; content="${content%</DefineConstants>*}"
    content="${content//\$(DefineConstants);/}"
    content="${content//;/ }"
    for tok in $content; do
      case "$tok" in *'$'*) continue;; esac
      case " $extra " in *" $tok "*) ;; *) extra="$extra $tok";; esac
    done
  done < <(grep -h "<DefineConstants>" "$d/src"/*.csproj 2>/dev/null || true)
  for tok in $extra; do
    defines_line="${defines_line%\]}, \"$tok\"]"
  done
  # description: reuse existing file's line if present (stable, no git).
  desc="Pulled from dotnet/runtime src/libraries/$lib"
  if [ -f "$rcp" ]; then
    old=$(grep -m1 '^description = ' "$rcp" || true)
    [ -n "$old" ] && desc="${old#description = \"}" && desc="${desc%\"}"
  fi
  cat > "$rcp" <<EOF
[meta]
name = "$lib"
version = "1"
description = "$desc"

[target]
platform = "linux_x86_64"
output = "shared"

[build]
sources = ["src/**/*.rc"]
entry-point = ""
$defines_line

[libs]
$libslines

[hooks]
pre-build = "bash ../rc-hooks.sh prebuild src"
post-build = "bash ../rc-hooks.sh postbuild $lib src"
EOF
  printf '[[project]]\npath = "%s/%s.rcp"\n\n' "$lib" "$lib" >> "$SOLUTION"
done

echo "regenerated ${#libdirs[@]} projects + solution"
