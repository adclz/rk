//! `--output-format=json-lines` is honoured by every command: a tool reading
//! `rk` under it never meets prose. The diagnostics honoured it and every
//! status line around them dropped it.

use std::process::Command;

fn rk(args: &[&str], ws: &std::path::Path) -> std::process::Output {
    // An isolated home: the commands that read it must not meet this
    // machine's identities, and none of them may write anything.
    Command::new(env!("CARGO_BIN_EXE_rk"))
        .args(["--output-format", "json-lines", "--workspace"])
        .arg(ws)
        .args(args)
        .env("RK_STDLIB_PATH", "")
        .env("RK_HOME", ws.join("home"))
        .output()
        .expect("run rk")
}

/// Every non-empty line is a JSON object, or the test names the offender.
fn records(bytes: &[u8]) -> Vec<serde_json::Value> {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str::<serde_json::Value>(line)
                .unwrap_or_else(|e| panic!("not a record: {line:?} ({e})"))
        })
        .collect()
}

#[test]
fn status_commands_speak_json_lines() {
    let ws = tempfile::tempdir().unwrap();
    std::fs::write(ws.path().join("config.toml"), "[project]\nname = \"T\"\nversion = \"0.0\"\n").unwrap();

    // A table command.
    let out = rk(&["env"], ws.path());
    let rows = records(&out.stdout);
    assert!(
        rows.iter().any(|r| r["type"] == "env" && r["key"] == "workspace"),
        "env rows are records: {rows:?}"
    );
    records(&out.stderr);

    // A reference command.
    let out = rk(&["explain", "E0301"], ws.path());
    let rows = records(&out.stdout);
    assert_eq!(rows[0]["type"], "explain");
    assert_eq!(rows[0]["code"], "E0301");

    // A path command, and a table command with nothing to list.
    let out = rk(&["path"], ws.path());
    let rows = records(&out.stdout);
    assert_eq!(rows[0]["type"], "path", "{rows:?}");
    let out = rk(&["cert", "list"], ws.path());
    records(&out.stdout);
    records(&out.stderr);

    // A failing wire command: the error is a record too, and the exit says so.
    let out = rk(&["start", "--id", "no-such-runtime-ever"], ws.path());
    assert!(!out.status.success());
    let errors = records(&out.stderr);
    assert!(
        errors.iter().any(|r| r["type"] == "message" && r["level"] == "error"),
        "the failure is a record: {errors:?}"
    );
    records(&out.stdout);
}
