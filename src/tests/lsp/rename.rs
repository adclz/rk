use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::{Url, WorkspaceEdit};
use db::RootDatabase;
use ide_proto::{handlers::RenameHandler, walk::descendant_at};
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{add_sources, with_db};

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

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    let offset = source.find("MyFB").unwrap();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let edit = node.rename(&with_db, "RenamedFB").unwrap();

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

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    let offset = source.find("x : INT").unwrap();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let edit = node.rename(&with_db, "counter").unwrap();

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

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    let offset = source.find("x := 1").unwrap();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let edit = node.rename(&with_db, "counter").unwrap();

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
    let edit = node.rename(&with_db, "CommonFB").unwrap();

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

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    let offset = source.find("x : INT").unwrap();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let edit = node.rename(&with_db, "y").unwrap();

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
    let edit = node.rename(&with_db, "NewNs").unwrap();

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

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    let offset = source.find("fuel: BOOL").unwrap();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let edit = node.rename(&with_db, "gas").unwrap();

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

    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

    // Rename the `engine` field of Engine — referenced through nested access
    let offset = source.find("engine: SubEngine").unwrap();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let edit = node.rename(&with_db, "motor").unwrap();

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
    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();

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
        let edit = node.rename(&with_db, "pump").unwrap();
        assert_eq!(
            apply_rename(&with_db, &edit, &[source]).trim(),
            expected.replace("\n    ", "\n").trim(),
            "renamed from `{needle}`"
        );
    }
}

/// A library file is not the workspace's to edit. Renaming one of its names
/// rewrote every use and left the declaration behind, so the next check
/// reported E0210 on code the reader had not touched.
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
    add_sources(&mut with_db, &[source]);
    let file = *with_db.get_files().iter().last().unwrap();
    let offset = source.find(needle).expect("the name");
    let node = ide_proto::walk::descendant_at(&with_db, file, offset).expect("a node");

    let written = ide_proto::handlers::RenameHandler::rename(&node, &with_db, "Renamed").map(|e| {
        e.changes
            .map(|c| c.values().map(|v| v.len()).sum::<usize>())
            .unwrap_or(0)
    });

    assert_eq!(written, edits);
}
