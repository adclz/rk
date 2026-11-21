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

/// The scalars a future crosses with, as WIT names them, how
/// [`a_future_of_each_scalar_crosses`]'s module stores one, the read the
/// program declares for it, and the value, as ST writes it. A char is not
/// here: no read gives one back yet.
const SCALARS: [(&str, &str, &str, &str); 11] = [
    ("bool", "i32.const 1 i32.store8", "BOOL", "TRUE"),
    ("u8", "i32.const 7 i32.store8", "U8", "7"),
    ("s8", "i32.const -7 i32.store8", "S8", "-7"),
    ("u16", "i32.const 700 i32.store16", "U16", "700"),
    ("s16", "i32.const -700 i32.store16", "S16", "-700"),
    ("u32", "i32.const 70000 i32.store", "U32", "70000"),
    ("s32", "i32.const -70000 i32.store", "S32", "-70000"),
    ("u64", "i64.const 7000000000 i64.store", "U64", "7000000000"),
    (
        "s64",
        "i64.const -7000000000 i64.store",
        "S64",
        "-7000000000",
    ),
    ("f32", "f32.const 2.5 f32.store", "F32", "2.5"),
    ("f64", "f64.const 2.5 f64.store", "F64", "2.5"),
];

/// A future of each of the [`SCALARS`] crosses from a component to the
/// program, its value as the component wrote it: wasmtime's host API carries
/// each as its own Rust type.
#[test]
fn a_future_of_each_scalar_crosses() {
    super::disable_stdlib();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let interface = "acme:values/values@1.0.0";
    let functions: String = SCALARS
        .iter()
        .map(|(ty, ..)| format!("    f-{ty}: func() -> future<{ty}>;\n"))
        .collect();
    let wit = format!(
        "package acme:values@1.0.0;\n\ninterface values {{\n{functions}}}\n\n\
         world valued {{\n    export values;\n}}\n"
    );
    let imports: String = SCALARS
        .iter()
        .enumerate()
        .map(|(i, (ty, ..))| {
            format!(
                "(import \"[export]{interface}\" \"[future-new-0]f-{ty}\" \
                 (func $new_{i} (result i64)))\n\
                 (import \"[export]{interface}\" \"[async-lower][future-write-0]f-{ty}\" \
                 (func $write_{i} (param i32 i32) (result i32)))\n"
            )
        })
        .collect();
    let bodies: String = SCALARS
        .iter()
        .enumerate()
        .map(|(i, (ty, store, ..))| {
            let at = 256 + i * 8;
            format!(
                "(func (export \"{interface}#f-{ty}\") (result i32) (local $ends i64)\n\
                 i32.const {at} {store}\n\
                 call $new_{i} local.set $ends\n\
                 local.get $ends i64.const 32 i64.shr_u i32.wrap_i64\n\
                 i32.const {at} call $write_{i} drop\n\
                 local.get $ends i32.wrap_i64)\n"
            )
        })
        .collect();
    let wat = format!("(module\n{imports}(memory (export \"memory\") 1)\n{bodies})");
    let values = component_of_wat(&wit, "valued", &wat);
    library(&root, "values", &declarations(&values), Some(&values));
    let ws = program(&root, "values = { path = \"../values\" }\n", "0");
    let (mut vars, mut starts, mut reads, mut all) =
        (String::new(), String::new(), String::new(), Vec::new());
    for (ty, _, read, value) in SCALARS {
        vars.push_str(&format!(
            "    h_{ty} : UDINT;\n    r_{ty} : Acme.Values.Values.{read}_FUTURE_READ;\n    \
             ok_{ty} : BOOL;\n"
        ));
        starts.push_str(&format!(
            "        h_{ty} := Acme.Values.Values.F_{}();\n",
            ty.to_uppercase()
        ));
        reads.push_str(&format!(
            "    IF NOT ok_{ty} THEN\n        r_{ty}(future := h_{ty});\n        \
             IF r_{ty}.DONE THEN\n            IF r_{ty}.value <> {value} THEN\n                \
             __RAISE('the future of {ty}');\n            END_IF;\n            \
             ok_{ty} := TRUE;\n        END_IF;\n    END_IF;\n"
        ));
        all.push(format!("ok_{ty}"));
    }
    let test = format!(
        "{{test}}\nFUNCTION_BLOCK test_values\nVAR_OUTPUT DONE : BOOL; END_VAR\nVAR\n    \
         started : BOOL;\n{vars}END_VAR\n    IF NOT started THEN\n{starts}        \
         started := TRUE;\n    END_IF;\n{reads}    DONE := {};\nEND_FUNCTION_BLOCK\n",
        all.join(" AND ")
    );
    write(&ws, &[("main.st", &test)]);
    tested(&ws, None).expect("each future's value crosses");
}

/// A component runs in a store of its own: a handle of its own resource
/// crosses to it, as the host's proxy, and so does a stream of bytes or of
/// numbers, through a channel; a stream of lists does not, for now.
#[test]
fn what_crosses_to_a_component() {
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
            wide: func() -> stream<u16>;
            chunks: func() -> stream<list<u8>>;
        }

        world logger {
            export logs;
        }
    "#;
    library(
        &root,
        "files",
        "{extern 'acme:logs/logs@1.0.0' 'tail'}\nFUNCTION TAIL : UDINT\nEND_FUNCTION\n\
         {extern 'acme:logs/logs@1.0.0' 'wide'}\nFUNCTION WIDE : UDINT\nEND_FUNCTION\n\
         {extern 'acme:logs/logs@1.0.0' 'chunks'}\nFUNCTION CHUNKS : UDINT\nEND_FUNCTION\n",
        Some(&component_of_world(logs, "logger")),
    );
    let calls = |call: &str| {
        write(
            &ws,
            &[(
                "main.st",
                &format!("{{export}}\nFUNCTION Run : UDINT\n    Run := {call};\nEND_FUNCTION\n"),
            )],
        );
    };
    calls("Files.TAIL()");
    compiled(&ws).expect("a stream of bytes crosses");
    calls("Files.WIDE()");
    compiled(&ws).expect("a stream of numbers crosses");
    calls("Files.CHUNKS()");
    let refused = refusal(&ws);
    assert!(
        refused.contains(
            "`files.CHUNKS` imports `chunks` of `acme:logs/logs@1.0.0`, which the component of \
             'files' exports, and it passes a stream of a list: only a stream or a future of \
             numbers, bools, chars or strings, and a future of no value, cross to a component \
             beside the program, for now"
        ),
        "{refused}"
    );
}

/// The items of the streams [`a_stream_of_each_type_crosses`] relays, by the
/// WIT name of their type: three, as the component lays them out in its
/// memory, each so many bytes wide. A bool is 0 or 1: anything else does not
/// lift. A char is no stream's item: the component model has no `stream<char>`.
const ITEMS: [(&str, &str, u32); 11] = [
    ("bool", r"\01\00\01", 1),
    ("u8", r"\07\08\ff", 1),
    ("s8", r"\f9\08\80", 1),
    ("u16", r"\bc\02\01\00\ff\ff", 2),
    ("s16", r"\44\fd\01\00\00\80", 2),
    ("u32", r"\70\11\01\00\01\00\00\00\ff\ff\ff\ff", 4),
    ("s32", r"\90\ee\fe\ff\01\00\00\00\00\00\00\80", 4),
    (
        "u64",
        r"\00\86\3b\a1\01\00\00\00\01\00\00\00\00\00\00\00\ff\ff\ff\ff\ff\ff\ff\ff",
        8,
    ),
    (
        "s64",
        r"\00\7a\c4\5e\fe\ff\ff\ff\01\00\00\00\00\00\00\00\00\00\00\00\00\00\00\80",
        8,
    ),
    ("f32", r"\00\00\20\40\00\00\80\bf\00\00\00\3f", 4),
    (
        "f64",
        r"\00\00\00\00\00\00\04\40\00\00\00\00\00\00\f0\bf\00\00\00\00\00\00\e0\3f",
        8,
    ),
];

