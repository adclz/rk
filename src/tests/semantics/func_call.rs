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
    Error: 
       ,-[ file:///test0.st:7:5 ]
       |
     4 |         test: INT;
       |         ^^|^  ^|^  
       |           `-------- 'test' is declared here
       |                |   
       |                `--- type defined here
       | 
     7 |     test();
       |     ^^|^  
       |       `--- cannot call non-callable type 'test'
       | 
       | Note: only functions, function blocks or methods can be called
    ---'
    ");
}

#[rstest]
fn unused_return_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn: BOOL

END_FUNCTION

FUNCTION_BLOCK fb1
    fn();

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Warning: 
       ,-[ file:///test0.st:7:5 ]
       |
     2 | FUNCTION fn: BOOL
       |          ^|  ^^|^  
       |           `-------- 'fn' is declared here
       |                |   
       |                `--- type defined here
       | 
     7 |     fn();
       |     ^|  
       |      `-- unused return type of 'fn'
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
    Error: 
        ,-[ file:///test0.st:10:9 ]
        |
      2 | ,-> FUNCTION fn
        | |            ^|  
        | |             `-- 'fn' is declared here
        : :   
      6 | |-> END_FUNCTION
        | |                  
        | `------------------ type defined here
        | 
     10 |             unknown := TRUE
        |             ^^^|^^^  
        |                `----- unknown input parameter 'unknown'
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
     2 |   ,-> FUNCTION fn
       |   |            ^|  
       |   |             `-- 'fn' is declared here
     3 |   |-> END_FUNCTION
       |   |                  
       |   `------------------ type defined here
       | 
     6 | ,--->     fn(
       : :     
     8 | |--->     );
       | |              
       | `-------------- 'fn' expected 0 parameters, but got 1
    ---'
    Error: 
       ,-[ file:///test0.st:7:9 ]
       |
     2 | ,-> FUNCTION fn
       | |            ^|  
       | |             `-- 'fn' is declared here
     3 | |-> END_FUNCTION
       | |                  
       | `------------------ type defined here
       | 
     7 |             unknown => TRUE
       |             ^^^|^^^  
       |                `----- unknown output parameter 'unknown'
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
        |     ^^^|^^  ^|^  
        |        `--------- 'param1' is declared here
        |              |   
        |              `--- type defined here
        | 
     11 |         param1 := 5.5,
        |                   ^|^  
        |                    `--- parameter expression mismatch: invalid INT literal
    ----'
    Error: 
        ,-[ file:///test0.st:12:19 ]
        |
      5 |     param2: REAL;
        |     ^^^|^^  ^^|^  
        |        `---------- 'param2' is declared here
        |               |   
        |               `--- type defined here
        | 
     12 |         param2 := 10
        |                   ^|  
        |                    `-- parameter expression mismatch: expected a 32-bit floating point number
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
        |     ^^^|^^  ^|^  
        |        `--------- 'param3' is declared here
        |              |   
        |              `--- type defined here
        | 
     15 |         variable1: BOOL;
        |         ^^^^|^^^^  ^^|^  
        |             `------------ 'variable1' is declared here
        |                      |   
        |                      `--- type defined here
        | 
     21 |         param3 => variable1
        |                   ^^^^|^^^^  
        |                       `------ invalid output assignment: type mismatch: expected INT, found BOOL
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
      2 | ,-> FUNCTION fn
        | |            ^|  
        | |             `-- 'fn' is declared here
        : :   
      7 | |-> END_FUNCTION
        | |                  
        | `------------------ type defined here
        | 
     11 |         fn(0, 1.5, 5);
        |         ^^^^^^|^^^^^^  
        |               `-------- 'fn' expected 2 parameters, but got 3
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
      2 | TYPE b1 : INT
        |      ^|   ^|^  
        |       `-------- 'b1' is declared here
        |            |   
        |            `--- type defined here
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
    Warning: 
        ,-[ file:///test0.st:13:18 ]
        |
     10 |         b1: INT;
        |         ^|  
        |          `-- 'b1' is declared here
        | 
     13 |     fn(param1 => b1);
        |                  ^|  
        |                   `-- 'b1' is an input variable and should not be assigned
    ----'
    ");
}
