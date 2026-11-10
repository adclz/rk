// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! What `rk compile` writes: one component named after the project, and
//! the WIT that types it in `wit/` beside it.

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

fn workspace() -> tempfile::TempDir {
    let ws = tempfile::tempdir().unwrap();
    std::fs::write(
        ws.path().join("config.toml"),
        "[project]\nname = \"Plant_A\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    std::fs::write(
        ws.path().join("main.st"),
        r#"
PROGRAM P
VAR_EXTERNAL input : DINT; output : DINT; END_VAR
    output := input + 1;
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL
    input AT %ID0 : DINT;
    output AT %QD0 : DINT;
END_VAR
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM Main WITH T : P;
    END_RESOURCE
END_CONFIGURATION
"#,
    )
    .unwrap();
    ws
}

/// The build is one component, `<project>.wasm`, and no core module: the
/// world names the project's package and exports the control interface,
/// and the package it exports is copied under `deps/`.
#[test]
fn a_build_is_one_component_and_its_wit() {
    let ws = workspace();
    let out = rk(&["compile"], ws.path());
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let built = ws.path().join("rk_build").join("debug");
    let component = std::fs::read(built.join("Plant_A.wasm")).expect("the component");
    assert!(
        wasmparser::Parser::is_component(&component),
        "the build is a component"
    );
    assert!(!built.join("core.wasm").exists(), "and no core module");

    let world = std::fs::read_to_string(built.join("wit").join("world.wit")).unwrap();
    assert!(world.contains("package local:plant-a;"), "{world}");
    assert!(world.contains("export rk:plc/control@0.1.0;"), "{world}");
    let control = std::fs::read_to_string(
        built
            .join("wit")
            .join("deps")
            .join("rk-plc")
            .join("control.wit"),
    )
    .unwrap();
    assert!(control.contains("interface control {"), "{control}");
}

/// A rebuild writes each package folder whole: a file an earlier build left
/// in one goes, and a folder rk does not write stays.
#[test]
fn a_rebuild_replaces_the_package_folders_it_writes() {
    let ws = workspace();
    assert!(rk(&["compile"], ws.path()).status.success());
    let deps = ws
        .path()
        .join("rk_build")
        .join("debug")
        .join("wit")
        .join("deps");
    let stale = deps.join("rk-plc").join("stale.wit");
    std::fs::write(&stale, "package rk:plc@0.0.1;\n").unwrap();
    let own = deps.join("mine").join("mine.wit");
    std::fs::create_dir_all(own.parent().unwrap()).unwrap();
    std::fs::write(&own, "package my:own;\n").unwrap();

    let out = rk(&["compile"], ws.path());
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!stale.exists(), "the stale file went with its folder");
    assert!(deps.join("rk-plc").join("control.wit").exists());
    assert!(own.exists(), "a folder rk does not write stays");
}

/// `--output` places the component, and its WIT beside it.
#[test]
fn the_wit_goes_beside_an_output_given() {
    let ws = workspace();
    let output = ws.path().join("out").join("plc.wasm");
    let out = rk(
        &["compile", "--output", output.to_str().unwrap()],
        ws.path(),
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(wasmparser::Parser::is_component(
        &std::fs::read(&output).unwrap()
    ));
    assert!(ws.path().join("out").join("wit").join("world.wit").exists());
}

/// An interface of the host's own, named by an interface's name, is a
/// package rk writes beside rk's own, from the `{extern}` FUNCTIONs that
/// import it.
#[test]
fn the_host_s_own_package_is_written_beside_rk_s() {
    let ws = workspace();
    std::fs::write(
        ws.path().join("display.st"),
        r#"
{extern 'acme:display/text@1.0.0' 'show'}
FUNCTION Show : BOOL
VAR_INPUT line : STRING; END_VAR
END_FUNCTION
"#,
    )
    .unwrap();
    let out = rk(&["compile"], ws.path());
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let wit = ws.path().join("rk_build").join("debug").join("wit");
    let world = std::fs::read_to_string(wit.join("world.wit")).unwrap();
    assert!(world.contains("import acme:display/text@1.0.0;"), "{world}");
    let package =
        std::fs::read_to_string(wit.join("deps").join("acme-display").join("text.wit")).unwrap();
    assert!(
        package.contains("package acme:display@1.0.0;")
            && package.contains("show: func(line: string) -> bool;"),
        "{package}"
    );
}
