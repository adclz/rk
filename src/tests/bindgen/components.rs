// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A library's component: rk-bindgen reads what it exports from its binary,
//! which carries its types, and writes it as a program imports it. What the
//! component itself imports is its own.

use std::path::Path;

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;
use wit_parser::Resolve;

use super::{Bindings, assert_abi, built, shown};
use crate::tests::utils::with_db;

/// The component of the world `world` of `wit`, as wit-component builds one
/// from a module that implements it with traps: the binary carries its
/// world.
fn component(wit: &str, world: &str) -> Vec<u8> {
    let mut resolve = Resolve::default();
    let package = resolve
        .push_str("component.wit", wit)
        .expect("the WIT resolves");
    let world = resolve
        .select_world(&[package], Some(world))
        .expect("the world");
    let mut module =
        wit_component::dummy_module(&resolve, world, wit_parser::ManglingAndAbi::Standard32);
    wit_component::embed_component_metadata(
        &mut module,
        &resolve,
        world,
        wit_component::StringEncoding::UTF8,
        false,
    )
    .expect("the module is typed with its world");
    wit_component::ComponentEncoder::default()
        .validate(true)
        .module(&module)
        .and_then(|encoder| encoder.encode())
        .expect("the module wraps into a component")
}

/// A component that exports an axis, whose position is a record of
/// another interface, and imports a log of its own.
const DRIVE: &str = r#"
    package acme:drive@1.0.0;

    interface types {
        record pose { x: s32, y: s32 }
    }

    interface axis {
        use types.{pose};
        position: func() -> pose;
        move: async func(target: s32);
    }

    interface log {
        write: func(text: string);
    }

    world drive {
        import log;
        export axis;
    }
"#;

/// What the component exports is what the program imports: `axis`, after
/// `types`, whose record it takes. `log`, which the component imports, is
/// the component's own, and is not written. A `func` is a FUNCTION, an
/// `async func` a block, as for a host's interface.
#[rstest]
fn a_component_s_exports_are_what_a_program_imports(mut with_db: RootDatabase) {
    let (resolve, world) =
        rk_bindgen::component_exports(&component(DRIVE, "drive")).expect("the component reads");
    let files = rk_bindgen::generate(&resolve, world).expect("its exports are written");
    let bindings = Bindings { resolve, files };
    let (mir, module) = built(&mut with_db, &bindings.files, &[]);
    assert_abi(&bindings, &mir, &module);
    assert_snapshot!(shown(&bindings.files), @r"
    // acme-drive-types.st
    NAMESPACE INTERNAL Acme
    	NAMESPACE Drive
    		NAMESPACE Types
    			TYPE POSE : STRUCT
    				x : DINT;
    				y : DINT;
    			END_STRUCT END_TYPE
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE

    // acme-drive-axis.st
    NAMESPACE INTERNAL Acme
    	NAMESPACE Drive
    		NAMESPACE Axis
    			/* `position` as the host gives it: the values of its result, which
    			   `POSITION` puts in its members. */
    			{extern 'acme:drive/axis@1.0.0' 'position'}
    			FUNCTION PRIVATE POSITION_RAW
    			VAR_OUTPUT
    				x : DINT;
    				y : DINT;
    			END_VAR
    			END_FUNCTION

    			FUNCTION POSITION : Acme.Drive.Types.POSE
    				POSITION_RAW(x => POSITION.x, y => POSITION.y);
    			END_FUNCTION

    			{extern async 'acme:drive/axis@1.0.0' 'move'}
    			FUNCTION_BLOCK MOVE
    			VAR_INPUT
    				target : DINT;
    			END_VAR
    			VAR_OUTPUT
    				DONE : BOOL;
    			END_VAR
    			END_FUNCTION_BLOCK
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// A function the world exports itself, outside an interface, is refused:
/// rk imports a function from an interface. A component that exports no
/// interface has nothing to write, and a WIT package is no component.
#[test]
fn what_is_no_component_s_interface_is_refused() {
    let bare = r#"
        package acme:bare@1.0.0;
        world bare {
            export ping: func() -> u32;
        }
    "#;
    let error = rk_bindgen::component_exports(&component(bare, "bare")).unwrap_err();
    assert_eq!(
        error,
        "the component exports `ping` itself: rk imports a function from an interface"
    );

    let none = r#"
        package acme:none@1.0.0;
        interface log {
            write: func(text: string);
        }
        world none {
            import log;
        }
    "#;
    let error = rk_bindgen::component_exports(&component(none, "none")).unwrap_err();
    assert_eq!(error, "the component exports no interface");

    let mut resolve = Resolve::default();
    let package = resolve
        .push_str("drive.wit", DRIVE)
        .expect("the WIT resolves");
    let package_bytes =
        wit_component::encode(&resolve, package, false).expect("a WIT package encodes");
    let error = rk_bindgen::component_exports(&package_bytes).unwrap_err();
    assert!(
        error.starts_with("a WIT package, not a component"),
        "{error}"
    );
}

/// `rk bindgen` in a library's folder reads the component of its
/// `component/` folder, and writes what it exports into `generated/`,
/// which the library then checks with. The folder holds one `.wasm`: two,
/// or none, are refused.
#[test]
fn rk_bindgen_reads_a_library_s_component() {
    let dir = tempfile::tempdir().unwrap();
    let library = dir.path().join("drives");
    std::fs::create_dir_all(library.join("component")).unwrap();
    std::fs::write(
        library.join("config.toml"),
        "[project]\nname = \"drives\"\nversion = \"1\"\ntype = \"library\"\n",
    )
    .unwrap();
    let wasm = library.join("component").join("drive.wasm");
    std::fs::write(&wasm, component(DRIVE, "drive")).unwrap();
    let run = |command: &str, library: &Path| {
        rk::run_args([
            "rk".as_ref(),
            command.as_ref(),
            "--workspace".as_ref(),
            library.as_os_str(),
        ])
    };

    // A component has its own world: `--world` picks one of a WIT folder.
    let with_world = rk::run_args([
        "rk".as_ref(),
        "bindgen".as_ref(),
        "--world".as_ref(),
        "drive".as_ref(),
        "--workspace".as_ref(),
        library.as_os_str(),
    ]);
    assert!(with_world.is_err(), "a world picked for a component");
    run("bindgen", &library).expect("the component's exports are written");
    let mut written: Vec<String> = std::fs::read_dir(library.join("generated"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    written.sort();
    assert_eq!(written, ["acme-drive-axis.st", "acme-drive-types.st"]);
    run("check", &library).expect("the library checks with what was written");

    let other = library.join("component").join("other.wasm");
    std::fs::copy(&wasm, &other).unwrap();
    assert!(run("bindgen", &library).is_err(), "two components");
    std::fs::remove_file(&other).unwrap();
    std::fs::remove_file(&wasm).unwrap();
    assert!(run("bindgen", &library).is_err(), "no component");
}
