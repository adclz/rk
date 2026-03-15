use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn assign_mismatch_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: INT;
    END_VAR

    test := ULINT#5;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:7:13 ]
       |
     4 |         test: INT;
       |         ^^|^
       |           `--- type is declared by variable 'test' here
       |
     7 |     test := ULINT#5;
       |             ^^^|^^^
       |                `----- expected 'INT', got 'ULINT'
       |                |
       |                `----- consider explicitly casting with 'ULINT_TO_INT(ULINT#5)'
       |
       | Help: insert explicit cast 'INT_TO_ULINT(ULINT#5)'
    ---'
    ");
}

#[rstest]
fn assign_undeclared_pou_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb2

END_FUNCTION_BLOCK

FUNCTION_BLOCK fb1

    fb2 := ULINT#5;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0228] Error: semantic violation
       ,-[ file:///test0.st:8:5 ]
       |
     8 |     fb2 := ULINT#5;
       |     ^|^
       |      `--- cannot use direct type 'fb2' here
    ---'
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:8:12 ]
       |
     2 | FUNCTION_BLOCK fb2
       |                ^|^
       |                 `--- FUNCTION_BLOCK 'fb2' is defined here
       |
     8 |     fb2 := ULINT#5;
       |            ^^^|^^^
       |               `----- expected 'fb2', got 'ULINT'
    ---'
    ");
}

#[rstest]
fn assign_function_return_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : INT

    fn1 := ULINT#5;

END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:12 ]
       |
     2 | FUNCTION fn1 : INT
       |          ^|^
       |           `--- FUNCTION 'fn1' is defined here, with return type 'INT'
       |
     4 |     fn1 := ULINT#5;
       |            ^^^|^^^
       |               `----- expected 'INT', got 'ULINT'
       |               |
       |               `----- consider explicitly casting with 'ULINT_TO_INT(ULINT#5)'
       |
       | Help: insert explicit cast 'INT_TO_ULINT(ULINT#5)'
    ---'
    ");
}

#[rstest]
fn assign_function_with_no_return_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1

    fn1 := ULINT#5;

END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:12 ]
       |
     2 | FUNCTION fn1
       |          ^|^
       |           `--- FUNCTION 'fn1' is defined here
       |
     4 |     fn1 := ULINT#5;
       |            ^^^|^^^
       |               `----- 'fn1' is void and can not be assigned
    ---'
    ");
}

#[rstest]
fn assign_undeclared_type(mut with_db: RootDatabase) {
    let source = r#"
TYPE
    T1 : INT;
END_TYPE

FUNCTION_BLOCK fb1

    T1 := ULINT#5;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0228] Error: semantic violation
       ,-[ file:///test0.st:8:5 ]
       |
     8 |     T1 := ULINT#5;
       |     ^|
       |      `-- cannot use direct type 'T1' here
    ---'
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:8:11 ]
       |
     3 |     T1 : INT;
       |          ^|^
       |           `--- type is defined by 'T1' here
       |
     8 |     T1 := ULINT#5;
       |           ^^^|^^^
       |              `----- expected 'T1', got 'ULINT'
       |              |
       |              `----- consider explicitly casting with 'ULINT_TO_INT(ULINT#5)'
       |
       | Help: insert explicit cast 'INT_TO_ULINT(ULINT#5)'
    ---'
    ");
}

#[rstest]
fn assign_indirect_pou(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb2

END_FUNCTION_BLOCK

FUNCTION_BLOCK fb1
    VAR
        d_fb2: fb2;
    END_VAR

    d_fb2 := ULINT#5;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0226] Error: semantic violation
        ,-[ file:///test0.st:11:5 ]
        |
     11 |     d_fb2 := ULINT#5;
        |     ^^|^^
        |       `---- 'fb2' is a callable type and can not be assigned
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:11:14 ]
        |
      2 | FUNCTION_BLOCK fb2
        |                ^|^
        |                 `--- FUNCTION_BLOCK 'fb2' is defined here
        |
     11 |     d_fb2 := ULINT#5;
        |              ^^^|^^^
        |                 `----- expected 'fb2', got 'ULINT'
    ----'
    ");
}

#[rstest]
fn assign_var_input(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR_INPUT
        test: INT;
    END_VAR

    test := 5;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn assign_void_return_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1

END_FUNCTION

FUNCTION_BLOCK fb1
    VAR
        test: INT;
    END_VAR

    test := fn1();

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:11:13 ]
        |
      8 |         test: INT;
        |         ^^|^
        |           `--- type is declared by variable 'test' here
        |
     11 |     test := fn1();
        |             ^^|^^
        |               `---- expected 'INT', got 'void'
    ----'
    ");
}

#[rstest]
fn assign_invalid_return_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : BOOL

END_FUNCTION

FUNCTION_BLOCK fb1
    VAR
        test: INT;
    END_VAR

    test := fn1();

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:11:13 ]
        |
      8 |         test: INT;
        |         ^^|^
        |           `--- type is declared by variable 'test' here
        |
     11 |     test := fn1();
        |             ^^|^^
        |               `---- expected 'INT', got 'BOOL'
        |               |
        |               `---- consider explicitly casting with 'BOOL_TO_INT(fn1())'
        |
        | Help: insert explicit cast 'INT_TO_BOOL(fn1())'
    ----'
    ");
}

#[rstest]
fn assign_compare_bool(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        test: INT;
    END_VAR

    test := TRUE AND FALSE;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:7:13 ]
       |
     4 |         test: INT;
       |         ^^|^
       |           `--- type is declared by variable 'test' here
       |
     7 |     test := TRUE AND FALSE;
       |             ^^^^^^^|^^^^^^
       |                    `-------- expected 'INT', got 'BOOL'
       |                    |
       |                    `-------- consider explicitly casting with 'BOOL_TO_INT(TRUE AND FALSE)'
       |
       | Help: insert explicit cast 'INT_TO_BOOL(TRUE AND FALSE)'
    ---'
    ");
}

