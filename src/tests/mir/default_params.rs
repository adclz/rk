use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use super::utils::mir_exports;
use crate::tests::utils::with_db;

#[rstest]
fn default_string_param_omitted(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION greet
VAR_INPUT
    name: STRING := 'world';
END_VAR
END_FUNCTION

FUNCTION test
    greet();
END_FUNCTION
    "#;
    // `greet` has an empty body (just declares VAR_INPUT, returns nothing
    // — the typical "stub for spec testing" shape). It now appears in
    // exports as a no-op; previously the codegen dropped empty-body
    // POUs entirely, which left call sites resolving to func idx 0.
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export greet(String { capacity: 80 })
    export test()
    ");
}

#[rstest]
fn default_int_param_omitted(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION add_default : INT
VAR_INPUT
    a: INT;
    b: INT := 10;
END_VAR
    add_default := a + b;
END_FUNCTION

FUNCTION test : INT
    test := add_default(a := 5);
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export add_default(Int, Int) -> Int
    export test() -> Int
    ");
}

#[rstest]
fn default_param_with_explicit_override(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION add_default : INT
VAR_INPUT
    a: INT;
    b: INT := 10;
END_VAR
    add_default := a + b;
END_FUNCTION

FUNCTION test : INT
    test := add_default(a := 5, b := 20);
END_FUNCTION
    "#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export add_default(Int, Int) -> Int
    export test() -> Int
    ");
}
