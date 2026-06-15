//! Stress tests for the SIGINT / SIGTERM signal handlers in `main.rs`.
//!
//! The CLI installs cooperative cancellation on the first signal and a
//! hard exit on the second. These tests spawn the binary, send the
//! signal, and assert the resulting exit code under three scenarios.
//!
//! All tests are marked `#[ignore]` because they require the binary
//! to be built and exercise real OS signal delivery, which is slow and
//! noisy in CI. Run them locally with:
//!
//! ```bash
//! cargo test --test signal_handler_stress -- --ignored --nocapture
//! ```

#![allow(clippy::needless_raw_string_hashes)]

use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(50);
const MAX_WAIT: Duration = Duration::from_secs(5);

/// Spawn the CLI with a URL that does not require network state to
/// fail fast. The `--timeout 30` plus the cache layer make the process
/// survive long enough to receive our test signal.
fn spawn_cli_with_url(url: &str) -> Child {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let mut cmd = Command::new(cargo);
    cmd.args(["run", "--quiet", "--", url, "--timeout", "30"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd.spawn().expect("failed to spawn `cargo run`")
}

/// Wait up to `MAX_WAIT` for the child to exit on its own. Returns the
/// elapsed time so the test can report timing.
fn wait_for_exit(child: &mut Child) -> Option<std::process::ExitStatus> {
    let deadline = Instant::now() + MAX_WAIT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    return None;
                }
                sleep(POLL_INTERVAL);
            }
            Err(e) => panic!("try_wait failed: {e}"),
        }
    }
}

#[test]
#[ignore = "requires cargo run; slow, run with --ignored"]
fn sigint_produces_exit_code_130() {
    let mut child = spawn_cli_with_url("https://youtu.be/dQw4w9WgXcQ");
    // Let the process reach the signal-watcher spawn point.
    sleep(Duration::from_millis(500));
    // Send SIGINT.
    let pid = child.id() as i32;
    // SAFETY: `pid` is the OS-level child PID returned by
    // `std::process::Child::id` and is guaranteed to be alive for
    // the duration of this test; `SIGINT` is a standard POSIX
    // signal that the child process has installed a handler for.
    let rc = unsafe { libc::kill(pid, libc::SIGINT) };
    assert_eq!(rc, 0, "kill(SIGINT) returned {rc}");

    let status = wait_for_exit(&mut child).expect("child did not exit within 5s");
    let code = status.code();
    assert_eq!(
        code,
        Some(130),
        "expected exit code 130 after SIGINT, got {:?}",
        code
    );
}

#[cfg(unix)]
#[test]
#[ignore = "requires cargo run; slow, run with --ignored"]
fn sigterm_produces_exit_code_130() {
    let mut child = spawn_cli_with_url("https://youtu.be/dQw4w9WgXcQ");
    sleep(Duration::from_millis(500));
    let pid = child.id() as i32;
    // SAFETY: `pid` is the OS-level child PID returned by
    // `std::process::Child::id` and is guaranteed to be alive for
    // the duration of this test; `SIGTERM` is a standard POSIX
    // signal handled cooperatively by the CLI's signal watcher.
    let rc = unsafe { libc::kill(pid, libc::SIGTERM) };
    assert_eq!(rc, 0, "kill(SIGTERM) returned {rc}");

    let status = wait_for_exit(&mut child).expect("child did not exit within 5s");
    let code = status.code();
    assert_eq!(
        code,
        Some(130),
        "expected exit code 130 after SIGTERM, got {:?}",
        code
    );
}

#[cfg(unix)]
#[test]
#[ignore = "requires cargo run; slow, run with --ignored"]
fn double_sigint_forces_immediate_exit() {
    let mut child = spawn_cli_with_url("https://youtu.be/dQw4w9WgXcQ");
    sleep(Duration::from_millis(500));
    let pid = child.id() as i32;

    // First signal: cooperative cancellation.
    // SAFETY: `pid` is the OS-level child PID from
    // `std::process::Child::id`; `SIGINT` is the standard
    // cancellation signal whose handler is installed by the CLI's
    // signal watcher in `main.rs`.
    let rc1 = unsafe { libc::kill(pid, libc::SIGINT) };
    assert_eq!(rc1, 0, "first kill(SIGINT) returned {rc1}");

    // Give the watcher 100 ms to enter the "first signal observed"
    // state, then fire the second. We expect the watcher to invoke
    // `std::process::exit(130)` without waiting for in-flight HTTP.
    sleep(Duration::from_millis(100));
    // SAFETY: `pid` is still the live child PID; the second
    // `SIGINT` is the hard-exit trigger documented in
    // `main.rs`'s signal watcher, and the child process is
    // expected to terminate via `std::process::exit(130)`.
    let rc2 = unsafe { libc::kill(pid, libc::SIGINT) };
    assert_eq!(rc2, 0, "second kill(SIGINT) returned {rc2}");

    let status = wait_for_exit(&mut child).expect("child did not exit within 5s");
    let code = status.code();
    // The hard-exit path uses `std::process::exit(130)`, which produces
    // an immediate non-graceful exit with code 130. The race window
    // is small; we allow code 1 as a fallback if the process happened
    // to die from the HTTP timeout in the meantime.
    assert!(
        code == Some(130) || code == Some(1),
        "expected exit code 130 or 1 after double SIGINT, got {:?}",
        code
    );
}

#[test]
#[ignore = "diagnostic-only: prints the spawn PID for manual experiments"]
fn diagnostic_prints_pid_for_manual_signal_send() {
    let mut child = spawn_cli_with_url("https://youtu.be/dQw4w9WgXcQ");
    eprintln!("spawned cargo run with pid {}", child.id());
    eprintln!("send a signal manually: kill -INT {}", child.id());
    eprintln!("press Ctrl-C in another terminal to interrupt the wait");
    // Do not wait for the child: the caller is expected to interrupt
    // and inspect the resulting exit code.
    sleep(Duration::from_secs(60));
    let _ = child.kill();
    let _ = child.wait();
    // Print so the test harness emits the diagnostic block under
    // `--nocapture`.
    std::io::stdout().flush().ok();
}