/// [`a_stream_of_each_type_crosses`]'s module, but for the functions of
/// each type: `cabi_realloc`, a bump through the memory, where the strings
/// a read gives land; `$same`, whether `n` bytes at `a` and at `b` are the
/// same; `$string_is`, whether the string whose place is at `pair` is the
/// `len` bytes at `at`; three strings for the stream of strings, their
/// places at 1376, `hello` for the future of a string, its place at 3200,
/// and `€` for the future of a char, at 3208.
const RELAY_WAT: &str = r#"
    (memory (export "memory") 1)
    (global $heap (mut i32) (i32.const 8192))
    (data (i32.const 1376) "\00\0c\00\00\01\00\00\00\01\0c\00\00\02\00\00\00\03\0c\00\00\04\00\00\00")
    (data (i32.const 3072) "abcd\c3\a9f")
    (data (i32.const 3100) "hello")
    (data (i32.const 3200) "\1c\0c\00\00\05\00\00\00")
    (data (i32.const 3208) "\ac\20\00\00")
    (func (export "cabi_realloc") (param i32 i32) (param $align i32) (param $size i32) (result i32)
        (local $at i32)
        global.get $heap
        local.get $align
        i32.add
        i32.const 1
        i32.sub
        i32.const 0
        local.get $align
        i32.sub
        i32.and
        local.tee $at
        local.get $size
        i32.add
        global.set $heap
        local.get $at)
    (func $same (param $a i32) (param $b i32) (param $n i32) (result i32)
        (block $differ
            (loop $next
                local.get $n
                i32.eqz
                if
                    i32.const 1
                    return
                end
                local.get $a
                i32.load8_u
                local.get $b
                i32.load8_u
                i32.ne
                br_if $differ
                local.get $a
                i32.const 1
                i32.add
                local.set $a
                local.get $b
                i32.const 1
                i32.add
                local.set $b
                local.get $n
                i32.const 1
                i32.sub
                local.set $n
                br $next))
        i32.const 0)
    (func $string_is (param $pair i32) (param $at i32) (param $len i32) (result i32)
        local.get $pair
        i32.load offset=4
        local.get $len
        i32.ne
        if (result i32)
            i32.const 0
        else
            local.get $pair
            i32.load
            local.get $at
            local.get $len
            call $same
        end)
"#;

/// A stream of each type wasmtime's host API carries, the [`ITEMS`] and
/// strings, crosses from a component to the program and from the program
/// back to the component, and so does a future of a string and one of a
/// char: the program passes on what ST cannot read yet, and the component
/// finds in its memory what it wrote.
#[test]
fn a_stream_of_each_type_crosses() {
    super::disable_stdlib();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let interface = "acme:relay/relay@1.0.0";
    let types: Vec<&str> = ITEMS.iter().map(|(ty, ..)| *ty).chain(["string"]).collect();
    let functions: String = types
        .iter()
        .map(|ty| {
            format!(
                "    tail-{ty}: func() -> stream<{ty}>;\n    sink-{ty}: func(data: stream<{ty}>);\n    \
                 got-{ty}: func() -> u32;\n"
            )
        })
        .collect();
    let wit = format!(
        "package acme:relay@1.0.0;\n\ninterface relay {{\n{functions}    \
         later-string: func() -> future<string>;\n    relay-string: func(f: future<string>);\n    \
         relayed-string: func() -> u32;\n    later-char: func() -> future<char>;\n    \
         relay-char: func(f: future<char>);\n    relayed-char: func() -> u32;\n}}\n\n\
         world relayer {{\n    export relay;\n}}\n"
    );
    // Each type's items are written from 1024 + 32 i and read into 2048 + 32 i.
    let (mut imports, mut definitions) = (String::new(), String::new());
    for (i, ty) in types.iter().enumerate() {
        let (data, buffer) = (1024 + 32 * i, 2048 + 32 * i);
        let got = match ITEMS.get(i) {
            Some((_, bytes, width)) => {
                definitions.push_str(&format!("(data (i32.const {data}) \"{bytes}\")\n"));
                format!(
                    "i32.const {data} i32.const {buffer} i32.const {} call $same",
                    3 * width
                )
            }
            None => format!(
                "i32.const {buffer} i32.const 3072 i32.const 1 call $string_is \
                 i32.const {} i32.const 3073 i32.const 2 call $string_is i32.and \
                 i32.const {} i32.const 3075 i32.const 4 call $string_is i32.and",
                buffer + 8,
                buffer + 16
            ),
        };
        imports.push_str(&format!(
            "(import \"[export]{interface}\" \"[stream-new-0]tail-{ty}\" (func $new_{i} (result i64)))\n\
             (import \"[export]{interface}\" \"[async-lower][stream-write-0]tail-{ty}\" \
             (func $write_{i} (param i32 i32 i32) (result i32)))\n\
             (import \"[export]{interface}\" \"[async-lower][stream-read-0]sink-{ty}\" \
             (func $read_{i} (param i32 i32 i32) (result i32)))\n"
        ));
        definitions.push_str(&format!(
            "(func (export \"{interface}#tail-{ty}\") (result i32) (local $ends i64)\n\
             call $new_{i} local.set $ends\n\
             local.get $ends i64.const 32 i64.shr_u i32.wrap_i64\n\
             i32.const {data} i32.const 3 call $write_{i} drop\n\
             local.get $ends i32.wrap_i64)\n\
             (func (export \"{interface}#sink-{ty}\") (param $data i32)\n\
             local.get $data i32.const {buffer} i32.const 3 call $read_{i} drop)\n\
             (func (export \"{interface}#got-{ty}\") (result i32) {got})\n"
        ));
    }
    let wat = format!(
        "(module\n{imports}\
         (import \"[export]{interface}\" \"[future-new-0]later-string\" (func $future_new (result i64)))\n\
         (import \"[export]{interface}\" \"[async-lower][future-write-0]later-string\" \
         (func $future_write (param i32 i32) (result i32)))\n\
         (import \"[export]{interface}\" \"[async-lower][future-read-0]relay-string\" \
         (func $future_read (param i32 i32) (result i32)))\n\
         (import \"[export]{interface}\" \"[future-new-0]later-char\" (func $char_new (result i64)))\n\
         (import \"[export]{interface}\" \"[async-lower][future-write-0]later-char\" \
         (func $char_write (param i32 i32) (result i32)))\n\
         (import \"[export]{interface}\" \"[async-lower][future-read-0]relay-char\" \
         (func $char_read (param i32 i32) (result i32)))\n\
         {RELAY_WAT}{definitions}\
         (func (export \"{interface}#later-string\") (result i32) (local $ends i64)\n\
         call $future_new local.set $ends\n\
         local.get $ends i64.const 32 i64.shr_u i32.wrap_i64 i32.const 3200 call $future_write drop\n\
         local.get $ends i32.wrap_i64)\n\
         (func (export \"{interface}#relay-string\") (param $f i32)\n\
         local.get $f i32.const 3300 call $future_read drop)\n\
         (func (export \"{interface}#relayed-string\") (result i32)\n\
         i32.const 3300 i32.const 3100 i32.const 5 call $string_is)\n\
         (func (export \"{interface}#later-char\") (result i32) (local $ends i64)\n\
         call $char_new local.set $ends\n\
         local.get $ends i64.const 32 i64.shr_u i32.wrap_i64 i32.const 3208 call $char_write drop\n\
         local.get $ends i32.wrap_i64)\n\
         (func (export \"{interface}#relay-char\") (param $f i32)\n\
         local.get $f i32.const 3316 call $char_read drop)\n\
         (func (export \"{interface}#relayed-char\") (result i32)\n\
         i32.const 3316 i32.const 3208 i32.const 4 call $same))\n"
    );
    let relay = component_of_wat(&wit, "relayer", &wat);
    library(&root, "relay", &declarations(&relay), Some(&relay));
    let ws = program(&root, "relay = { path = \"../relay\" }\n", "0");
    let mut tests = String::new();
    for ty in &types {
        let name = ty.to_uppercase();
        tests.push_str(&format!(
            "{{test}}\nFUNCTION_BLOCK test_stream_of_{ty}\nVAR_OUTPUT DONE : BOOL; END_VAR\n\
             VAR started : BOOL; END_VAR\n    IF NOT started THEN\n        \
             Acme.Relay.Relay.SINK_{name}(data := Acme.Relay.Relay.TAIL_{name}());\n        \
             started := TRUE;\n    END_IF;\n    DONE := Acme.Relay.Relay.GOT_{name}() = 1;\n\
             END_FUNCTION_BLOCK\n\n"
        ));
    }
    for ty in ["string", "char"] {
        let name = ty.to_uppercase();
        tests.push_str(&format!(
            "{{test}}\nFUNCTION_BLOCK test_future_of_{ty}\nVAR_OUTPUT DONE : BOOL; END_VAR\n\
             VAR started : BOOL; END_VAR\n    IF NOT started THEN\n        \
             Acme.Relay.Relay.RELAY_{name}(f := Acme.Relay.Relay.LATER_{name}());\n        \
             started := TRUE;\n    END_IF;\n    DONE := Acme.Relay.Relay.RELAYED_{name}() = 1;\n\
             END_FUNCTION_BLOCK\n\n"
        ));
    }
    write(&ws, &[("main.st", &tests)]);
    tested(&ws, None).expect("each stream and the future cross both ways");
}

