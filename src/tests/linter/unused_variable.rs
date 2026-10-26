// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn unused_local_variable(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR
            x : INT;
            y : INT;
        END_VAR
            fn1 := x;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unused-variable"), @r"
    [L0201] Info: unused variable
       ,-[ file:///test0.st:5:13 ]
       |
     5 |             y : INT;
       |             |
       |             `-- unused variable 'y'
       |
       | Note 1: if this is intentional, prefix it with an underscore:
       |         '_y'
       |
       | Note 2: lint rule: unused-variable
    ---'
    ");
}

#[rstest]
fn unused_variable_in_function_block(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK fb1
        VAR
            x : INT;
            y : INT;
        END_VAR
            x := 1;
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unused-variable"), @r"
    [L0201] Info: unused variable
       ,-[ file:///test0.st:5:13 ]
       |
     5 |             y : INT;
       |             |
       |             `-- unused variable 'y'
       |
       | Note 1: if this is intentional, prefix it with an underscore:
       |         '_y'
       |
       | Note 2: lint rule: unused-variable
    ---'
    ");
}

#[rstest]
fn unused_variable_in_program(mut with_db: RootDatabase) {
    let source = r#"
        PROGRAM prog1
        VAR
            x : INT;
            y : INT;
        END_VAR
            x := 1;
        END_PROGRAM
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unused-variable"), @r"
    [L0201] Info: unused variable
       ,-[ file:///test0.st:5:13 ]
       |
     5 |             y : INT;
       |             |
       |             `-- unused variable 'y'
       |
       | Note 1: if this is intentional, prefix it with an underscore:
       |         '_y'
       |
       | Note 2: lint rule: unused-variable
    ---'
    ");
}

#[rstest]
fn all_variables_used(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR
            x : INT;
            y : INT;
        END_VAR
            fn1 := x + y;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unused-variable"), @r"");
}

#[rstest]
fn output_not_reported(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK fb1
        VAR_OUTPUT
            result : INT;
        END_VAR
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unused-variable"), @"");
}

#[rstest]
fn inout_not_reported(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK fb1
        VAR_IN_OUT
            data : INT;
        END_VAR
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unused-variable"), @"");
}

#[rstest]
fn input_on_program_not_reported(mut with_db: RootDatabase) {
    let source = r#"
        PROGRAM prog1
        VAR_INPUT
            sensor : INT;
        END_VAR
        END_PROGRAM
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unused-variable"), @"");
}

#[rstest]
fn unused_input_on_function(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR_INPUT
            a : INT;
            b : INT;
        END_VAR
            fn1 := a;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unused-variable"), @r"
    [L0201] Info: unused variable
       ,-[ file:///test0.st:5:13 ]
       |
     5 |             b : INT;
       |             |
       |             `-- unused variable 'b'
       |
       | Note 1: if this is intentional, prefix it with an underscore:
       |         '_b'
       |
       | Note 2: lint rule: unused-variable
    ---'
    ");
}

#[rstest]
fn underscore_not_reported(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR
            _ : INT;
        END_VAR
            fn1 := 0;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unused-variable"), @r"");
}

#[rstest]
fn variable_used_via_this_in_method_not_flagged(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Motor
        VAR
            speed : INT;
            unused_var : INT;
        END_VAR

        METHOD start
            THIS.speed := 100;
        END_METHOD
        END_FUNCTION_BLOCK
    "#;
    // speed is used via THIS in a method - should NOT be flagged
    // unused_var is never used anywhere - should be flagged
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unused-variable"), @r"
    [L0201] Info: unused variable
       ,-[ file:///test0.st:5:13 ]
       |
     5 |             unused_var : INT;
       |             ^^^^^|^^^^
       |                  `------ unused variable 'unused_var'
       |
       | Note 1: if this is intentional, prefix it with an underscore:
       |         '_unused_var'
       |
       | Note 2: lint rule: unused-variable
    ---'
    ");
}

#[rstest]
fn extern_pragma_param_counts_as_used(mut with_db: RootDatabase) {
    // `IN` is referenced only by the {extern} pragma - that should
    // count as a use, not a dead variable.
    let source = r#"
        {extern 'math' 'abs'}
        FUNCTION abs : REAL
        VAR_INPUT IN : REAL; END_VAR
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unused-variable"), @"");
}

#[rstest]
fn wasm_pragma_params_count_as_used(mut with_db: RootDatabase) {
    // `IN` and `N` are only referenced by the {wasm} pragma - both
    // should count as used.
    let source = r#"
        FUNCTION SHL : DWORD
        VAR_INPUT
            IN : DWORD;
            N : INT;
        END_VAR
            {wasm 'i32.shl' (params IN N) (result SHL)}
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unused-variable"), @"");
}

/// A fold is the only way to consume a variadic pack, so `...args+` has to
/// count as a use — otherwise every variadic function ever written reports its
/// own parameter as unused.
#[rstest]
fn variadic_pack_consumed_by_a_fold_is_used(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION sum_all : INT
        VAR_INPUT
            args : INT...;
        END_VAR
            sum_all := ...args+;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unused-variable"), @r"");
}

/// A PROGRAM's variable its configuration names is used: a function block a
/// task runs, and an instance a VAR_CONFIG path goes through.
#[rstest]
fn named_by_the_configuration_not_reported(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Counter
        VAR_OUTPUT n : INT; END_VAR
            n := n + 1;
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Drive
        VAR out AT %Q* : INT; END_VAR
        END_FUNCTION_BLOCK

        PROGRAM G
        VAR fb1 : Counter; d : Drive; idle : INT; END_VAR
        END_PROGRAM

        CONFIGURATION Cfg
        VAR_CONFIG
            Res.P1.d.out AT %QW0 : INT;
        END_VAR
            RESOURCE Res ON CPU
                TASK SLOW(INTERVAL := T#20ms, PRIORITY := 2);
                TASK FAST(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH SLOW : G(fb1 WITH FAST);
            END_RESOURCE
        END_CONFIGURATION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unused-variable"), @r"
    [L0201] Info: unused variable
        ,-[ file:///test0.st:12:39 ]
        |
     12 |         VAR fb1 : Counter; d : Drive; idle : INT; END_VAR
        |                                       ^^|^
        |                                         `--- unused variable 'idle'
        |
        | Note 1: if this is intentional, prefix it with an underscore:
        |         '_idle'
        |
        | Note 2: lint rule: unused-variable
    ----'
    ");
}

/// A global of the configuration no POU uses is unused. One read through a
/// VAR_EXTERNAL, one by its own name and one the configuration connects are
/// used, and a name starting with an underscore is kept on purpose.
#[rstest]
fn an_unused_global(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM Main
VAR_INPUT pulse : BOOL; END_VAR
VAR_EXTERNAL level : INT; END_VAR
VAR x : INT; END_VAR
    x := level + count;
    IF pulse THEN x := 0; END_IF;
END_PROGRAM

CONFIGURATION Plant
VAR_GLOBAL
    level : INT;
    count : INT;
    start : BOOL;
    spare : INT;
    _reserve : INT;
END_VAR
    RESOURCE Res ON CPU
        TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM M WITH Fast : Main(pulse := start);
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unused-variable"), @r"
    [L0201] Info: unused variable
        ,-[ file:///test0.st:15:5 ]
        |
     15 |     spare : INT;
        |     ^^|^^
        |       `---- unused variable 'spare'
        |
        | Help: remove 'spare'
        |
        | Note 1: no POU of the workspace reads or writes it
        |
        | Note 2: lint rule: unused-variable
    ----'
    ");
}

/// An argument of a call that names nothing callable still reads its
/// variables: the callee is the error, reported where it is.
#[rstest]
fn the_arguments_of_a_failed_call_are_read(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION unknown_callee : INT
        VAR_INPUT IN : INT; END_VAR
            unknown_callee := NOT_IMPORTED(IN);
        END_FUNCTION

        FUNCTION not_callable : INT
        VAR v, x : INT; END_VAR
            not_callable := v(IN := x);
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unused-variable"), @r"
    [E0201] Error: unknown name
       ,-[ file:///test0.st:4:31 ]
       |
     4 |             unknown_callee := NOT_IMPORTED(IN);
       |                               ^^^^^^|^^^^^
       |                                     `------- no item 'NOT_IMPORTED' found in scope
    ---'
    [E0808] Error: call of a non-callable type
       ,-[ file:///test0.st:9:29 ]
       |
     9 |             not_callable := v(IN := x);
       |                             |
       |                             `-- 'INT' is not a callable type
    ---'
    ");
}
