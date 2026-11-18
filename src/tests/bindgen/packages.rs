// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! A package of any namespace: its declarations are in the namespace its
//! package's namespace names, where a type one interface takes from
//! another is found; that root is INTERNAL unless they are written for
//! every program; and a WIT folder is read as WIT's tools lay one out, the
//! packages it uses in its `deps/`.

use db::RootDatabase;
use insta::assert_snapshot;
use rk_bindgen::Reach;
use rstest::rstest;

use super::{Bindings, assert_abi, built, checked, shown, summary};
use crate::tests::utils::with_db;

/// A package whose interfaces share a type.
const DRIVE: &str = r#"
    package example:drive@0.1.0;

    interface types {
        /// What an axis does.
        enum mode { idle, run }
    }

    interface axis {
        use types.{mode};
        current: func() -> mode;
    }

    world plc {
        import axis;
    }
"#;

/// The root is the package's namespace, `example` as `Example`, and a type
/// one interface declares is named from it in another.
#[rstest]
fn a_package_s_namespace_is_its_root(mut with_db: RootDatabase) {
    assert_snapshot!(checked(&mut with_db, DRIVE), @r"
    // example-drive-types.st
    NAMESPACE INTERNAL Example
    	NAMESPACE Drive
    		NAMESPACE Types
    			/* What an axis does. */
    			TYPE MODE : USINT (
    				IDLE := 0,
    				RUN := 1
    			); END_TYPE
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE

    // example-drive-axis.st
    NAMESPACE INTERNAL Example
    	NAMESPACE Drive
    		NAMESPACE Axis
    			{extern 'example:drive/axis@0.1.0' 'current'}
    			FUNCTION CURRENT : Example.Drive.Types.MODE
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
}

/// Written for every program, the root is no longer INTERNAL.
#[rstest]
fn declarations_every_program_reaches(mut with_db: RootDatabase) {
    let (resolve, world) = super::resolved(DRIVE).expect("the WIT resolves");
    let files =
        rk_bindgen::generate_reaching(&resolve, world, Reach::Everyone).expect("it generates");
    let roots: Vec<&str> = files
        .iter()
        .filter_map(|file| file.text.lines().find(|line| line.starts_with("NAMESPACE")))
        .collect();
    assert_eq!(roots, ["NAMESPACE Example", "NAMESPACE Example"]);
    let bindings = Bindings { resolve, files };
    let (mir, module) = built(&mut with_db, &bindings.files, &[]);
    assert_abi(&bindings, &mir, &module);
}

/// A WIT folder: the package's own files, and in `deps/` a package it uses,
/// whose declarations are written too. The world is picked by its name.
#[rstest]
fn a_wit_folder_and_the_packages_in_its_deps(mut with_db: RootDatabase) {
    let folder = tempfile::tempdir().expect("a folder");
    let wit = folder.path().join("wit");
    let units = wit.join("deps").join("units");
    std::fs::create_dir_all(&units).expect("the folders");
    std::fs::write(
        wit.join("drive.wit"),
        r#"
            package example:drive@0.1.0;

            interface axis {
                use example:units/lengths@0.1.0.{meters};
                position: func() -> meters;
            }

            world plc {
                import axis;
            }

            world monitor {
                import example:units/lengths@0.1.0;
            }
        "#,
    )
    .expect("the package");
    std::fs::write(
        units.join("lengths.wit"),
        r#"
            package example:units@0.1.0;

            interface lengths {
                type meters = u32;
            }
        "#,
    )
    .expect("the package it uses");
    let (resolve, world) =
        rk_bindgen::resolve_folder(&wit, Some("plc")).expect("the folder resolves");
    let files = rk_bindgen::generate(&resolve, world).expect("it generates");
    assert_snapshot!(summary(&files), @r"
    example-units-lengths.st: METERS
    example-drive-axis.st: POSITION
    ");
    let bindings = Bindings { resolve, files };
    let (mir, module) = built(&mut with_db, &bindings.files, &[]);
    assert_abi(&bindings, &mir, &module);
    assert_snapshot!(shown(&bindings.files), @r"
    // example-units-lengths.st
    NAMESPACE INTERNAL Example
    	NAMESPACE Units
    		NAMESPACE Lengths
    			TYPE METERS : UDINT; END_TYPE
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE

    // example-drive-axis.st
    NAMESPACE INTERNAL Example
    	NAMESPACE Drive
    		NAMESPACE Axis
    			{extern 'example:drive/axis@0.1.0' 'position'}
    			FUNCTION POSITION : Example.Units.Lengths.METERS
    			END_FUNCTION
    		END_NAMESPACE
    	END_NAMESPACE
    END_NAMESPACE
    ");
    // Two worlds: one must be named.
    let error = rk_bindgen::resolve_folder(&wit, None)
        .map(|_| ())
        .unwrap_err();
    assert!(error.starts_with("selecting the world: "), "{error}");
}

/// `files`, each a path under `folder` and its text, written there.
fn written(folder: &std::path::Path, files: &[(&str, &str)]) {
    for (path, text) in files {
        let path = folder.join(path);
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("the folders");
        std::fs::write(path, text).expect("the file");
    }
}

/// A host's package dropped into a WIT folder needs no world: every
/// interface it declares is written, and the types they take from a package
/// in `deps/`, whose other interfaces are not.
#[test]
fn a_package_without_a_world_is_written_whole() {
    let folder = tempfile::tempdir().expect("a folder");
    let wit = folder.path().join("wit");
    written(
        &wit,
        &[
            (
                "drive.wit",
                "package example:drive@0.1.0;\n\ninterface axis {\n  use example:units/lengths@0.1.0.{meters};\n  position: func() -> meters;\n}\n\ninterface motor {\n  start: func(speed: u16);\n}\n",
            ),
            (
                "deps/units/lengths.wit",
                "package example:units@0.1.0;\n\ninterface lengths {\n  type meters = u32;\n}\n\ninterface areas {\n  type square-meters = u32;\n}\n",
            ),
        ],
    );
    let (resolve, world) = rk_bindgen::resolve_folder(&wit, None).expect("the folder resolves");
    let files = rk_bindgen::generate(&resolve, world).expect("it generates");
    assert_snapshot!(summary(&files), @r"
    example-units-lengths.st: METERS
    example-drive-axis.st: POSITION
    example-drive-motor.st: START
    ");
}

/// A WIT folder of `deps/` alone: every interface of each package in it is
/// written, a package in a single `.wit` file too, and a name spelled like a
/// keyword with them.
#[test]
fn a_folder_of_deps_alone_writes_each_package() {
    let folder = tempfile::tempdir().expect("a folder");
    let wit = folder.path().join("wit");
    written(
        &wit,
        &[
            (
                "deps/drive/axis.wit",
                "package example:drive@0.1.0;\n\ninterface axis {\n  position: func() -> u32;\n}\n",
            ),
            (
                "deps/list.wit",
                "package example:%list@0.1.0;\n\ninterface %stream {\n  next: func() -> u8;\n}\n",
            ),
        ],
    );
    let (resolve, world) = rk_bindgen::resolve_folder(&wit, None).expect("the folder resolves");
    let files = rk_bindgen::generate(&resolve, world).expect("it generates");
    assert_snapshot!(summary(&files), @r"
    example-drive-axis.st: POSITION
    example-list-stream.st: NEXT
    ");
}

/// A world named for a package that declares none is refused, not left
/// aside for every interface of the package.
#[test]
fn a_world_named_for_a_package_without_one_is_refused() {
    let folder = tempfile::tempdir().expect("a folder");
    let wit = folder.path().join("wit");
    written(
        &wit,
        &[(
            "drive.wit",
            "package example:drive@0.1.0;\n\ninterface axis {\n  position: func() -> u32;\n}\n",
        )],
    );
    let error = rk_bindgen::resolve_folder(&wit, Some("plc"))
        .map(|_| ())
        .unwrap_err();
    assert!(
        error.starts_with("selecting the world: ") && error.contains("`plc`"),
        "{error}"
    );
}

/// A folder whose packages declare no interface has nothing to write, and
/// says so.
#[test]
fn a_folder_with_no_interface_is_refused() {
    let folder = tempfile::tempdir().expect("a folder");
    let wit = folder.path().join("wit");
    written(&wit, &[("empty.wit", "package example:empty@0.1.0;\n")]);
    let error = rk_bindgen::resolve_folder(&wit, None)
        .map(|_| ())
        .unwrap_err();
    assert_eq!(
        error,
        format!(
            "`{}` holds no interface to generate: no package with one, at its top or in its deps/",
            wit.display()
        )
    );
}
