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

// INTO constraint - narrowing is not allowed (LINT -> SINT)
#[rstest]
fn invalid_into_narrowing_lint_to_sint(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION narrow<A: ANY_SIGNED, B: ANY_SIGNED + INTO<A>> : A
    VAR_INPUT x: B; END_VAR
    narrow := x;
END_FUNCTION

FUNCTION test : SINT
    test := narrow<SINT, LINT>(LINT#5);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0316] Error: type argument INTO constraint mismatch
       ,-[ file:///test0.st:8:13 ]
       |
     8 |     test := narrow<SINT, LINT>(LINT#5);
       |             ^^^|^^
       |                `---- 'LINT' cannot be implicitly cast into 'SINT' (INTO constraint on 'B')
    ---'
    ");
}

// INTO constraint - unsigned into signed is not implicit
#[rstest]
fn invalid_into_uint_to_sint(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION convert<A: ANY_INT, B: ANY_INT + INTO<A>> : A
    VAR_INPUT x: B; END_VAR
    convert := x;
END_FUNCTION

FUNCTION test : SINT
    test := convert<SINT, UINT>(UINT#5);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0316] Error: type argument INTO constraint mismatch
       ,-[ file:///test0.st:8:13 ]
       |
     8 |     test := convert<SINT, UINT>(UINT#5);
       |             ^^^|^^^
       |                `----- 'UINT' cannot be implicitly cast into 'SINT' (INTO constraint on 'B')
    ---'
    ");
}

// INTO constraint - REAL cannot be implicitly cast to INT
#[rstest]
fn invalid_into_real_to_int(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION convert<A: ANY_NUM, B: ANY_NUM + INTO<A>> : A
    VAR_INPUT x: B; END_VAR
    convert := x;
END_FUNCTION

FUNCTION test : INT
    test := convert<INT, REAL>(1.5);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0316] Error: type argument INTO constraint mismatch
       ,-[ file:///test0.st:8:13 ]
       |
     8 |     test := convert<INT, REAL>(1.5);
       |             ^^^|^^^
       |                `----- 'REAL' cannot be implicitly cast into 'INT' (INTO constraint on 'B')
    ---'
    ");
}

// INTO constraint - chained: A <- B <- C, C cannot cast into A
#[rstest]
fn invalid_into_chained_constraints(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION chain<A: ANY_SIGNED, B: ANY_SIGNED + INTO<A>, C: ANY_SIGNED + INTO<B>> : A
    VAR_INPUT
        x: B;
        y: C;
    END_VAR
    chain := x;
END_FUNCTION

FUNCTION test : SINT
    test := chain<SINT, INT, LINT>(INT#1, LINT#2);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0316] Error: type argument INTO constraint mismatch
        ,-[ file:///test0.st:11:13 ]
        |
     11 |     test := chain<SINT, INT, LINT>(INT#1, LINT#2);
        |             ^^|^^
        |               `---- 'INT' cannot be implicitly cast into 'SINT' (INTO constraint on 'B')
    ----'
    [E0316] Error: type argument INTO constraint mismatch
        ,-[ file:///test0.st:11:13 ]
        |
     11 |     test := chain<SINT, INT, LINT>(INT#1, LINT#2);
        |             ^^|^^
        |               `---- 'LINT' cannot be implicitly cast into 'INT' (INTO constraint on 'C')
    ----'
    ");
}

// INTO constraint - valid widening across bit types
#[rstest]
fn valid_into_byte_to_word(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION widen<A: ANY_BIT, B: ANY_BIT + INTO<A>> : A
    VAR_INPUT x: B; END_VAR
    widen := x;
END_FUNCTION

FUNCTION test : WORD
    test := widen<WORD, BYTE>(BYTE#16#FF);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// INTO constraint - DWORD cannot narrow into BYTE
#[rstest]
fn invalid_into_dword_to_byte(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION narrow<A: ANY_BIT, B: ANY_BIT + INTO<A>> : A
    VAR_INPUT x: B; END_VAR
    narrow := x;
END_FUNCTION

FUNCTION test : BYTE
    test := narrow<BYTE, DWORD>(DWORD#16#FF);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0316] Error: type argument INTO constraint mismatch
       ,-[ file:///test0.st:8:13 ]
       |
     8 |     test := narrow<BYTE, DWORD>(DWORD#16#FF);
       |             ^^^|^^
       |                `---- 'DWORD' cannot be implicitly cast into 'BYTE' (INTO constraint on 'B')
    ---'
    ");
}

// INTO constraint - valid: SINT widens into LREAL via implicit cast chain
#[rstest]
fn valid_into_sint_to_lreal(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION widen<A: ANY_NUM, B: ANY_NUM + INTO<A>> : A
    VAR_INPUT x: B; END_VAR
    widen := x;
END_FUNCTION

FUNCTION test : LREAL
    test := widen<LREAL, SINT>(SINT#5);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
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

// New generic group tests

#[rstest]
fn valid_any_num_with_int(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1<T: ANY_NUM> : T
    VAR_INPUT x: T; END_VAR
    fn1 := x;
END_FUNCTION

FUNCTION test : INT
    test := fn1<INT>(42);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_any_num_with_real(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1<T: ANY_NUM> : T
    VAR_INPUT x: T; END_VAR
    fn1 := x;
END_FUNCTION

FUNCTION test : REAL
    test := fn1<REAL>(1.5);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn invalid_any_num_with_bool(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1<T: ANY_NUM> : T
    VAR_INPUT x: T; END_VAR
    fn1 := x;
END_FUNCTION

FUNCTION test : INT
    test := fn1<BOOL>(TRUE);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0315] Error: type argument constraint mismatch
       ,-[ file:///test0.st:8:13 ]
       |
     8 |     test := fn1<BOOL>(TRUE);
       |             ^|^
       |              `--- type 'BOOL' does not satisfy constraint 'ANY_NUM' (on generic parameter 'T')
    ---'
    ");
}

#[rstest]
fn valid_any_magnitude_with_time(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1<T: ANY_MAGNITUDE> : T
    VAR_INPUT x: T; END_VAR
    fn1 := x;
END_FUNCTION

FUNCTION test : TIME
    test := fn1<TIME>(T#5s);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_any_chars_with_string(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1<T: ANY_CHARS> : T
    VAR_INPUT x: T; END_VAR
    fn1 := x;
END_FUNCTION

FUNCTION test : STRING
    test := fn1<STRING>('hello');
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_any_char_with_char(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1<T: ANY_CHAR> : T
    VAR_INPUT x: T; END_VAR
    fn1 := x;
END_FUNCTION

FUNCTION test : CHAR
    test := fn1<CHAR>(CHAR#'a');
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn invalid_any_char_with_string(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1<T: ANY_CHAR> : T
    VAR_INPUT x: T; END_VAR
    fn1 := x;
END_FUNCTION

FUNCTION test : STRING
    test := fn1<STRING>('hello');
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0315] Error: type argument constraint mismatch
       ,-[ file:///test0.st:8:13 ]
       |
     8 |     test := fn1<STRING>('hello');
       |             ^|^
       |              `--- type 'STRING' does not satisfy constraint 'ANY_CHAR' (on generic parameter 'T')
    ---'
    ");
}

#[rstest]
fn valid_generic_return_type_inferred_in_assignment(mut with_db: RootDatabase) {
    // generic function return type must resolve to the
    // concrete type inferred from arguments, not remain as the constraint.
    // e.g. LIMIT<T: ANY_NUM>(REAL, REAL, REAL) should return REAL.
    let source = r#"
FUNCTION LIMIT<T: ANY_NUM> : T
    VAR_INPUT
        MN: T;
        IN: T;
        MX: T;
    END_VAR
    LIMIT := IN;
END_FUNCTION

FUNCTION SCALE : REAL
    VAR_INPUT
        X : REAL;
        MN : REAL;
        MX : REAL;
    END_VAR
    SCALE := LIMIT(MN, X, MX);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_generic_inferred_from_array_index(mut with_db: RootDatabase) {
    // Generic type inference must use the adjusted type (array element)
    // not the raw array type. COS(IN := DECADES[0]) should infer T = REAL.
    let source = r#"
TYPE
    CONSTANTS_SETUP : STRUCT
        DECADES : ARRAY[0..8] OF REAL := [1.0, 10.0, 100.0];
    END_STRUCT
END_TYPE

FUNCTION COS<T: ANY_REAL> : T
    VAR_INPUT
        IN: T;
    END_VAR
END_FUNCTION

FUNCTION fn0 : REAL
    fn0 := COS(CONSTANTS_SETUP.DECADES[0]);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_generic_inferred_from_array_index_formal(mut with_db: RootDatabase) {
    // Same as above but with formal parameter syntax (IN := ...).
    let source = r#"
TYPE
    CONSTANTS_SETUP : STRUCT
        DECADES : ARRAY[0..8] OF REAL := [1.0, 10.0, 100.0];
    END_STRUCT
END_TYPE

FUNCTION COS<T: ANY_REAL> : T
    VAR_INPUT
        IN: T;
    END_VAR
END_FUNCTION

FUNCTION fn0 : REAL
    fn0 := COS(IN := CONSTANTS_SETUP.DECADES[0]);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn generic_does_not_cascade_on_never_arg(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION EXP<T: ANY_REAL> : T
    VAR_INPUT
        IN: T;
    END_VAR
END_FUNCTION

FUNCTION TANH : REAL
    VAR_INPUT
        X: REAL;
    END_VAR
    // lowercase 'x' is unresolved — should only report "no item found",
    // not cascade into "does not satisfy constraint" on EXP's generic.
    TANH := 1.0 - 2.0 / (EXP(2.0 * x) + 1.0);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r#"
[E0204] Error: no item found in scope
    ,-[ file:///test0.st:14:36 ]
    |
 14 |     TANH := 1.0 - 2.0 / (EXP(2.0 * x) + 1.0);
    |                                    |
    |                                    `-- no item "x" found in scope
    |
    | Note 1: 'TANH' has item with similar name:
    |         - X
    |
    | Note 2: an item with similar name available in scope:
    |         - EXP
----'
"#);
}

#[rstest]
fn generic_with_unresolved_arg_in_binary_expr(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION ROR<T: ANY_BIT, Y: ANY_INT> : T
    VAR_INPUT
        IN: T;
        N: Y;
    END_VAR
END_FUNCTION

FUNCTION SWAP_BYTE2: DWORD
    VAR_INPUT
        IN: DWORD;
    END_VAR

    SWAP_BYTE2 := (ROR(in ,8) AND 16#FF00FF00);

END_FUNCTION
"#;
    // lowercase 'in' is unresolved — should only report "no item found",
    // not panic from unresolved generic type in binary expression coercion.
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r#"
    [E0204] Error: no item found in scope
        ,-[ file:///test0.st:14:24 ]
        |
     14 |     SWAP_BYTE2 := (ROR(in ,8) AND 16#FF00FF00);
        |                        ^|
        |                         `-- no item "in" found in scope
        |
        | Note: 'SWAP_BYTE2' has item with similar name:
        |       - IN
    ----'
    [E0309] Error: invalid literal
        ,-[ file:///test0.st:14:35 ]
        |
     14 |     SWAP_BYTE2 := (ROR(in ,8) AND 16#FF00FF00);
        |                                   ^^^^^|^^^^^
        |                                        `------- cannot infer '<integer>' to 'INT': number too large to fit in target type
        |                                        |
        |                                        `------- 'INT' is expected due to this
    ----'
    "#);
}
