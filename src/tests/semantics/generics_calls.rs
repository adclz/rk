use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

// -- Valid: ANY_* return type with INTO(fn) params ------------------------

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

// -- Valid: ANY_* group membership at call site ---------------------------

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

// -- Invalid: concrete type not in ANY_* group ---------------------------

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

// -- FUNCTION_BLOCK with ANY_* specs -------------------------------------

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
        c : container<INT>;
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
        c : container<INT>;
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

// -- INTO(ref) constraint tests ------------------------------------------

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

// -- Return type inference from ANY_* ------------------------------------

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

// -- ANY_* with array element access -------------------------------------

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

// -- Error cascading: unresolved args should not cascade -----------------

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
    // lowercase 'x' is unresolved - should only report "no item found",
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
    // lowercase 'in' is unresolved - should only report "no item found",
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

// -- INTO / ANY_* bindings at a call site -------------------------------
//
// For an `ANY_*` return type, the concrete result type is resolved by
// folding all args tied to `ANY_*` / `INTO(X)` params through the inference
// table, which promotes to the *widest* concrete arg type. Per-arg coercion
// still uses the abstract bound (ANY_BIT accepts any bit type individually),
// so widening stays silent. Narrowing is caught at the assignment level by
// the existing E0301 rule - no separate "identity" check is needed.

#[rstest]
fn into_binding_same_concrete_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION ROR : ANY_BIT
    VAR_INPUT
        IN: INTO(ROR);
        N: INTO(ROR);
    END_VAR
END_FUNCTION

FUNCTION test : DWORD
    test := ROR(DWORD#0, DWORD#8);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn into_binding_heterogeneous_widens_to_master(mut with_db: RootDatabase) {
    // Master (from `test : DWORD`) = DWORD. BYTE arg widens to DWORD.
    let source = r#"
FUNCTION ROR : ANY_BIT
    VAR_INPUT
        IN: INTO(ROR);
        N: INTO(ROR);
    END_VAR
END_FUNCTION

FUNCTION test : DWORD
    test := ROR(BYTE#0, DWORD#8);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn into_binding_reversed_widening(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION ROR : ANY_BIT
    VAR_INPUT
        IN: INTO(ROR);
        N: INTO(ROR);
    END_VAR
END_FUNCTION

FUNCTION test : DWORD
    test := ROR(DWORD#0, BYTE#8);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn independent_any_int_params_are_uncorrelated(mut with_db: RootDatabase) {
    // Two bare `ANY_INT` params are independent polymorphic slots - a caller
    // is free to pass different concrete int types. If the user wants them
    // correlated, they write `INTO(other_param)` on one of them.
    let source = r#"
FUNCTION add_same : INT
    VAR_INPUT
        a: ANY_INT;
        b: ANY_INT;
    END_VAR
    add_same := 0;
END_FUNCTION

FUNCTION test : INT
    test := add_same(INT#1, DINT#2);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn ror_heterogeneous_args_widens_to_dword_context(mut with_db: RootDatabase) {
    // Widest of {BYTE, DWORD} = DWORD; return type = DWORD; assigns cleanly
    // into DWORD context.
    let source = r#"
FUNCTION ROR : ANY_BIT
    VAR_INPUT
        IN: INTO(ROR);
        N: INTO(ROR);
    END_VAR
END_FUNCTION

FUNCTION SWAP_BYTE2: DWORD
    VAR_INPUT IN: DWORD; END_VAR
    SWAP_BYTE2 := ROR(BYTE#0, DWORD#08);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn ror_incompatible_arg_type_does_not_hijack_return(mut with_db: RootDatabase) {
    // A REAL in an ANY_BIT slot is already an E0301 at the per-arg level.
    // Previously the inference-table's byte-size promotion let REAL take
    // over the BYTE first-arg (REAL is "larger"), which resolved the return
    // to REAL and produced a second cascading assignment error. The fixed
    // table only promotes when implicit widening is actually defined, so
    // REAL is skipped here and only one error is reported.
    let source = r#"
FUNCTION ROR : ANY_BIT
    VAR_INPUT
        IN: INTO(ROR);
        N: INTO(ROR);
    END_VAR
END_FUNCTION

FUNCTION SWAP_BYTE2: BYTE
    VAR_INPUT IN: DWORD; END_VAR
    SWAP_BYTE2 := ROR(BYTE#0, 0.8);
END_FUNCTION
"#;
    let diagnostics = test_diagnostics(&mut with_db, &[source]);
    // Exactly one E0301 - the per-arg one about 0.8 being REAL, not the
    // return-type cascade.
    assert_eq!(
        diagnostics.matches("[E0301]").count(),
        1,
        "expected exactly one E0301; got:\n{}",
        diagnostics
    );
    assert!(
        diagnostics.contains("expected 'ANY_BIT', got 'REAL'"),
        "expected the per-arg ANY_BIT mismatch; got:\n{}",
        diagnostics
    );
}

#[rstest]
fn ror_heterogeneous_args_fails_to_narrow_into_byte_context(mut with_db: RootDatabase) {
    // Widest of {BYTE, DWORD} = DWORD; return type = DWORD; BYTE target
    // needs narrowing - E0301 fires at the assignment, regardless of arg
    // order. Previously the arg order silently determined whether this
    // error fired.
    let source = r#"
FUNCTION ROR : ANY_BIT
    VAR_INPUT
        IN: INTO(ROR);
        N: INTO(ROR);
    END_VAR
END_FUNCTION

FUNCTION SWAP_BYTE2: BYTE
    VAR_INPUT IN: DWORD; END_VAR
    SWAP_BYTE2 := ROR(BYTE#0, DWORD#08);
END_FUNCTION
"#;
    // Expect E0301 at the assignment site.
    let diagnostics = test_diagnostics(&mut with_db, &[source]);
    assert!(
        diagnostics.contains("[E0301]"),
        "expected E0301 narrowing error; got:\n{}",
        diagnostics
    );
}

#[rstest]
fn distinct_any_kinds_are_independent(mut with_db: RootDatabase) {
    // ANY_INT and ANY_REAL are separate slots - heterogeneous args are fine.
    let source = r#"
FUNCTION pair_ok : INT
    VAR_INPUT
        i: ANY_INT;
        r: ANY_REAL;
    END_VAR
    pair_ok := 0;
END_FUNCTION

FUNCTION test : INT
    test := pair_ok(INT#1, LREAL#2.0);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}
