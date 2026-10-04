// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::{Url, WorkspaceEdit};
use db::RootDatabase;
use ide_proto::{handlers::RenameHandler, walk::descendant_at};
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{add_source, add_sources, with_db};

/// Apply a WorkspaceEdit to the original sources and return the modified text.
/// Sources are concatenated with `---` separators for multi-file snapshots.
fn apply_rename(db: &RootDatabase, edit: &WorkspaceEdit, sources: &[&str]) -> String {
    let changes = edit.changes.as_ref().unwrap();
    let mut results = vec![];

    for (i, source) in sources.iter().enumerate() {
        let url = Url::parse(&format!("file:///test{i}.st")).unwrap();
        let mut text = source.to_string();

        if let Some(edits) = changes.get(&url) {
            // Apply edits in reverse offset order to preserve positions
            let mut sorted: Vec<_> = edits.iter().collect();
            sorted.sort_by(|a, b| {
                b.range
                    .start
                    .cmp(&a.range.start)
                    .then(b.range.end.cmp(&a.range.end))
            });

            let file = db.get_file(&url).unwrap();

            for edit in sorted {
                let start =
                    ide_proto::walk::position_to_offset(db, file, edit.range.start).unwrap();
                let end = ide_proto::walk::position_to_offset(db, file, edit.range.end).unwrap();
                text.replace_range(start..end, &edit.new_text);
            }
        }

        results.push(text);
    }

    results.join("---\n")
}

