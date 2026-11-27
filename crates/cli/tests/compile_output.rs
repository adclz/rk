// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! What `rk compile` writes: one component named after the project, which
//! carries the types of what it imports and exports.

use std::process::Command;

mod common;

use common::{Builds, tests_cache};

fn rk(args: &[&str], ws: &std::path::Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_rk"))
        .arg("--workspace")
        .arg(ws)
        .args(args)
        .env("RK_STDLIB_PATH", "")
        .env("RK_HOME", ws.join("home"))
        .env("RK_CACHE_DIR", tests_cache())
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

/// The world a host's tools read from a component's binary, as WIT text,
/// which is all the WIT a build gives: rk writes none beside it.
fn world_of(component: &[u8]) -> String {
    let wit_parser::decoding::DecodedWasm::Component(resolve, world) =
        wit_parser::decoding::decode(component).expect("the binary's types")
    else {
        panic!("a component, not a WIT package");
    };
    let package = resolve.worlds[world].package.expect("the world's package");
    let mut printer = wit_component::WitPrinter::default();
    printer.print(&resolve, package, &[]).expect("printed");
    printer.output.to_string()
}

/// The build is one component, `<project>.wasm`, with no core module and
/// no WIT beside it: the binary carries the world, which exports the
/// control interface.
#[test]
fn a_build_is_one_component() {
    let ws = workspace();
    let builds = Builds::of(ws.path());
    let out = rk(&["compile"], ws.path());
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let built = builds.dir().join("debug");
    let component = std::fs::read(built.join("Plant_A.wasm")).expect("the component");
    assert!(
        wasmparser::Parser::is_component(&component),
        "the build is a component"
    );
    assert!(!built.join("core.wasm").exists(), "and no core module");
    assert!(!built.join("wit").exists(), "and no WIT beside it");
    let world = world_of(&component);
    assert!(world.contains("export rk:plc/control@0.1.0;"), "{world}");
}

/// A build writes nothing in the workspace: the component lands in the
/// workspace's build folder in the cache, which `rk env build` prints, and
/// whose `workspace` file names the workspace. `--output` puts it where it
/// says, and then nothing goes in the cache.
#[test]
fn a_build_writes_nothing_in_the_workspace() {
    let ws = workspace();
    let builds = Builds::of(ws.path());
    let held = |dir: &std::path::Path| {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    };
    let out = rk(&["compile"], ws.path());
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(held(ws.path()), ["config.toml", "main.st"]);
    assert!(builds.dir().join("debug").join("Plant_A.wasm").is_file());
    assert_eq!(
        std::fs::read_to_string(builds.dir().join("workspace")).unwrap(),
        format!("{}\n", ws.path().canonicalize().unwrap().display())
    );
    let env = rk(&["env", "build"], ws.path());
    assert_eq!(
        String::from_utf8_lossy(&env.stdout).trim_end(),
        builds.dir().display().to_string()
    );

    std::fs::remove_dir_all(builds.dir()).unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let output = elsewhere.path().join("plant.wasm");
    let out = rk(
        &["compile", "--output", output.to_str().unwrap()],
        ws.path(),
    );
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(output.is_file());
    assert!(!builds.dir().exists(), "nothing in the cache");
    assert_eq!(held(ws.path()), ["config.toml", "main.st"]);
}

/// A library is built into the programs that list it, never alone: `rk
/// compile` refuses its folder and writes nothing, where `rk check` and `rk
/// test` take it.
#[test]
fn a_library_is_not_compiled_alone() {
    let ws = tempfile::tempdir().unwrap();
    std::fs::write(
        ws.path().join("config.toml"),
        "[project]\nname = \"drives\"\nversion = \"0.1.0\"\ntype = \"library\"\n",
    )
    .unwrap();
    std::fs::write(
        ws.path().join("Axis.st"),
        "FUNCTION Twice : INT\nVAR_INPUT n : INT; END_VAR\n    Twice := n * 2;\nEND_FUNCTION\n\n\
         {test}\nFUNCTION test_twice\n    IF Twice(2) <> 4 THEN __RAISE('twice'); END_IF;\nEND_FUNCTION\n",
    )
    .unwrap();
    let builds = Builds::of(ws.path());
    let out = rk(&["compile"], ws.path());
    assert!(!out.status.success(), "a library compiled alone");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains(
            "this workspace is a library (`type = \"library\"`): `rk check` and `rk test` take it, and a program that lists it in its `[dependencies]` builds it"
        ),
        "{stderr}"
    );
    assert!(!builds.dir().exists(), "nothing written");
    for command in ["check", "test"] {
        let out = rk(&[command], ws.path());
        assert!(
            out.status.success(),
            "rk {command}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

/// `rk check` in a program reports its libraries' errors, as `rk compile`
/// does, and its summary says how many are theirs.
#[test]
fn rk_check_counts_a_library_s_errors() {
    let root = tempfile::tempdir().unwrap();
    let (ws, motion) = (root.path().join("ws"), root.path().join("motion"));
    for (path, text) in [
        (
            ws.join("config.toml"),
            "[project]\nname = \"ws\"\nversion = \"0.1.0\"\n\n[dependencies]\nmotion = { path = \"../motion\" }\n",
        ),
        (
            ws.join("main.st"),
            "FUNCTION Run : INT\n    Run := 1;\nEND_FUNCTION\n",
        ),
        (
            motion.join("config.toml"),
            "[project]\nname = \"motion\"\nversion = \"0.1.0\"\ntype = \"library\"\n",
        ),
        (
            motion.join("Motion.st"),
            "FUNCTION Twice : INT\n    Twice := Nope();\nEND_FUNCTION\n",
        ),
    ] {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    let out = rk(&["check"], &ws);
    assert!(!out.status.success(), "a library's error passed the check");
    // Without the terminal's colors.
    let plain = |bytes: &[u8]| {
        let text = String::from_utf8_lossy(bytes).into_owned();
        let mut out = String::new();
        let mut chars = text.chars();
        while let Some(c) = chars.next() {
            if c == '\u{1b}' {
                chars.by_ref().find(|c| *c == 'm');
            } else {
                out.push(c);
            }
        }
        out
    };
    let stdout = plain(&out.stdout);
    assert!(
        stdout.contains("1 error(s) (1 in library files), 0 warning(s) found."),
        "{stdout}"
    );
    let stderr = plain(&out.stderr);
    assert!(stderr.contains("E0201"), "{stderr}");
}

/// `--output` places the component, and nothing beside it.
#[test]
fn an_output_given_holds_the_component() {
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
    assert!(!ws.path().join("out").join("wit").exists());
}

/// An interface of a package is typed by the package's WIT, which rk does
/// not write: the build is refused until a library ships it in its `wit/`,
/// and then the component imports it. The program carries no WIT: it calls
/// the host's interfaces through the library, and its own `wit/` folder is
/// refused.
#[test]
fn a_host_s_package_comes_through_a_library() {
    let ws = workspace();
    let root = ws.path();
    let builds = Builds::of(root);
    std::fs::write(
        root.join("config.toml"),
        "[project]\nname = \"Plant_A\"\nversion = \"0.1.0\"\n\n[dependencies]\n\
         display = { path = \"display\" }\n",
    )
    .unwrap();
    let library = root.join("display");
    std::fs::create_dir_all(&library).unwrap();
    std::fs::write(
        library.join("config.toml"),
        "[project]\nname = \"display\"\nversion = \"1.0.0\"\ntype = \"library\"\n",
    )
    .unwrap();
    std::fs::write(
        library.join("Text.st"),
        "{extern 'acme:display/text@1.0.0' 'show'}\nFUNCTION Show : BOOL\n\
         VAR_INPUT line : STRING; END_VAR\nEND_FUNCTION\n",
    )
    .unwrap();
    std::fs::write(
        root.join("ping.st"),
        "{export}\nFUNCTION Ping : BOOL\n    Ping := Display.Show(line := 'ping');\nEND_FUNCTION\n",
    )
    .unwrap();
    let refused = |out: std::process::Output| {
        assert!(!out.status.success(), "built");
        String::from_utf8_lossy(&out.stderr).into_owned()
    };
    let stderr = refused(rk(&["compile"], root));
    assert!(
        stderr.contains(
            "the module imports `acme:display/text@1.0.0`, which no library's `wit/` \
             describes, and no library's component exports"
        ),
        "{stderr}"
    );

    let package = "package acme:display@1.0.0;\n\ninterface text {\n    \
                   show: func(line: string) -> bool;\n}\n";
    std::fs::create_dir_all(root.join("wit")).unwrap();
    std::fs::write(root.join("wit").join("display.wit"), package).unwrap();
    let stderr = refused(rk(&["compile"], root));
    assert!(
        stderr.contains("this program carries WIT text in `wit/`"),
        "{stderr}"
    );
    std::fs::remove_dir_all(root.join("wit")).unwrap();

    std::fs::create_dir_all(library.join("wit")).unwrap();
    std::fs::write(library.join("wit").join("display.wit"), package).unwrap();
    let out = rk(&["compile"], root);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let component = std::fs::read(builds.dir().join("debug").join("Plant_A.wasm")).unwrap();
    let world = world_of(&component);
    assert!(world.contains("import acme:display/text@1.0.0;"), "{world}");
    assert!(
        !builds.dir().join("debug").join("components").exists(),
        "nothing beside: the host gives the interface"
    );
}

/// A program calls a component through a library: one with a `component/`
/// folder of its own is refused by the check and by the build, and nothing
/// is built.
#[test]
fn a_program_carries_no_component() {
    let ws = workspace();
    let root = ws.path();
    let builds = Builds::of(root);
    std::fs::create_dir_all(root.join("component")).unwrap();
    std::fs::write(root.join("component").join("drive.wasm"), b"\0asm").unwrap();
    for command in ["check", "compile"] {
        let out = rk(&[command], root);
        assert!(!out.status.success(), "rk {command} let it through");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("this program carries a component in `component/`"),
            "{stderr}"
        );
    }
    assert!(!builds.dir().exists(), "nothing built");
}

/// The `{export}` FUNCTIONs are in an interface named after the project,
/// in the component's world.
#[test]
fn the_exported_functions_are_in_the_project_s_interface() {
    let ws = workspace();
    std::fs::write(
        ws.path().join("api.st"),
        r#"
NAMESPACE Recipe
    {export}
    FUNCTION Set_Name : BOOL
    VAR_INPUT name : STRING[32]; END_VAR
        Set_Name := name <> '';
    END_FUNCTION
END_NAMESPACE
"#,
    )
    .unwrap();
    let builds = Builds::of(ws.path());
    for args in [&["compile"][..], &["compile", "--release"][..]] {
        let out = rk(args, ws.path());
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    for profile in ["debug", "release"] {
        let component = std::fs::read(builds.dir().join(profile).join("Plant_A.wasm")).unwrap();
        let world = world_of(&component);
        assert!(
            world.contains("export plant-a: interface {")
                && world.contains("recipe-set-name: func(name: string) -> result<bool, string>;"),
            "{profile}: {world}"
        );
    }
}
