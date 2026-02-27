use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::Url;
use db::RootDatabase;
use ide_proto::handlers::references::ReferenceLocation;
use ide_proto::walk::descendant_at;
use insta::assert_debug_snapshot;
use rstest::rstest;

use crate::tests::utils::{add_sources, with_db};

/// Helper: format reference locations as (line, col_start, col_end) for readable snapshots
fn format_references(locations: &[ReferenceLocation]) -> Vec<String> {
    locations
        .iter()
        .map(|loc| {
            let range: auto_lsp::lsp_types::Range = loc.span.into();
            format!(
                "{}:{}-{}:{}",
                range.start.line,
                range.start.character,
                range.end.line,
                range.end.character
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

#[rstest]
fn namespace_references(mut with_db: RootDatabase) {
    let source1 = r#"NAMESPACE MyNs
    FUNCTION fn1 : INT
    END_FUNCTION
END_NAMESPACE
"#;

    let source2 = r#"NAMESPACE MyNs
    FUNCTION fn2 : INT
    END_FUNCTION
END_NAMESPACE
"#;

    add_sources(&mut with_db, &[source1, source2]);
    let file1 = with_db
        .get_file(&Url::parse("file:///test0.st").unwrap())
        .unwrap();

    // Click on "MyNs" in the first file's namespace declaration
    let offset = source1.find("MyNs").unwrap();
    let node = descendant_at(&with_db, file1, offset).unwrap();
    let refs = node.references(&with_db, true).unwrap();

    // Should find both namespace declarations
    assert_debug_snapshot!(refs.len(), @"2");
}

#[rstest]
fn namespace_references_with_using(mut with_db: RootDatabase) {
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

    // Click on "MyNs" in the namespace declaration
    let offset = source1.find("MyNs").unwrap();
    let node = descendant_at(&with_db, file1, offset).unwrap();
    let refs = node.references(&with_db, true).unwrap();

    // Should find the namespace declaration + the USING statement
    assert_debug_snapshot!(refs.len(), @"2");
}

#[rstest]
fn using_references_from_using(mut with_db: RootDatabase) {
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
    let file2 = with_db
        .get_file(&Url::parse("file:///test1.st").unwrap())
        .unwrap();

    // Click on "MyNs" in the USING statement
    let offset = source2.find("MyNs").unwrap();
    let node = descendant_at(&with_db, file2, offset).unwrap();
    let refs = node.references(&with_db, true).unwrap();

    // Should find namespace declaration + USING statement
    assert_debug_snapshot!(refs.len(), @"2");
}