/// A component that feeds the program a stream of the bytes of `hello`, a
/// future of 42 and a future of no value, and reads a stream the program
/// feeds it: `first` is the first byte that came; and a future the program
/// passes on to it: `relayed` is its value.
const FEED: &str = r#"
    package acme:feed@1.0.0;

    interface feed {
        tail: func() -> stream<u8>;
        later: func() -> future<u32>;
        ready: func() -> future;
        sink: func(data: stream<u8>);
        first: func() -> u8;
        close: func() -> u32;
        relay: func(f: future<u32>);
        relayed: func() -> u32;
    }

    world feeder {
        export feed;
    }
"#;

/// [`FEED`]'s module: `tail`, `later` and `ready` make their stream or
/// future, start the write of what it carries, which goes on once the
/// function has returned, and give the reading end; `close` cancels the
/// write and lets go of the stream's writing end, and gives what the write
/// came to: the bytes taken, `status >> 4`, five once the program took them
/// all; the low four bits say how it ended. `sink` starts a read of the
/// stream it is given into its memory, where `first` looks; `relay` the
/// read of the future it is given, where `relayed` looks.
const FEED_WAT: &str = r#"
    (module
        (import "[export]acme:feed/feed@1.0.0" "[stream-new-0]tail" (func $stream_new (result i64)))
        (import "[export]acme:feed/feed@1.0.0" "[async-lower][stream-write-0]tail" (func $stream_write (param i32 i32 i32) (result i32)))
        (import "[export]acme:feed/feed@1.0.0" "[future-new-0]later" (func $future_new (result i64)))
        (import "[export]acme:feed/feed@1.0.0" "[async-lower][future-write-0]later" (func $future_write (param i32 i32) (result i32)))
        (import "[export]acme:feed/feed@1.0.0" "[async-lower][stream-read-0]sink" (func $stream_read (param i32 i32 i32) (result i32)))
        (import "[export]acme:feed/feed@1.0.0" "[stream-cancel-write-0]tail" (func $cancel_write (param i32) (result i32)))
        (import "[export]acme:feed/feed@1.0.0" "[stream-drop-writable-0]tail" (func $drop_writable (param i32)))
        (import "[export]acme:feed/feed@1.0.0" "[future-new-0]ready" (func $ready_new (result i64)))
        (import "[export]acme:feed/feed@1.0.0" "[async-lower][future-write-0]ready" (func $ready_write (param i32 i32) (result i32)))
        (import "[export]acme:feed/feed@1.0.0" "[async-lower][future-read-0]relay" (func $future_read (param i32 i32) (result i32)))
        (memory (export "memory") 1)
        (global $writable (mut i32) (i32.const 0))
        (data (i32.const 32) "hello")
        (data (i32.const 64) "\2a\00\00\00")
        (func (export "acme:feed/feed@1.0.0#tail") (result i32)
            (local $ends i64)
            call $stream_new
            local.set $ends
            local.get $ends
            i64.const 32
            i64.shr_u
            i32.wrap_i64
            global.set $writable
            global.get $writable
            i32.const 32
            i32.const 5
            call $stream_write
            drop
            local.get $ends
            i32.wrap_i64)
        (func (export "acme:feed/feed@1.0.0#close") (result i32)
            global.get $writable
            call $cancel_write
            global.get $writable
            call $drop_writable)
        (func (export "acme:feed/feed@1.0.0#later") (result i32)
            (local $ends i64)
            call $future_new
            local.set $ends
            local.get $ends
            i64.const 32
            i64.shr_u
            i32.wrap_i64
            i32.const 64
            call $future_write
            drop
            local.get $ends
            i32.wrap_i64)
        (func (export "acme:feed/feed@1.0.0#ready") (result i32)
            (local $ends i64)
            call $ready_new
            local.set $ends
            local.get $ends
            i64.const 32
            i64.shr_u
            i32.wrap_i64
            i32.const 0
            call $ready_write
            drop
            local.get $ends
            i32.wrap_i64)
        (func (export "acme:feed/feed@1.0.0#sink") (param $data i32)
            local.get $data
            i32.const 128
            i32.const 16
            call $stream_read
            drop)
        (func (export "acme:feed/feed@1.0.0#first") (result i32)
            i32.const 128
            i32.load8_u)
        (func (export "acme:feed/feed@1.0.0#relay") (param $f i32)
            local.get $f
            i32.const 192
            call $future_read
            drop)
        (func (export "acme:feed/feed@1.0.0#relayed") (result i32)
            i32.const 192
            i32.load))
"#;

/// The program's tests of [`FEED`]: it reads the stream until it has the
/// five bytes, and the futures, cycle by cycle, then has the component close
/// the stream and reads on until it ends, with no byte more; it writes a
/// stream of its own to the component, until the component has the first
/// byte; it passes the component's future of 42 back to it, which reads it
/// there, from the program's store.
const FEED_TESTS: &str = "
{test}
FUNCTION_BLOCK test_feed
VAR_OUTPUT DONE : BOOL; END_VAR
VAR
    started : BOOL;
    tail : UDINT;
    later : UDINT;
    bytes : Acme.Feed.Feed.BYTES_STREAM_READ_ARRAY;
    value : Acme.Feed.Feed.U32_FUTURE_READ;
    received : ARRAY[0..15] OF BYTE;
    got : UDINT;
    i : UDINT;
    counted : BOOL;
    closing : BOOL;
    ended : BOOL;
    ready : UDINT;
    signal : Acme.Feed.Feed.UNIT_FUTURE_READ;
    signalled : BOOL;
