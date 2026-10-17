//! Execute compiled test artifacts on the rubyc-dev VM over ssh.
//!
//! Project rule: compiled executables never run on the host laptop.
//! Every helper here copies the artifact over, runs it remotely, and
//! captures the result — build locally, run on the VM, compare here.
//!
//! Exit codes pass through ssh transparently (remote 139 stays 139);
//! only ssh's own failures surface as 255. Remote hangs are bounded by
//! the `timeout(1)` wrapper (exit 124), and our side has its own
//! deadline well above it.

use super::{spawn_and_collect_timeout, Outcome};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

/// VM-side per-run deadline: comfortably above any sane test.
const VM_DEADLINE: Duration = Duration::from_secs(60);
/// Our-side deadline: VM deadline plus ssh/scp overhead.
const HOST_DEADLINE: Duration = Duration::from_secs(90);

static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Cap concurrent VM round-trips: seventeen threads opening scp+ssh at
/// once trips over sshd connection throttling and produces flaky 255s.
/// Four lanes keep full speed without storms.
static LANES: std::sync::Mutex<u32> = std::sync::Mutex::new(0);
static LANE_CVAR: std::sync::Condvar = std::sync::Condvar::new();
const MAX_LANES: u32 = 4;

struct LaneGuard;
impl LaneGuard {
    fn take() -> Self {
        let mut lanes = LANES.lock().unwrap();
        while *lanes >= MAX_LANES {
            lanes = LANE_CVAR.wait(lanes).unwrap();
        }
        *lanes += 1;
        LaneGuard
    }
}
impl Drop for LaneGuard {
    fn drop(&mut self) {
        let mut lanes = LANES.lock().unwrap();
        *lanes -= 1;
        LANE_CVAR.notify_one();
    }
}

/// True when exit 255 came from ssh itself (transport), not the guest:
/// guest stderr never contains ssh's own diagnostics.
fn is_transport_failure(out: &Outcome) -> bool {
    // Retry when the guest status marker is ABSENT: the remote shell
    // never reached the echo, so ssh's own exit code (255 for a refused
    // or reset transport, a timed-out stage, a closed channel — often
    // with no stderr text) is all we have, and it is not a guest
    // result. A guest that ran at all always carries the marker, so
    // its own failures (including a legitimate 255) are never retried.
    out.code == 255 && !out.stdout_str().contains(GUEST_RC_PREFIX)
}

fn ssh_base() -> Command {
    let mut cmd = Command::new("ssh");
    cmd.args(["-o", "ConnectTimeout=15", "-o", "BatchMode=yes", "rubyc-dev"]);
    cmd
}

/// Copy `local` to a fresh VM temp dir; returns the remote dir on success.
fn stage(local: &Path) -> Result<String, String> {
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = format!("/tmp/rubyc_vmtest_{}_{}", std::process::id(), n);
    let mut mkdir = ssh_base();
    mkdir.arg(format!("mkdir -p {dir}"));
    let out = super::spawn_and_collect_timeout(&mut mkdir, None, HOST_DEADLINE);
    if out.code != 0 {
        return Err(format!("vm mkdir failed: {}", out.stderr_str()));
    }
    let remote = format!("{dir}/t");
    let mut scp = Command::new("scp");
    scp.args([
        "-o",
        "ConnectTimeout=15",
        "-o",
        "BatchMode=yes",
        &local.to_string_lossy(),
        &format!("rubyc-dev:{remote}"),
    ]);
    let mut out = super::spawn_and_collect_timeout(&mut scp, None, upload_deadline(local));
    if out.code != 0 {
        // Name the upload budget explicitly: a scp timeout is a
        // transport hiccup the retry wrapper must classify (bare
        // exit 255 looks like a real guest failure otherwise).
        if out.timed_out {
            out.stderr.extend_from_slice(b"[vm_exec: scp host-side timeout]");
        }
        return Err(format!("scp failed: {}", out.stderr_str()));
    }
    Ok(dir)
}

/// Upload budget for `scp`: the flat [`HOST_DEADLINE`] is sized for
/// command round-trips, not for pushing a multi-megabyte static
/// image (the pulled-world exes are ~45 MB; a slow link exceeded
/// 90 s and the stage timed out as bare exit 255 — an unmarked
/// transport failure the retry wrapper could not classify, so the
/// run reported a bogus test failure instead of retrying).
/// Budget scales with the payload (10 s + ~1 s per MB) and keeps
/// the old floor for small artifacts.
fn upload_deadline(local: &Path) -> Duration {
    let mb = std::fs::metadata(local).map(|m| m.len() / (1024 * 1024)).unwrap_or(0);
    HOST_DEADLINE.saturating_add(Duration::from_secs(mb))
}

fn cleanup(dir: &str) {
    let mut rm = ssh_base();
    rm.arg(format!("rm -rf {dir}"));
    let _ = super::spawn_and_collect_timeout(&mut rm, None, Duration::from_secs(30));
}

