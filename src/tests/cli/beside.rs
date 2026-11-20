// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A library's component runs beside the program, in a store of its own:
//! `rk compile` types the program's imports of its interfaces from the
//! component's binary, and copies the library's `component/` beside the
//! program, into `components/<library>/`, where the runtime links each
//! import to the one that exports it.

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

/// Why `rk compile` of `ws` refuses the build, as `main` prints it: the
/// message itself, whose paths are as the system writes them.
fn refusal(ws: &Path) -> String {
    match compiled(ws) {
        Err(rk::error::CliError::Message(message)) => message,
        Err(rk::error::CliError::Failed) => panic!("refused with diagnostics, not a message"),
        Ok(()) => panic!("it builds"),
    }
}

/// What `ws/out/components/` holds: each file, by its path in the folder.
fn beside(ws: &Path) -> Vec<String> {
    fn files(folder: &Path, under: &str, found: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(folder) else {
            return;
        };
        for entry in entries {
            let entry = entry.unwrap();
            let name = format!("{under}{}", entry.file_name().to_string_lossy());
            if entry.path().is_dir() {
                files(&entry.path(), &format!("{name}/"), found);
            } else {
                found.push(name);
            }
        }
    }
    let mut found = Vec::new();
    files(&ws.join("out").join("components"), "", &mut found);
    found.sort();
    found
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
/// the component's binary, and the component goes beside it, in the folder
/// named after its library, byte for byte.
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
    assert_eq!(beside(&ws), ["drives/c.wasm"]);
    assert_eq!(
        std::fs::read(
            ws.join("out")
                .join("components")
                .join("drives")
                .join("c.wasm")
        )
        .unwrap(),
        axis
    );
}

/// The library's `component/` goes beside the program whole: the
/// component's LICENSE, and whatever else its author put there, travel with
/// it, as the library holds them at each build.
#[test]
fn the_component_s_folder_goes_beside_it_whole() {
    super::disable_stdlib();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    library(
        &root,
        "drives",
        POSITION,
        Some(&component_of_world(AXIS, "drive")),
    );
    write(
        &root,
        &[
            ("drives/component/LICENSE", "Apache-2.0"),
            ("drives/component/NOTICE", "Acme"),
            ("drives/component/licenses/crates.txt", "MIT"),
        ],
    );
    let ws = program(
        &root,
        "drives = { path = \"../drives\" }\n",
        "Drives.Axis.POSITION()",
    );
    compiled(&ws).expect("it builds");
    assert_eq!(
        beside(&ws),
        [
            "drives/LICENSE",
            "drives/NOTICE",
            "drives/c.wasm",
            "drives/licenses/crates.txt"
        ]
    );
    let copied = ws.join("out").join("components").join("drives");
    assert_eq!(
        std::fs::read_to_string(copied.join("LICENSE")).unwrap(),
        "Apache-2.0"
    );
    assert_eq!(
        std::fs::read_to_string(copied.join("licenses").join("crates.txt")).unwrap(),
        "MIT"
    );

    // A file the library no longer holds is no longer beside the program.
    std::fs::remove_file(root.join("drives").join("component").join("NOTICE")).unwrap();
    compiled(&ws).expect("it builds again");
    assert_eq!(
        beside(&ws),
        [
            "drives/LICENSE",
            "drives/c.wasm",
            "drives/licenses/crates.txt"
        ]
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
    assert_eq!(beside(&ws), ["belts/c.wasm", "drives/c.wasm"]);
}

/// A component the program does not call is not written beside it, and
/// one a build left there before is removed, as a library's folder or as
/// a `.wasm` of its own: the runtime runs what is there. A folder that
/// holds no `.wasm` is no build's, and is left as it is.
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
    write(
        &ws,
        &[
            ("out/components/stale.wasm", "an old build's"),
            ("out/components/belts/c.wasm", "an old build's"),
            ("out/components/belts/LICENSE", "an old build's"),
            ("out/components/notes/todo.txt", "someone's"),
        ],
    );
    compiled(&ws).expect("it builds");
    assert_eq!(imported(&ws), [] as [&str; 0]);
    assert_eq!(beside(&ws), ["notes/todo.txt"]);

    std::fs::remove_dir_all(ws.join("out").join("components").join("notes")).unwrap();
    compiled(&ws).expect("it builds again");
    assert!(
        !ws.join("out").join("components").exists(),
        "an empty folder goes"
    );
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
    assert_eq!(beside(&ws), ["a/c.wasm"]);

    library(
        &root,
        "b",
        "",
        Some(&component_of_world(AXIS_WITH_SPEED, "drive")),
    );
    let refused = refusal(&ws);
    assert!(
        refused.contains(
            "the components of 'a' and of 'b' both export `acme:drive/axis@1.0.0`: the runtime \
             links an import to one component"
        ),
        "{refused}"
    );
}