END_VAR
    IF NOT started THEN
        tail := Acme.Feed.Feed.TAIL();
        later := Acme.Feed.Feed.LATER();
        ready := Acme.Feed.Feed.READY();
        started := TRUE;
    END_IF;
    IF got < 5 THEN
        bytes(stream := tail);
        IF bytes.DONE AND bytes.read > 0 THEN
            IF got + bytes.read > 16 THEN
                __RAISE('more bytes than hello');
            END_IF;
            FOR i := 0 TO bytes.read - 1 DO
                received[got + i] := bytes.data[i];
            END_FOR;
            got := got + bytes.read;
        END_IF;
    END_IF;
    IF NOT counted THEN
        value(future := later);
        IF value.DONE THEN
            IF value.value <> 42 THEN
                __RAISE('the future did not hold 42');
            END_IF;
            counted := TRUE;
        END_IF;
    END_IF;
    IF got >= 5 AND NOT closing THEN
        IF received[0] <> BYTE#16#68 OR received[1] <> BYTE#16#65 OR received[2] <> BYTE#16#6C
            OR received[3] <> BYTE#16#6C OR received[4] <> BYTE#16#6F THEN
            __RAISE('the bytes are not hello');
        END_IF;
        IF Acme.Feed.Feed.CLOSE() / 16 <> 5 THEN
            __RAISE('the component did not see its five bytes taken');
        END_IF;
        closing := TRUE;
    ELSIF closing AND NOT ended THEN
        bytes(stream := tail);
        IF bytes.DONE THEN
            IF bytes.read > 0 THEN
                __RAISE('bytes the component did not write');
            END_IF;
            ended := bytes.closed;
        END_IF;
    END_IF;
    IF NOT signalled THEN
        signal(future := ready);
        signalled := signal.DONE;
    END_IF;
    DONE := ended AND counted AND signalled;
END_FUNCTION_BLOCK

{test}
FUNCTION_BLOCK test_sink
VAR_OUTPUT DONE : BOOL; END_VAR
VAR
    started : BOOL;
    readable : UDINT;
    writable : UDINT;
    write : Acme.Feed.Feed.BYTES_STREAM_WRITE;
    written : BOOL;
END_VAR
    IF NOT started THEN
        Acme.Feed.Feed.BYTES_STREAM_NEW(readable => readable, writable => writable);
        Acme.Feed.Feed.SINK(data := readable);
        started := TRUE;
    END_IF;
    IF NOT written THEN
        write(stream := writable, data := 'hello');
        IF write.DONE THEN
            written := TRUE;
        END_IF;
    END_IF;
    DONE := Acme.Feed.Feed.FIRST() = USINT#16#68;
END_FUNCTION_BLOCK

{test}
FUNCTION_BLOCK test_relay
VAR_OUTPUT DONE : BOOL; END_VAR
VAR started : BOOL; END_VAR
    IF NOT started THEN
        Acme.Feed.Feed.RELAY(f := Acme.Feed.Feed.LATER());
        started := TRUE;
    END_IF;
    DONE := Acme.Feed.Feed.RELAYED() = 42;
END_FUNCTION_BLOCK
";

/// `rk test` carries a component's stream and futures to the program, and
/// the program's stream and a future it passes on to the component: the
/// writer writes in its store,
/// each store's event loop runs, the component's in a task of its own, and
/// the reader reads in its own store, over the program's cycles.
#[test]
fn rk_test_carries_a_component_s_stream_and_future() {
    super::disable_stdlib();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let feed = component_of_wat(FEED, "feeder", FEED_WAT);
    library(&root, "feed", &declarations(&feed), Some(&feed));
    let ws = program(&root, "feed = { path = \"../feed\" }\n", "0");
    write(&ws, &[("main.st", FEED_TESTS)]);
    tested(&ws, Some("test_feed")).expect("the component's stream and future cross");
    tested(&ws, Some("test_sink")).expect("the program's stream crosses");
    tested(&ws, Some("test_relay")).expect("a future the program passes on crosses");
}

/// A component whose copies wait on the program's store: `listen` reads a
/// stream the program never writes to and `stop-listening` cancels the read;
/// `open` gives the program a stream it never reads, `flood` writes into it
/// a byte at a time while each write goes at once and gives how many went,
/// `stop-flooding` cancels the write that waits; `poke` writes once more.
/// A cancel and `poke` give the copy's status: how many items went,
/// `status >> 4`, then how it ended, 1 dropped, 2 cancelled.
const WAIT: &str = r#"
    package acme:wait@1.0.0;

    interface wait {
        listen: func(data: stream<u8>);
        stop-listening: async func() -> u32;
        open: func() -> stream<u8>;
        flood: func() -> u32;
        stop-flooding: async func() -> u32;
        poke: func() -> u32;
    }

    world waiter {
        export wait;
    }
"#;

/// [`WAIT`]'s module. The cancels wait for the other store to end the copy,
/// which wasmtime allows a call of an `async func` and refuses one of a
/// `func`: each is lifted async, cancels, and gives the status through
/// `task.return`.
const WAIT_WAT: &str = r#"
    (module
        (import "[export]acme:wait/wait@1.0.0" "[async-lower][stream-read-0]listen" (func $read (param i32 i32 i32) (result i32)))
        (import "[export]acme:wait/wait@1.0.0" "[stream-cancel-read-0]listen" (func $cancel_read (param i32) (result i32)))
        (import "[export]acme:wait/wait@1.0.0" "[task-return]stop-listening" (func $listening_return (param i32)))
        (import "[export]acme:wait/wait@1.0.0" "[stream-new-0]open" (func $new (result i64)))
        (import "[export]acme:wait/wait@1.0.0" "[async-lower][stream-write-0]open" (func $write (param i32 i32 i32) (result i32)))
        (import "[export]acme:wait/wait@1.0.0" "[stream-cancel-write-0]open" (func $cancel_write (param i32) (result i32)))
        (import "[export]acme:wait/wait@1.0.0" "[task-return]stop-flooding" (func $flooding_return (param i32)))
        (memory (export "memory") 1)
        (global $readable (mut i32) (i32.const 0))
        (global $writable (mut i32) (i32.const 0))
        (func (export "acme:wait/wait@1.0.0#listen") (param $data i32)
            local.get $data
            global.set $readable
            local.get $data
            i32.const 256
            i32.const 16
            call $read
            drop)
        (func (export "[async-lift]acme:wait/wait@1.0.0#stop-listening") (result i32)
            global.get $readable
            call $cancel_read
            call $listening_return
            ;; EXIT: the call is over.
            i32.const 0)
        (func (export "[callback][async-lift]acme:wait/wait@1.0.0#stop-listening")
            (param i32 i32 i32) (result i32)
            i32.const 0)
        (func (export "acme:wait/wait@1.0.0#open") (result i32)
            (local $ends i64)
            call $new
            local.tee $ends
            i64.const 32
            i64.shr_u
            i32.wrap_i64
            global.set $writable
            local.get $ends
            i32.wrap_i64)
        (func (export "acme:wait/wait@1.0.0#flood") (result i32)
            (local $went i32)
            (block $waits
                (loop $next
                    global.get $writable
                    i32.const 512
                    i32.const 1
                    call $write
                    i32.const -1
                    i32.eq
                    br_if $waits
                    local.get $went
                    i32.const 1
                    i32.add
                    local.tee $went
                    i32.const 64
                    i32.lt_u
                    br_if $next))
            local.get $went)
        (func (export "[async-lift]acme:wait/wait@1.0.0#stop-flooding") (result i32)
            global.get $writable
            call $cancel_write
            call $flooding_return
            i32.const 0)
        (func (export "[callback][async-lift]acme:wait/wait@1.0.0#stop-flooding")
            (param i32 i32 i32) (result i32)
            i32.const 0)
        (func (export "acme:wait/wait@1.0.0#poke") (result i32)
            global.get $writable
            i32.const 512
            i32.const 1
            call $write))
"#;

/// The program's tests of [`WAIT`]: a read and a write that wait on the
/// program end cancelled, with nothing moved, and a write after the program
/// let go of the reading end finds the stream dropped.
const WAIT_TESTS: &str = "
{test}
FUNCTION_BLOCK test_cancelled_read
VAR_OUTPUT DONE : BOOL; END_VAR
VAR
    started : BOOL;
    readable : UDINT;
    writable : UDINT;
    stop : Acme.Wait.Wait.STOP_LISTENING;
END_VAR
    IF NOT started THEN
        Acme.Wait.Wait.BYTES_STREAM_NEW(readable => readable, writable => writable);
        Acme.Wait.Wait.LISTEN(data := readable);
        started := TRUE;
    END_IF;
    stop();
    IF stop.DONE THEN
        IF stop.value <> 2 THEN
            __RAISE('the read did not end cancelled');
        END_IF;
        DONE := TRUE;
    END_IF;
