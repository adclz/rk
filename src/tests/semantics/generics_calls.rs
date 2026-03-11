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
fn valid_generic_with_into_constraint(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION widen<A: ANY_SIGNED, B: ANY_SIGNED + INTO<A>> : A
    VAR_INPUT
        x: B;
    END_VAR
    widen := x;
END_FUNCTION

FUNCTION test : INT
    test := widen<INT, SINT>(SINT#5);
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

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
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
        |             ^|^
        |              `--- expected 1 type argument(s), got 2
    ----'
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
    [E0315] Error: type argument constraint mismatch
        ,-[ file:///test0.st:11:13 ]
        |
     11 |     test := max<REAL>(1.5, 2.5);
        |             ^|^
        |              `--- type 'REAL' does not satisfy constraint 'ANY_INT' (on generic parameter 'T')
    ----'
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

#[rstest]
fn inferred_generic_function_call_with_int(mut with_db: RootDatabase) {
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
    test := max(5, 10);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn inferred_generic_function_with_multiple_params(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION identity<T: ANY_INT, U: ANY_INT> : T
    VAR_INPUT
        a: T;
        b: U;
    END_VAR
    identity := a;
END_FUNCTION

FUNCTION test : INT
    VAR
        x : INT := 5;
        y : DINT := 10;
    END_VAR
    test := identity(x, y);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn inferred_generic_conflicting_types(mut with_db: RootDatabase) {
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

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// INTO constraint tests (cross-parameter)

#[rstest]
fn valid_into_constraint_cross_param(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION widen<A: ANY_SIGNED, B: ANY_SIGNED + INTO<A>> : A
    VAR_INPUT
        x: B;
    END_VAR
    widen := x;
END_FUNCTION

FUNCTION test : INT
    test := widen<INT, SINT>(SINT#5);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_into_constraint_same_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION identity<A: ANY_INT, B: ANY_INT + INTO<A>> : A
    VAR_INPUT
        x: B;
    END_VAR
    identity := x;
END_FUNCTION

FUNCTION test : INT
    test := identity<INT, INT>(42);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn invalid_into_constraint_cross_param(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION widen<A: ANY_INT, B: ANY_INT + INTO<A>> : A
    VAR_INPUT
        x: B;
    END_VAR
    widen := x;
END_FUNCTION

FUNCTION test : INT
    test := widen<INT, DINT>(5);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0316] Error: type argument INTO constraint mismatch
        ,-[ file:///test0.st:10:13 ]
        |
     10 |     test := widen<INT, DINT>(5);
        |             ^^|^^
        |               `---- 'DINT' cannot be implicitly cast into 'INT' (INTO constraint on 'B')
    ----'
    ");
}

#[rstest]
fn invalid_into_constraint_inferred(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION widen<A: ANY_INT, B: ANY_INT + INTO<A>> : A
    VAR_INPUT
        x: A;
        y: B;
    END_VAR
    widen := x;
END_FUNCTION

FUNCTION test : INT
    VAR
        x : INT := 5;
        y : DINT := 10;
    END_VAR
    test := widen(x, y);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0316] Error: type argument INTO constraint mismatch
        ,-[ file:///test0.st:15:13 ]
        |
     15 |     test := widen(x, y);
        |             ^^|^^
        |               `---- 'DINT' cannot be implicitly cast into 'INT' (INTO constraint on 'B')
    ----'
    ");
}

// FUNCTION_BLOCK generic tests

#[rstest]
fn valid_generic_fb_definition(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK container<T: ANY_INT>
    VAR_INPUT
        value: T;
    END_VAR
    VAR
        stored: T;
    END_VAR
    stored := value;
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_generic_fb_inferred_call(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK container<T: ANY_INT>
    VAR_INPUT
        value: T;
    END_VAR
END_FUNCTION_BLOCK

FUNCTION test : INT
    VAR
        c : container;
    END_VAR
    c(value := 42);
    test := 0;
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn invalid_generic_fb_inferred_constraint_mismatch(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK container<T: ANY_INT>
    VAR_INPUT
        value: T;
    END_VAR
END_FUNCTION_BLOCK

FUNCTION test : INT
    VAR
        c : container;
    END_VAR
    c(value := 1.5);
    test := 0;
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0315] Error: type argument constraint mismatch
        ,-[ file:///test0.st:12:5 ]
        |
     12 |     c(value := 1.5);
        |     |
        |     `-- type 'REAL' does not satisfy constraint 'ANY_INT' (on generic parameter 'T')
    ----'
    ");
}

#[rstest]
fn invalid_generic_body_assignment_violates_constraint(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn<T: ANY_REAL> : T
    VAR
        c: T;
    END_VAR
    c := TRUE;
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:6:10 ]
       |
     6 |     c := TRUE;
       |          ^^|^
       |            `--- expected 'ANY_REAL', got 'BOOL'
    ---'
    ");
}