#[rstest]
fn assign_parenthesized_comparison_valid(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        a: INT := 5;
        b: INT := 10;
        result: BOOL;
    END_VAR

    // Parenthesized comparison should return BOOL
    result := (a < b);

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn assign_parenthesized_boolean_op_valid(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        a: BOOL := TRUE;
        b: BOOL := FALSE;
        result: BOOL;
    END_VAR

    // Parenthesized boolean operator should work
    result := (a AND b);
    result := NOT (a OR b);
    result := (a >= b) AND (a <= b);

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn assign_parenthesized_comparison_to_int_invalid(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        a: INT := 5;
        b: INT := 10;
        result: INT;
    END_VAR

    // Parenthesized comparison returns BOOL, not INT
    result := (a < b);

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:10:15 ]
        |
      6 |         result: INT;
        |         ^^^|^^
        |            `---- type is declared by variable 'result' here
        |
     10 |     result := (a < b);
        |               ^^^|^^^
        |                  `----- expected 'INT', got 'BOOL'
        |                  |
        |                  `----- consider explicitly casting with 'BOOL_TO_INT((a < b))'
        |
        | Help: insert explicit cast 'INT_TO_BOOL((a < b))'
    ----'
    ");
}

#[rstest]
fn direct_type_on_rhs_function_block(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Motor
END_FUNCTION_BLOCK

PROGRAM A
    VAR
        x: INT;
    END_VAR

    x := Motor;
END_PROGRAM"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0228] Error: semantic violation
        ,-[ file:///test0.st:10:10 ]
        |
     10 |     x := Motor;
        |          ^^|^^
        |            `---- cannot use direct type 'Motor' here
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:10:10 ]
        |
      7 |         x: INT;
        |         |
        |         `-- type is declared by variable 'x' here
        |
     10 |     x := Motor;
        |          ^^|^^
        |            `---- expected 'INT', got 'Motor'
    ----'
    ");
}

#[rstest]
fn direct_type_on_rhs_class(mut with_db: RootDatabase) {
    let source = r#"
CLASS ClBase
END_CLASS

PROGRAM A
    VAR
        x: INT;
    END_VAR

    x := ClBase;
END_PROGRAM"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0228] Error: semantic violation
        ,-[ file:///test0.st:10:10 ]
        |
     10 |     x := ClBase;
        |          ^^^|^^
        |             `---- cannot use direct type 'ClBase' here
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:10:10 ]
        |
      7 |         x: INT;
        |         |
        |         `-- type is declared by variable 'x' here
        |
     10 |     x := ClBase;
        |          ^^^|^^
        |             `---- expected 'INT', got 'ClBase'
    ----'
    ");
}

#[rstest]
fn direct_type_in_condition(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Motor
END_FUNCTION_BLOCK

PROGRAM A
    IF Motor THEN
    END_IF;
END_PROGRAM"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0228] Error: semantic violation
       ,-[ file:///test0.st:6:8 ]
       |
     6 |     IF Motor THEN
       |        ^^|^^
       |          `---- cannot use direct type 'Motor' here
    ---'
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:6:8 ]
       |
     6 |     IF Motor THEN
       |        ^^|^^
       |          `---- expected 'BOOL', got 'Motor'
    ---'
    ");
}

#[rstest]
fn direct_type_in_arithmetic(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Motor
END_FUNCTION_BLOCK

PROGRAM A
    VAR
        x: INT;
    END_VAR

    x := 5 + Motor;
END_PROGRAM"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0228] Error: semantic violation
        ,-[ file:///test0.st:10:14 ]
        |
     10 |     x := 5 + Motor;
        |              ^^|^^
        |                `---- cannot use direct type 'Motor' here
    ----'
    [E0318] Error: type mismatch
        ,-[ file:///test0.st:10:10 ]
        |
      2 | FUNCTION_BLOCK Motor
        |                ^^|^^
        |                  `---- FUNCTION_BLOCK 'Motor' is defined here
        |
     10 |     x := 5 + Motor;
        |          ^^^^|^^^^
        |              `------ operator '+' cannot be applied to type 'Motor'
    ----'
    [E0303] Error: type mismatch
        ,-[ file:///test0.st:10:14 ]
        |
     10 |     x := 5 + Motor;
        |              ^^|^^
        |                `---- can not add 'INT' with 'Motor'
    ----'
    ");
}

#[rstest]
fn direct_type_field_access_valid(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Motor
    VAR
        speed: INT;
    END_VAR
END_FUNCTION_BLOCK

PROGRAM A
    VAR
        m: Motor;
        x: INT;
    END_VAR

    x := m.speed;
END_PROGRAM"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn direct_type_self_assignment_valid(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : INT
    fn1 := 5;
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn assign_to_var_constant(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : REAL
    VAR CONSTANT
        A: REAL := 3.90802E-3;
        B: REAL := -5.802E-7;
    END_VAR
    VAR
        x: REAL;
    END_VAR

    A := 1.0;
    x := A + B;
END_FUNCTION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1004] Error: semantic violation
        ,-[ file:///test0.st:11:5 ]
        |
     11 |     A := 1.0;
        |     |
        |     `-- cannot assign to constant type
    ----'
    ");
}

#[rstest]
fn assign_to_var_constant_valid_read(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : REAL
    VAR CONSTANT
        A: REAL := 3.90802E-3;
    END_VAR
    VAR
        x: REAL;
    END_VAR

    x := A;
END_FUNCTION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}