END_FUNCTION_BLOCK

{test}
FUNCTION_BLOCK test_cancelled_write
VAR_OUTPUT DONE : BOOL; END_VAR
VAR
    started : BOOL;
    unread : UDINT;
    stop : Acme.Wait.Wait.STOP_FLOODING;
END_VAR
    IF NOT started THEN
        unread := Acme.Wait.Wait.OPEN();
        IF Acme.Wait.Wait.FLOOD() <> 9 THEN
            __RAISE('the channel did not hold nine chunks');
        END_IF;
        started := TRUE;
    END_IF;
    stop();
    IF stop.DONE THEN
        IF stop.value <> 2 THEN
            __RAISE('the write did not end cancelled');
        END_IF;
        DONE := TRUE;
    END_IF;
END_FUNCTION_BLOCK

{test}
FUNCTION_BLOCK test_reader_left
VAR_OUTPUT DONE : BOOL; END_VAR
VAR
    opened : BOOL;
    unread : UDINT;
END_VAR
    IF NOT opened THEN
        unread := Acme.Wait.Wait.OPEN();
        Acme.Wait.Wait.BYTES_STREAM_DROP_READABLE(stream := unread);
        opened := TRUE;
    ELSE
        IF Acme.Wait.Wait.POKE() <> 1 THEN
            __RAISE('the write did not find the stream dropped');
        END_IF;
        DONE := TRUE;
    END_IF;
END_FUNCTION_BLOCK
";

/// A copy that waits on the other store ends when its guest cancels it, as
/// wasmtime's contract asks of a producer and a consumer: a read with
/// nothing in the channel, and a write once the channel holds its nine
/// chunks, eight and the one its sender keeps. A writer whose reader left
/// finds the stream dropped.
#[test]
fn a_copy_waiting_on_the_other_store_ends_when_cancelled() {
    super::disable_stdlib();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let wait = component_of_wat(WAIT, "waiter", WAIT_WAT);
    library(&root, "wait", &declarations(&wait), Some(&wait));
    let ws = program(&root, "wait = { path = \"../wait\" }\n", "0");
    write(&ws, &[("main.st", WAIT_TESTS)]);
    tested(&ws, Some("test_cancelled_read")).expect("a waiting read is cancelled");
    tested(&ws, Some("test_cancelled_write")).expect("a waiting write is cancelled");
    tested(&ws, Some("test_reader_left")).expect("the writer finds its reader gone");
}

/// The program's tests of `_GET` on [`FEED`]'s futures: called at each
/// cycle, the block gives each value once and sets the handle to 0; it let
/// go of the reading end, which a second drop finds gone, where a plain
/// read leaves the drop to the program.
const GET_TESTS: &str = "
{test}
FUNCTION_BLOCK test_get_reads_once
VAR_OUTPUT DONE : BOOL; END_VAR
VAR
    started : BOOL;
    later : UDINT;
    ready : UDINT;
    value : Acme.Feed.Feed.U32_FUTURE_GET;
    signal : Acme.Feed.Feed.UNIT_FUTURE_GET;
    got : BOOL;
    signalled : BOOL;
    after : INT;
END_VAR
    IF NOT started THEN
        later := Acme.Feed.Feed.LATER();
        ready := Acme.Feed.Feed.READY();
        started := TRUE;
    END_IF;
    value(future := later);
    IF value.DONE THEN
        IF got OR value.value <> 42 OR later <> 0 THEN
            __RAISE('the value did not come once, 42, its future gone');
        END_IF;
        got := TRUE;
    END_IF;
    signal(future := ready);
    IF signal.DONE THEN
        IF signalled OR ready <> 0 THEN
            __RAISE('the signal did not come once, its future gone');
        END_IF;
        signalled := TRUE;
    END_IF;
    IF got AND signalled THEN
        after := after + 1;
    END_IF;
    DONE := after > 3;
END_FUNCTION_BLOCK

{test}
FUNCTION_BLOCK test_get_lets_go
VAR_OUTPUT DONE : BOOL; END_VAR
VAR
    started : BOOL;
    later : UDINT;
    kept : UDINT;
    value : Acme.Feed.Feed.U32_FUTURE_GET;
END_VAR
    IF NOT started THEN
        later := Acme.Feed.Feed.LATER();
        kept := later;
        started := TRUE;
    END_IF;
    value(future := later);
    IF value.DONE THEN
        Acme.Feed.Feed.U32_FUTURE_DROP_READABLE(future := kept);
        DONE := TRUE;
    END_IF;
END_FUNCTION_BLOCK

{test}
FUNCTION_BLOCK test_read_keeps
VAR_OUTPUT DONE : BOOL; END_VAR
VAR
    started : BOOL;
    later : UDINT;
    value : Acme.Feed.Feed.U32_FUTURE_READ;
END_VAR
    IF NOT started THEN
        later := Acme.Feed.Feed.LATER();
        started := TRUE;
    END_IF;
    value(future := later);
    IF value.DONE THEN
        Acme.Feed.Feed.U32_FUTURE_DROP_READABLE(future := later);
        DONE := TRUE;
    END_IF;
END_FUNCTION_BLOCK
";

/// A future's `_GET` reads it once and lets go of its reading end: the
/// program's handle is 0 after, a later call reads nothing, and a second
/// drop of the end traps, where a plain read leaves it to the program.
#[test]
fn a_future_s_get_reads_it_and_lets_go() {
    super::disable_stdlib();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let feed = component_of_wat(FEED, "feeder", FEED_WAT);
    library(&root, "feed", &declarations(&feed), Some(&feed));
    let ws = program(&root, "feed = { path = \"../feed\" }\n", "0");
    write(&ws, &[("main.st", GET_TESTS)]);
    tested(&ws, Some("test_get_reads_once")).expect("each value comes once");
    tested(&ws, Some("test_read_keeps")).expect("a plain read leaves the end to the program");
    assert!(
        tested(&ws, Some("test_get_lets_go")).is_err(),
        "_GET let go of the end: a second drop traps"
    );
}

/// A component that reads the stream it is given until the writer lets go:
/// `take` keeps the reading end, where the bytes land from `offset` on, and
/// answers with a future of how many came; `ended` reads on, a read at a
/// time, and gives how many bytes came once the writer let go, -1 until
/// then, writing the future then; `came` gives the byte at `index`; `stop`
/// lets go of the reading end.
const DRAIN: &str = r#"
    package acme:drain@1.0.0;

    interface drain {
        take: func(data: stream<u8>, offset: u32) -> future<u32>;
        ended: func() -> u32;
        came: func(index: u32) -> u8;
        stop: func();
    }

    world drainer {
        export drain;
    }
"#;

