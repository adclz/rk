use db::RootDatabase;
use ide_proto::handlers::workspace_symbols::workspace_symbols;
use insta::assert_debug_snapshot;
use rstest::rstest;

use crate::tests::utils::add_sources;
use crate::tests::utils::with_db;

fn query(db: &RootDatabase, query: &str) -> Vec<(String, String, Option<String>)> {
    workspace_symbols(db, query)
        .into_iter()
        .map(|s| (s.name, format!("{:?}", s.kind), s.container_name))
        .collect()
}

#[rstest]
fn empty_query(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : INT
END_FUNCTION
"#;
    add_sources(&mut with_db, &[source]);
    assert!(query(&with_db, "").is_empty());
}

#[rstest]
fn exact_match(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : INT
END_FUNCTION

FUNCTION_BLOCK fb1
END_FUNCTION_BLOCK
"#;
    add_sources(&mut with_db, &[source]);

    assert_debug_snapshot!(query(&with_db, "fn1"), @r#"
    [
        (
            "fn1",
            "Function",
            None,
        ),
    ]
    "#);
}

#[rstest]
fn fuzzy_match(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION MyLongFunction : INT
END_FUNCTION

FUNCTION_BLOCK MyFunctionBlock
END_FUNCTION_BLOCK
"#;
    add_sources(&mut with_db, &[source]);

    assert_debug_snapshot!(query(&with_db, "mlf"), @r#"
    [
        (
            "MyLongFunction",
            "Function",
            None,
        ),
    ]
    "#);
}

#[rstest]
fn all_pou_kinds(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : INT
END_FUNCTION

FUNCTION_BLOCK fb1
END_FUNCTION_BLOCK

CLASS cls1
END_CLASS

INTERFACE iface1
END_INTERFACE

TYPE
    MyStruct : STRUCT
        x : INT;
    END_STRUCT;
END_TYPE
"#;
    add_sources(&mut with_db, &[source]);

    assert_debug_snapshot!(query(&with_db, "fn1"), @r#"
    [
        (
            "fn1",
            "Function",
            None,
        ),
    ]
    "#);

    assert_debug_snapshot!(query(&with_db, "fb1"), @r#"
    [
        (
            "fb1",
            "Function",
            None,
        ),
    ]
    "#);

    assert_debug_snapshot!(query(&with_db, "cls1"), @r#"
    [
        (
            "cls1",
            "Class",
            None,
        ),
    ]
    "#);

    assert_debug_snapshot!(query(&with_db, "iface1"), @r#"
    [
        (
            "iface1",
            "Interface",
            None,
        ),
    ]
    "#);

    assert_debug_snapshot!(query(&with_db, "MyStruct"), @r#"
    [
        (
            "MyStruct",
            "Struct",
            None,
        ),
    ]
    "#);
}

#[rstest]
fn namespace_symbols(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE MyNamespace
    FUNCTION inner_fn : INT
    END_FUNCTION
END_NAMESPACE
"#;
    add_sources(&mut with_db, &[source]);

    assert_debug_snapshot!(query(&with_db, "inner_fn"), @r#"
    [
        (
            "inner_fn",
            "Function",
            Some(
                "MyNamespace",
            ),
        ),
    ]
    "#);

    assert_debug_snapshot!(query(&with_db, "MyNamespace"), @r#"
    [
        (
            "MyNamespace",
            "Namespace",
            None,
        ),
    ]
    "#);
}

#[rstest]
fn no_match(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : INT
END_FUNCTION
"#;
    add_sources(&mut with_db, &[source]);
    assert!(query(&with_db, "zzzzz").is_empty());
}

#[rstest]
fn multiple_files(mut with_db: RootDatabase) {
    let source1 = r#"
FUNCTION alpha : INT
END_FUNCTION
"#;
    let source2 = r#"
FUNCTION beta : INT
END_FUNCTION
"#;
    add_sources(&mut with_db, &[source1, source2]);

    let results = query(&with_db, "a");
    assert!(results.iter().any(|(name, _, _)| name == "alpha"));
    assert!(results.iter().any(|(name, _, _)| name == "beta"));
}

#[rstest]
fn case_insensitive(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION MyFunc : INT
END_FUNCTION
"#;
    add_sources(&mut with_db, &[source]);

    assert_debug_snapshot!(query(&with_db, "myfunc"), @r#"
    [
        (
            "MyFunc",
            "Function",
            None,
        ),
    ]
    "#);
}