use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn missing_identifier(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE
END_NAMESPACE"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0019] Error: syntax
       ,-[ file:///test0.st:2:10 ]
       |
     2 | NAMESPACE
       |          |
       |          `- Syntax error: Missing 'identifier'
       |          |
       |          `- add missing identifier here
    ---'
    ");
}

#[rstest]
fn missing_end_keyword(mut with_db: RootDatabase) {
    let source = r#"
    FUNCTION myFunc : INT


    "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0019] Error: syntax
       ,-[ file:///test0.st:2:26 ]
       |
     2 |     FUNCTION myFunc : INT
       |                          |
       |                          `- Syntax error: Missing 'END_FUNCTION'
       |                          |
       |                          `- add missing END_FUNCTION here
    ---'
    ");
}

#[rstest]
fn unexpected_symbol(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE test ;
END_NAMESPACE"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0050] Error: syntax
       ,-[ file:///test0.st:2:16 ]
       |
     2 | NAMESPACE test ;
       |                |
       |                `-- Unexpected token(s): ';'
    ---'
    ");
}

#[rstest]
fn implements_before_extends(mut with_db: RootDatabase) {
    let source = r#"
CLASS b
END_CLASS

FUNCTION_BLOCK fn IMPLEMENTS a EXTENDS b

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0003] Error: syntax
       ,-[ file:///test0.st:5:19 ]
       |
     5 | FUNCTION_BLOCK fn IMPLEMENTS a EXTENDS b
       |                   ^^^^^^|^^^^^
       |                         `------- implements must be declared after extends
    ---'
    ");
}

#[rstest]
fn implements_multiple_times(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE a
END_INTERFACE

INTERFACE b
END_INTERFACE

FUNCTION_BLOCK fn IMPLEMENTS a IMPLEMENTS b

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0002] Error: syntax
       ,-[ file:///test0.st:8:32 ]
       |
     8 | FUNCTION_BLOCK fn IMPLEMENTS a IMPLEMENTS b
       |                                ^^^^^^|^^^^^
       |                                      `------- multiple implements declarations
    ---'
    ");
}

#[rstest]
fn extends_multiple_times(mut with_db: RootDatabase) {
    let source = r#"
CLASS a
END_CLASS

CLASS b
END_CLASS

FUNCTION_BLOCK fn EXTENDS a EXTENDS b

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0001] Error: syntax
       ,-[ file:///test0.st:8:29 ]
       |
     8 | FUNCTION_BLOCK fn EXTENDS a EXTENDS b
       |                             ^^^^|^^^^
       |                                 `------ multiple extends declarations
    ---'
    ");
}

#[rstest]
fn variable_with_no_spec(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn
    VAR_INPUT
        empty
    END_VAR

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0006] Error: syntax
       ,-[ file:///test0.st:4:9 ]
       |
     4 |         empty
       |         ^^|^^
       |           `---- variable type is missing
    ---'
    ");
}

#[rstest]
fn function_call_as_assignment(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn
    fn() := 0;
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0011] Error: syntax
       ,-[ file:///test0.st:3:5 ]
       |
     3 |     fn() := 0;
       |     ^^|^
       |       `--- assignment to function call is not allowed
    ---'
    ");
}

#[rstest]
fn unexpected_this_in_path(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn
    fn.THIS.p := 5
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0009] Error: syntax
       ,-[ file:///test0.st:3:8 ]
       |
     3 |     fn.THIS.p := 5
       |        ^^|^
       |          `--- 'THIS' is not valid in this context
    ---'
    ");
}

#[rstest]
fn unexpected_super_in_path(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn
    fn.SUPER.p := 5
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0010] Error: syntax
       ,-[ file:///test0.st:3:8 ]
       |
     3 |     fn.SUPER.p := 5
       |        ^^|^^
       |          `---- 'SUPER' is not valid in this context
    ---'
    ");
}

#[rstest]
fn empty_right_hand_assignment(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn
    a :=
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0012] Error: syntax
       ,-[ file:///test0.st:3:7 ]
       |
     3 |     a :=
       |       ^|
       |        `-- right-hand side of assignment cannot be empty
    ---'
    ");
}

#[rstest]
fn function_call_in_init_expression(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn
  VAR
    ml : ARRAY [0..2] OF INT := [10(call(IN := 5, OUT => OUT))]
  END_VAR

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0017] Error: syntax
       ,-[ file:///test0.st:4:37 ]
       |
     4 |     ml : ARRAY [0..2] OF INT := [10(call(IN := 5, OUT => OUT))]
       |                                     ^^^^^^^^^^^^|^^^^^^^^^^^^
       |                                                 `-------------- function call in initialization expression is not allowed
    ---'
    [E0605] Error: invalid array access
       ,-[ file:///test0.st:4:34 ]
       |
     4 |     ml : ARRAY [0..2] OF INT := [10(call(IN := 5, OUT => OUT))]
       |                                  ^^^^^^^^^^^^^^|^^^^^^^^^^^^^^
       |                                                `---------------- too many elements in array initializer (expected at most 3)
    ---'
    ");
}

#[rstest]
fn missing_dot_in_assign(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn
    a = 0;
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0013] Error: syntax
       ,-[ file:///test0.st:3:7 ]
       |
     3 |     a = 0;
       |       ^|^
       |        `--- '=' is not a valid assignment sign
       |
       | Help: replace '=' with ':='
    ---'
    ");
}

#[rstest]
fn missing_equal_in_assign(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn
    a : 0;
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0014] Error: syntax
       ,-[ file:///test0.st:3:7 ]
       |
     3 |     a : 0;
       |       ^|^
       |        `--- ':' is not a valid assignment sign
       |
       | Help: replace ':' with ':='
    ---'
    ");
}

#[rstest]
fn missing_dot_in_for_list(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
  VAR i : INT END_VAR
	FOR i = 0 TO 10 END_FOR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0015] Error: syntax
       ,-[ file:///test0.st:4:8 ]
       |
     4 |     FOR i = 0 TO 10 END_FOR
       |           |
       |           `-- '=' is not a valid assignment sign
       |
       | Help: replace '=' with ':='
    ---'
    ");
}

#[rstest]
fn missing_equal_in_for_list(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
	VAR i : INT END_VAR
	FOR i : 0 TO 10 END_FOR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0016] Error: syntax
       ,-[ file:///test0.st:4:8 ]
       |
     4 |     FOR i : 0 TO 10 END_FOR
       |           |
       |           `-- ':' is not a valid assignment sign
       |
       | Help: replace ':' with ':='
    ---'
    ");
}

#[rstest]
fn class_variables_before_method(mut with_db: RootDatabase) {
    let source = r#"
CLASS base

    METHOD PROTECTED myProtectedMethod END_METHOD

    VAR
        obj: Base;
    END_VAR

    VAR
        obj: Base;
    END_VAR

END_CLASS"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0004] Error: syntax
        ,-[ file:///test0.st:6:5 ]
        |
      6 | ,->     VAR
        : :
     12 | |->     END_VAR
        | |
        | `----------------- class variable declarations must appear before methods
    ----'
    ");
}

#[rstest]
fn fb_variables_before_method(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1

    METHOD PROTECTED myProtectedMethod END_METHOD

    VAR
        obj: Base;
    END_VAR

    VAR
        obj: Base;
    END_VAR

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0005] Error: syntax
        ,-[ file:///test0.st:6:5 ]
        |
      6 | ,->     VAR
        : :
     12 | |->     END_VAR
        | |
        | `----------------- FB variable declarations must appear before methods
    ----'
    ");
}

#[rstest]
fn output_assign_in_assignment(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn
    a => 0;
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0020] Error: syntax
       ,-[ file:///test0.st:3:7 ]
       |
     3 |     a => 0;
       |       ^^|^
       |         `--- '=>' is not a valid assignment sign
       |
       | Help: replace '=>' with ':='
    ---'
    ");
}

#[rstest]
fn output_assign_in_for_list(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
  VAR i : INT END_VAR
  FOR i => 0 TO 10 END_FOR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0021] Error: syntax
       ,-[ file:///test0.st:4:9 ]
       |
     4 |   FOR i => 0 TO 10 END_FOR
       |         ^|
       |          `-- '=>' is not a valid assignment sign
       |
       | Help: replace '=>' with ':='
    ---'
    ");
}

#[rstest]
fn program_not_allowed_in_namespace(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE test
    PROGRAM myProgram
    END_PROGRAM
END_NAMESPACE
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0022] Error: syntax
       ,-[ file:///test0.st:3:5 ]
       |
     3 | ,->     PROGRAM myProgram
     4 | |->     END_PROGRAM
       | |
       | `--------------------- programs are not allowed in namespaces
    ---'
    ");
}

#[rstest]
fn config_not_allowed_in_namespace(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE test
    CONFIGURATION myConfig
    END_CONFIGURATION
END_NAMESPACE
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0023] Error: syntax
       ,-[ file:///test0.st:3:5 ]
       |
     3 | ,->     CONFIGURATION myConfig
     4 | |->     END_CONFIGURATION
       | |
       | `--------------------------- configs are not allowed in namespaces
    ---'
    ");
}

#[rstest]
fn invalid_class_variable_sections(mut with_db: RootDatabase) {
    let source = r#"
CLASS cl
  VAR_IN_OUT

  END_VAR

  VAR_TEMP

  END_VAR

  VAR_ACCESS

  END_VAR

  VAR_CONFIG

  END_VAR

  VAR_LOCATED

  END_VAR

  VAR_EXTERNAL

  END_VAR

  VAR_GLOBAL

  END_VAR
END_CLASS
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0024] Error: syntax
       ,-[ file:///test0.st:3:3 ]
       |
     3 | ,->   VAR_IN_OUT
       : :
     5 | |->   END_VAR
       | |
       | `--------------- VAR_IN_OUT is not allowed in this context
       |
       |     Note: VAR_IN_OUT can only be used inside FUNCTION, FUNCTION_BLOCK
    ---'
    [E0025] Error: syntax
       ,-[ file:///test0.st:7:3 ]
       |
     7 | ,->   VAR_TEMP
       : :
     9 | |->   END_VAR
       | |
       | `--------------- VAR_TEMP is not allowed in this context
       |
       |     Note: VAR_TEMP can only be used inside FUNCTION, FUNCTION_BLOCK
    ---'
    [E0026] Error: syntax
        ,-[ file:///test0.st:11:3 ]
        |
     11 | ,->   VAR_ACCESS
        : :
     13 | |->   END_VAR
        | |
        | `--------------- VAR_ACCESS is not allowed in this context
        |
        |     Note: VAR_ACCESS can only be used inside PROGRAM
    ----'
    [E0027] Error: syntax
        ,-[ file:///test0.st:15:3 ]
        |
     15 | ,->   VAR_CONFIG
        : :
     17 | |->   END_VAR
        | |
        | `--------------- VAR_CONFIG is not allowed in this context
        |
        |     Note: VAR_CONFIG can only be used inside CONFIGURATION
    ----'
    [E0028] Error: syntax
        ,-[ file:///test0.st:19:3 ]
        |
     19 | ,->   VAR_LOCATED
        : :
     21 | |->   END_VAR
        | |
        | `--------------- VAR_LOCATED is not allowed in this context
        |
        |     Note: VAR_LOCATED can only be used inside PROGRAM
    ----'
    [E0030] Error: syntax
        ,-[ file:///test0.st:27:3 ]
        |
     27 | ,->   VAR_GLOBAL
        : :
     29 | |->   END_VAR
        | |
        | `--------------- VAR_GLOBAL is not allowed in this context
        |
        |     Note: VAR_GLOBAL can only be used inside PROGRAM, CONFIGURATION
    ----'
    ");
}

#[rstest]
fn invalid_fb_variable_sections(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK db
  VAR_ACCESS

  END_VAR

  VAR_CONFIG

  END_VAR

  VAR_LOCATED

  END_VAR

  VAR_GLOBAL

  END_VAR
END_FUNCTION_BLOCK
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0026] Error: syntax
       ,-[ file:///test0.st:3:3 ]
       |
     3 | ,->   VAR_ACCESS
       : :
     5 | |->   END_VAR
       | |
       | `--------------- VAR_ACCESS is not allowed in this context
       |
       |     Note: VAR_ACCESS can only be used inside PROGRAM
    ---'
    [E0027] Error: syntax
       ,-[ file:///test0.st:7:3 ]
       |
     7 | ,->   VAR_CONFIG
       : :
     9 | |->   END_VAR
       | |
       | `--------------- VAR_CONFIG is not allowed in this context
       |
       |     Note: VAR_CONFIG can only be used inside CONFIGURATION
    ---'
    [E0028] Error: syntax
        ,-[ file:///test0.st:11:3 ]
        |
     11 | ,->   VAR_LOCATED
        : :
     13 | |->   END_VAR
        | |
        | `--------------- VAR_LOCATED is not allowed in this context
        |
        |     Note: VAR_LOCATED can only be used inside PROGRAM
    ----'
    [E0030] Error: syntax
        ,-[ file:///test0.st:15:3 ]
        |
     15 | ,->   VAR_GLOBAL
        : :
     17 | |->   END_VAR
        | |
        | `--------------- VAR_GLOBAL is not allowed in this context
        |
        |     Note: VAR_GLOBAL can only be used inside PROGRAM, CONFIGURATION
    ----'
    ");
}

#[rstest]
fn invalid_function_variable_sections(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn
  VAR_ACCESS

  END_VAR

  VAR_CONFIG

  END_VAR

  VAR_LOCATED

  END_VAR

  VAR_GLOBAL

  END_VAR
END_FUNCTION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0026] Error: syntax
       ,-[ file:///test0.st:3:3 ]
       |
     3 | ,->   VAR_ACCESS
       : :
     5 | |->   END_VAR
       | |
       | `--------------- VAR_ACCESS is not allowed in this context
       |
       |     Note: VAR_ACCESS can only be used inside PROGRAM
    ---'
    [E0027] Error: syntax
       ,-[ file:///test0.st:7:3 ]
       |
     7 | ,->   VAR_CONFIG
       : :
     9 | |->   END_VAR
       | |
       | `--------------- VAR_CONFIG is not allowed in this context
       |
       |     Note: VAR_CONFIG can only be used inside CONFIGURATION
    ---'
    [E0028] Error: syntax
        ,-[ file:///test0.st:11:3 ]
        |
     11 | ,->   VAR_LOCATED
        : :
     13 | |->   END_VAR
        | |
        | `--------------- VAR_LOCATED is not allowed in this context
        |
        |     Note: VAR_LOCATED can only be used inside PROGRAM
    ----'
    [E0030] Error: syntax
        ,-[ file:///test0.st:15:3 ]
        |
     15 | ,->   VAR_GLOBAL
        : :
     17 | |->   END_VAR
        | |
        | `--------------- VAR_GLOBAL is not allowed in this context
        |
        |     Note: VAR_GLOBAL can only be used inside PROGRAM, CONFIGURATION
    ----'
    ");
}


#[rstest]
fn invalid_method_prototypes_variable_sections(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE in
  METHOD m
    VAR_ACCESS
    
    END_VAR

    VAR_CONFIG

    END_VAR

    VAR_LOCATED

    END_VAR

    VAR_EXTERNAL

    END_VAR

    VAR_GLOBAL

    END_VAR
    
    VAR_TEMP

    END_VAR

    VAR_IN_OUT

    END_VAR
  END_METHOD
END_INTERFACE
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0026] Error: syntax
       ,-[ file:///test0.st:4:5 ]
       |
     4 | ,->     VAR_ACCESS
       : :
     6 | |->     END_VAR
       | |
       | `----------------- VAR_ACCESS is not allowed in this context
       |
       |     Note: VAR_ACCESS can only be used inside PROGRAM
    ---'
    [E0027] Error: syntax
        ,-[ file:///test0.st:8:5 ]
        |
      8 | ,->     VAR_CONFIG
        : :
     10 | |->     END_VAR
        | |
        | `----------------- VAR_CONFIG is not allowed in this context
        |
        |     Note: VAR_CONFIG can only be used inside CONFIGURATION
    ----'
    [E0028] Error: syntax
        ,-[ file:///test0.st:12:5 ]
        |
     12 | ,->     VAR_LOCATED
        : :
     14 | |->     END_VAR
        | |
        | `----------------- VAR_LOCATED is not allowed in this context
        |
        |     Note: VAR_LOCATED can only be used inside PROGRAM
    ----'
    [E0029] Error: syntax
        ,-[ file:///test0.st:16:5 ]
        |
     16 | ,->     VAR_EXTERNAL
        : :
     18 | |->     END_VAR
        | |
        | `----------------- VAR_EXTERNAL is not allowed in this context
        |
        |     Note: VAR_EXTERNAL can only be used inside PROGRAM, FUNCTION_BLOCK, FUNCTION
    ----'
    [E0030] Error: syntax
        ,-[ file:///test0.st:20:5 ]
        |
     20 | ,->     VAR_GLOBAL
        : :
     22 | |->     END_VAR
        | |
        | `----------------- VAR_GLOBAL is not allowed in this context
        |
        |     Note: VAR_GLOBAL can only be used inside PROGRAM, CONFIGURATION
    ----'
    [E0025] Error: syntax
        ,-[ file:///test0.st:24:5 ]
        |
     24 | ,->     VAR_TEMP
        : :
     26 | |->     END_VAR
        | |
        | `----------------- VAR_TEMP is not allowed in this context
        |
        |     Note: VAR_TEMP can only be used inside FUNCTION, FUNCTION_BLOCK
    ----'
    ");
}


#[rstest]
fn invalid_config_variable_sections(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyCfg
  VAR

  END_VAR

  VAR_IN_OUT

  END_VAR

  VAR_TEMP

  END_VAR

  VAR_CONFIG

  END_VAR

  VAR_LOCATED

  END_VAR

  VAR_EXTERNAL

  END_VAR
END_CONFIGURATION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0031] Error: syntax
       ,-[ file:///test0.st:3:3 ]
       |
     3 | ,->   VAR
       : :
     5 | |->   END_VAR
       | |
       | `--------------- VAR is not allowed in this context
       |
       |     Note: VAR can only be used inside FUNCTION, FUNCTION_BLOCK, PROGRAM
    ---'
    [E0024] Error: syntax
       ,-[ file:///test0.st:7:3 ]
       |
     7 | ,->   VAR_IN_OUT
       : :
     9 | |->   END_VAR
       | |
       | `--------------- VAR_IN_OUT is not allowed in this context
       |
       |     Note: VAR_IN_OUT can only be used inside FUNCTION, FUNCTION_BLOCK
    ---'
    [E0025] Error: syntax
        ,-[ file:///test0.st:11:3 ]
        |
     11 | ,->   VAR_TEMP
        : :
     13 | |->   END_VAR
        | |
        | `--------------- VAR_TEMP is not allowed in this context
        |
        |     Note: VAR_TEMP can only be used inside FUNCTION, FUNCTION_BLOCK
    ----'
    [E0027] Error: syntax
        ,-[ file:///test0.st:15:3 ]
        |
     15 | ,->   VAR_CONFIG
        : :
     17 | |->   END_VAR
        | |
        | `--------------- VAR_CONFIG is not allowed in this context
        |
        |     Note: VAR_CONFIG can only be used inside CONFIGURATION
    ----'
    [E0028] Error: syntax
        ,-[ file:///test0.st:19:3 ]
        |
     19 | ,->   VAR_LOCATED
        : :
     21 | |->   END_VAR
        | |
        | `--------------- VAR_LOCATED is not allowed in this context
        |
        |     Note: VAR_LOCATED can only be used inside PROGRAM
    ----'
    [E0029] Error: syntax
        ,-[ file:///test0.st:23:3 ]
        |
     23 | ,->   VAR_EXTERNAL
        : :
     25 | |->   END_VAR
        | |
        | `--------------- VAR_EXTERNAL is not allowed in this context
        |
        |     Note: VAR_EXTERNAL can only be used inside PROGRAM, FUNCTION_BLOCK, FUNCTION
    ----'
    ");
}

#[rstest]
fn invalid_method_declarations_variable_sections(mut with_db: RootDatabase) {
    let source = r#"
CLASS cl

  METHOD m
    VAR_ACCESS
    
    END_VAR

    VAR_CONFIG

    END_VAR

    VAR_LOCATED

    END_VAR

    VAR_EXTERNAL

    END_VAR

    VAR_GLOBAL

    END_VAR

    VAR_TEMP

    END_VAR

    VAR_IN_OUT

    END_VAR
  END_METHOD
END_CLASS
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0026] Error: syntax
       ,-[ file:///test0.st:5:5 ]
       |
     5 | ,->     VAR_ACCESS
       : :
     7 | |->     END_VAR
       | |
       | `----------------- VAR_ACCESS is not allowed in this context
       |
       |     Note: VAR_ACCESS can only be used inside PROGRAM
    ---'
    [E0027] Error: syntax
        ,-[ file:///test0.st:9:5 ]
        |
      9 | ,->     VAR_CONFIG
        : :
     11 | |->     END_VAR
        | |
        | `----------------- VAR_CONFIG is not allowed in this context
        |
        |     Note: VAR_CONFIG can only be used inside CONFIGURATION
    ----'
    [E0028] Error: syntax
        ,-[ file:///test0.st:13:5 ]
        |
     13 | ,->     VAR_LOCATED
        : :
     15 | |->     END_VAR
        | |
        | `----------------- VAR_LOCATED is not allowed in this context
        |
        |     Note: VAR_LOCATED can only be used inside PROGRAM
    ----'
    [E0030] Error: syntax
        ,-[ file:///test0.st:21:5 ]
        |
     21 | ,->     VAR_GLOBAL
        : :
     23 | |->     END_VAR
        | |
        | `----------------- VAR_GLOBAL is not allowed in this context
        |
        |     Note: VAR_GLOBAL can only be used inside PROGRAM, CONFIGURATION
    ----'
    ");
}

#[rstest]
fn single_after_interval(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyCfg
    TASK t1(INTERVAL := 1, SINGLE := 1, PRIORITY := 1);
END_CONFIGURATION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0032] Error: syntax
       ,-[ file:///test0.st:3:28 ]
       |
     3 |     TASK t1(INTERVAL := 1, SINGLE := 1, PRIORITY := 1);
       |                            ^^^^^^|^^^^^
       |                                  `------- SINGLE cannot be declared after INTERVAL
    ---'
    ");
}

#[rstest]
fn interval_after_priority(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyCfg
    TASK t1(PRIORITY := 1, INTERVAL := 1);
END_CONFIGURATION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0033] Error: syntax
       ,-[ file:///test0.st:3:26 ]
       |
     3 |     TASK t1(PRIORITY := 1, INTERVAL := 1);
       |                          ^^^^^^^|^^^^^^^
       |                                 `--------- INTERVAL cannot be declared after PRIORITY
    ---'
    ");
}

#[rstest]
fn single_after_priority(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyCfg
    TASK t1(PRIORITY := 1, SINGLE := 1);
END_CONFIGURATION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0034] Error: syntax
       ,-[ file:///test0.st:3:26 ]
       |
     3 |     TASK t1(PRIORITY := 1, SINGLE := 1);
       |                          ^^^^^^|^^^^^^
       |                                `-------- SINGLE cannot be declared after PRIORITY
    ---'
    ");
}