/// [`DRAIN`]'s module: a read that waits joins a waitable set, which
/// `ended` polls for the read's end; the bytes land from 256 + `offset`
/// on, the count the future gives at 900.
const DRAIN_WAT: &str = r#"
    (module
        (import "$root" "[waitable-set-new]" (func $set_new (result i32)))
        (import "$root" "[waitable-join]" (func $join (param i32 i32)))
        (import "$root" "[waitable-set-poll]" (func $set_poll (param i32 i32) (result i32)))
        (import "[export]acme:drain/drain@1.0.0" "[async-lower][stream-read-0]take" (func $read (param i32 i32 i32) (result i32)))
        (import "[export]acme:drain/drain@1.0.0" "[stream-drop-readable-0]take" (func $drop (param i32)))
        (import "[export]acme:drain/drain@1.0.0" "[future-new-1]take" (func $future_new (result i64)))
        (import "[export]acme:drain/drain@1.0.0" "[async-lower][future-write-1]take" (func $future_write (param i32 i32) (result i32)))
        (memory (export "memory") 1)
        (global $readable (mut i32) (i32.const 0))
        (global $base (mut i32) (i32.const 256))
        (global $answer (mut i32) (i32.const 0))
        (global $set (mut i32) (i32.const 0))
        (global $total (mut i32) (i32.const 0))
        (global $pending (mut i32) (i32.const 0))
        (global $over (mut i32) (i32.const 0))
        (func (export "acme:drain/drain@1.0.0#take") (param $data i32) (param $offset i32) (result i32)
            (local $ends i64)
            local.get $data
            global.set $readable
            i32.const 256
            local.get $offset
            i32.add
            global.set $base
            call $set_new
            global.set $set
            call $future_new
            local.tee $ends
            i64.const 32
            i64.shr_u
            i32.wrap_i64
            global.set $answer
            local.get $ends
            i32.wrap_i64)
        (func (export "acme:drain/drain@1.0.0#ended") (result i32)
            (local $status i32)
            (block $wait
                (loop $more
                    global.get $over
                    if
                        global.get $total
                        return
                    end
                    global.get $pending
                    if
                        global.get $set
                        i32.const 768
                        call $set_poll
                        ;; STREAM_READ: the read ended, its status at 772.
                        i32.const 2
                        i32.ne
                        br_if $wait
                        i32.const 772
                        i32.load
                        local.set $status
                        i32.const 0
                        global.set $pending
                    else
                        global.get $readable
                        global.get $base
                        global.get $total
                        i32.add
                        i32.const 64
                        call $read
                        local.tee $status
                        i32.const -1
                        i32.eq
                        if
                            global.get $readable
                            global.get $set
                            call $join
                            i32.const 1
                            global.set $pending
                            br $wait
                        end
                    end
                    ;; The bytes that came, status >> 4, then whether the
                    ;; writer let go, 1 in the low bits.
                    global.get $total
                    local.get $status
                    i32.const 4
                    i32.shr_u
                    i32.add
                    global.set $total
                    local.get $status
                    i32.const 15
                    i32.and
                    i32.const 1
                    i32.eq
                    if
                        i32.const 1
                        global.set $over
                        global.get $readable
                        call $drop
                        ;; The answer: how many bytes came.
                        i32.const 900
                        global.get $total
                        i32.store
                        global.get $answer
                        i32.const 900
                        call $future_write
                        drop
                    end
                    br $more))
            i32.const -1)
        (func (export "acme:drain/drain@1.0.0#came") (param $index i32) (result i32)
            i32.const 256
            local.get $index
            i32.add
            i32.load8_u)
        (func (export "acme:drain/drain@1.0.0#stop")
            global.get $readable
            call $drop))
"#;

/// The program's tests of a `BYTES_STREAM_WRITER`, handing its reading end
/// to [`DRAIN`]: what it writes reaches the reader, and `CLOSE` ends the
/// stream, once a write that waits for its reader is over; a request after
/// is an error, as one is once the reader left; an empty text is done at
/// once.
const WRITER_TESTS: &str = "
{test}
FUNCTION_BLOCK test_writer_ends_the_stream
VAR_OUTPUT DONE : BOOL; END_VAR
VAR
    step : INT;
    w : Acme.Drain.Drain.BYTES_STREAM_WRITER;
    total : UDINT;
END_VAR
    CASE step OF
        0:
            w();
            Acme.Drain.Drain.U32_FUTURE_DROP_READABLE(
                future := Acme.Drain.Drain.TAKE(data := w.readable, offset := 0));
            w(REQ := TRUE, IN := 'hello');
            step := 1;
        1:
            w(REQ := TRUE, IN := 'hello');
            IF NOT w.BUSY THEN
                w(REQ := FALSE, CLOSE := TRUE);
                step := 2;
            END_IF;
        2:
            total := Acme.Drain.Drain.ENDED();
            IF total <> UDINT#16#FFFFFFFF THEN
                IF total <> 5 OR Acme.Drain.Drain.CAME(index := 0) <> 16#68 THEN
                    __RAISE('the reader did not get hello, then the end');
                END_IF;
                w(REQ := TRUE, IN := 'more');
                IF NOT w.ERROR THEN
                    __RAISE('a request after CLOSE is no error');
                END_IF;
                DONE := TRUE;
            END_IF;
    END_CASE;
END_FUNCTION_BLOCK

{test}
FUNCTION_BLOCK test_writer_closes_after_its_write
VAR_OUTPUT DONE : BOOL; END_VAR
VAR
    started : BOOL;
    w : Acme.Drain.Drain.BYTES_STREAM_WRITER;
    total : UDINT;
END_VAR
    IF NOT started THEN
        w();
        w(REQ := TRUE, IN := 'hello');
        w(REQ := TRUE, CLOSE := TRUE);
        IF NOT w.BUSY THEN
            __RAISE('the write did not wait for its reader');
        END_IF;
        Acme.Drain.Drain.U32_FUTURE_DROP_READABLE(
            future := Acme.Drain.Drain.TAKE(data := w.readable, offset := 0));
        started := TRUE;
    END_IF;
    w(REQ := TRUE, CLOSE := TRUE);
    total := Acme.Drain.Drain.ENDED();
    IF total <> UDINT#16#FFFFFFFF THEN
        IF total <> 5 THEN
            __RAISE('the write did not go before the end');
        END_IF;
        DONE := TRUE;
    END_IF;
END_FUNCTION_BLOCK

{test}
FUNCTION_BLOCK test_writer_takes_an_empty_text
VAR_OUTPUT DONE : BOOL; END_VAR
VAR
    w : Acme.Drain.Drain.BYTES_STREAM_WRITER;
END_VAR
    w(REQ := TRUE, IN := '');
    IF NOT w.DONE OR w.BUSY THEN
        __RAISE('an empty text is not done at once');
    END_IF;
    DONE := TRUE;
END_FUNCTION_BLOCK

{test}
FUNCTION_BLOCK test_writer_sees_its_reader_leave
VAR_OUTPUT DONE : BOOL; END_VAR
VAR
    started : BOOL;
    w : Acme.Drain.Drain.BYTES_STREAM_WRITER;
END_VAR
    IF NOT started THEN
        w();
        Acme.Drain.Drain.U32_FUTURE_DROP_READABLE(
            future := Acme.Drain.Drain.TAKE(data := w.readable, offset := 0));
        Acme.Drain.Drain.STOP();
        started := TRUE;
    ELSIF NOT w.ERROR THEN
        w(REQ := TRUE, IN := 'hello');
        IF w.DONE THEN
            __RAISE('a write whose reader left took the text');
        END_IF;
    ELSE
        // The writing end went with the reader, and CLOSE drops it no
        // second time.
        IF w.writable <> 0 THEN
            __RAISE('the writer kept its end once the reader left');
        END_IF;
        w(REQ := FALSE, CLOSE := TRUE);
        DONE := TRUE;
    END_IF;
END_FUNCTION_BLOCK

{test}
FUNCTION_BLOCK test_writer_lets_go_when_its_reader_left
VAR_OUTPUT DONE : BOOL; END_VAR
VAR
    started : BOOL;
    held : UDINT;
    w : Acme.Drain.Drain.BYTES_STREAM_WRITER;
END_VAR
    IF NOT started THEN
        w();
        held := w.writable;
        Acme.Drain.Drain.U32_FUTURE_DROP_READABLE(
            future := Acme.Drain.Drain.TAKE(data := w.readable, offset := 0));
        Acme.Drain.Drain.STOP();
        started := TRUE;
    ELSIF NOT w.ERROR THEN
        w(REQ := TRUE, IN := 'hello');
    ELSE
        // A second drop of the end the writer let go of traps.
        Acme.Drain.Drain.BYTES_STREAM_DROP_WRITABLE(stream := held);
        DONE := TRUE;
    END_IF;
END_FUNCTION_BLOCK
";

