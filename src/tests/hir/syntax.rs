// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

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
    [E0002] Error: missing element
       ,-[ file:///test0.st:2:10 ]
       |
     2 | NAMESPACE
       |          |
       |          `- missing 'identifier'
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
    [E0002] Error: missing element
       ,-[ file:///test0.st:2:26 ]
       |
     2 |     FUNCTION myFunc : INT
       |                          |
       |                          `- missing 'END_FUNCTION'
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
    [E0001] Error: syntax error
       ,-[ file:///test0.st:2:16 ]
       |
     2 | NAMESPACE test ;
       |                |
       |                `-- unexpected token(s): ';'
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
    [E1103] Error: IMPLEMENTS before EXTENDS
       ,-[ file:///test0.st:5:19 ]
       |
     5 | FUNCTION_BLOCK fn IMPLEMENTS a EXTENDS b
       |                   ^^^^^^|^^^^^         |
       |                         `----------------- IMPLEMENTS comes after EXTENDS
       |                                        |
       |                                        `-- move 'IMPLEMENTS a' here
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
    [E1102] Error: more than one IMPLEMENTS clause
       ,-[ file:///test0.st:8:32 ]
       |
     8 | FUNCTION_BLOCK fn IMPLEMENTS a IMPLEMENTS b
       |                              | ^^^^^^|^^^^^
       |                              `--------------- merge into single clause: 'IMPLEMENTS a, b'
       |                                      |
       |                                      `------- second IMPLEMENTS clause
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
    [E1101] Error: more than one EXTENDS clause
       ,-[ file:///test0.st:8:29 ]
       |
     8 | FUNCTION_BLOCK fn EXTENDS a EXTENDS b
       |                           | ^^^^|^^^^
       |                           `------------ merge into single clause: 'EXTENDS a, b'
       |                                 |
       |                                 `------ second EXTENDS clause
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
    [E0003] Error: missing variable type
       ,-[ file:///test0.st:4:9 ]
       |
     4 |         empty
       |         ^^|^^
       |           `---- the variable has no type
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
    [E0013] Error: assignment to a function call
       ,-[ file:///test0.st:3:5 ]
       |
     3 |     fn() := 0;
       |     ^^|^
       |       `--- a function call cannot be assigned
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
    [E0015] Error: THIS after a dot in a path
       ,-[ file:///test0.st:3:8 ]
       |
     3 |     fn.THIS.p := 5
       |        ^^|^
       |          `--- 'THIS' cannot follow a dot
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
    [E0016] Error: SUPER after a dot in a path
       ,-[ file:///test0.st:3:8 ]
       |
     3 |     fn.SUPER.p := 5
       |        ^^|^^
       |          `---- 'SUPER' cannot follow a dot
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
    [E0005] Error: empty right-hand side of assignment
       ,-[ file:///test0.st:3:7 ]
       |
     3 |     a :=
       |       ^|
       |        `-- the assignment has no right-hand side
    ---'
    ");
}

#[rstest]
fn function_call_in_init_expression(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn
  VAR
    ml : ARRAY [0..2] OF INT := [1(call(IN := 5, OUT => OUT))]
  END_VAR

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0402] Error: function call in a repeat count
       ,-[ file:///test0.st:4:36 ]
       |
     4 |     ml : ARRAY [0..2] OF INT := [1(call(IN := 5, OUT => OUT))]
       |                                    ^^^^^^^^^^^^|^^^^^^^^^^^^
       |                                                `-------------- the repeat count is a call, not a constant
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
    [E0006] Error: invalid assignment sign '='
       ,-[ file:///test0.st:3:7 ]
       |
     3 |     a = 0;
       |       |
       |       `-- '=' is not a valid assignment sign
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
    [E0007] Error: invalid assignment sign ':'
       ,-[ file:///test0.st:3:7 ]
       |
     3 |     a : 0;
       |       |
       |       `-- ':' is not a valid assignment sign
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
    [E0009] Error: invalid assignment sign '=' in FOR list
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
    [E0010] Error: invalid assignment sign ':' in FOR list
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
    [E0027] Error: variables after methods in a CLASS
       ,-[ file:///test0.st:6:5 ]
       |
     4 |     METHOD PROTECTED myProtectedMethod END_METHOD
       |     ^^^|^^
       |        `---- the first method is declared here
       |
     6 |     VAR
       |     ^|^
       |      `--- the variable sections of a CLASS come before its methods
    ---'
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
    [E0026] Error: variables after methods in a FUNCTION_BLOCK
       ,-[ file:///test0.st:6:5 ]
       |
     4 |     METHOD PROTECTED myProtectedMethod END_METHOD
       |     ^^^|^^
       |        `---- the first method is declared here
       |
     6 |     VAR
       |     ^|^
       |      `--- the variable sections of a FUNCTION_BLOCK come before its methods
    ---'
    ");
}

#[rstest]
fn output_assign_in_assignment(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fn
    a => 0;
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0008] Error: invalid assignment sign '=>' in assignment
       ,-[ file:///test0.st:3:7 ]
       |
     3 |     a => 0;
       |       ^|
       |        `-- '=>' is not a valid assignment sign
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
    [E0011] Error: invalid assignment sign '=>' in FOR list
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
    [E0024] Error: PROGRAM in a NAMESPACE
       ,-[ file:///test0.st:3:5 ]
       |
     3 |     PROGRAM myProgram
       |     ^^^|^^^
       |        `----- a PROGRAM cannot be declared in a NAMESPACE
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
    [E0025] Error: CONFIGURATION in a NAMESPACE
       ,-[ file:///test0.st:3:5 ]
       |
     3 |     CONFIGURATION myConfig
       |     ^^^^^^|^^^^^^
       |           `-------- a CONFIGURATION cannot be declared in a NAMESPACE
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


  VAR_EXTERNAL

  END_VAR

  VAR_GLOBAL

  END_VAR
END_CLASS
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0018] Error: VAR_IN_OUT not allowed in this context
       ,-[ file:///test0.st:3:3 ]
       |
     3 |   VAR_IN_OUT
       |   ^^^^^|^^^^
       |        `------ VAR_IN_OUT is not allowed here
       |
       | Note: VAR_IN_OUT is a parameter of a FUNCTION, FUNCTION_BLOCK, PROGRAM or METHOD
    ---'
    [E0019] Error: VAR_TEMP not allowed in this context
       ,-[ file:///test0.st:7:3 ]
       |
     7 |   VAR_TEMP
       |   ^^^^|^^^
       |       `----- VAR_TEMP is not allowed here
       |
       | Note: VAR_TEMP declares the per-call variables of a FUNCTION, FUNCTION_BLOCK, PROGRAM or METHOD
    ---'
    [E0022] Error: VAR_ACCESS not allowed in this context
        ,-[ file:///test0.st:11:3 ]
        |
     11 |   VAR_ACCESS
        |   ^^^^^|^^^^
        |        `------ VAR_ACCESS is not allowed here
        |
        | Note: VAR_ACCESS goes in a PROGRAM or a CONFIGURATION
    ----'
    [E0023] Error: VAR_CONFIG not allowed in this context
        ,-[ file:///test0.st:15:3 ]
        |
     15 |   VAR_CONFIG
        |   ^^^^^|^^^^
        |        `------ VAR_CONFIG is not allowed here
        |
        | Note: VAR_CONFIG goes in a CONFIGURATION
    ----'
    [E0021] Error: VAR_GLOBAL not allowed in this context
        ,-[ file:///test0.st:24:3 ]
        |
     24 |   VAR_GLOBAL
        |   ^^^^^|^^^^
        |        `------ VAR_GLOBAL is not allowed here
        |
        | Note: VAR_GLOBAL goes in a CONFIGURATION
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


  VAR_GLOBAL

  END_VAR
END_FUNCTION_BLOCK
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0022] Error: VAR_ACCESS not allowed in this context
       ,-[ file:///test0.st:3:3 ]
       |
     3 |   VAR_ACCESS
       |   ^^^^^|^^^^
       |        `------ VAR_ACCESS is not allowed here
       |
       | Note: VAR_ACCESS goes in a PROGRAM or a CONFIGURATION
    ---'
    [E0023] Error: VAR_CONFIG not allowed in this context
       ,-[ file:///test0.st:7:3 ]
       |
     7 |   VAR_CONFIG
       |   ^^^^^|^^^^
       |        `------ VAR_CONFIG is not allowed here
       |
       | Note: VAR_CONFIG goes in a CONFIGURATION
    ---'
    [E0021] Error: VAR_GLOBAL not allowed in this context
        ,-[ file:///test0.st:12:3 ]
        |
     12 |   VAR_GLOBAL
        |   ^^^^^|^^^^
        |        `------ VAR_GLOBAL is not allowed here
        |
        | Note: VAR_GLOBAL goes in a CONFIGURATION
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


  VAR_GLOBAL

  END_VAR
END_FUNCTION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0022] Error: VAR_ACCESS not allowed in this context
       ,-[ file:///test0.st:3:3 ]
       |
     3 |   VAR_ACCESS
       |   ^^^^^|^^^^
       |        `------ VAR_ACCESS is not allowed here
       |
       | Note: VAR_ACCESS goes in a PROGRAM or a CONFIGURATION
    ---'
    [E0023] Error: VAR_CONFIG not allowed in this context
       ,-[ file:///test0.st:7:3 ]
       |
     7 |   VAR_CONFIG
       |   ^^^^^|^^^^
       |        `------ VAR_CONFIG is not allowed here
       |
       | Note: VAR_CONFIG goes in a CONFIGURATION
    ---'
    [E0021] Error: VAR_GLOBAL not allowed in this context
        ,-[ file:///test0.st:12:3 ]
        |
     12 |   VAR_GLOBAL
        |   ^^^^^|^^^^
        |        `------ VAR_GLOBAL is not allowed here
        |
        | Note: VAR_GLOBAL goes in a CONFIGURATION
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
    [E0022] Error: VAR_ACCESS not allowed in this context
       ,-[ file:///test0.st:4:5 ]
       |
     4 |     VAR_ACCESS
       |     ^^^^^|^^^^
       |          `------ VAR_ACCESS is not allowed here
       |
       | Note: VAR_ACCESS goes in a PROGRAM or a CONFIGURATION
    ---'
    [E0023] Error: VAR_CONFIG not allowed in this context
       ,-[ file:///test0.st:8:5 ]
       |
     8 |     VAR_CONFIG
       |     ^^^^^|^^^^
       |          `------ VAR_CONFIG is not allowed here
       |
       | Note: VAR_CONFIG goes in a CONFIGURATION
    ---'
    [E0020] Error: VAR_EXTERNAL not allowed in this context
        ,-[ file:///test0.st:13:5 ]
        |
     13 |     VAR_EXTERNAL
        |     ^^^^^^|^^^^^
        |           `------- VAR_EXTERNAL is not allowed here
        |
        | Note: VAR_EXTERNAL names a global from a FUNCTION, FUNCTION_BLOCK, PROGRAM, CLASS or METHOD
    ----'
    [E0021] Error: VAR_GLOBAL not allowed in this context
        ,-[ file:///test0.st:17:5 ]
        |
     17 |     VAR_GLOBAL
        |     ^^^^^|^^^^
        |          `------ VAR_GLOBAL is not allowed here
        |
        | Note: VAR_GLOBAL goes in a CONFIGURATION
    ----'
    [E0019] Error: VAR_TEMP not allowed in this context
        ,-[ file:///test0.st:21:5 ]
        |
     21 |     VAR_TEMP
        |     ^^^^|^^^
        |         `----- VAR_TEMP is not allowed here
        |
        | Note: VAR_TEMP declares the per-call variables of a FUNCTION, FUNCTION_BLOCK, PROGRAM or METHOD
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


  VAR_EXTERNAL

  END_VAR
END_CONFIGURATION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0017] Error: VAR not allowed in this context
       ,-[ file:///test0.st:3:3 ]
       |
     3 |   VAR
       |   ^|^
       |    `--- VAR is not allowed here
       |
       | Note: VAR declares the variables of a FUNCTION, FUNCTION_BLOCK, PROGRAM, CLASS or METHOD
    ---'
    [E0018] Error: VAR_IN_OUT not allowed in this context
       ,-[ file:///test0.st:7:3 ]
       |
     7 |   VAR_IN_OUT
       |   ^^^^^|^^^^
       |        `------ VAR_IN_OUT is not allowed here
       |
       | Note: VAR_IN_OUT is a parameter of a FUNCTION, FUNCTION_BLOCK, PROGRAM or METHOD
    ---'
    [E0019] Error: VAR_TEMP not allowed in this context
        ,-[ file:///test0.st:11:3 ]
        |
     11 |   VAR_TEMP
        |   ^^^^|^^^
        |       `----- VAR_TEMP is not allowed here
        |
        | Note: VAR_TEMP declares the per-call variables of a FUNCTION, FUNCTION_BLOCK, PROGRAM or METHOD
    ----'
    [E0020] Error: VAR_EXTERNAL not allowed in this context
        ,-[ file:///test0.st:20:3 ]
        |
     20 |   VAR_EXTERNAL
        |   ^^^^^^|^^^^^
        |         `------- VAR_EXTERNAL is not allowed here
        |
        | Note: VAR_EXTERNAL names a global from a FUNCTION, FUNCTION_BLOCK, PROGRAM, CLASS or METHOD
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
    [E0022] Error: VAR_ACCESS not allowed in this context
       ,-[ file:///test0.st:5:5 ]
       |
     5 |     VAR_ACCESS
       |     ^^^^^|^^^^
       |          `------ VAR_ACCESS is not allowed here
       |
       | Note: VAR_ACCESS goes in a PROGRAM or a CONFIGURATION
    ---'
    [E0023] Error: VAR_CONFIG not allowed in this context
       ,-[ file:///test0.st:9:5 ]
       |
     9 |     VAR_CONFIG
       |     ^^^^^|^^^^
       |          `------ VAR_CONFIG is not allowed here
       |
       | Note: VAR_CONFIG goes in a CONFIGURATION
    ---'
    [E0021] Error: VAR_GLOBAL not allowed in this context
        ,-[ file:///test0.st:18:5 ]
        |
     18 |     VAR_GLOBAL
        |     ^^^^^|^^^^
        |          `------ VAR_GLOBAL is not allowed here
        |
        | Note: VAR_GLOBAL goes in a CONFIGURATION
    ----'
    ");
}

#[rstest]
fn single_after_interval(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyCfg
    RESOURCE Res ON CPU
        TASK t1(INTERVAL := 1, SINGLE := 1, PRIORITY := 1);
    END_RESOURCE
END_CONFIGURATION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1408] Error: SINGLE after INTERVAL in TASK
       ,-[ file:///test0.st:4:32 ]
       |
     4 |         TASK t1(INTERVAL := 1, SINGLE := 1, PRIORITY := 1);
       |                                ^^^^^^|^^^^^
       |                                      `------- SINGLE cannot be declared after INTERVAL
    ---'
    ");
}

#[rstest]
fn interval_after_priority(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyCfg
    RESOURCE Res ON CPU
        TASK t1(PRIORITY := 1, INTERVAL := 1);
    END_RESOURCE
END_CONFIGURATION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1407] Error: INTERVAL after PRIORITY in TASK
       ,-[ file:///test0.st:4:30 ]
       |
     4 |         TASK t1(PRIORITY := 1, INTERVAL := 1);
       |                              ^^^^^^^|^^^^^^^
       |                                     `--------- INTERVAL cannot be declared after PRIORITY
    ---'
    ");
}

#[rstest]
fn single_after_priority(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyCfg
    RESOURCE Res ON CPU
        TASK t1(PRIORITY := 1, SINGLE := 1);
    END_RESOURCE
END_CONFIGURATION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1409] Error: SINGLE after PRIORITY in TASK
       ,-[ file:///test0.st:4:30 ]
       |
     4 |         TASK t1(PRIORITY := 1, SINGLE := 1);
       |                              ^^^^^^|^^^^^^
       |                                    `-------- SINGLE cannot be declared after PRIORITY
    ---'
    ");
}

#[rstest]
fn missing_priority(mut with_db: RootDatabase) {
    let source = r#"
CONFIGURATION MyCfg
    RESOURCE Res ON CPU
        TASK t1()
    END_RESOURCE
END_CONFIGURATION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1405] Error: missing PRIORITY in TASK
       ,-[ file:///test0.st:4:9 ]
       |
     4 |         TASK t1()
       |         ^^^^|^^^^
       |             `------ the TASK has no PRIORITY
    ---'
    ");
}

#[rstest]
fn access_spec_not_allowed_in_method_prototype(mut with_db: RootDatabase) {
    let source = r#"
INTERFACE IMotor
    METHOD PUBLIC start
    END_METHOD
    METHOD PRIVATE stop : BOOL
    END_METHOD
END_INTERFACE
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1120] Error: access specifier on interface method prototype
       ,-[ file:///test0.st:3:12 ]
       |
     3 |     METHOD PUBLIC start
       |            ^^^|^^
       |               `---- access specifiers are not allowed on interface method prototypes
       |
       | Note: interface methods are implicitly PUBLIC
    ---'
    [E1120] Error: access specifier on interface method prototype
       ,-[ file:///test0.st:5:12 ]
       |
     5 |     METHOD PRIVATE stop : BOOL
       |            ^^^|^^^
       |               `----- access specifiers are not allowed on interface method prototypes
       |
       | Note: interface methods are implicitly PUBLIC
    ---'
    ");
}

#[rstest]
fn method_decl_in_body(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    x := 1;
    METHOD m1
    END_METHOD
END_FUNCTION_BLOCK
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0028] Error: METHOD declared in a body
       ,-[ file:///test0.st:4:5 ]
       |
     4 |     METHOD m1
       |     ^^^|^^
       |        `---- a METHOD cannot be declared in a body
    ---'
    [E0201] Error: unknown name
       ,-[ file:///test0.st:3:5 ]
       |
     3 |     x := 1;
       |     |
       |     `-- no item 'x' found in scope
    ---'
    ");
}

#[rstest]
fn comma_index_access_is_the_standard_form(mut with_db: RootDatabase) {
    // `m[i, j]` is the standard's multi-dimensional access; each index
    // consumes one dimension, exactly like the chained `m[i][j]`. Both are
    // accepted and lower identically. (the code that rejected the comma form
    // is retired.)
    let source = r#"
        TYPE Matrix : ARRAY[0..1, 0..2] OF INT; END_TYPE
        FUNCTION test : INT
        VAR m : Matrix; END_VAR
            test := m[0, 0];
        END_FUNCTION
        "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn comma_subscript_constant_bounds_check_names_its_dimension(mut with_db: RootDatabase) {
    // The compile-time bounds check walks comma subscripts per dimension:
    // 9 violates dimension 2's [1..3], and the diagnostic says which.
    let source = r#"
        FUNCTION test : INT
        VAR m : ARRAY[1..3, 1..3] OF INT; END_VAR
            test := m[1, 9];
        END_FUNCTION
        "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0506] Error: constant index out of bounds
       ,-[ file:///test0.st:4:26 ]
       |
     4 |             test := m[1, 9];
       |                          |
       |                          `-- index 9 is out of bounds (the dimension is declared 1..3)
       |
       | Note: this error occurred in array dimension 2
    ---'
    ");
}

/// A condition compares, so `:=` in one is an assignment where `=` was meant.
/// It used to fall out as a generic syntax error plus a type mismatch on the
/// left operand, where `FOR` has said which sign belongs there all along.
#[rstest]
fn invalid_assign_in_a_condition(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION f : INT
VAR
    x : INT;
END_VAR
    IF x := 1 THEN
        x := 2;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0012] Error: assignment sign ':=' in a condition
       ,-[ file:///test0.st:6:8 ]
       |
     6 |     IF x := 1 THEN
       |        ^^^|^^
       |           `---- a condition compares with '=', not ':='
       |
       | Help: replace ':=' with '='
    ---'
    ");
}

/// The same sign in the other two conditions the language has.
#[rstest]
fn invalid_assign_in_a_loop_condition(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION f : INT
VAR
    x : INT;
END_VAR
    WHILE x := 1 DO
        x := 2;
    END_WHILE;

    REPEAT
        x := 2;
    UNTIL x := 1 END_REPEAT;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0012] Error: assignment sign ':=' in a condition
       ,-[ file:///test0.st:6:11 ]
       |
     6 |     WHILE x := 1 DO
       |           ^^^|^^
       |              `---- a condition compares with '=', not ':='
       |
       | Help: replace ':=' with '='
    ---'
    [E0012] Error: assignment sign ':=' in a condition
        ,-[ file:///test0.st:12:11 ]
        |
     12 |     UNTIL x := 1 END_REPEAT;
        |           ^^^|^^
        |              `---- a condition compares with '=', not ':='
        |
        | Help: replace ':=' with '='
    ----'
    ");
}

/// An ELSIF is a condition too.
#[rstest]
fn invalid_assign_in_an_elsif_condition(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION f : INT
VAR
    x : INT;
END_VAR
    IF x = 1 THEN
        x := 2;
    ELSIF x := 2 THEN
        x := 3;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0012] Error: assignment sign ':=' in a condition
       ,-[ file:///test0.st:8:11 ]
       |
     8 |     ELSIF x := 2 THEN
       |           ^^^|^^
       |              `---- a condition compares with '=', not ':='
       |
       | Help: replace ':=' with '='
    ---'
    ");
}

/// A tree the generated AST has no place for is the compiler's fault, and it
/// is reported as a syntax error rather than a crash: `REF_TO TIME` once hit
/// an `unreachable!` here, because auto-lsp-codegen flattened nested
/// supertypes one level only. The grammar no longer produces one, so the
/// error is built by hand.
#[rstest]
fn a_node_the_ast_has_no_place_for_is_a_syntax_error(mut with_db: RootDatabase) {
    use auto_lsp::core::errors::{AstError, ParseError};
    use auto_lsp::default::db::BaseDatabase;
    use auto_lsp::tree_sitter::{Point, Range};
    use hir::check::errors::{ToIdeDiagnostic, e00_syntax::SyntaxError};

    crate::tests::utils::add_sources(&mut with_db, &["FUNCTION f : INT\nEND_FUNCTION\n"]);
    let file = *with_db.get_files().iter().next().expect("the file");
    let range = Range {
        start_byte: 0,
        end_byte: 8,
        start_point: Point { row: 0, column: 0 },
        end_point: Point { row: 0, column: 8 },
    };
    let err = ParseError::AstError {
        span: range,
        error: AstError::UnexpectedSymbol {
            range,
            symbol: "ref_type_spec",
            parent_name: "ArrayTypeSpec_DataTypeAccess",
        },
    };
    let diagnostic = SyntaxError::from_parse_error(&with_db, file, &err)
        .to_diagnostic(&with_db, file)
        .inner();
    assert!(format!("{:?}", diagnostic.code).contains("E0001"));
    assert!(
        diagnostic.message.starts_with("compiler bug:"),
        "{}",
        diagnostic.message
    );
}

/// A `;` ends a statement and is not required: a statement may go without
/// one, and so may a declaration, a TYPE, a VAR_CONFIG entry, a TASK or a
/// USING. The program compiles the same.
#[rstest]
fn valid_statements_and_declarations_without_semicolons(mut with_db: RootDatabase) {
    let source = r#"
TYPE
    Pt : STRUCT x : INT y : INT END_STRUCT
    Row : ARRAY[0..1] OF INT
    Mode : (Off, On)
END_TYPE

FUNCTION_BLOCK Fb
VAR_INPUT a : INT b : INT := 2 END_VAR
VAR_OUTPUT q : INT END_VAR
VAR n : INT END_VAR
    q := a + b + n
END_FUNCTION_BLOCK

FUNCTION f : INT
VAR x : INT y : INT END_VAR
VAR_TEMP t : INT END_VAR
    IF x = 1 THEN RETURN END_IF
    FOR x := 0 TO 2 DO IF x = 1 THEN CONTINUE END_IF y := y + 1 END_FOR
    WHILE x > 5 DO x := x - 1 IF x = 7 THEN EXIT END_IF END_WHILE
    REPEAT x := x + 1 UNTIL x > 3 END_REPEAT
    CASE x OF 1: y := 1 2: y := 2 ELSE y := 0 END_CASE
    f := y
END_FUNCTION

PROGRAM Main
VAR fb : Fb p : Pt r : Row m : Mode END_VAR
    fb(a := 1)
    p.x := fb.q
    r[0] := p.x
    m := Mode#On
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL g : INT END_VAR
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1)
        PROGRAM P1 WITH T : Main
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// An input named `R` or `F` after a BOOL one without a `;`. The recovery
/// for an edge qualifier started and not finished read the bare letter as
/// one, and the input lost its name. `R` is the usual name of a reset input,
/// as in the stdlib's counters.
#[rstest]
fn valid_inputs_named_r_and_f_without_semicolons(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Ctu
VAR_INPUT
    CU : BOOL
    R : BOOL
    F : BOOL
    E : BOOL R_EDGE
END_VAR
VAR_OUTPUT Q : BOOL END_VAR
    Q := CU AND NOT R AND F
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// An edge qualifier started and not finished is still E0014, with or
/// without the `;`.
#[rstest]
fn invalid_incomplete_edge_qualifier(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Ctu
VAR_INPUT
    a : BOOL R_;
    b : BOOL F_ED
    c : BOOL
END_VAR
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0014] Error: incomplete edge qualifier
       ,-[ file:///test0.st:4:14 ]
       |
     4 |     a : BOOL R_;
       |              ^|
       |               `-- incomplete edge qualifier
    ---'
    [E0014] Error: incomplete edge qualifier
       ,-[ file:///test0.st:5:14 ]
       |
     5 |     b : BOOL F_ED
       |              ^^|^
       |                `--- incomplete edge qualifier
    ---'
    ");
}

/// The empty statement, a `;` on its own: as a whole body, after a
/// statement's own `;`, in a branch, and after a section's `END_VAR`.
/// IEC 61131-3 writes a statement list as `( Stmt? ';' )*`.
#[rstest]
fn valid_empty_statements(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Idle
    ;
END_FUNCTION_BLOCK

FUNCTION f : INT
VAR_INPUT a : INT; END_VAR;
VAR x : INT; END_VAR;
    ;
    x := a;;
    IF x = 1 THEN ; END_IF;
    IF x = 2 THEN ; ELSE ; END_IF
    CASE x OF 1: ; 2: x := 2;; ELSE ; END_CASE;
    FOR x := 0 TO 2 DO ; END_FOR;
    WHILE x > 5 DO ; END_WHILE
    REPEAT ; UNTIL x > 3 END_REPEAT;
    ;;
    f := x
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}
