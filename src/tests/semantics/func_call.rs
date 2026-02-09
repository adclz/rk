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
    [E1004] Error: control flow violation
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
    [E1004] Error: control flow violation
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

// should not emit any errors
#[rstest]
fn array_of_fb_instances(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1

END_FUNCTION_BLOCK

FUNCTION_BLOCK fb2
    VAR
        instances: ARRAY[0..1] OF fb1;
    END_VAR

    instances[0]();

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
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
    [E0208] Error: function call parameter mismatch
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
    [E0205] Error: function call parameter mismatch
       ,-[ file:///test0.st:6:5 ]
       |
     6 |     fn(
       |     ^|  
       |      `-- 'fn' expects 0 parameters, but got 1
    ---'
    [E0209] Error: function call parameter mismatch
       ,-[ file:///test0.st:7:9 ]
       |
     7 |         unknown => TRUE
       |         ^^^|^^^  
       |            `----- unknown output parameter 'unknown'
    ---'
    ");
}

#[rstest]
fn type_check_input_param(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
  VAR_INPUT
    param1: LINT;
    param2: LREAL;
  END_VAR
END_FUNCTION

FUNCTION_BLOCK fb1
    fn(
        param1 := 5.5,
        param2 := TRUE
    );

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0309] Error: invalid literal
        ,-[ file:///test0.st:11:19 ]
        |
      4 |     param1: LINT;
        |     ^^^|^^  
        |        `---- type is declared by variable 'param1' here
        | 
     11 |         param1 := 5.5,
        |                   ^|^  
        |                    `--- cannot infer '<float>' to 'LINT': invalid LINT literal
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:12:19 ]
        |
      5 |     param2: LREAL;
        |     ^^^|^^  
        |        `---- type is declared by variable 'param2' here
        | 
     12 |         param2 := TRUE
        |                   ^^|^  
        |                     `--- expected 'LREAL', got 'BOOL'
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
        param1 := TRUE,
        param2 := TRUE,
        param3 => variable1
    );

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:19:19 ]
        |
      4 |     param1: INT;
        |     ^^^|^^  
        |        `---- type is declared by variable 'param1' here
        | 
     19 |         param1 := TRUE,
        |                   ^^|^  
        |                     `--- expected 'INT', got 'BOOL'
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:20:19 ]
        |
      5 |     param2: REAL;
        |     ^^^|^^  
        |        `---- type is declared by variable 'param2' here
        | 
     20 |         param2 := TRUE,
        |                   ^^|^  
        |                     `--- expected 'REAL', got 'BOOL'
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:21:19 ]
        |
     15 |         variable1: BOOL;
        |         ^^^^|^^^^  
        |             `------ type is declared by variable 'variable1' here
        | 
     21 |         param3 => variable1
        |                   ^^^^|^^^^  
        |                       `------ expected 'INT', got 'BOOL'
    ----'
    ");
}

// valid if the order of parameters is correct
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

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
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
    [E0205] Error: function call parameter mismatch
        ,-[ file:///test0.st:11:2 ]
        |
     11 |     fn(0, 1.5, 5);
        |     ^|  
        |      `-- 'fn' expects 2 parameters, but got 3
    ----'
    [E0206] Error: function call parameter mismatch
        ,-[ file:///test0.st:11:13 ]
        |
     11 |     fn(0, 1.5, 5);
        |                |  
        |                `-- no parameter at index '2'
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
    [E0108] Error: duplicate definitions
        ,-[ file:///test0.st:11:21 ]
        |
     11 |     fn(param1 := 0, param1 := 1);
        |        ^^^^^|^^^^^  ^^^^^|^^^^^  
        |             `-------------------- previously defined here
        |                          |       
        |                          `------- duplicate parameter 'param1' found
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
    [E0108] Error: duplicate definitions
        ,-[ file:///test0.st:15:22 ]
        |
     15 |     fn(param1 => a1, param1 => a2);
        |        ^^^^^^|^^^^^  ^^^^^^|^^^^^  
        |              `--------------------- previously defined here
        |                            |       
        |                            `------- duplicate parameter 'param1' found
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
    [E1003] Error: assignment violation
        ,-[ file:///test0.st:13:18 ]
        |
     13 |     fn(param1 => b1);
        |                  ^|  
        |                   `-- cannot use direct type 'b1' here
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
    [E1002] Error: assignment violation
        ,-[ file:///test0.st:13:18 ]
        |
     13 |     fn(param1 => b1);
        |                  ^|  
        |                   `-- b1 is an input variable and can not be assigned
    ----'
    ");
}