/// The program's tests of the writers of [`DRAIN`]'s `take` and of
/// [`FEED`]'s `sink`: the block makes the stream at the first request,
/// calls the function with its reading end and its other inputs, writes,
/// and once `CLOSE` ended the stream, gives the future's value; one of a
/// function that answers with nothing writes.
const FUNCTION_WRITER_TESTS: &str = "
{test}
FUNCTION_BLOCK test_function_writer_answers
VAR_OUTPUT DONE : BOOL; END_VAR
VAR
    step : INT;
    w : Acme.Drain.Drain.TAKE_WRITER;
    total : UDINT;
    wrote : BOOL;
END_VAR
    CASE step OF
        0:
            w(CLOSE := FALSE, offset := 10);
            w(REQ := TRUE, IN := 'hello');
            wrote := w.DONE;
            step := 1;
        1:
            w(REQ := TRUE, IN := 'hello');
            wrote := wrote OR w.DONE;
            IF wrote AND NOT w.BUSY THEN
                w(REQ := FALSE, CLOSE := TRUE);
                step := 2;
            END_IF;
        2:
            w(CLOSE := TRUE);
            IF w.ENDED THEN
                IF w.value <> 5 OR Acme.Drain.Drain.CAME(index := 10) <> 16#68 THEN
                    __RAISE('the answer is not the 5 bytes of hello, from 10 on');
                END_IF;
                w(REQ := TRUE, IN := 'more');
                IF NOT w.ERROR THEN
                    __RAISE('a request after CLOSE is no error');
                END_IF;
                DONE := TRUE;
            END_IF;
    END_CASE;
    IF step > 0 THEN
        total := Acme.Drain.Drain.ENDED();
        IF total = UDINT#16#FFFFFFFF AND w.ENDED THEN
            __RAISE('the answer came before the stream ended');
        END_IF;
    END_IF;
END_FUNCTION_BLOCK

{test}
FUNCTION_BLOCK test_function_writer_is_busy_while_the_reader_waits
VAR_OUTPUT DONE : BOOL; END_VAR
VAR
    w : Acme.Drain.Drain.TAKE_WRITER;
    request : BOOL;
    written : INT;
END_VAR
    // The component reads nothing: the bridge's channel fills, and the
    // write after its nine chunks waits.
    request := NOT request;
    w(REQ := request, IN := 'x');
    IF w.DONE THEN
        written := written + 1;
    END_IF;
    IF w.BUSY THEN
        IF written <> 9 THEN
            __RAISE('the write did not wait once the channel was full');
        END_IF;
        DONE := TRUE;
    END_IF;
END_FUNCTION_BLOCK

{test}
FUNCTION_BLOCK test_function_writer_waits_for_a_request
VAR_OUTPUT DONE : BOOL; END_VAR
VAR
    w : Acme.Drain.Drain.TAKE_WRITER;
    cycles : INT;
END_VAR
    // A CLOSE before any request ends no stream: there is none yet, and
    // the first request makes one and writes it.
    cycles := cycles + 1;
    IF cycles < 4 THEN
        w(CLOSE := TRUE);
    ELSE
        w(REQ := TRUE, IN := 'x', CLOSE := FALSE);
        IF w.ERROR OR NOT w.DONE THEN
            __RAISE('a CLOSE before any request ended the stream');
        END_IF;
        DONE := TRUE;
    END_IF;
END_FUNCTION_BLOCK
";

/// The program's test of the writer of [`FEED`]'s `sink`, which answers
/// with nothing: the bytes reach the component.
const SINK_WRITER_TESTS: &str = "
{test}
FUNCTION_BLOCK test_function_writer_of_nothing
VAR_OUTPUT DONE : BOOL; END_VAR
VAR
    w : Acme.Feed.Feed.SINK_WRITER;
END_VAR
    w(REQ := TRUE, IN := 'hello');
    DONE := Acme.Feed.Feed.FIRST() = USINT#16#68;
END_FUNCTION_BLOCK
";

/// A function's writer, `<NAME>_WRITER`, makes the stream the function
/// takes at the first request, calls the function with it and with its
/// other inputs, writes it, and gives the value of the future the function
/// answers with once it comes; nothing happens before a request.
#[test]
fn a_function_s_writer_opens_writes_and_reads_the_answer() {
    super::disable_stdlib();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let drain = component_of_wat(DRAIN, "drainer", DRAIN_WAT);
    library(&root, "drain", &declarations(&drain), Some(&drain));
    let ws = program(&root, "drain = { path = \"../drain\" }\n", "0");
    write(&ws, &[("main.st", FUNCTION_WRITER_TESTS)]);
    tested(&ws, Some("test_function_writer_answers")).expect("the text goes, the answer comes");
    tested(&ws, Some("test_function_writer_waits_for_a_request"))
        .expect("nothing opens before a request");
    tested(
        &ws,
        Some("test_function_writer_is_busy_while_the_reader_waits"),
    )
    .expect("BUSY while the write waits");

    let feed = component_of_wat(FEED, "feeder", FEED_WAT);
    library(&root, "feed", &declarations(&feed), Some(&feed));
    let ws = program(&root, "feed = { path = \"../feed\" }\n", "0");
    write(&ws, &[("main.st", SINK_WRITER_TESTS)]);
    tested(&ws, Some("test_function_writer_of_nothing")).expect("the bytes reach the component");
}

/// A component whose `upload`, an `async func`, reads the stream it is
/// given until the writer lets go, then returns how many bytes came plus
/// `offset`; `first` gives the first byte that came.
const UPLOAD: &str = r#"
    package acme:upload@1.0.0;

    interface upload {
        upload: async func(data: stream<u8>, offset: u32) -> u32;
        first: func() -> u8;
    }

    world uploader {
        export upload;
    }
"#;

/// [`UPLOAD`]'s module: `upload` is lifted async, with a callback. It reads
/// until a read waits, then waits on a set holding the stream; its callback
/// takes what the read gave and reads on, and the writer leaving ends the
/// call, through `task.return`. The bytes land from 256 on.
const UPLOAD_WAT: &str = r#"
    (module
        (import "$root" "[waitable-set-new]" (func $set_new (result i32)))
        (import "$root" "[waitable-join]" (func $join (param i32 i32)))
        (import "[export]acme:upload/upload@1.0.0" "[async-lower][stream-read-0]upload" (func $read (param i32 i32 i32) (result i32)))
        (import "[export]acme:upload/upload@1.0.0" "[stream-drop-readable-0]upload" (func $drop (param i32)))
        (import "[export]acme:upload/upload@1.0.0" "[task-return]upload" (func $return (param i32)))
        (memory (export "memory") 1)
        (global $readable (mut i32) (i32.const 0))
        (global $set (mut i32) (i32.const 0))
        (global $total (mut i32) (i32.const 0))
        (global $offset (mut i32) (i32.const 0))
        ;; Takes what a read gave: the bytes that came, status >> 4, then
        ;; whether the writer let go, 1 in the low bits, which ends the call.
        (func $took (param $status i32) (result i32)
            global.get $total
            local.get $status
            i32.const 4
            i32.shr_u
            i32.add
            global.set $total
            local.get $status
            i32.const 15
            i32.and
            i32.const 1
            i32.eq
            if
                global.get $readable
                call $drop
                global.get $total
                global.get $offset
                i32.add
                call $return
                ;; EXIT: the call is over.
                i32.const 0
                return
            end
            call $more)
        ;; Reads on until a read waits, then WAITs on the set.
        (func $more (result i32)
            (local $status i32)
            global.get $readable
            i32.const 256
            global.get $total
            i32.add
            i32.const 64
            call $read
            local.tee $status
            i32.const -1
            i32.eq
            if
                global.get $set
                i32.const 4
                i32.shl
                i32.const 2
                i32.or
                return
            end
            local.get $status
            call $took)
        (func (export "[async-lift]acme:upload/upload@1.0.0#upload") (param $data i32) (param $offset i32) (result i32)
            local.get $data
            global.set $readable
            local.get $offset
            global.set $offset
            call $set_new
            global.set $set
            global.get $readable
            global.get $set
            call $join
            call $more)
        (func (export "[callback][async-lift]acme:upload/upload@1.0.0#upload") (param $event i32) (param $handle i32) (param $payload i32) (result i32)
            local.get $payload
            call $took)
        (func (export "acme:upload/upload@1.0.0#first") (result i32)
            i32.const 256
            i32.load8_u))
