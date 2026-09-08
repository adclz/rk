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

/// A METHOD is findable by name. The index held POUs and namespaces only,
/// so searching for one returned every subsequence match except it.
#[rstest]
fn a_method_is_a_workspace_symbol(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Motor
METHOD Spin : INT
END_METHOD
END_FUNCTION_BLOCK

CLASS Engine
METHOD Start : INT
END_METHOD
END_CLASS

INTERFACE Drivable
METHOD Halt : INT
END_METHOD
END_INTERFACE
"#;
    add_sources(&mut with_db, &[source]);

    let found: Vec<String> = ["Spin", "Start", "Halt"]
        .iter()
        .flat_map(|q| ide_proto::handlers::workspace_symbols::workspace_symbols(&with_db, q))
        .map(|symbol| format!("{} {:?}", symbol.name, symbol.kind))
        .collect();

    assert_eq!(found, ["Spin Method", "Start Method", "Halt Method"]);
}

/// Subsequence matching is what a symbol search wants, so `Motor` answers
/// with `MotorController` and `StepperMotor` too. It also lets
/// `test_expt_matches_operator` in, so the ORDER has to say which is which.
#[rstest]
fn workspace_symbols_are_ranked(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Motor
END_FUNCTION_BLOCK
FUNCTION_BLOCK MotorController
END_FUNCTION_BLOCK
FUNCTION_BLOCK StepperMotor
END_FUNCTION_BLOCK
FUNCTION test_expt_matches_operator : INT
END_FUNCTION
"#;
    add_sources(&mut with_db, &[source]);

    let found: Vec<String> =
        ide_proto::handlers::workspace_symbols::workspace_symbols(&with_db, "Motor")
            .into_iter()
            .map(|symbol| symbol.name)
            .collect();

    // Exact, then prefix, then containing, then merely a subsequence.
    assert_eq!(
        found,
        [
            "Motor",
            "MotorController",
            "StepperMotor",
            "test_expt_matches_operator"
        ]
    );
}
