// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A library's component runs beside the program, in a store of its own:
//! `rk compile` types the program's imports of its interfaces from the
//! component's binary, and writes the component into `components/` beside
//! the program, where the runtime links each import to the one that exports
//! it.

use std::path::{Path, PathBuf};

use crate::tests::utils::{component_of_wat, component_of_world, package_binary};

/// A component that exports an axis.
const AXIS: &str = r#"
    package acme:drive@1.0.0;

    interface axis {
        position: func() -> s32;
    }

    world drive {
        export axis;
    }
"#;

/// The same interface, with a function more: another component.
const AXIS_WITH_SPEED: &str = r#"
    package acme:drive@1.0.0;

    interface axis {
        position: func() -> s32;
        speed: func() -> s32;
    }

    world drive {
        export axis;
    }
"#;

/// A component whose interface gives a resource's handle.
const FILES: &str = r#"
    package acme:files@1.0.0;

    interface files {
        resource file {
            size: func() -> u32;
        }
        open: func() -> file;
    }

    world storage {
        export files;
    }
"#;

/// The declarations of `axis` a library writes for its programs.
const POSITION: &str = "NAMESPACE Axis
    {extern 'acme:drive/axis@1.0.0' 'position'}
    FUNCTION POSITION : DINT
    END_FUNCTION
END_NAMESPACE
";

/// A program that calls `call` from an exported FUNCTION, whose
/// `[dependencies]` are `dependencies`.
fn program(root: &Path, dependencies: &str, call: &str) -> PathBuf {
    write(
        root,
        &[
            (
                "ws/config.toml",
                &format!(
                    "[project]\nname = \"plant\"\nversion = \"1\"\n\n[dependencies]\n{dependencies}"
                ),
            ),
            (
                "ws/main.st",
                &format!("{{export}}\nFUNCTION Run : DINT\n    Run := {call};\nEND_FUNCTION\n"),
            ),
        ],
    );
    root.join("ws")
}

/// A library `name` in `root/<name>`, with `declarations` and the component
/// `component`, when it carries one.
fn library(root: &Path, name: &str, declarations: &str, component: Option<&[u8]>) {
    write(
        root,
        &[
            (
                &format!("{name}/config.toml"),
                &format!("[project]\nname = \"{name}\"\nversion = \"1\"\ntype = \"library\"\n"),
            ),
            (&format!("{name}/Declarations.st"), declarations),
        ],
    );
    if let Some(bytes) = component {
        std::fs::create_dir_all(root.join(name).join("component")).unwrap();
        std::fs::write(root.join(name).join("component").join("c.wasm"), bytes).unwrap();
    }
}