"#;

/// The program's test of [`UPLOAD`]'s writer: the call starts with the
/// stream at the first request, with `offset` as it is then, runs while the
/// block writes, returns once `CLOSE` ended the stream, and is not started
/// again after.
const ASYNC_WRITER_TESTS: &str = "
{test}
FUNCTION_BLOCK test_async_writer_returns
VAR_OUTPUT DONE : BOOL; END_VAR
VAR
    step : INT;
    w : Acme.Upload.Upload.UPLOAD_WRITER;
    wrote : BOOL;
    after : INT;
END_VAR
    CASE step OF
        0:
            w(CLOSE := FALSE, offset := 100);
            w(REQ := TRUE, IN := 'hello');
            wrote := w.DONE;
            step := 1;
        1:
            w(REQ := TRUE, IN := 'hello');
            wrote := wrote OR w.DONE;
            IF w.ENDED THEN
                __RAISE('the call returned before the stream ended');
            END_IF;
            IF wrote AND NOT w.BUSY THEN
                w(REQ := FALSE, CLOSE := TRUE);
                step := 2;
            END_IF;
        2:
            w(CLOSE := TRUE);
            IF w.ENDED THEN
                IF w.value <> 105 OR Acme.Upload.Upload.FIRST() <> 16#68 THEN
                    __RAISE('the call did not return the 5 bytes of hello and the offset');
                END_IF;
                step := 3;
            END_IF;
        3:
            // The call returned: later calls of the block start none.
            w(REQ := TRUE, IN := 'more');
            IF NOT w.ERROR OR NOT w.ENDED OR w.value <> 105 THEN
                __RAISE('a request after CLOSE is no error, or the result went');
            END_IF;
            after := after + 1;
            DONE := after > 3;
    END_CASE;
END_FUNCTION_BLOCK
";

/// The writer of an `async func` that takes a stream starts the call with
/// the stream it makes at the first request, writes while the call runs,
/// and keeps what the call returns once `CLOSE` ended the stream; it starts
/// no second call.
#[test]
fn an_async_function_s_writer_starts_the_call_and_keeps_its_result() {
    super::disable_stdlib();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let upload = component_of_wat(UPLOAD, "uploader", UPLOAD_WAT);
    library(&root, "upload", &declarations(&upload), Some(&upload));
    let ws = program(&root, "upload = { path = \"../upload\" }\n", "0");
    write(&ws, &[("main.st", ASYNC_WRITER_TESTS)]);
    tested(&ws, None).expect("the call returns what came, once");
}

/// [`UPLOAD`] whose `upload` returns a result: the error `many` once more
/// than 5 bytes came, else none.
fn result_uploader() -> Vec<u8> {
    let returns = "upload: async func(data: stream<u8>, offset: u32) -> u32;";
    assert_eq!(UPLOAD.matches(returns).count(), 1);
    let wit = UPLOAD.replace(
        returns,
        "enum fault { few, many }
        upload: async func(data: stream<u8>, offset: u32) -> result<_, fault>;",
    );
    let total = "global.get $total
                global.get $offset
                i32.add
                call $return";
    assert_eq!(UPLOAD_WAT.matches(total).count(), 1);
    // A result is its case, 1 for an error, then the error's.
    let wat = UPLOAD_WAT
        .replace(
            "(func $return (param i32))",
            "(func $return (param i32 i32))",
        )
        .replace(
            total,
            "global.get $total
                i32.const 5
                i32.gt_u
                if (result i32 i32)
                    i32.const 1
                    i32.const 1
                else
                    i32.const 0
                    i32.const 0
                end
                call $return",
        );
    component_of_wat(&wit, "uploader", &wat)
}

/// The program's tests of [`result_uploader`]'s writer: the error of the
/// result the call returns is the writer's `ERROR` and `ERRORID`, and a
/// result of no error leaves `ERROR` FALSE.
const RESULT_WRITER_TESTS: &str = "
FUNCTION_BLOCK send
VAR_INPUT text : STRING; END_VAR
VAR_OUTPUT ENDED, ERROR : BOOL; ERRORID : Acme.Upload.Upload.FAULT; END_VAR
VAR
    step : INT;
    w : Acme.Upload.Upload.UPLOAD_WRITER;
END_VAR
    CASE step OF
        0:
            w(REQ := TRUE, IN := text, offset := 100);
            step := 1;
        1:
            w(REQ := TRUE);
            IF NOT w.BUSY THEN
                w(REQ := FALSE, CLOSE := TRUE);
                step := 2;
            END_IF;
        2:
            w(CLOSE := TRUE);
    END_CASE;
    IF w.ERROR AND NOT w.ENDED THEN
        __RAISE('ERROR before the call returned');
    END_IF;
    ENDED := w.ENDED;
    ERROR := w.ERROR;
    ERRORID := w.ERRORID;
END_FUNCTION_BLOCK

{test}
FUNCTION_BLOCK test_result_writer_reports_the_error
VAR_OUTPUT DONE : BOOL; END_VAR
VAR s : send; END_VAR
    s(text := 'hello world');
    IF s.ENDED THEN
        IF NOT s.ERROR OR s.ERRORID <> Acme.Upload.Upload.FAULT#MANY THEN
            __RAISE('the error is not ERROR with ERRORID many');
        END_IF;
        DONE := TRUE;
    END_IF;
END_FUNCTION_BLOCK

{test}
FUNCTION_BLOCK test_result_writer_without_an_error
VAR_OUTPUT DONE : BOOL; END_VAR
VAR s : send; END_VAR
    s(text := 'hi');
    IF s.ENDED THEN
        IF s.ERROR THEN
            __RAISE('an answer of no error is an ERROR');
        END_IF;
        DONE := TRUE;
    END_IF;
END_FUNCTION_BLOCK
";

/// A function's writer folds the error of the result it answers with into
/// `ERROR` and `ERRORID`.
#[test]
fn a_function_s_writer_reports_the_error_it_answers_with() {
    super::disable_stdlib();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let upload = result_uploader();
    library(&root, "upload", &declarations(&upload), Some(&upload));
    let ws = program(&root, "upload = { path = \"../upload\" }\n", "0");
    write(&ws, &[("main.st", RESULT_WRITER_TESTS)]);
    tested(&ws, Some("test_result_writer_reports_the_error"))
        .expect("the error is ERROR and ERRORID");
    tested(&ws, Some("test_result_writer_without_an_error")).expect("no error, no ERROR");
}

/// A `BYTES_STREAM_WRITER` makes its stream at its first call and writes
/// it to the reader it is handed to, a component here; `CLOSE` ends the
/// stream once no write is under way, and a request after it, or after the
/// reader left, is an error; the writer lets go of its end once the reader
/// left.
#[test]
fn a_stream_s_writer_writes_and_ends_it() {
    super::disable_stdlib();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let drain = component_of_wat(DRAIN, "drainer", DRAIN_WAT);
    library(&root, "drain", &declarations(&drain), Some(&drain));
    let ws = program(&root, "drain = { path = \"../drain\" }\n", "0");
    write(&ws, &[("main.st", WRITER_TESTS)]);
    tested(&ws, Some("test_writer_ends_the_stream")).expect("the text goes, then the end");
    tested(&ws, Some("test_writer_closes_after_its_write"))
        .expect("CLOSE waits for the write under way");
    tested(&ws, Some("test_writer_sees_its_reader_leave"))
        .expect("the writer sees its reader leave");
    assert!(
        tested(&ws, Some("test_writer_lets_go_when_its_reader_left")).is_err(),
        "the writer let go of its end once the reader left: a second drop traps"
    );
    tested(&ws, Some("test_writer_takes_an_empty_text"))
        .expect("an empty text is done at once, with no reader");
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
