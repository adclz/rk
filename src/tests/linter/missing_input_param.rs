use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn missing_one_input(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION add : INT
VAR_INPUT
    a : INT;
    b : INT;
END_VAR
    add := a + b;
END_FUNCTION

FUNCTION_BLOCK caller
    add(a := 1);
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "missing-input-param"), @r"
    [L0116] Advice: missing input parameter
        ,-[ file:///test0.st:11:5 ]
        |
      5 |     b : INT;
        |     ^^^|^^^
        |        `----- 'b' declared here
        |
     11 |     add(a := 1);
        |     ^^^^^|^^^^^
        |          `------- call to 'add' is missing 1 input parameter: b
        |
        | Note: lint rule: missing-input-param
    ----'
    ");
}

#[rstest]
fn all_inputs_provided(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION add : INT
VAR_INPUT
    a : INT;
    b : INT;
END_VAR
    add := a + b;
END_FUNCTION

FUNCTION_BLOCK caller
    add(a := 1, b := 2);
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "missing-input-param"), @"");
}

#[rstest]
fn no_warning_no_inputs(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION noop : INT
    noop := 0;
END_FUNCTION

FUNCTION_BLOCK caller
    noop();
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "missing-input-param"), @"");
}

#[rstest]
fn missing_multiple_inputs(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION compute : INT
VAR_INPUT
    x : INT;
    y : INT;
    z : INT;
END_VAR
    compute := x + y + z;
END_FUNCTION

FUNCTION_BLOCK caller
    compute(x := 1);
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "missing-input-param"), @r"
    [L0116] Advice: missing input parameter
        ,-[ file:///test0.st:12:5 ]
        |
      5 |     y : INT;
        |     ^^^|^^^
        |        `----- 'y' declared here
      6 |     z : INT;
        |     ^^^|^^^
        |        `----- 'z' declared here
        |
     12 |     compute(x := 1);
        |     ^^^^^^^|^^^^^^^
        |            `--------- call to 'compute' is missing 2 input parameters: y, z
        |
        | Note: lint rule: missing-input-param
    ----'
    ");
}

#[rstest]
fn method_missing_input(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFb
    METHOD PUBLIC set_values
    VAR_INPUT
        a : INT;
        b : INT;
    END_VAR
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK caller
VAR
    fb : MyFb;
END_VAR
    fb.set_values(a := 1);
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "missing-input-param"), @r"
    [L0116] Advice: missing input parameter
        ,-[ file:///test0.st:15:5 ]
        |
      6 |         b : INT;
        |         ^^^|^^^
        |            `----- 'b' declared here
        |
     15 |     fb.set_values(a := 1);
        |     ^^^^^^^^^^|^^^^^^^^^^
        |               `------------ call to 'set_values' is missing 1 input parameter: b
        |
        | Note: lint rule: missing-input-param
    ----'
    ");
}

#[rstest]
fn positional_params_all_provided(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION add : INT
VAR_INPUT
    a : INT;
    b : INT;
END_VAR
    add := a + b;
END_FUNCTION

FUNCTION_BLOCK caller
    add(1, 2);
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "missing-input-param"), @"");
}
