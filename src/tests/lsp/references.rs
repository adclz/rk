use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::Url;
use db::RootDatabase;
use ide_proto::walk::descendant_at;
use insta::assert_debug_snapshot;
use rstest::rstest;

use crate::tests::utils::{add_sources, with_db};

/// Helper: format reference locations as (line, col_start, col_end) for readable snapshots
fn format_references(locations: &[auto_lsp::lsp_types::Location]) -> Vec<String> {
    locations
        .iter()
        .map(|loc| {
            format!(
                "{}:{}-{}:{}",
                loc.range.start.line,
                loc.range.start.character,
                loc.range.end.line,
                loc.range.end.character
            )
        })
        .collect()
}

#[rstest]
fn pou_references(mut with_db: RootDatabase) {
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

    // Click on "MyFB" in the declaration (byte offset within "FUNCTION_BLOCK MyFB")
    let offset = source.find("MyFB").unwrap();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let refs = node.references(&with_db, true).unwrap();

    assert_debug_snapshot!(format_references(&refs), @r#"
    [
        "0:15-0:19",
        "5:8-5:12",
    ]
    "#);
}

#[rstest]
fn pou_references_exclude_declaration(mut with_db: RootDatabase) {
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
    let refs = node.references(&with_db, false).unwrap();

    assert_debug_snapshot!(format_references(&refs), @r#"
    [
        "5:8-5:12",
    ]
    "#);
}

#[rstest]
fn variable_references(mut with_db: RootDatabase) {
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

    // Click on "x" in VAR declaration
    let offset = source.find("x : INT").unwrap();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let refs = node.references(&with_db, true).unwrap();

    assert_debug_snapshot!(format_references(&refs), @r#"
    [
        "2:4-2:5",
        "4:4-4:5",
        "5:4-5:5",
        "5:9-5:10",
    ]
    "#);
}

#[rstest]
fn variable_references_from_usage(mut with_db: RootDatabase) {
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

    // Click on "x" in "x := 1;" (first usage in body)
    let body_start = source.find("x := 1").unwrap();
    let node = descendant_at(&with_db, file, body_start).unwrap();
    let refs = node.references(&with_db, true).unwrap();

    assert_debug_snapshot!(format_references(&refs), @r#"
    [
        "2:4-2:5",
        "4:4-4:5",
        "5:4-5:5",
        "5:9-5:10",
    ]
    "#);
}

#[rstest]
fn cross_file_pou_references(mut with_db: RootDatabase) {
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
    // source1 is file:///test0.st
    let file1 = with_db
        .get_file(&Url::parse("file:///test0.st").unwrap())
        .unwrap();

    // Click on "SharedFB" in the declaration
    let offset = source1.find("SharedFB").unwrap();
    let node = descendant_at(&with_db, file1, offset).unwrap();
    let refs = node.references(&with_db, true).unwrap();

    assert_debug_snapshot!(refs.len(), @"3");
}

#[rstest]
fn no_cross_scope_variable_leak(mut with_db: RootDatabase) {
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

    // Click on "x" in fn1's VAR declaration
    let offset = source.find("x : INT").unwrap();
    let node = descendant_at(&with_db, file, offset).unwrap();
    let refs = node.references(&with_db, true).unwrap();

    // Should only find references in fn1, not fn2
    assert_debug_snapshot!(format_references(&refs), @r#"
    [
        "2:4-2:5",
        "4:4-4:5",
    ]
    "#);
}