/// Run a staged executable on the VM: `args` passed through, `stdin`
/// forwarded over the ssh channel, stdout/stderr/exit captured.
///
/// Remote command is wrapped in `timeout -s KILL 50`: hangs surface as
/// exit 124 (never a hung test suite). `timed_out` on the outcome
/// means OUR side gave up (ssh wedged); prefer `code == 124` checks
/// for guest hangs.
pub fn vm_exec(local_exe: &Path, args: &[&str], stdin: Option<&[u8]>) -> Outcome {
    // Serialize VM lanes and retry transport failures with backoff:
    // a throttled/rejected ssh connection must never fail a test.
    let _lane = LaneGuard::take();
    let mut attempt = 0;
    loop {
        attempt += 1;
        let out = vm_exec_once(local_exe, args, stdin);
        if !is_transport_failure(&out) || attempt >= 4 {
            return out;
        }
        std::thread::sleep(std::time::Duration::from_millis(400 * attempt as u64));
    }
}

/// Shell-quote one argv element for the remote command line:
/// single-quote wrap with `'\''` escapes. Without this, args
/// containing spaces split on the VM (`"a b"` arrived as two args
/// and broke argv echo) — and quoting is a no-op for plain args.
fn sh_quote(arg: &str) -> String {
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('\'');
    out.push_str(&arg.replace('\'', "'\\''"));
    out.push('\'');
    out
}

/// Single attempt; see [`vm_exec`] for the retry wrapper.
fn vm_exec_once(local_exe: &Path, args: &[&str], stdin: Option<&[u8]>) -> Outcome {
    let dir = match stage(local_exe) {
        Ok(d) => d,
        Err(e) => {
            return Outcome {
                code: 255,
                stdout: vec![],
                stderr: e.into_bytes(),
                timed_out: false,
            };
        }
    };
    let quoted: Vec<String> = args.iter().map(|a| sh_quote(a)).collect();
    // No `exec` in the remote command: exec replaces the shell, so a
    // guest killed by a signal just closes the channel and ssh
    // reports its OWN failure code (255) — indistinguishable from a
    // transport error, which let a real SIGSEGV masquerade as a
    // transport hiccup (and got "retried" into silence). The guest
    // status is echoed as a marker instead: real guest failures stay
    // verbatim (139 = SIGSEGV, 124 = timeout), and a genuine
    // transport failure still arrives with no marker at all.
    let remote_cmd = format!(
        "chmod +x {dir}/t; timeout -s KILL 50 {dir}/t {}; echo \"{}$?\"",
        quoted.join(" "),
        GUEST_RC_PREFIX
    );
    let mut ssh = ssh_base();
    ssh.arg(remote_cmd);
    let mut out = spawn_and_collect_timeout(&mut ssh, stdin, HOST_DEADLINE);
    match extract_guest_rc(&mut out) {
        Some(code) => out.code = code,
        None if out.code == 255 && out.timed_out => {
            out.stderr
                .extend_from_slice(b"[vm_exec: host-side timeout]");
        }
        None => {}
    }
    cleanup(&dir);
    out
}

/// Marker framing the echoed guest exit status (see `vm_exec_once`).
const GUEST_RC_PREFIX: &str = "__GUEST_RC__:";

/// Pull the echoed guest status out of stdout, strip the marker, and
/// hand back the guest's own exit code.
fn extract_guest_rc(out: &mut Outcome) -> Option<i32> {
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let at = text.rfind(GUEST_RC_PREFIX)?;
    // The status is the marker line's trailing digits (shell `echo`
    // adds a newline; the guest's own output precedes the marker).
    let tail = &text[at + GUEST_RC_PREFIX.len()..];
    let digits: String = tail
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '-')
        .collect();
    let code: i32 = digits.parse().ok()?;
    // Keep guest stdout pristine: everything before the marker is the
    // program's own output.
    let keep = text[..at].as_bytes().to_vec();
    out.stdout = keep;
    Some(code)
}

/// Copy a data file next to the staged exe (for stdin/file tests that
/// need remote fixtures). Returns the remote path.
pub fn vm_stage_file(local: &Path, name: &str) -> Result<String, String> {
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = format!("/tmp/rubyc_vmtest_{}_{}", std::process::id(), n);
    let mut mkdir = ssh_base();
    mkdir.arg(format!("mkdir -p {dir}"));
    let out = super::spawn_and_collect_timeout(&mut mkdir, None, HOST_DEADLINE);
    if out.code != 0 {
        return Err(format!("vm mkdir failed: {}", out.stderr_str()));
    }
    let remote = format!("{dir}/{name}");
    let mut scp = Command::new("scp");
    scp.args([
        "-o",
        "ConnectTimeout=15",
        "-o",
        "BatchMode=yes",
        &local.to_string_lossy(),
        &format!("rubyc-dev:{remote}"),
    ]);
    let out = super::spawn_and_collect_timeout(&mut scp, None, HOST_DEADLINE);
    if out.code != 0 {
        return Err(format!("scp failed: {}", out.stderr_str()));
    }
    Ok(remote)
}

/// Remove a remote staging dir created by [`vm_stage_file`].
pub fn vm_cleanup_file(remote_path: &str) {
    if let Some(dir) = std::path::Path::new(remote_path).parent() {
        cleanup(&dir.to_string_lossy());
    }
}
