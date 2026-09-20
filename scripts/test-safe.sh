#!/usr/bin/env bash
# scripts/test-safe.sh — fence `cargo test` so neither a runaway build nor a
# miscompiled program can take down the desktop again.
#
# Usage:
#   ./scripts/test-safe.sh                  # full suite
#   ./scripts/test-safe.sh comparisons      # one test-name filter
#
# Env overrides:
#   RUBYC_TEST_MEM=1500M      memory cap for the whole session (systemd path)
#   RUBYC_TEST_JOBS=2         rustc/link parallelism
#   RUBYC_TEST_TIMEOUT=900    wall-clock seconds for the entire run
#
# Layers:
#   1. systemd-run scope with MemoryMax — if anything balloons, the OOM
#      killer reaps ONLY this scope; GNOME/Rider are untouched.
#   2. -j caps rustc/link parallelism (the desktop-killer last time).
#   3. timeout kills a hung session.
#
# Note: no `ulimit -v` fallback — rustc/LLVM reserve terabytes of virtual
# address space and RLIMIT_AS breaks them. Without a systemd user session,
# the jobs limit + timeout still apply.

set -euo pipefail
cd "$(dirname "$0")/.."

MEM="${RUBYC_TEST_MEM:-1500M}"
JOBS="${RUBYC_TEST_JOBS:-2}"
WALL="${RUBYC_TEST_TIMEOUT:-900}"
THREADS="${RUBYC_TEST_THREADS:-4}"

if command -v systemd-run >/dev/null 2>&1 && [ -n "${DBUS_SESSION_BUS_ADDRESS:-}" ]; then
  echo "[test-safe] scope: MemoryMax=${MEM} TasksMax=512 jobs=${JOBS} threads=${THREADS} timeout=${WALL}s"
  exec systemd-run --user --scope -q \
    -p MemoryMax="${MEM}" \
    -p TasksMax=512 \
    -- env RUST_TEST_THREADS="${THREADS}" \
    timeout "${WALL}" cargo test -j "${JOBS}" --no-fail-fast "$@"
else
  echo "[test-safe] no systemd user session: jobs=${JOBS} threads=${THREADS} timeout=${WALL}s (no memory cap)"
  exec env RUST_TEST_THREADS="${THREADS}" timeout "${WALL}" \
    cargo test -j "${JOBS}" --no-fail-fast "$@"
fi
