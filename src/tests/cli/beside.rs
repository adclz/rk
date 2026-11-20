// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A library's component runs beside the program, in a store of its own:
//! `rk compile` types the program's imports of its interfaces from the
//! component's binary, and writes the component into `components/` beside
//! the program, where the runtime links each import to the one that exports
//! it.

use std::path::{Path, PathBuf};

use crate::tests::utils::component_of_world;

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