fn write(root: &Path, files: &[(&str, &str)]) {
    for (path, text) in files {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
}

/// `rk compile` of the workspace `ws` into `ws/out/plant.wasm`.
fn compiled(ws: &Path) -> rk::error::CliResult<()> {
    rk::run(
        ws,
        false,
        rk::cli::OutputFormat::Concise,
        rk::cli::Command::Compile {
            output: Some(ws.join("out").join("plant.wasm")),
            release: false,
            opt_level: None,
            debug: false,
            watch: false,
        },
    )
}

/// The `.wasm` files of `ws/out/components/`, by name.
fn beside(ws: &Path) -> Vec<String> {
    let folder = ws.join("out").join("components");
    let mut names: Vec<String> = match std::fs::read_dir(&folder) {
        Ok(entries) => entries
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect(),
        Err(_) => Vec::new(),
    };
    names.sort();
    names
}

/// What the program built in `ws` imports, by interface.
fn imported(ws: &Path) -> Vec<String> {
    let bytes = std::fs::read(ws.join("out").join("plant.wasm")).expect("the program");
    let wit_parser::decoding::DecodedWasm::Component(resolve, world) =
        wit_parser::decoding::decode(&bytes).expect("the binary's types")
    else {
        panic!("a component");
    };
    let mut names: Vec<String> = resolve.worlds[world]
        .imports
        .values()
        .filter_map(|item| match item {
            wit_parser::WorldItem::Interface { id, .. } => resolve.id_of(*id),
            _ => None,
        })
        .collect();
    names.sort();
    names
}

/// The program imports what the library's component exports, typed from
/// the component's binary, and the component goes beside it, named after
/// its library, byte for byte.
#[test]
fn a_library_s_component_runs_beside_the_program() {
    super::disable_stdlib();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let axis = component_of_world(AXIS, "drive");
    library(&root, "drives", POSITION, Some(&axis));
    let ws = program(
        &root,
        "drives = { path = \"../drives\" }\n",
        "Drives.Axis.POSITION()",
    );
    compiled(&ws).expect("it builds");
    assert_eq!(imported(&ws), ["acme:drive/axis@1.0.0"]);
    assert_eq!(beside(&ws), ["drives.wasm"]);
    assert_eq!(
        std::fs::read(ws.join("out").join("components").join("drives.wasm")).unwrap(),
        axis
    );
}

/// Two libraries' components, each its own interface, run beside the
/// program together: their binaries' types merge into the program's world.
#[test]
fn two_components_run_beside_the_program() {
    super::disable_stdlib();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    library(
        &root,
        "drives",
        POSITION,
        Some(&component_of_world(AXIS, "drive")),
    );
    let conveyor = r#"
        package acme:conveyor@1.0.0;
        interface belt {
            length: func() -> s32;
        }
        world conveyor {
            export belt;
        }
    "#;
    library(
        &root,
        "belts",
        "{extern 'acme:conveyor/belt@1.0.0' 'length'}\nFUNCTION LENGTH : DINT\nEND_FUNCTION\n",
        Some(&component_of_world(conveyor, "conveyor")),
    );
    let ws = program(
        &root,
        "drives = { path = \"../drives\" }\nbelts = { path = \"../belts\" }\n",
        "Drives.Axis.POSITION() + Belts.LENGTH()",
    );
    compiled(&ws).expect("it builds");
    assert_eq!(
        imported(&ws),
        ["acme:conveyor/belt@1.0.0", "acme:drive/axis@1.0.0"]
    );
    assert_eq!(beside(&ws), ["belts.wasm", "drives.wasm"]);
}

/// A component the program does not call is not written beside it, and
/// one a build left there before is removed: the runtime runs what is
/// there.
#[test]
fn a_component_the_program_does_not_call_stays_out() {
    super::disable_stdlib();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    library(
        &root,
        "drives",
        POSITION,
        Some(&component_of_world(AXIS, "drive")),
    );
    let ws = program(&root, "drives = { path = \"../drives\" }\n", "1");
    write(&ws, &[("out/components/stale.wasm", "an old build's")]);
    compiled(&ws).expect("it builds");
    assert_eq!(imported(&ws), [] as [&str; 0]);
    assert_eq!(beside(&ws), [] as [&str; 0]);
}

/// Two components that export one interface are refused: the runtime links
/// an import to one. The same component, carried by two libraries, is one,
/// written once.
#[test]
fn one_interface_has_one_component() {
    super::disable_stdlib();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let axis = component_of_world(AXIS, "drive");
    library(&root, "a", POSITION, Some(&axis));
    library(&root, "b", "", Some(&axis));
    let ws = program(
        &root,
        "a = { path = \"../a\" }\nb = { path = \"../b\" }\n",
        "A.Axis.POSITION()",
    );
    compiled(&ws).expect("one component, carried twice");
    assert_eq!(beside(&ws), ["a.wasm"]);

    library(
        &root,
        "b",
        "",
        Some(&component_of_world(AXIS_WITH_SPEED, "drive")),
    );
    let refused = compiled(&ws).expect_err("two components export the interface");
    assert!(
        format!("{refused:?}").contains(
            "the components of 'a' and of 'b' both export `acme:drive/axis@1.0.0`: the runtime \
             links an import to one component"
        ),
        "{refused:?}"
    );
}

/// A component runs in a store of its own: what crosses to it is plain
/// values only, for now, and a function that gives a resource's handle is
/// refused.
#[test]
fn a_component_s_interface_passes_plain_values_only() {
    super::disable_stdlib();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    library(
        &root,
        "files",
        "{extern 'acme:files/files@1.0.0' 'open'}\nFUNCTION OPEN : UDINT\nEND_FUNCTION\n",
        Some(&component_of_world(FILES, "storage")),
    );
    let ws = program(&root, "files = { path = \"../files\" }\n", "0");
    // The handle is a UDINT: the FUNCTION returns it as one.
    write(
        &ws,
        &[(
            "main.st",
            "{export}\nFUNCTION Run : UDINT\n    Run := Files.OPEN();\nEND_FUNCTION\n",
        )],
    );
    let refused = compiled(&ws).expect_err("a resource crosses");
    assert!(
        format!("{refused:?}").contains(
            "`files.OPEN` imports `open` of `acme:files/files@1.0.0`, which the component of \
             'files' exports, and it passes a resource's handle: a component beside the program \
             passes plain values only, for now"
        ),
        "{refused:?}"
    );
}

/// A component that computes: an axis at 42, a text sent back as it came,
/// the time WASI's monotonic clock gives it, and a reading of a bus no host
/// gives a test.
const MACHINE: &str = r#"
    package acme:drive@1.0.0;

    interface axis {
        position: func() -> s32;
        echo: func(text: string) -> string;
        uptime: func() -> u64;
        stuck: func() -> s32;
    }

    interface bus {
        read: func() -> u32;
    }

    world drive {
        import bus;
        import wasi:clocks/monotonic-clock@0.3.0;
        export axis;
    }
"#;

/// [`MACHINE`]'s module: `echo` gives back the bytes it was given, which
/// the call placed in its memory.
const MACHINE_WAT: &str = r#"
    (module
        (import "acme:drive/bus@1.0.0" "read" (func $read (result i32)))
        (import "wasi:clocks/monotonic-clock@0.3.0" "now" (func $now (result i64)))
        (memory (export "memory") 1)
        (global $next (mut i32) (i32.const 1024))
        (func $realloc (export "cabi_realloc") (param i32 i32 i32 i32) (result i32)
            (local $at i32)
            global.get $next
            local.get 2 i32.const 1 i32.sub i32.add
            local.get 2 i32.const 1 i32.sub i32.const -1 i32.xor i32.and
            local.tee $at
            local.get 3 i32.add
            global.set $next
            local.get $at)
        (func (export "acme:drive/axis@1.0.0#position") (result i32)
            i32.const 42)
        (func (export "acme:drive/axis@1.0.0#echo") (param $text i32) (param $len i32) (result i32)
            (local $result i32)
            i32.const 0 i32.const 0 i32.const 4 i32.const 8
            call $realloc
            local.tee $result
            local.get $text
            i32.store
            local.get $result
            local.get $len
            i32.store offset=4
            local.get $result)
        (func (export "cabi_post_acme:drive/axis@1.0.0#echo") (param i32)
            i32.const 1024
            global.set $next)
        (func (export "acme:drive/axis@1.0.0#uptime") (result i64)
            call $now)
        (func (export "acme:drive/axis@1.0.0#stuck") (result i32)
            call $read))
"#;

/// The tests of a program that calls [`MACHINE`]'s axis.
const MACHINE_TESTS: &str = "
{test}
FUNCTION test_position
    IF Acme.Drive.Axis.POSITION() <> 42 THEN
        __RAISE('the axis is not at 42');
    END_IF;
END_FUNCTION

{test}
FUNCTION test_echo
VAR back : STRING; END_VAR
    Acme.Drive.Axis.ECHO(text := 'hello', value := back);
    IF back <> 'hello' THEN
        __RAISE(back);
    END_IF;
END_FUNCTION

{test}
FUNCTION test_uptime
    IF Acme.Drive.Axis.UPTIME() = 0 THEN
        __RAISE('the clock stands at 0');
    END_IF;
END_FUNCTION

{test}
FUNCTION test_stuck
    IF Acme.Drive.Axis.STUCK() = 0 THEN
        __RAISE('nothing was read');
    END_IF;
END_FUNCTION

{test}
FUNCTION_BLOCK test_advance
VAR_OUTPUT DONE : BOOL; END_VAR
VAR belt : Acme.Conveyor.Belt.ADVANCE; END_VAR
    belt(steps := 4);
    IF belt.DONE THEN
        IF belt.value <> 40 THEN
            __RAISE('the belt did not advance by 40');
        END_IF;
        DONE := TRUE;
    END_IF;
END_FUNCTION_BLOCK
";

/// A component whose function is async, lifted as such: it returns what
/// the call asked for through `task.return`, ten times the steps.
const CONVEYOR: &str = r#"
    package acme:conveyor@1.0.0;

    interface belt {
        advance: async func(steps: s32) -> s32;
    }

    world conveyor {
        export belt;
    }
"#;

const CONVEYOR_WAT: &str = r#"
    (module
        (import "[export]acme:conveyor/belt@1.0.0" "[task-return]advance"
            (func $return (param i32)))
        (memory (export "memory") 1)
        (func (export "[async-lift]acme:conveyor/belt@1.0.0#advance")
            (param $steps i32) (result i32)
            local.get $steps
            i32.const 10
            i32.mul
            call $return
            ;; EXIT: the call is over.
            i32.const 0)
        (func (export "[callback][async-lift]acme:conveyor/belt@1.0.0#advance")
            (param i32 i32 i32) (result i32)
            i32.const 0))
"#;

/// What rk-bindgen writes for every program from what `component` exports,
/// in one file.
fn declarations(component: &[u8]) -> String {
    let (resolve, world) = rk_bindgen::component_exports(component).expect("the component reads");
    rk_bindgen::generate_reaching(&resolve, world, rk_bindgen::Reach::Everyone)
        .expect("its exports are written")
        .into_iter()
        .map(|file| file.text)
        .collect()
}

/// `rk test` of the workspace `ws`, of the tests whose name holds `only`
/// when it is given.
fn tested(ws: &Path, only: Option<&str>) -> rk::error::CliResult<()> {
    rk::run(
        ws,
        false,
        rk::cli::OutputFormat::Concise,
        rk::cli::Command::Test {
            test_name: only.map(str::to_string),
            opt_level: None,
            timeout: None,
        },
    )
}

/// `rk test` runs the program's calls into the libraries' components, as a
/// runtime runs them: each component in a store of its own, beside the
/// program, with the program's WASI 0.3. A number and a string cross both
/// ways, and an async call goes to the component that exports it. A trap in
/// a component fails the test that called it, named after its library, at
/// the program's call.
#[test]
fn rk_test_runs_the_program_s_calls_into_a_component() {
    super::disable_stdlib();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let machine = component_of_wat(MACHINE, "drive", MACHINE_WAT);
    library(&root, "drives", &declarations(&machine), Some(&machine));
    let conveyor = component_of_wat(CONVEYOR, "conveyor", CONVEYOR_WAT);
    library(&root, "belts", &declarations(&conveyor), Some(&conveyor));
    let ws = program(
        &root,
        "drives = { path = \"../drives\" }\nbelts = { path = \"../belts\" }\n",
        "0",
    );
    write(&ws, &[("main.st", MACHINE_TESTS)]);
    tested(&ws, Some("test_echo")).expect("the string comes back");
    assert!(tested(&ws, None).is_err(), "test_stuck fails");

    // What ran: the program, and the components beside it, byte for byte.
    let built = ws.join("rk_build").join("test");
    assert_eq!(
        std::fs::read(built.join("components").join("drives.wasm")).unwrap(),
        machine
    );
    assert_eq!(
        std::fs::read(built.join("components").join("belts.wasm")).unwrap(),
        conveyor
    );
    let beside = rk::test_runner::beside_of(&built.join("plant.wasm")).unwrap();
    let records = rk::test_host::run_each(
        &std::fs::read(built.join("plant.wasm")).unwrap(),
        &beside,
        None,
        None,
        |_| {},
    )
    .expect("the tests run");
    let outcomes: Vec<(&str, Option<&str>)> = records
        .iter()
        .map(|record| (record.name.as_str(), record.reason.as_deref()))
        .collect();
    assert_eq!(
        outcomes,
        [
            ("test_advance", None),
            ("test_echo", None),
            ("test_position", None),
            (
                "test_stuck",
                Some(
                    "trapped in the component of 'drives': unknown import: \
                     `acme:drive/bus@1.0.0#read` has not been defined"
                )
            ),
            ("test_uptime", None),
        ]
    );
    // The program's call, not the component's insides.
    let stuck = &records[3].backtrace;
    assert_eq!(
        stuck
            .iter()
            .map(|frame| (frame.function.as_str(), frame.line))
            .collect::<Vec<_>>(),
        [("test_stuck", Some(27))]
    );
}

/// A component that does not start, its memory too small for its data,
/// fails each test, named after its library: none runs without it.
#[test]
fn a_component_that_does_not_start_fails_each_test() {
    super::disable_stdlib();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let broken = component_of_wat(
        AXIS,
        "drive",
        r#"
        (module
            (memory (export "memory") 1)
            (data (i32.const 70000) "past the end")
            (func (export "acme:drive/axis@1.0.0#position") (result i32)
                i32.const 1))
        "#,
    );
    library(&root, "drives", POSITION, Some(&broken));
    let ws = program(&root, "drives = { path = \"../drives\" }\n", "0");
    write(
        &ws,
        &[(
            "main.st",
            "{test}\nFUNCTION test_position\n    IF Drives.Axis.POSITION() <> 1 THEN\n        \
             __RAISE('the axis moved');\n    END_IF;\nEND_FUNCTION\n",
        )],
    );
    assert!(tested(&ws, None).is_err(), "the test fails");
    let built = ws.join("rk_build").join("test");
    let records = rk::test_host::run_each(
        &std::fs::read(built.join("plant.wasm")).unwrap(),
        &rk::test_runner::beside_of(&built.join("plant.wasm")).unwrap(),
        None,
        None,
        |_| {},
    )
    .expect("the tests run");
    let outcomes: Vec<(&str, Option<&str>)> = records
        .iter()
        .map(|record| (record.name.as_str(), record.reason.as_deref()))
        .collect();
    assert_eq!(
        outcomes,
        [(
            "test_position",
            Some("the component of 'drives' did not start: out of bounds memory access")
        )]
    );
}

/// A library's own tests, in its own folder, call what its component
/// exports as a program does: `rk test` runs the component beside them.
#[test]
fn a_library_s_own_tests_run_its_component() {
    super::disable_stdlib();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let machine = component_of_wat(MACHINE, "drive", MACHINE_WAT);
    library(&root, "drives", &declarations(&machine), Some(&machine));
    write(
        &root,
        &[(
            "drives/Tests.st",
            "{test}\nFUNCTION test_position\n    IF Acme.Drive.Axis.POSITION() <> 42 THEN\n        \
             __RAISE('the axis is not at 42');\n    END_IF;\nEND_FUNCTION\n",
        )],
    );
    let drives = root.join("drives");
    tested(&drives, None).expect("the library's test runs its component");
    assert!(
        drives
            .join("rk_build")
            .join("test")
            .join("components")
            .join("drives.wasm")
            .is_file()
    );
}

/// An interface is a component's, beside the program, or the host's, not
/// both: a component that exports one that another library's `wit/`
/// declares is refused.
#[test]
fn an_interface_is_a_component_s_or_the_host_s() {
    super::disable_stdlib();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    library(
        &root,
        "a",
        POSITION,
        Some(&component_of_world(AXIS, "drive")),
    );
    library(&root, "b", "", None);
    write(&root, &[("b/wit/axis.wit", AXIS)]);
    let ws = program(
        &root,
        "a = { path = \"../a\" }\nb = { path = \"../b\" }\n",
        "A.Axis.POSITION()",
    );
    let refused = compiled(&ws).expect_err("two sources of one interface");
    assert!(
        format!("{refused:?}").contains(&format!(
            "the component of 'a' exports `acme:drive/axis@1.0.0`, which `{}` declares the \
             host gives: an import is linked to one or the other",
            root.join("b").join("wit").display()
        )),
        "{refused:?}"
    );
}

/// `component/` holds a component, with code: a WIT package's binary there
/// is refused, a host's package going in the library's `wit/`.
#[test]
fn a_wit_package_s_binary_is_no_component() {
    super::disable_stdlib();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    library(&root, "drives", POSITION, Some(&package_binary(AXIS)));
    let ws = program(
        &root,
        "drives = { path = \"../drives\" }\n",
        "Drives.Axis.POSITION()",
    );
    let refused = compiled(&ws).expect_err("a package in component/");
    assert!(
        format!("{refused:?}").contains(&format!(
            "{} is a WIT package, not a component: a library carries a host's package in its \
             `wit/`",
            root.join("drives")
                .join("component")
                .join("c.wasm")
                .display()
        )),
        "{refused:?}"
    );
}
