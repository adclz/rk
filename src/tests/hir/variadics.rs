// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn invalid_alias_type_for_variadic_variable(mut with_db: RootDatabase) {
    let source = r#"
TYPE MyStruct :
    STRUCT
        field1: INT;
    END_STRUCT
END_TYPE

FUNCTION sum_all : INT
    VAR_INPUT
        args: MyStruct...
    END_VAR

END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0811] Error: variadic parameter of a composite type
        ,-[ file:///test0.st:10:15 ]
        |
     10 |         args: MyStruct...
        |               ^^^^|^^^
        |                   `----- 'MyStruct' cannot be variadic
        |
        | Note: only an elementary type can be variadic
    ----'
    ");
}

#[rstest]
fn valid_type_for_variadic_variable(mut with_db: RootDatabase) {
    let source = r#"
TYPE MyValue: INT END_TYPE

FUNCTION sum_all : INT
    VAR_INPUT
        args: MyValue...
    END_VAR

END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn invalid_variadic_expression_on_non_variadic_variable(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION sum_all : INT
    VAR_INPUT
        args: INT
    END_VAR
    sum_all := ...args+
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0814] Error: not a variadic parameter
       ,-[ file:///test0.st:6:16 ]
       |
     6 |     sum_all := ...args+
       |                ^^^^|^^^
       |                    `----- 'args' is not a variadic parameter
       |
       | Help: declare one in VAR_INPUT, as `values : INT...`
       |
       | Note: the POU declares no variadic parameter
    ---'
    ");
}

#[rstest]
fn invalid_fold_math_operator_on_non_math_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION sum_all : BOOL
    VAR_INPUT
        args: BOOL...
    END_VAR

    sum_all := ...args+

END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0305] Error: operator not supported by the type
       ,-[ file:///test0.st:7:16 ]
       |
     4 |         args: BOOL...
       |         ^^|^
       |           `--- 'args' is declared here
       |
     7 |     sum_all := ...args+
       |                ^^^^|^^^
       |                    `----- operator '+' cannot be applied to type 'BOOL'
    ---'
    ");
}

