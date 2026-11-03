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
    // Lowering deeply-nested generated methods needs a large stack;
    // the default 8 MB overflows. Run on a thread with 64 MB.
    let handle = std::thread::Builder::new()
        .stack_size(1024 * 1024 * 1024)
        .spawn(move || rubyc::driver::cli_main(argv, &pipeline()))
        .expect("failed to spawn main thread");
    std::process::exit(handle.join().unwrap_or(1));
}

/// End-to-end suites: see `tests/mod.rs` for the file map.
#[cfg(test)]
#[path = "../tests/mod.rs"]
mod tests;
