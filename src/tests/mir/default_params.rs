use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::with_db;
use super::utils::mir_exports;

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
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @"export test()");
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