#[rstest]
fn rename_pou(mut with_db: RootDatabase) {
    let source = r#"FUNCTION_BLOCK MyFB
END_FUNCTION_BLOCK

FUNCTION fn1
VAR
    x : MyFB;
END_VAR
END_FUNCTION
"#;

    let file = add_source(&mut with_db, source);

    let offset = source.find("MyFB").unwrap();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let edit = node.rename(&with_db, offset, "RenamedFB").unwrap();

    assert_snapshot!(apply_rename(&with_db, &edit, &[source]), @r"
    FUNCTION_BLOCK RenamedFB
    END_FUNCTION_BLOCK

    FUNCTION fn1
    VAR
        x : RenamedFB;
    END_VAR
    END_FUNCTION
    ");
}

#[rstest]
fn rename_variable(mut with_db: RootDatabase) {
    let source = r#"FUNCTION fn1
VAR
    x : INT;
END_VAR
    x := 1;
    x := x + 2;
END_FUNCTION
"#;

    let file = add_source(&mut with_db, source);

    let offset = source.find("x : INT").unwrap();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let edit = node.rename(&with_db, offset, "counter").unwrap();

    assert_snapshot!(apply_rename(&with_db, &edit, &[source]), @r"
    FUNCTION fn1
    VAR
        counter : INT;
    END_VAR
        counter := 1;
        counter := counter + 2;
    END_FUNCTION
    ");
}

#[rstest]
fn rename_variable_from_usage(mut with_db: RootDatabase) {
    let source = r#"FUNCTION fn1
VAR
    x : INT;
END_VAR
    x := 1;
    x := x + 2;
END_FUNCTION
"#;

    let file = add_source(&mut with_db, source);

    let offset = source.find("x := 1").unwrap();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let edit = node.rename(&with_db, offset, "counter").unwrap();

    assert_snapshot!(apply_rename(&with_db, &edit, &[source]), @r"
    FUNCTION fn1
    VAR
        counter : INT;
    END_VAR
        counter := 1;
        counter := counter + 2;
    END_FUNCTION
    ");
}

#[rstest]
fn rename_cross_file_pou(mut with_db: RootDatabase) {
    let source1 = r#"FUNCTION_BLOCK SharedFB
END_FUNCTION_BLOCK
"#;

    let source2 = r#"FUNCTION user1
VAR
    a : SharedFB;
END_VAR
END_FUNCTION
"#;

    let source3 = r#"FUNCTION user2
VAR
    b : SharedFB;
END_VAR
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source1, source2, source3]);
    let file1 = with_db
        .get_file(&Url::parse("file:///test0.st").unwrap())
        .unwrap();

    let offset = source1.find("SharedFB").unwrap();
    let node = descendant_at(&with_db, file1, offset).unwrap();
    let edit = node.rename(&with_db, offset, "CommonFB").unwrap();

    assert_snapshot!(apply_rename(&with_db, &edit, &[source1, source2, source3]), @r"
    FUNCTION_BLOCK CommonFB
    END_FUNCTION_BLOCK
    ---
    FUNCTION user1
    VAR
        a : CommonFB;
    END_VAR
    END_FUNCTION
    ---
    FUNCTION user2
    VAR
        b : CommonFB;
    END_VAR
    END_FUNCTION
    ");
}

#[rstest]
fn rename_no_cross_scope_leak(mut with_db: RootDatabase) {
    let source = r#"FUNCTION fn1
VAR
    x : INT;
END_VAR
    x := 1;
END_FUNCTION

FUNCTION fn2
VAR
    x : INT;
END_VAR
    x := 2;
END_FUNCTION
"#;

    let file = add_source(&mut with_db, source);

    let offset = source.find("x : INT").unwrap();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let edit = node.rename(&with_db, offset, "y").unwrap();

    assert_snapshot!(apply_rename(&with_db, &edit, &[source]), @r"
    FUNCTION fn1
    VAR
        y : INT;
    END_VAR
        y := 1;
    END_FUNCTION

    FUNCTION fn2
    VAR
        x : INT;
    END_VAR
        x := 2;
    END_FUNCTION
    ");
}

#[rstest]
fn rename_namespace(mut with_db: RootDatabase) {
    let source1 = r#"NAMESPACE MyNs
    FUNCTION fn1 : INT
    END_FUNCTION
END_NAMESPACE
"#;

    let source2 = r#"USING MyNs;
FUNCTION fn2 : INT
END_FUNCTION
"#;

    add_sources(&mut with_db, &[source1, source2]);
    let file1 = with_db
        .get_file(&Url::parse("file:///test0.st").unwrap())
        .unwrap();

    let offset = source1.find("MyNs").unwrap();
    let node = descendant_at(&with_db, file1, offset).unwrap();
    let edit = node.rename(&with_db, offset, "NewNs").unwrap();

    assert_snapshot!(apply_rename(&with_db, &edit, &[source1, source2]), @r"
    NAMESPACE NewNs
        FUNCTION fn1 : INT
        END_FUNCTION
    END_NAMESPACE
    ---
    USING NewNs;
    FUNCTION fn2 : INT
    END_FUNCTION
    ");
}

#[rstest]
fn rename_struct_field(mut with_db: RootDatabase) {
    let source = r#"TYPE
    Engine: STRUCT
        oil: INT;
        fuel: BOOL;
    END_STRUCT
END_TYPE

FUNCTION_BLOCK fb
    VAR
        my_var: Engine;
    END_VAR

    my_var.fuel := my_var.fuel;

    my_var.fuel := 0;

END_FUNCTION_BLOCK
"#;

    let file = add_source(&mut with_db, source);

    let offset = source.find("fuel: BOOL").unwrap();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let edit = node.rename(&with_db, offset, "gas").unwrap();

    assert_snapshot!(apply_rename(&with_db, &edit, &[source]), @r"
    TYPE
        Engine: STRUCT
            oil: INT;
            gas: BOOL;
        END_STRUCT
    END_TYPE

    FUNCTION_BLOCK fb
        VAR
            my_var: Engine;
        END_VAR

        my_var.gas := my_var.gas;

        my_var.gas := 0;

    END_FUNCTION_BLOCK
    ");
}

#[rstest]
fn rename_nested_struct_field(mut with_db: RootDatabase) {
    let source = r#"TYPE
    SubEngine: STRUCT
        oil: INT;
        fuel: BOOL;
    END_STRUCT
    Engine: STRUCT
        engine: SubEngine;
    END_STRUCT
END_TYPE

FUNCTION_BLOCK fb
    VAR
        my_var: Engine;
    END_VAR

    my_var.engine.fuel := my_var.engine.fuel;

    my_var.engine.fuel := 0;

END_FUNCTION_BLOCK
"#;

    let file = add_source(&mut with_db, source);

    // Rename the `engine` field of Engine — referenced through nested access
    let offset = source.find("engine: SubEngine").unwrap();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let edit = node.rename(&with_db, offset, "motor").unwrap();

    assert_snapshot!(apply_rename(&with_db, &edit, &[source]), @r"
    TYPE
        SubEngine: STRUCT
            oil: INT;
            fuel: BOOL;
        END_STRUCT
        Engine: STRUCT
            motor: SubEngine;
        END_STRUCT
    END_TYPE

    FUNCTION_BLOCK fb
        VAR
            my_var: Engine;
        END_VAR

        my_var.motor.fuel := my_var.motor.fuel;

        my_var.motor.fuel := 0;

    END_FUNCTION_BLOCK
    ");
}

/// An instance is not its block. `motor` in `motor()` is inferred as the
/// block it invokes, and a rename taken from there used to rename `Engine`
/// itself, every declaration of that type included, while a rename from the
/// declaration missed the call. Both now rename the variable, and only it.
#[rstest]
fn rename_an_instance_not_its_block(mut with_db: RootDatabase) {
    let source = r#"FUNCTION_BLOCK Engine
    METHOD start : BOOL
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION fn1
VAR
    motor : Engine;
    other : Engine;
END_VAR
    motor();
    motor.start();
    other();
END_FUNCTION
"#;
    let file = add_source(&mut with_db, source);

    let expected = r"
    FUNCTION_BLOCK Engine
        METHOD start : BOOL
        END_METHOD
    END_FUNCTION_BLOCK

    FUNCTION fn1
    VAR
        pump : Engine;
        other : Engine;
    END_VAR
        pump();
        pump.start();
        other();
    END_FUNCTION
    ";
    for needle in ["motor : Engine", "motor();", "motor.start"] {
        let offset = source.find(needle).unwrap();
        let node = descendant_at(&with_db, file, offset).unwrap();
        let edit = node.rename(&with_db, offset, "pump").unwrap();
        assert_eq!(
            apply_rename(&with_db, &edit, &[source]).trim(),
            expected.replace("\n    ", "\n").trim(),
            "renamed from `{needle}`"
        );
    }
}

/// A library file is not the workspace's to edit. Renaming one of its names
/// rewrote every use and left the declaration behind, so the next check
/// reported E0203 on code the reader had not touched.
#[rstest]
#[case::a_library_use("LibFn()", None)]
#[case::a_workspace_use("Mine();", Some(2))]
#[case::a_workspace_declaration("Mine : INT", Some(2))]
pub fn a_library_name_is_not_renamed(
    mut with_db: RootDatabase,
    #[case] needle: &str,
    #[case] edits: Option<usize>,
) {
    crate::tests::utils::add_library_sources(
        &mut with_db,
        &["FUNCTION LibFn : INT\nEND_FUNCTION\n"],
    );
    let source = r#"
FUNCTION Mine : INT
END_FUNCTION

FUNCTION caller : INT
    caller := LibFn() + Mine();
END_FUNCTION
"#;
    let file = add_source(&mut with_db, source);
    let offset = source.find(needle).expect("the name");
    let node = ide_proto::walk::descendant_at(&with_db, file, offset).expect("a node");

    let written = ide_proto::handlers::RenameHandler::rename(&node, &with_db, offset, "Renamed")
        .map(|e| {
            e.changes
                .map(|c| c.values().map(|v| v.len()).sum::<usize>())
                .unwrap_or(0)
        });

    assert_eq!(written, edits);
}

/// A rename reaches the names a configuration writes: a program's variable in
/// its element list, a VAR_GLOBAL a connection names, and a member a
/// VAR_CONFIG path goes through. Renaming `x1` used to leave `F(x1 := ...)`
/// behind, where it became E1414.
#[rstest]
#[case::from_the_declaration("x1 : BOOL", false, "start", &["VAR_INPUT start : BOOL", "F(start := %IX0.0"])]
#[case::from_the_element("x1 := %IX", true, "start", &["VAR_INPUT start : BOOL", "F(start := %IX0.0"])]
#[case::global("w : UINT", false, "speed", &["VAR_GLOBAL speed : UINT", "x2 := speed,"])]
#[case::var_config_member("out AT %Q*", false, "level", &["VAR level AT %Q*", "Res.P1.d.level AT %QW0"])]
fn rename_in_a_program_configuration(
    mut with_db: RootDatabase,
    #[case] at: &str,
    #[case] in_config: bool,
    #[case] new_name: &str,
    #[case] expected: &[&str],
) {
    use crate::tests::lsp::{CONNECTED, connected_at};
    let file = add_source(&mut with_db, CONNECTED);

    let offset = connected_at(at, in_config);
    let node = descendant_at(&with_db, file, offset).unwrap();
    let edit = node.rename(&with_db, offset, new_name).expect("an edit");
    let renamed = apply_rename(&with_db, &edit, &[CONNECTED]);
    for text in expected {
        assert!(renamed.contains(text), "missing `{text}`:\n{renamed}");
    }
}

/// A rename edits names and nothing else. A binding `in := x` names its
/// parameter, so renaming the parameter edits `in`; a positional argument
/// names nothing and is left alone. Each used to be replaced whole, so
/// `g(a0 := 1, a1 := 3)` came back as `g(renamed, a1 := 3)` and `g(v)` as
/// `g(renamed)`.
#[rstest]
fn rename_a_parameter_edits_its_bindings_only(mut with_db: RootDatabase) {
    let source = r#"FUNCTION g : INT
VAR_INPUT a0 : INT; a1 : INT; END_VAR
VAR_OUTPUT o : INT; END_VAR
    g := a0 + a1;
END_FUNCTION

FUNCTION caller : INT
VAR v : INT; r : INT; END_VAR
    caller := g(a0 := 1, a1 := 3) + g(v, 2) + g(a0 := g(a0 := 4, a1 := 5), a1 := v, o => r);
END_FUNCTION
"#;
    let file = add_source(&mut with_db, source);
    let offset = source.find("a0 : INT").unwrap();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let edit = node.rename(&with_db, offset, "first").unwrap();

    assert_snapshot!(apply_rename(&with_db, &edit, &[source]), @r"
    FUNCTION g : INT
    VAR_INPUT first : INT; a1 : INT; END_VAR
    VAR_OUTPUT o : INT; END_VAR
        g := first + a1;
    END_FUNCTION

    FUNCTION caller : INT
    VAR v : INT; r : INT; END_VAR
        caller := g(first := 1, a1 := 3) + g(v, 2) + g(first := g(first := 4, a1 := 5), a1 := v, o => r);
    END_FUNCTION
    ");

    let offset = source.find("o : INT").unwrap();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let edit = node.rename(&with_db, offset, "out").unwrap();
    assert_snapshot!(apply_rename(&with_db, &edit, &[source]), @r"
    FUNCTION g : INT
    VAR_INPUT a0 : INT; a1 : INT; END_VAR
    VAR_OUTPUT out : INT; END_VAR
        g := a0 + a1;
    END_FUNCTION

    FUNCTION caller : INT
    VAR v : INT; r : INT; END_VAR
        caller := g(a0 := 1, a1 := 3) + g(v, 2) + g(a0 := g(a0 := 4, a1 := 5), a1 := v, out => r);
    END_FUNCTION
    ");
}

/// A namespace declared with a dotted name is renamed by its last segment,
/// in its declarations and in the USINGs that name it; the segments before
/// it are its parents. With the cursor on a parent segment nothing is
/// edited from here: the parent is renamed where it is declared. Replacing
/// the whole name wrote `NAMESPACE Drives` over `NAMESPACE App.Motors`.
#[rstest]
fn rename_a_dotted_namespace_by_its_last_segment(mut with_db: RootDatabase) {
    let source1 = r#"NAMESPACE App.Motors
    FUNCTION fn1 : INT
    END_FUNCTION
END_NAMESPACE

NAMESPACE App
    NAMESPACE Motors
        FUNCTION fn2 : INT
        END_FUNCTION
    END_NAMESPACE
END_NAMESPACE
"#;
    let source2 = r#"USING App.Motors;
FUNCTION fn3 : INT
END_FUNCTION
"#;
    add_sources(&mut with_db, &[source1, source2]);
    let file1 = with_db
        .get_file(&Url::parse("file:///test0.st").unwrap())
        .unwrap();

    let offset = source1.find("Motors").unwrap();
    let node = descendant_at(&with_db, file1, offset).unwrap();
    let edit = node.rename(&with_db, offset, "Drives").unwrap();
    assert_snapshot!(apply_rename(&with_db, &edit, &[source1, source2]), @r"
    NAMESPACE App.Drives
        FUNCTION fn1 : INT
        END_FUNCTION
    END_NAMESPACE

    NAMESPACE App
        NAMESPACE Drives
            FUNCTION fn2 : INT
            END_FUNCTION
        END_NAMESPACE
    END_NAMESPACE
    ---
    USING App.Drives;
    FUNCTION fn3 : INT
    END_FUNCTION
    ");

    let offset = source1.find("App.Motors").unwrap();
    let node = descendant_at(&with_db, file1, offset).unwrap();
    assert!(
        node.rename(&with_db, offset, "Plant").is_none(),
        "a parent segment is renamed at its own declaration"
    );
}

/// A qualified type reference, `x : Lib.T`, is edited at its name: the
/// whole path was replaced, and the namespace went with it. A located
/// variable declared without a name, `AT %QB4 : INT`, has its address
/// where a name would be, and is not renamed.
#[rstest]
fn rename_edits_names_only(mut with_db: RootDatabase) {
    let source = r#"NAMESPACE Lib
    FUNCTION_BLOCK T
    END_FUNCTION_BLOCK
END_NAMESPACE

PROGRAM Main
VAR t : Lib.T; u : Lib.T; END_VAR
VAR AT %QB4 : INT; END_VAR
END_PROGRAM
"#;
    let file = add_source(&mut with_db, source);
    let offset = source.find("FUNCTION_BLOCK T").unwrap() + "FUNCTION_BLOCK ".len();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let edit = node.rename(&with_db, offset, "Timer").unwrap();
    assert_snapshot!(apply_rename(&with_db, &edit, &[source]), @r"
    NAMESPACE Lib
        FUNCTION_BLOCK Timer
        END_FUNCTION_BLOCK
    END_NAMESPACE

    PROGRAM Main
    VAR t : Lib.Timer; u : Lib.Timer; END_VAR
    VAR AT %QB4 : INT; END_VAR
    END_PROGRAM
    ");

    let offset = source.find("%QB4").unwrap();
    let node = descendant_at(&with_db, file, offset).unwrap();
    assert!(node.rename(&with_db, offset, "out").is_none());
}
