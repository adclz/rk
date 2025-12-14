use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn not_a_callable_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: INT;
    END_VAR

    test();

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Advice: 
       ,-[ file:///test0.st:7:5 ]
       |
     7 |     test();
       |     ^^|^  
       |       `--- 'INT' is not a callable type
    ---'
    ");
}

#[rstest]
fn uninstantied_fb_call(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    fb2();
END_FUNCTION_BLOCK

FUNCTION_BLOCK fb2
END_FUNCTION_BLOCK
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Advice: 
       ,-[ file:///test0.st:3:5 ]
       |
     3 |     fb2();
       |     ^|^  
       |      `--- 'fb2' is not a callable type
       | 
       | Note: to call a FUNCTION_BLOCK, you need to instantiate it first.
    ---'
    ");
}

#[rstest]
fn unknown_input_param(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
     VAR_INPUT
        u: BOOL;
     END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1
    fn(
        unknown := TRUE
    );

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Advice: 
        ,-[ file:///test0.st:10:9 ]
        |
     10 |         unknown := TRUE
        |         ^^^|^^^  
        |            `----- unknown input parameter 'unknown'
    ----'
    ");
}

#[rstest]
fn unknown_output_param(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
END_FUNCTION

FUNCTION_BLOCK fb1
    fn(
        unknown => TRUE
    );

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error:
       ,-[ file:///test0.st:6:5 ]
       |
     2 | FUNCTION fn
       |          ^|
       |           `-- pou 'fn' declared here
       |
     6 |     fn(
       |     ^|
       |      `-- 'fn' expected 0 parameters, but got 1
    ---'
    Error:
       ,-[ file:///test0.st:7:9 ]
       |
     7 |         unknown => TRUE
       |         ^^^|^^^
       |            `----- unknown output 'unknown'
    ---'
    ");
}

#[rstest]
fn type_check_input_param(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
  VAR_INPUT
    param1: INT;
    param2: REAL;
  END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1
    fn(
        param1 := 5.5,
        param2 := 10
    );

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error:
        ,-[ file:///test0.st:11:19 ]
        |
      4 |     param1: INT;
        |             ^|^
        |              `--- expected type 'INT' here
        |
     11 |         param1 := 5.5,
        |                   ^|^
        |                    `--- invalid parameter: invalid INT literal
    ----'
    Error:
        ,-[ file:///test0.st:12:19 ]
        |
      5 |     param2: REAL;
        |             ^^|^
        |               `--- expected type 'REAL' here
        |
     12 |         param2 := 10
        |                   ^|
        |                    `-- invalid parameter: expected a 32-bit floating point number
    ----'
    ");
}

#[rstest]
fn type_check_output_param(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
  VAR_INPUT
    param1: INT;
    param2: REAL;
  END_VAR

  VAR_OUTPUT
    param3: INT;
  END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1
    VAR
        variable1: BOOL;
    END_VAR

    fn(
        param1 := 10,
        param2 := 5.5,
        param3 => variable1
    );

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error:
        ,-[ file:///test0.st:21:19 ]
        |
      9 |     param3: INT;
        |             ^|^
        |              `--- ... but found 'INT' instead
        |
     15 |         variable1: BOOL;
        |                    ^^|^
        |                      `--- expected 'BOOL' here
        |
     21 |         param3 => variable1
        |                   ^^^^|^^^^
        |                       `------ invalid output: expected 'BOOL', found 'INT'
    ----'
    ");
}

#[rstest]
fn mixing_non_formal_and_formal_parameters(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
    VAR_INPUT
        param1: INT;
        param2: REAL;
    END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1

    fn(param1 := 0, 1.2);

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error:
        ,-[ file:///test0.st:11:5 ]
        |
      2 | FUNCTION fn
        |          ^|
        |           `-- pou 'fn' declared here
        |
     11 |     fn(param1 := 0, 1.2);
        |     ^|
        |      `-- mixed formal and non-formal parameters in call to 'fn'
        |
        | Note: parameters must be either all formal or all non-formal
    ----'
    ");
}

#[rstest]
fn too_many_parameters(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
	VAR_INPUT
		param1: INT;
		param2: REAL;
	END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1

	fn(0, 1.5, 5);
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error:
        ,-[ file:///test0.st:11:2 ]
        |
      2 | FUNCTION fn
        |          ^|
        |           `-- pou 'fn' declared here
        |
     11 |     fn(0, 1.5, 5);
        |     ^|
        |      `-- 'fn' expected 2 parameters, but got 3
    ----'
    ");
}

#[rstest]
fn duplicate_input_parameter(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
    VAR_INPUT
        param1: INT;
        param2: INT;
    END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1

    fn(param1 := 0, param1 := 1);

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error:
        ,-[ file:///test0.st:11:21 ]
        |
     11 |     fn(param1 := 0, param1 := 1);
        |        ^^^|^^       ^^^|^^
        |           `----------------- parameter 'param1' is already defined here
        |                        |
        |                        `---- duplicate parameter 'param1'
    ----'
    ");
}

#[rstest]
fn duplicate_output_parameter(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
    VAR_OUTPUT
        param1: INT;
        param2: INT;
    END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1
    VAR
        a1: INT;
        a2: INT;
    END_VAR

    fn(param1 => a1, param1 => a2);

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error:
        ,-[ file:///test0.st:15:22 ]
        |
     15 |     fn(param1 => a1, param1 => a2);
        |        ^^^|^^        ^^^|^^
        |           `------------------ parameter 'param1' is already defined here
        |                         |
        |                         `---- duplicate parameter 'param1'
    ----'
    ");
}

#[rstest]
fn output_assignment_is_not_a_variable(mut with_db: RootDatabase) {
    let source = r#"
TYPE b1 : INT
END_TYPE

FUNCTION fn
    VAR_OUTPUT
        param1: INT;
    END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1

    fn(param1 => b1);

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error:
        ,-[ file:///test0.st:13:18 ]
        |
     13 |     fn(param1 => b1);
        |                  ^|
        |                   `-- 'b1' is a type and can not be assigned
        |
        | Note: types can only be assigned if they are declared in a VAR_* section
    ----'
    ");
}

#[rstest]
fn output_assignment_is_an_input_var(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
    VAR_OUTPUT
        param1: INT;
    END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1
    VAR_INPUT
        b1: INT;
    END_VAR

    fn(param1 => b1);

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error:
        ,-[ file:///test0.st:13:18 ]
        |
     10 |         b1: INT;
        |         ^|
        |          `-- variable 'b1' declared here
        |
     13 |     fn(param1 => b1);
        |                  ^|
        |                   `-- 'b1' is an input variable and can not be assigned
    ----'
    ");
}
