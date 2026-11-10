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

/// The tests run on the component, which is what lands in `rk_build/debug/`:
/// the file `rk compile` writes, and no core module.
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
    let built = ws.path().join("rk_build").join("debug");
    let component = std::fs::read(built.join("Plant_A.wasm")).expect("the component");
    assert!(wasmparser::Parser::is_component(&component));
    assert!(!built.join("core.wasm").exists(), "and no core module");
    assert!(built.join("wit").join("world.wit").exists());
}
