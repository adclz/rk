use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_lint_diagnostics, with_db};

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
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [W0104] Advice: unused return value
        ,-[ file:///test0.st:11:5 ]
        |
      2 | FUNCTION add : INT
        |          ^|^
        |           `--- FUNCTION 'add' is defined here, with return type 'INT'
        |
     11 |     add(a := 1);
        |     ^^^^^|^^^^^
        |          `------- unused return value of 'add'
    ----'
    [W0116] Warning: missing input parameter
        ,-[ file:///test0.st:11:5 ]
        |
      5 |     b : INT;
        |     ^^^|^^^
        |        `----- 'b' declared here
        |
     11 |     add(a := 1);
        |     ^^^^^|^^^^^
        |          `------- call to 'add' is missing 1 input parameter: b
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
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [W0104] Advice: unused return value
        ,-[ file:///test0.st:11:5 ]
        |
      2 | FUNCTION add : INT
        |          ^|^
        |           `--- FUNCTION 'add' is defined here, with return type 'INT'
        |
     11 |     add(a := 1, b := 2);
        |     ^^^^^^^^^|^^^^^^^^^
        |              `----------- unused return value of 'add'
    ----'
    ");
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
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [W0104] Advice: unused return value
       ,-[ file:///test0.st:7:5 ]
       |
     2 | FUNCTION noop : INT
       |          ^^|^
       |            `--- FUNCTION 'noop' is defined here, with return type 'INT'
       |
     7 |     noop();
       |     ^^^|^^
       |        `---- unused return value of 'noop'
    ---'
    ");
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
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [W0104] Advice: unused return value
        ,-[ file:///test0.st:12:5 ]
        |
      2 | FUNCTION compute : INT
        |          ^^^|^^^
        |             `----- FUNCTION 'compute' is defined here, with return type 'INT'
        |
     12 |     compute(x := 1);
        |     ^^^^^^^|^^^^^^^
        |            `--------- unused return value of 'compute'
    ----'
    [W0116] Warning: missing input parameter
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
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [W0101] Warning: unused code
       ,-[ file:///test0.st:5:9 ]
       |
     5 |         a : INT;
       |         ^^^|^^^
       |            `----- unused variable 'a'
       |
       | Note: if this is intentional, prefix it with an underscore:
       |       '_a'
    ---'
    [W0101] Warning: unused code
       ,-[ file:///test0.st:6:9 ]
       |
     6 |         b : INT;
       |         ^^^|^^^
       |            `----- unused variable 'b'
       |
       | Note: if this is intentional, prefix it with an underscore:
       |       '_b'
    ---'
    [W0116] Warning: missing input parameter
        ,-[ file:///test0.st:15:5 ]
        |
      6 |         b : INT;
        |         ^^^|^^^
        |            `----- 'b' declared here
        |
     15 |     fb.set_values(a := 1);
        |     ^^^^^^^^^^|^^^^^^^^^^
        |               `------------ call to 'set_values' is missing 1 input parameter: b
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
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [W0104] Advice: unused return value
        ,-[ file:///test0.st:11:5 ]
        |
      2 | FUNCTION add : INT
        |          ^|^
        |           `--- FUNCTION 'add' is defined here, with return type 'INT'
        |
     11 |     add(1, 2);
        |     ^^^^|^^^^
        |         `------ unused return value of 'add'
    ----'
    ");
}