/// A component runs on the WASI 0.3 the program gets: one that imports
/// WASI 0.2, or a WASI package with no version, is refused, the library and
/// the interface named. Any release of 0.3 passes, and a package of another
/// namespace is the runtime's own.
#[test]
fn a_component_runs_on_wasi_0_3() {
    super::disable_stdlib();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let ws = program(
        &root,
        "drives = { path = \"../drives\" }\n",
        "Drives.Axis.POSITION()",
    );
    // A component that imports the clock of the package `clocks`.
    let built_for = |clocks: &str| {
        let interface = match clocks.split_once('@') {
            Some((package, version)) => format!("{package}/monotonic-clock@{version}"),
            None => format!("{clocks}/monotonic-clock"),
        };
        let wit = format!(
            "package acme:drive@1.0.0;

            package {clocks} {{
                interface monotonic-clock {{
                    now: func() -> u64;
                }}
            }}

            interface axis {{
                position: func() -> s32;
            }}

            world drive {{
                import {interface};
                export axis;
            }}"
        );
        library(
            &root,
            "drives",
            POSITION,
            Some(&component_of_world(&wit, "drive")),
        );
    };
    for (clocks, interface) in [
        ("wasi:clocks@0.2.0", "wasi:clocks/monotonic-clock@0.2.0"),
        ("wasi:clocks", "wasi:clocks/monotonic-clock"),
    ] {
        built_for(clocks);
        assert_eq!(
            refusal(&ws),
            format!(
                "the component of 'drives' imports `{interface}`, which is no part of WASI 0.3: \
                 build it for WASI 0.3, with Rust's `--target wasm32-wasip3` for example"
            )
        );
    }
    for clocks in ["wasi:clocks@0.3.1", "acme:clocks@0.2.0"] {
        built_for(clocks);
        compiled(&ws).unwrap_or_else(|e| panic!("{clocks}: {e:?}"));
    }
}

/// A component runs in a store of its own: a handle of its own resource
/// crosses to it, as the host's proxy, but a stream or a future does not,
/// for now.
#[test]
fn a_component_s_resources_cross_and_its_streams_do_not() {
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
    compiled(&ws).expect("a handle of the component's resource crosses");

    let logs = r#"
        package acme:logs@1.0.0;

        interface logs {
            tail: func() -> stream<u8>;
        }

        world logger {
            export logs;
        }
    "#;
    library(
        &root,
        "files",
        "{extern 'acme:logs/logs@1.0.0' 'tail'}\nFUNCTION TAIL : UDINT\nEND_FUNCTION\n",
        Some(&component_of_world(logs, "logger")),
    );
    write(
        &ws,
        &[(
            "main.st",
            "{export}\nFUNCTION Run : UDINT\n    Run := Files.TAIL();\nEND_FUNCTION\n",
        )],
    );
    let refused = refusal(&ws);
    assert!(
        refused.contains(
            "`files.TAIL` imports `tail` of `acme:logs/logs@1.0.0`, which the component of \
             'files' exports, and it passes a stream: a stream or a future does not cross to a \
             component beside the program, for now"
        ),
        "{refused}"
    );
}

/// A component that keeps files: each is a resource whose handle the
/// program holds, made by its constructor, read by a method, lent to a
/// function, given back to another, found in an option, and dropped; the
/// component counts the files its destructor ran for.
const STORAGE: &str = r#"
    package acme:files@1.0.0;

    interface files {
        resource file {
            constructor(size: u32);
            size: func() -> u32;
        }
        peek: func(f: borrow<file>) -> u32;
        measure: func(f: file) -> u32;
        find: func(size: u32) -> option<file>;
        dropped: func() -> u32;
    }

    world storage {
        export files;
    }
"#;

