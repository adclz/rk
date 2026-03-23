use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

// ── Valid: ANY_* return type with INTO(fn) params ────────────────────────

#[rstest]
fn valid_any_int_function_with_int(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION max : ANY_INT
    VAR_INPUT
        a: INTO(max);
        b: INTO(max);
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
fn valid_any_real_function_with_real(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION max : ANY_REAL
    VAR_INPUT
        a: INTO(max);
        b: INTO(max);
    END_VAR
    IF a > b THEN
        max := a;
    ELSE
        max := b;
    END_IF
END_FUNCTION

FUNCTION test : REAL
    test := max(1.5, 2.5);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_any_int_variable_usage(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION identity : ANY_INT
    VAR_INPUT
        value: INTO(identity);
    END_VAR
    VAR
        temp: INTO(identity);
    END_VAR
    temp := value;
    identity := temp;
END_FUNCTION

FUNCTION test : INT
    test := identity(42);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_any_int_operations(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION add : ANY_INT
    VAR_INPUT
        a: INTO(add);
        b: INTO(add);
    END_VAR
    add := a + b;
END_FUNCTION

FUNCTION test : INT
    test := add(5, 10);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// ── Valid: ANY_* group membership at call site ───────────────────────────

#[rstest]
fn valid_any_num_with_int(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : ANY_NUM
    VAR_INPUT x: INTO(fn1); END_VAR
    fn1 := x;
END_FUNCTION

FUNCTION test : INT
    test := fn1(42);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_any_num_with_real(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : ANY_NUM
    VAR_INPUT x: INTO(fn1); END_VAR
    fn1 := x;
END_FUNCTION

FUNCTION test : REAL
    test := fn1(1.5);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_any_magnitude_with_time(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : ANY_MAGNITUDE
    VAR_INPUT x: INTO(fn1); END_VAR
    fn1 := x;
END_FUNCTION

FUNCTION test : TIME
    test := fn1(T#5s);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_any_chars_with_string(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : ANY_CHARS
    VAR_INPUT x: INTO(fn1); END_VAR
    fn1 := x;
END_FUNCTION

FUNCTION test : STRING
    test := fn1('hello');
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_any_char_with_char(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : ANY_CHAR
    VAR_INPUT x: INTO(fn1); END_VAR
    fn1 := x;
END_FUNCTION

FUNCTION test : CHAR
    test := fn1(CHAR#'a');
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// ── Invalid: concrete type not in ANY_* group ───────────────────────────

#[rstest]
fn invalid_any_num_with_bool(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : ANY_NUM
    VAR_INPUT x: INTO(fn1); END_VAR
    fn1 := x;
END_FUNCTION

FUNCTION test : INT
    test := fn1(TRUE);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:8:17 ]
       |
     3 |     VAR_INPUT x: INTO(fn1); END_VAR
       |               |
       |               `-- type is declared by variable 'x' here
       |
     8 |     test := fn1(TRUE);
       |                 ^^|^
       |                   `--- expected 'ANY_NUM', got 'BOOL'
    ---'
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:8:13 ]
       |
     7 | FUNCTION test : INT
       |          ^^|^
       |            `--- FUNCTION 'test' is defined here, with return type 'INT'
     8 |     test := fn1(TRUE);
       |             ^^^^|^^^^
       |                 `------ expected 'INT', got 'BOOL'
       |                 |
       |                 `------ consider explicitly casting with 'BOOL_TO_INT(fn1(TRUE))'
       |
       | Help: insert explicit cast 'BOOL_TO_INT(fn1(TRUE))'
    ---'
    ");
}

#[rstest]
fn invalid_any_char_with_string(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : ANY_CHAR
    VAR_INPUT x: INTO(fn1); END_VAR
    fn1 := x;
END_FUNCTION

FUNCTION test : STRING
    test := fn1('hello');
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:8:17 ]
       |
     3 |     VAR_INPUT x: INTO(fn1); END_VAR
       |               |
       |               `-- type is declared by variable 'x' here
       |
     8 |     test := fn1('hello');
       |                 ^^^|^^^
       |                    `----- expected 'ANY_CHAR', got 'STRING'
    ---'
    ");
}

// ── FUNCTION_BLOCK with ANY_* specs ─────────────────────────────────────

#[rstest]
fn valid_fb_with_any_int(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK container
    VAR_INPUT
        value: ANY_INT;
    END_VAR
    VAR
        stored: INTO(value);
    END_VAR
    stored := value;
END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_fb_call_with_any_int(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK container
    VAR_INPUT
        value: ANY_INT;
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
fn invalid_fb_any_int_with_real(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK container
    VAR_INPUT
        value: ANY_INT;
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
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:12:16 ]
        |
      4 |         value: ANY_INT;
        |         ^^|^^
        |           `---- type is declared by variable 'value' here
        |
     12 |     c(value := 1.5);
        |                ^|^
        |                 `--- expected 'ANY_INT', got 'REAL'
    ----'
    ");
}

// ── INTO(ref) constraint tests ──────────────────────────────────────────

#[rstest]
fn valid_into_constraint(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION widen : ANY_SIGNED
    VAR_INPUT
        x: INTO(widen);
    END_VAR
    widen := x;
END_FUNCTION

FUNCTION test : INT
    test := widen(SINT#3);
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// ── Return type inference from ANY_* ────────────────────────────────────

#[rstest]
fn valid_any_num_return_type_in_assignment(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION LIMIT : ANY_NUM
    VAR_INPUT
        MN: INTO(LIMIT);
        IN: INTO(LIMIT);
        MX: INTO(LIMIT);
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

// ── ANY_* with array element access ─────────────────────────────────────

#[rstest]
fn valid_any_real_inferred_from_array_index(mut with_db: RootDatabase) {
    let source = r#"
TYPE
    CONSTANTS_SETUP : STRUCT
        DECADES : ARRAY[0..8] OF REAL := [1.0, 10.0, 100.0];
    END_STRUCT
END_TYPE

FUNCTION COS : ANY_REAL
    VAR_INPUT
        IN: INTO(COS);
    END_VAR
END_FUNCTION

FUNCTION fn0 : REAL
    fn0 := COS(CONSTANTS_SETUP.DECADES[0]);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_any_real_inferred_from_array_index_formal(mut with_db: RootDatabase) {
    let source = r#"
TYPE
    CONSTANTS_SETUP : STRUCT
        DECADES : ARRAY[0..8] OF REAL := [1.0, 10.0, 100.0];
    END_STRUCT
END_TYPE

FUNCTION COS : ANY_REAL
    VAR_INPUT
        IN: INTO(COS);
    END_VAR
END_FUNCTION

FUNCTION fn0 : REAL
    fn0 := COS(IN := CONSTANTS_SETUP.DECADES[0]);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// ── Error cascading: unresolved args should not cascade ─────────────────

#[rstest]
fn any_real_does_not_cascade_on_never_arg(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION EXP : ANY_REAL
    VAR_INPUT
        IN: INTO(EXP);
    END_VAR
END_FUNCTION

FUNCTION TANH : REAL
    VAR_INPUT
        X: REAL;
    END_VAR
    // lowercase 'x' is unresolved — should only report "no item found",
    // not cascade into a type mismatch on EXP's parameter.
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
fn any_bit_with_unresolved_arg_in_binary_expr(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION ROR : ANY_BIT
    VAR_INPUT
        IN: INTO(ROR);
        N: ANY_INT;
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
    // not panic from unresolved type in binary expression coercion.
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
        |                    ^^^^^|^^^^     ^^^^^|^^^^^
        |                         `---------------------- 'INT' is expected due to this
        |                                        |
        |                                        `------- cannot infer '<integer>' to 'INT': number too large to fit in target type
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:14:19 ]
        |
      9 | FUNCTION SWAP_BYTE2: DWORD
        |          ^^^^^|^^^^
        |               `------ FUNCTION 'SWAP_BYTE2' is defined here, with return type 'DWORD'
        |
     14 |     SWAP_BYTE2 := (ROR(in ,8) AND 16#FF00FF00);
        |                   ^^^^^^^^^^^^^^|^^^^^^^^^^^^^
        |                                 `--------------- expected 'DWORD', got 'INT'
        |                                 |
        |                                 `--------------- consider explicitly casting with 'INT_TO_DWORD((ROR(in ,8) AND 16#FF00FF00))'
        |
        | Help: insert explicit cast 'INT_TO_DWORD((ROR(in ,8) AND 16#FF00FF00))'
    ----'
    "#);
}
