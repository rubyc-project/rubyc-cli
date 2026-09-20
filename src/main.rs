//! RubyC compiler command-line tool.
//!
//! This binary is the composition root: it wires the target-agnostic
//! `rubyc-core` to the concrete `linux_x86_64` backend through a
//! [`NativePipeline`], then maps argv to an exit code via [`rubyc::driver`].
//!
//! End-to-end tests live in `tests/` (one file per feature area) and
//! run as part of the binary's unit-test target.

use rubyc::native::target::NativePipeline;
use rubyc::native::target::Target;
use rubyc_target_linux_x86_64::{BACKEND, IMAGE_WRITER, LOADER, MEMORY};

/// Build the pipeline for the host platform this CLI is linked for.
fn pipeline() -> NativePipeline<'static> {
    NativePipeline::new(
        Target::LinuxX86_64,
        &BACKEND,
        &IMAGE_WRITER,
        &LOADER,
        &MEMORY,
    )
}

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    std::process::exit(rubyc::driver::cli_main(argv, &pipeline()));
}

/// End-to-end suites: see `tests/mod.rs` for the file map.
#[cfg(test)]
#[path = "../tests/mod.rs"]
mod tests;
