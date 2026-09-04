//! A reader that closes early is a normal thing on a pipeline, not a crash.

#![cfg(unix)]

use std::io::Write;
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};

/// `rk check | head -1` used to exit 101: Rust ignores SIGPIPE, so the first
/// write after the reader had gone was a panic. Every agent's `| head` or
/// `| grep -m1` read a compiler crash. Dying of the signal (13) is what every
/// other tool on the pipeline does, and what a shell reports as 141.
#[test]
fn a_closed_pipe_ends_the_process_with_sigpipe_not_a_panic() {
    let ws = tempfile::tempdir().unwrap();
    std::fs::write(ws.path().join("config.toml"), "[project]\nname = \"T\"\nversion = \"0.0\"\n").unwrap();
    // Enough diagnostics that output outlives a reader who took one line.
    let mut src = std::fs::File::create(ws.path().join("main.st")).unwrap();
    writeln!(src, "FUNCTION f : INT").unwrap();
    for i in 0..60 {
        writeln!(src, "    f := undefined_{i};").unwrap();
    }
    writeln!(src, "END_FUNCTION").unwrap();
    drop(src);

    let mut child = Command::new(env!("CARGO_BIN_EXE_rk"))
        .arg("check")
        .arg("--workspace")
        .arg(ws.path())
        .env("RK_STDLIB_PATH", "")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rk");
    // The reader leaves before reading anything: every write meets EPIPE.
    drop(child.stdout.take());
    drop(child.stderr.take());
    let status = child.wait().expect("wait");

    assert_ne!(status.code(), Some(101), "a closed pipe must not be a panic: {status:?}");
    assert_eq!(status.signal(), Some(libc::SIGPIPE), "dies of the signal: {status:?}");
}
