use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn valid_generic_function_call_with_int(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION max<T: ANY_INT> : T
    VAR_INPUT
        a: T;
        b: T;
    END_VAR
    IF a > b THEN
        max := a;
    ELSE
        max := b;
    END_IF
END_FUNCTION

FUNCTION test : INT
    test := max<INT>(5, 10);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_generic_function_call_with_real(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION max<T: ANY_REAL> : T
    VAR_INPUT
        a: T;
        b: T;
    END_VAR
    IF a > b THEN
        max := a;
    ELSE
        max := b;
    END_IF
END_FUNCTION

FUNCTION test : REAL
    test := max<REAL>(1.5, 2.5);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_generic_function_with_multiple_params(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION convert<T: ANY_INT, U: ANY_REAL> : U
    VAR_INPUT
        value: T;
    END_VAR
    convert := value;
END_FUNCTION

FUNCTION test : REAL
    test := convert<INT, REAL>(42);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_generic_with_constraint(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn<A: ANY_INT + INTO<INT>, B: ANY_INT> : B
    VAR_INPUT
        x: A;
    END_VAR
    fn := x;
END_FUNCTION

FUNCTION test : INT
    test := fn<INT, INT>(42);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn invalid_generic_function_call_missing_type_args(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION max<T: ANY_INT> : T
    VAR_INPUT
        a: T;
        b: T;
    END_VAR
    max := a;
END_FUNCTION

FUNCTION test : INT
    test := max(5, 10);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0313] Error: missing type arguments
       ,-[ file:///test0.st:11:13 ]
       |
    11 |     test := max(5, 10);
       |             ^^^
       |             |
       |             `--- generic function 'max' requires explicit type arguments
    ---'
    ");
}

#[rstest]
fn invalid_generic_function_call_wrong_arity(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION max<T: ANY_INT> : T
    VAR_INPUT
        a: T;
        b: T;
    END_VAR
    max := a;
END_FUNCTION

FUNCTION test : INT
    test := max<INT, REAL>(5, 10);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0314] Error: wrong number of type arguments
       ,-[ file:///test0.st:11:13 ]
       |
    11 |     test := max<INT, REAL>(5, 10);
       |             ^^^^^^^^^^^^^^
       |             |
       |             `--- expected 1 type argument(s), got 2
    ---'
    ");
}

#[rstest]
fn invalid_generic_function_call_type_arg_mismatch(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION max<T: ANY_INT> : T
    VAR_INPUT
        a: T;
        b: T;
    END_VAR
    max := a;
END_FUNCTION

FUNCTION test : REAL
    test := max<REAL>(1.5, 2.5);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0315] Error: type argument does not satisfy constraint
       ,-[ file:///test0.st:11:17 ]
       |
    11 |     test := max<REAL>(1.5, 2.5);
       |                 ^^^^
       |                 |
       |                 `--- type 'REAL' does not satisfy constraint 'ANY_INT'
    ---'
    ");
}

#[rstest]
fn valid_generic_variable_usage(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION identity<T: ANY_INT> : T
    VAR_INPUT
        value: T;
    END_VAR
    VAR
        temp: T;
    END_VAR
    temp := value;
    identity := temp;
END_FUNCTION

FUNCTION test : INT
    test := identity<INT>(42);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_generic_operations(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION add<T: ANY_INT> : T
    VAR_INPUT
        a: T;
        b: T;
    END_VAR
    add := a + b;
END_FUNCTION

FUNCTION test : INT
    test := add<INT>(5, 10);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}