#[rstest]
fn valid_variadic_function_with_fold_add(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION sum_all : INT
    VAR_INPUT
        args: INT...
    END_VAR
    sum_all := ...args+
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_variadic_function_with_fold_mul(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION product_all : INT
    VAR_INPUT
        args: INT...
    END_VAR
    product_all := ...args*
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_variadic_function_with_fold_and(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION all_true : BOOL
    VAR_INPUT
        args: BOOL...
    END_VAR
    all_true := ...args&
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_variadic_function_with_fold_comparison(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION all_equal : BOOL
    VAR_INPUT
        args: INT...
    END_VAR
    all_equal := ...args=
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn multiple_variadic_variables(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : INT
    VAR_INPUT
        a: INT...
        b: INT...
    END_VAR
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0812] Error: more than one variadic parameter
       ,-[ file:///test0.st:5:9 ]
       |
     4 |         a: INT...
       |         |
       |         `-- first variadic parameter 'a' is declared here
     5 |         b: INT...
       |         |
       |         `-- 'b' is a second variadic parameter
    ---'
    ");
}

#[rstest]
fn valid_variadic_fn_call(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION all_equal : BOOL
    VAR_INPUT
        args: INT...
    END_VAR
    all_equal := ...args=
END_FUNCTION

FUNCTION fn 
    all_equal(1, 2, 3);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_variadic_fn_call_with_output(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION all_equal : BOOL
    VAR_INPUT
        args: INT...
    END_VAR

    VAR_OUTPUT
        result: BOOL;
    END_VAR
    all_equal := ...args=
END_FUNCTION

FUNCTION fn 
    VAR_OUTPUT
        result: BOOL;
    END_VAR
    
    all_equal(1, 2, 3, result => result);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_variadic_fn_call_with_in_out(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION sum_all : INT
    VAR_INPUT
        args: INT...
    END_VAR

    VAR_IN_OUT
        accumulator: INT;
    END_VAR
    sum_all := ...args+
END_FUNCTION

FUNCTION fn
    VAR
        acc: INT := 0;
    END_VAR

    sum_all(1, 2, 3, accumulator := acc);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_variadic_fn_call_with_output_and_in_out(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION sum_all : INT
    VAR_INPUT
        args: INT...
    END_VAR

    VAR_OUTPUT
        count: INT;
    END_VAR

    VAR_IN_OUT
        accumulator: INT;
    END_VAR
    sum_all := ...args+
END_FUNCTION

FUNCTION fn
    VAR
        acc: INT := 0;
        cnt: INT;
    END_VAR

    sum_all(1, 2, 3, count => cnt, accumulator := acc);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn invalid_variadic_fn_call_type_mismatch(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION sum_all : INT
    VAR_INPUT
        args: INT...
    END_VAR
    sum_all := ...args+
END_FUNCTION

FUNCTION fn
    sum_all(1, 2, 'hello');
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0308] Error: literal of the wrong kind
        ,-[ file:///test0.st:10:19 ]
        |
      4 |         args: INT...
        |         ^^|^
        |           `--- 'args' is declared here
        |
     10 |     sum_all(1, 2, 'hello');
        |                   ^^^|^^^
        |                      `----- cannot use string literal as INT
    ----'
    ");
}

#[rstest]
fn valid_variadic_fn_call_single_arg(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION sum_all : INT
    VAR_INPUT
        args: INT...
    END_VAR
    sum_all := ...args+
END_FUNCTION

FUNCTION fn
    sum_all(42);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_variadic_fn_call_many_args(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION sum_all : INT
    VAR_INPUT
        args: INT...
    END_VAR
    sum_all := ...args+
END_FUNCTION

FUNCTION fn
    sum_all(1, 2, 3, 4, 5, 6, 7, 8, 9, 10);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn variadic_mixed_with_other_inputs(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : INT
    VAR_INPUT
        x: INT;
        args: INT...
    END_VAR
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0815] Error: variadic parameter beside other inputs
       ,-[ file:///test0.st:4:9 ]
       |
     4 |         x: INT;
       |         |
       |         `-- 'x' is declared beside the variadic parameter 'args'
     5 |         args: INT...
       |         ^^|^
       |           `--- variadic parameter 'args' is declared here
       |
       | Note: a variadic parameter takes every argument of the call
    ---'
    ");
}

/// A variadic pack must collect at least one argument: `...args+` has no value
/// over an empty pack, so an empty call has nothing to fold. Refused in HIR so
/// MIR never receives an arity it cannot lower.
#[rstest]
fn invalid_variadic_fn_call_with_no_arguments(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION sum_all : INT
    VAR_INPUT
        args: INT...
    END_VAR
    sum_all := ...args+
END_FUNCTION

FUNCTION fn1 : INT
    fn1 := sum_all()
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0813] Error: variadic call without arguments
        ,-[ file:///test0.st:10:12 ]
        |
      4 |         args: INT...
        |         ^^|^
        |           `--- variadic parameter 'args' is declared here
        |
     10 |     fn1 := sum_all()
        |            ^^^|^^^
        |               `----- the call to 'sum_all' passes no argument to the variadic parameter 'args'
    ----'
    ");
}

/// One argument is enough — the fold of a single element is that element.
#[rstest]
fn valid_variadic_fn_call_with_one_argument(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION sum_all : INT
    VAR_INPUT
        args: INT...
    END_VAR
    sum_all := ...args+
END_FUNCTION

FUNCTION fn1 : INT
    fn1 := sum_all(1)
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// The same rule on a METHOD, the other POU kind that may declare a variadic.
#[rstest]
fn invalid_variadic_method_call_with_no_arguments(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    METHOD PUBLIC sum_all : INT
    VAR_INPUT
        args: INT...
    END_VAR
        sum_all := ...args+
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION fn1 : INT
VAR
    f: fb1;
END_VAR
    fn1 := f.sum_all()
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0813] Error: variadic call without arguments
        ,-[ file:///test0.st:15:12 ]
        |
      5 |         args: INT...
        |         ^^|^
        |           `--- variadic parameter 'args' is declared here
        |
     15 |     fn1 := f.sum_all()
        |            ^^^^|^^^^
        |                `------ the call to 'sum_all' passes no argument to the variadic parameter 'args'
    ----'
    ");
}

/// A fold over a name that is not the POU's pack: a member, a name that is
/// nothing, or a POU without a pack. Each was a silent `Never` and an
/// internal compiler error in MIR.
#[rstest]
fn invalid_fold_over_something_else(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Fb
    VAR m : INT; END_VAR
    METHOD Sum : INT
        VAR_INPUT values : INT...; END_VAR
        Sum := ...m+ + ...unknown+;
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION g : INT
    g := ...nothing+;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0814] Error: not a variadic parameter
       ,-[ file:///test0.st:6:16 ]
       |
     5 |         VAR_INPUT values : INT...; END_VAR
       |                   ^^^|^^
       |                      `---- the variadic parameter here is 'values'
     6 |         Sum := ...m+ + ...unknown+;
       |                ^^|^^
       |                  `---- 'm' is not a variadic parameter
       |
       | Note: a fold reads it: `...values+` adds every argument the call passed
    ---'
    [E0814] Error: not a variadic parameter
       ,-[ file:///test0.st:6:24 ]
       |
     5 |         VAR_INPUT values : INT...; END_VAR
       |                   ^^^|^^
       |                      `---- the variadic parameter here is 'values'
     6 |         Sum := ...m+ + ...unknown+;
       |                        ^^^^^|^^^^^
       |                             `------- 'unknown' is not a variadic parameter
       |
       | Note: a fold reads it: `...values+` adds every argument the call passed
    ---'
    [E0814] Error: not a variadic parameter
        ,-[ file:///test0.st:11:10 ]
        |
     11 |     g := ...nothing+;
        |          ^^^^^|^^^^^
        |               `------- 'nothing' is not a variadic parameter
        |
        | Help: declare one in VAR_INPUT, as `values : INT...`
        |
        | Note: the POU declares no variadic parameter
    ----'
    ");
}

/// A pack read, written, counted, passed or put in an initializer: only a
/// fold consumes it. The reads panicked in codegen; the writes went nowhere.
#[rstest]
fn invalid_variadic_parameter_outside_a_fold(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION sum_all : INT
    VAR_INPUT values : INT...; END_VAR
    sum_all := ...values+;
END_FUNCTION

FUNCTION misuse : INT
    VAR_INPUT values : INT...; END_VAR
    VAR first : INT := values; i : INT; END_VAR
    misuse := values;
    values := 0;
    FOR values := 1 TO 3 DO i := i + 1; END_FOR;
    misuse := sum_all(values);
    misuse := values[1];
    misuse := values^;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0816] Error: variadic parameter used outside a fold
       ,-[ file:///test0.st:9:24 ]
       |
     8 |     VAR_INPUT values : INT...; END_VAR
       |               ^^^|^^
       |                  `---- 'values' is declared variadic here
     9 |     VAR first : INT := values; i : INT; END_VAR
       |                        ^^^|^^
       |                           `---- variadic parameter 'values' is used outside a fold
       |
       | Help: read it with a fold: `...values+` adds them, `...values=` compares them
       |
       | Note: a pack stands for as many parameters as the call passed
    ---'
    [E0816] Error: variadic parameter used outside a fold
        ,-[ file:///test0.st:10:15 ]
        |
      8 |     VAR_INPUT values : INT...; END_VAR
        |               ^^^|^^
        |                  `---- 'values' is declared variadic here
        |
     10 |     misuse := values;
        |               ^^^|^^
        |                  `---- variadic parameter 'values' is used outside a fold
        |
        | Help: read it with a fold: `...values+` adds them, `...values=` compares them
        |
        | Note: a pack stands for as many parameters as the call passed
    ----'
    [E0816] Error: variadic parameter used outside a fold
        ,-[ file:///test0.st:11:5 ]
        |
      8 |     VAR_INPUT values : INT...; END_VAR
        |               ^^^|^^
        |                  `---- 'values' is declared variadic here
        |
     11 |     values := 0;
        |     ^^^|^^
        |        `---- variadic parameter 'values' is used outside a fold
        |
        | Help: read it with a fold: `...values+` adds them, `...values=` compares them
        |
        | Note: a pack stands for as many parameters as the call passed
    ----'
    [E0816] Error: variadic parameter used outside a fold
        ,-[ file:///test0.st:12:9 ]
        |
      8 |     VAR_INPUT values : INT...; END_VAR
        |               ^^^|^^
        |                  `---- 'values' is declared variadic here
        |
     12 |     FOR values := 1 TO 3 DO i := i + 1; END_FOR;
        |         ^^^|^^
        |            `---- variadic parameter 'values' is used outside a fold
        |
        | Help: read it with a fold: `...values+` adds them, `...values=` compares them
        |
        | Note: a pack stands for as many parameters as the call passed
    ----'
    [E0816] Error: variadic parameter used outside a fold
        ,-[ file:///test0.st:13:23 ]
        |
      8 |     VAR_INPUT values : INT...; END_VAR
        |               ^^^|^^
        |                  `---- 'values' is declared variadic here
        |
     13 |     misuse := sum_all(values);
        |                       ^^^|^^
        |                          `---- variadic parameter 'values' is used outside a fold
        |
        | Help: read it with a fold: `...values+` adds them, `...values=` compares them
        |
        | Note: a pack stands for as many parameters as the call passed
    ----'
    [E0816] Error: variadic parameter used outside a fold
        ,-[ file:///test0.st:14:15 ]
        |
      8 |     VAR_INPUT values : INT...; END_VAR
        |               ^^^|^^
        |                  `---- 'values' is declared variadic here
        |
     14 |     misuse := values[1];
        |               ^^^|^^
        |                  `---- variadic parameter 'values' is used outside a fold
        |
        | Help: read it with a fold: `...values+` adds them, `...values=` compares them
        |
        | Note: a pack stands for as many parameters as the call passed
    ----'
    [E0816] Error: variadic parameter used outside a fold
        ,-[ file:///test0.st:15:15 ]
        |
      8 |     VAR_INPUT values : INT...; END_VAR
        |               ^^^|^^
        |                  `---- 'values' is declared variadic here
        |
     15 |     misuse := values^;
        |               ^^^|^^
        |                  `---- variadic parameter 'values' is used outside a fold
        |
        | Help: read it with a fold: `...values+` adds them, `...values=` compares them
        |
        | Note: a pack stands for as many parameters as the call passed
    ----'
    ");
}

/// A PROGRAM's or a FUNCTION_BLOCK's input cannot be a pack: neither has an
/// argument count to be specialized for. Refused as it is parsed, so no
/// variadic variable exists outside a FUNCTION or METHOD.
#[rstest]
fn invalid_variadic_parameter_outside_a_function(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM Main
    VAR_INPUT values : INT...; END_VAR
END_PROGRAM

FUNCTION_BLOCK Fb
    VAR_INPUT values : INT...; END_VAR
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0029] Error: variadic parameter outside a FUNCTION or METHOD
       ,-[ file:///test0.st:3:15 ]
       |
     3 |     VAR_INPUT values : INT...; END_VAR
       |               ^^^^^^^|^^^^^^^
       |                      `--------- a PROGRAM takes no variadic parameter
       |
       | Note: the inputs of a PROGRAM are members of one instance
    ---'
    [E0029] Error: variadic parameter outside a FUNCTION or METHOD
       ,-[ file:///test0.st:7:15 ]
       |
     7 |     VAR_INPUT values : INT...; END_VAR
       |               ^^^^^^^|^^^^^^^
       |                      `--------- a FUNCTION_BLOCK takes no variadic parameter
       |
       | Note: the inputs of a FUNCTION_BLOCK are members of one instance
    ---'
    ");
}
