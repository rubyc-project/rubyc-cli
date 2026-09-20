//! Shared test utilities: run compiled programs with a hard timeout and
//! bounded output capture, so a miscompiled loop can never exhaust memory.
//!
//! Suites that execute generated code go through [`run`] (stdin bytes) or
//! [`exec`] (no stdin). Both return an [`Outcome`] with the exit code and
//! captured streams; `Outcome::timed_out` distinguishes a kill.

#![allow(dead_code)] // shared across suites; not every helper used everywhere

use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// VM-side execution (scp + ssh): compiled artifacts never run locally.
pub mod vm;

/// Kill the child after this long, no matter what it is doing.
pub const EXEC_TIMEOUT: Duration = Duration::from_secs(10);
/// Stop *storing* output past this size; keep draining so pipes never fill.
const MAX_CAPTURE: usize = 1 << 20; // 1 MiB

pub struct Outcome {
    /// Process exit code, or `-1` on timeout/kill.
    pub code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    /// True when the child was killed for exceeding [`EXEC_TIMEOUT`].
    pub timed_out: bool,
}

impl Outcome {
    pub fn stdout_str(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }
    pub fn stderr_str(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }
}

fn drain<R>(reader: Option<R>) -> std::sync::mpsc::Receiver<Vec<u8>>
where
    R: Read + Send + 'static,
{
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut stored = Vec::new();
        if let Some(mut r) = reader {
            let mut chunk = [0u8; 8192];
            loop {
                match r.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if stored.len() < MAX_CAPTURE {
                            let take = n.min(MAX_CAPTURE - stored.len());
                            stored.extend_from_slice(&chunk[..take]);
                        }
                        // past the cap: still draining, just not storing
                    }
                }
            }
        }
        let _ = tx.send(stored);
    });
    rx
}

/// Spawn `cmd`, feed `stdin_data` when given, wait with a hard timeout.
pub fn spawn_and_collect(cmd: &mut Command, stdin_data: Option<&[u8]>) -> Outcome {
    spawn_and_collect_timeout(cmd, stdin_data, EXEC_TIMEOUT)
}

/// Spawn `cmd` with a caller-chosen hard deadline (VM round-trips need
/// longer than local runs).
pub fn spawn_and_collect_timeout(
    cmd: &mut Command,
    stdin_data: Option<&[u8]>,
    deadline_after: Duration,
) -> Outcome {
    cmd.stdin(if stdin_data.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    })
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());

    let mut child: Child = cmd.spawn().expect("spawn failed");
    if let Some(data) = stdin_data {
        let mut pin = child.stdin.take().expect("stdin pipe");
        let _ = pin.write_all(data); // child may exit early; ignore EPIPE
    }
    // `pin` dropped above closes the pipe → child sees EOF.

    let out_rx = drain(child.stdout.take());
    let err_rx = drain(child.stderr.take());

    let deadline = Instant::now() + deadline_after;
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    timed_out = true;
                    break None;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(_) => {
                timed_out = true;
                break None;
            }
        }
    };

    // Readers finish promptly after exit/kill because the pipes close.
    let stdout = out_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap_or_default();
    let stderr = err_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap_or_default();

    Outcome {
        code: status.map(|s| s.code().unwrap_or(-1)).unwrap_or(-1),
        stdout,
        stderr,
        timed_out,
    }
}

/// Run `cmd` with `stdin_data`, capture bounded output.
pub fn run(cmd: &mut Command, stdin_data: &[u8]) -> Outcome {
    spawn_and_collect(cmd, Some(stdin_data))
}

/// Run `cmd` with no stdin, capture bounded output.
pub fn exec(cmd: &mut Command) -> Outcome {
    spawn_and_collect(cmd, None)
}

/// Locate the `rubyc` binary this suite belongs to.
///
/// Unit tests cannot use `env!("CARGO_BIN_EXE_rubyc")` (that variable is
/// only set for integration-test targets), so resolve the binary at
/// runtime: cargo places it next to the `deps/` directory that holds the
/// running test harness.
pub fn rubyc_path() -> std::path::PathBuf {
    if let Some(p) = std::env::var_os("CARGO_BIN_EXE_rubyc") {
        return p.into();
    }
    let exe = std::env::current_exe().expect("current_exe of the test harness");
    if let Some(binary) = exe
        .ancestors()
        .find(|dir| dir.join("rubyc").is_file())
        .map(|dir| dir.join("rubyc"))
    {
        return binary;
    }
    // Coverage and custom-target runners place the harness below an
    // alternate target directory without building a sibling CLI binary.
    // Fall back to the workspace's ordinary debug artifact.
    if let Some(root) = exe.ancestors().find(|dir| dir.join("Cargo.toml").is_file()) {
        let binary = root.join("target/debug/rubyc");
        if binary.is_file() {
            return binary;
        }
    }
    panic!("cannot locate the rubyc binary from {exe:?}")
}

/// A `Command` pointing at the `rubyc` binary.
pub fn rubyc() -> Command {
    Command::new(rubyc_path())
}
