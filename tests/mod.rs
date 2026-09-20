//! End-to-end tests for the `rubyc` binary.
//!
//! One file per feature area; the filename says what's inside. These are
//! unit-test modules of the binary crate (see `main.rs`), so they run with
//! plain `cargo test` and share [`common`] for process plumbing: bounded
//! output capture, hard timeouts, and binary discovery.

pub(crate) mod common;

// language features, executed through both JIT and ELF paths
mod access_modifiers;
mod assembly_blocks;
pub(crate) mod comparisons;
mod control_flow;
mod enums_structs_static;
mod enums_tests;
mod inline_methods;
mod int_widths;
mod switch_stmt;
mod variables;
mod workspace;

// compiler surfaces exercised through the CLI
mod cli_pipes;
mod extension_dirs;
mod diagnostics_quality;
mod jit;
mod lints;
mod native_exec;
mod native_exec_vm;
mod perf_gates;
mod project_system;
mod stress_complex;
mod stress_fixture;
mod stress_output;
mod stress_test;

// native targets and interop
mod library_matrix;
mod native_interop;
mod target_os_consistency;

// dynamic plugin ABI
mod plugin_e2e;
