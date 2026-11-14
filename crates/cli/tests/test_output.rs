// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! What `rk test` writes: the component it ran, unoptimized, named after the
//! project, with the WIT that types it.

use std::process::Command;

fn rk(args: &[&str], ws: &std::path::Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rk"))
        .arg("--workspace")
        .arg(ws)
        .args(args)
        .env("RK_STDLIB_PATH", "")
        .env("RK_HOME", ws.join("home"))
        .output()
        .expect("run rk")
}

/// The tests run on the component, which lands in `rk_build/test/` with no
/// core module: the debug build, which also tells its host where each raise
/// is, so no other command writes it.
#[test]
fn rk_test_writes_the_component_it_ran() {
    let ws = tempfile::tempdir().unwrap();
    std::fs::write(
        ws.path().join("config.toml"),
        "[project]\nname = \"Plant_A\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    std::fs::write(
        ws.path().join("main.st"),
        r#"
{test}
FUNCTION test_passes
END_FUNCTION
"#,
    )
    .unwrap();
    let out = rk(&["test"], ws.path());
    assert!(
        out.status.success(),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let built = ws.path().join("rk_build").join("test");
    let component = std::fs::read(built.join("Plant_A.wasm")).expect("the component");
    assert!(wasmparser::Parser::is_component(&component));
    assert!(!built.join("core.wasm").exists(), "and no core module");
    let world = std::fs::read_to_string(built.join("wit").join("world.wit")).unwrap();
    assert!(world.contains("import rk:test/raise@0.1.0;"), "{world}");
    assert!(
        !ws.path().join("rk_build").join("debug").exists(),
        "what `rk compile` writes is left alone"
    );
}

/// The declarations `Std.Console` reaches WASI through, as the stdlib has
/// them, for a workspace that has no standard library.
const CONSOLE_WASI: &[(&str, &str)] = &[
    (
        "wasi-cli-types.st",
        include_str!("../../../stdlib/generated/wasi-cli-types.st"),
    ),
    (
        "wasi-cli-stdout.st",
        include_str!("../../../stdlib/generated/wasi-cli-stdout.st"),
    ),
    (
        "wasi-cli-stderr.st",
        include_str!("../../../stdlib/generated/wasi-cli-stderr.st"),
    ),
];

/// `Std.Console`, for a workspace that has no standard library: the blocks
/// a test writes with, as the stdlib has them, without its own tests, which
/// read `Std.Unit`. It reaches WASI through [`CONSOLE_WASI`].
fn console() -> String {
    let source = include_str!("../../../stdlib/Console.st");
    let tests = "\tNAMESPACE Test\n";
    let end = "\tEND_NAMESPACE\n";
    let start = source.find(tests).expect("Console's tests");
    let stop = source.rfind(end).expect("the end of Console's tests") + end.len();
    format!("{}{}", &source[..start], &source[stop..])
}

/// What a failed test wrote is under its failure in the report, on its
/// line in the concise format, and the record's `output` in `json-lines`.
/// A test that passes shows none.
#[test]
fn rk_test_shows_what_a_failed_test_wrote() {
    let ws = tempfile::tempdir().unwrap();
    std::fs::write(
        ws.path().join("config.toml"),
        "[project]\nname = \"plant\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    std::fs::write(ws.path().join("console.st"), console()).unwrap();
    for (name, text) in CONSOLE_WASI {
        std::fs::write(ws.path().join(name), text).unwrap();
    }
    std::fs::write(
        ws.path().join("main.st"),
        r#"
USING Std.Console;

CONFIGURATION Plant
VAR_GLOBAL out : STDOUT; err : STDERR; END_VAR
END_CONFIGURATION

{test}
FUNCTION test_pressure
VAR_EXTERNAL out : STDOUT; err : STDERR; END_VAR
    out(REQ := TRUE, IN := 'reading the sensor$N');
    err(REQ := TRUE, IN := 'pressure is high$N');
    __RAISE('pressure over 3 bar');
END_FUNCTION

{test}
FUNCTION test_quiet
VAR_EXTERNAL out : STDOUT; END_VAR
    out(REQ := TRUE, IN := 'not shown$N');
END_FUNCTION
"#,
    )
    .unwrap();

    let out = rk(&["test"], ws.path());
    assert!(!out.status.success(), "a test fails");
    let full = String::from_utf8_lossy(&out.stdout);
    assert!(
        full.contains("             output:\n               reading the sensor\n               pressure is high\n"),
        "{full}"
    );
    assert!(!full.contains("not shown"), "{full}");

    let out = rk(&["--output-format", "concise", "test"], ws.path());
    let concise = String::from_utf8_lossy(&out.stdout);
    assert!(
        concise.contains(r#" [output: "reading the sensor\npressure is high\n"]"#),
        "{concise}"
    );

    let out = rk(&["--output-format", "json-lines", "test"], ws.path());
    let lines = String::from_utf8_lossy(&out.stdout);
    assert!(
        lines.contains(r#""output":"reading the sensor\npressure is high\n""#),
        "{lines}"
    );
}
