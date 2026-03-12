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
    [E0224] Error: invalid type
        ,-[ file:///test0.st:10:9 ]
        |
     10 |         args: MyStruct...
        |         ^^^^^^^^|^^^^^^^^
        |                 `---------- variable 'args' is declared as variadic but has non-variadic type 'MyStruct'
        |
        | Note: only elementary types can be variadic
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
    [E0317] Error: type mismatch
       ,-[ file:///test0.st:6:16 ]
       |
     6 |     sum_all := ...args+
       |                ^^^^|^^^
       |                    `----- variable 'args' is not variadic
       |
       | Note: ... can only be used on VAR_INPUT variables that are declared variadic with the same operator (e.g: INT...)
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
    [E0318] Error: type mismatch
       ,-[ file:///test0.st:7:16 ]
       |
     4 |         args: BOOL...
       |         ^^|^
       |           `--- type is declared by variable 'args' here
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
    [E0227] Error: invalid variadic declaration
       ,-[ file:///test0.st:5:9 ]
       |
     4 |         a: INT...
       |         ^^^^|^^^^
       |             `------ first variadic variable 'a' declared here
     5 |         b: INT...
       |         ^^^^|^^^^
       |             `------ only one variadic variable is allowed per POU
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
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:10:19 ]
        |
      4 |         args: INT...
        |         ^^|^
        |           `--- type is declared by variable 'args' here
        |
     10 |     sum_all(1, 2, 'hello');
        |                   ^^^|^^^
        |                      `----- expected 'INT', got 'STRING'
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
    [E0228] Error: invalid variadic declaration
       ,-[ file:///test0.st:4:9 ]
       |
     4 |         x: INT;
       |         ^^^|^^
       |            `---- variadic parameter 'args' must be the only VAR_INPUT parameter
     5 |         args: INT...
       |         ^^^^^^|^^^^^
       |               `------- variadic parameter 'args' declared here
       |
       | Note: a variadic parameter must be the only parameter in VAR_INPUT
    ---'
    ");
}