/// [`STORAGE`]'s module: a file's size is its representation, a borrowed
/// handle comes as the representation, an owned one as the component's
/// own handle, which `measure` drops, and the destructor counts.
const STORAGE_WAT: &str = r#"
    (module
        (import "[export]acme:files/files@1.0.0" "[resource-new]file" (func $new (param i32) (result i32)))
        (import "[export]acme:files/files@1.0.0" "[resource-rep]file" (func $rep (param i32) (result i32)))
        (import "[export]acme:files/files@1.0.0" "[resource-drop]file" (func $drop (param i32)))
        (memory (export "memory") 1)
        (global $dropped (mut i32) (i32.const 0))
        (func (export "acme:files/files@1.0.0#[constructor]file") (param $size i32) (result i32)
            local.get $size
            call $new)
        (func (export "acme:files/files@1.0.0#[method]file.size") (param $rep i32) (result i32)
            local.get $rep)
        (func (export "acme:files/files@1.0.0#peek") (param $rep i32) (result i32)
            local.get $rep)
        (func (export "acme:files/files@1.0.0#measure") (param $handle i32) (result i32)
            (local $size i32)
            local.get $handle
            call $rep
            local.set $size
            local.get $handle
            call $drop
            local.get $size)
        (func (export "acme:files/files@1.0.0#find") (param $size i32) (result i32)
            i32.const 16
            i32.const 1
            i32.store8
            i32.const 20
            local.get $size
            call $new
            i32.store
            i32.const 16)
        (func (export "acme:files/files@1.0.0#dropped") (result i32)
            global.get $dropped)
        (func (export "acme:files/files@1.0.0#[dtor]file") (param $rep i32)
            global.get $dropped
            i32.const 1
            i32.add
            global.set $dropped))
"#;

/// The program's tests of [`STORAGE`]'s files.
const STORAGE_TESTS: &str = "
{test}
FUNCTION test_files
VAR a : Acme.Files.Files.FILE; b : Acme.Files.Files.FILE; found : BOOL; END_VAR
    a := Acme.Files.Files.FILE_NEW(size := 41);
    IF Acme.Files.Files.FILE_SIZE(self := a) <> 41 THEN
        __RAISE('a method of the handle');
    END_IF;
    IF Acme.Files.Files.PEEK(f := a) <> 41 THEN
        __RAISE('the handle lent');
    END_IF;
    Acme.Files.Files.FILE_DROP(handle := a);
    IF Acme.Files.Files.DROPPED() <> 1 THEN
        __RAISE('the drop did not reach the component');
    END_IF;
    b := Acme.Files.Files.FILE_NEW(size := 7);
    IF Acme.Files.Files.MEASURE(f := b) <> 7 THEN
        __RAISE('the handle given back');
    END_IF;
    IF Acme.Files.Files.DROPPED() <> 2 THEN
        __RAISE('the component did not drop what it was given');
    END_IF;
    Acme.Files.Files.FIND(size := 5, is_some => found, value => a);
    IF NOT found OR Acme.Files.Files.FILE_SIZE(self := a) <> 5 THEN
        __RAISE('the handle in an option');
    END_IF;
END_FUNCTION
";

/// `rk test` passes a component's resources between the program and the
/// component as the runtimes that link components across stores do: the
/// program holds the host's proxy, the component's store the handle. A
/// method, a handle lent or given back, one inside an option, and a drop,
/// which runs the component's destructor before the program's next call
/// into it.
#[test]
fn rk_test_passes_a_component_s_resources() {
    super::disable_stdlib();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let storage = component_of_wat(STORAGE, "storage", STORAGE_WAT);
    library(&root, "files", &declarations(&storage), Some(&storage));
    let ws = program(&root, "files = { path = \"../files\" }\n", "0");
    write(&ws, &[("main.st", STORAGE_TESTS)]);
    tested(&ws, None).expect("the handles cross");
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
    let component = |library: &str| {
        std::fs::read(built.join("components").join(library).join("c.wasm")).unwrap()
    };
    assert_eq!(component("drives"), machine);
    assert_eq!(component("belts"), conveyor);
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
            .join("drives")
            .join("c.wasm")
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
    let refused = refusal(&ws);
    assert!(
        refused.contains(&format!(
            "the component of 'a' exports `acme:drive/axis@1.0.0`, which `{}` declares the \
             host gives: an import is linked to one or the other",
            root.join("b").join("wit").display()
        )),
        "{refused}"
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
    let refused = refusal(&ws);
    assert!(
        refused.contains(&format!(
            "{} is a WIT package, not a component: a library carries a host's package in its \
             `wit/`",
            root.join("drives")
                .join("component")
                .join("c.wasm")
                .display()
        )),
        "{refused}"
    );
}
