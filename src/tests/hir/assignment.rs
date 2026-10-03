// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

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
       |           `--- 'test' is declared here
       |
     7 |     test := ULINT#5;
       |             ^^^|^^^
       |                `----- expected 'INT', got 'ULINT'
       |
       | Help: insert explicit cast 'ULINT_TO_INT(ULINT#5)'
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
    [E0317] Error: type name used as a value
       ,-[ file:///test0.st:8:5 ]
       |
     8 |     fb2 := ULINT#5;
       |     ^|^
       |      `--- 'fb2' is not a value
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
       |           `--- FUNCTION 'fn1' is declared here, with return type 'INT'
       |
     4 |     fn1 := ULINT#5;
       |            ^^^|^^^
       |               `----- expected 'INT', got 'ULINT'
       |
       | Help: insert explicit cast 'ULINT_TO_INT(ULINT#5)'
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
    [E0319] Error: assignment to a missing return value
       ,-[ file:///test0.st:4:5 ]
       |
     2 | FUNCTION fn1
       |          ^|^
       |           `--- FUNCTION 'fn1' is declared here
       |
     4 |     fn1 := ULINT#5;
       |     ^|^
       |      `--- 'fn1' has no return value to assign
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
    [E0317] Error: type name used as a value
       ,-[ file:///test0.st:8:5 ]
       |
     8 |     T1 := ULINT#5;
       |     ^|
       |      `-- 'T1' is not a value
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
    [E0318] Error: assignment to an instance
        ,-[ file:///test0.st:11:5 ]
        |
     11 |     d_fb2 := ULINT#5;
        |     ^^|^^
        |       `---- an instance of 'fb2' cannot be assigned
        |
        | Help: pass the instance as a VAR_IN_OUT, or assign its members one by one
    ----'
    ");
}

/// A CLASS instance cannot be assigned either (E0318). It has no body, so it
/// is not callable, and the check that refuses a FUNCTION_BLOCK's assignment
/// let it through as a copy.
#[rstest]
fn invalid_class_instance_assigned(mut with_db: RootDatabase) {
    let source = r#"
CLASS Sensor
VAR n : INT; END_VAR
END_CLASS

FUNCTION_BLOCK Holder
VAR a : Sensor; b : Sensor; END_VAR
    b := a;
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0318] Error: assignment to an instance
       ,-[ file:///test0.st:8:5 ]
       |
     8 |     b := a;
       |     |
       |     `-- an instance of 'Sensor' cannot be assigned
       |
       | Help: pass the instance as a VAR_IN_OUT, or assign its members one by one
    ---'
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
        |           `--- 'test' is declared here
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
        |           `--- 'test' is declared here
        |
     11 |     test := fn1();
        |             ^^|^^
        |               `---- expected 'INT', got 'BOOL'
        |
        | Help: insert explicit cast 'BOOL_TO_INT(fn1())'
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
       |           `--- 'test' is declared here
       |
     7 |     test := TRUE AND FALSE;
       |             ^^^^^^^|^^^^^^
       |                    `-------- expected 'INT', got 'BOOL'
       |
       | Help: insert explicit cast 'BOOL_TO_INT(TRUE AND FALSE)'
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
        |            `---- 'result' is declared here
        |
     10 |     result := (a < b);
        |               ^^^|^^^
        |                  `----- expected 'INT', got 'BOOL'
        |
        | Help: insert explicit cast 'BOOL_TO_INT((a < b))'
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
    [E0317] Error: type name used as a value
        ,-[ file:///test0.st:10:10 ]
        |
     10 |     x := Motor;
        |          ^^|^^
        |            `---- 'Motor' is not a value
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
    [E0317] Error: type name used as a value
        ,-[ file:///test0.st:10:10 ]
        |
     10 |     x := ClBase;
        |          ^^^|^^
        |             `---- 'ClBase' is not a value
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
    [E0317] Error: type name used as a value
       ,-[ file:///test0.st:6:8 ]
       |
     6 |     IF Motor THEN
       |        ^^|^^
       |          `---- 'Motor' is not a value
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
    [E0317] Error: type name used as a value
        ,-[ file:///test0.st:10:14 ]
        |
     10 |     x := 5 + Motor;
        |              ^^|^^
        |                `---- 'Motor' is not a value
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
    [E0404] Error: write to a constant
        ,-[ file:///test0.st:11:5 ]
        |
      4 |         A: REAL := 3.90802E-3;
        |         |
        |         `-- 'A' is declared CONSTANT here
        |
     11 |     A := 1.0;
        |     |
        |     `-- cannot write to constant 'A'
        |
        | Help: copy it into a variable to change the copy
        |
        | Note: a CONSTANT keeps the value it is declared with
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

// An element or a field of a constant is the constant. A field write and an
// output bound to a part passed, and the constant changed.
#[rstest]
fn invalid_write_into_part_of_a_constant(mut with_db: RootDatabase) {
    let source = r#"
TYPE Pt : STRUCT x : INT; END_STRUCT; END_TYPE

CONFIGURATION Cfg
VAR_GLOBAL CONSTANT G : Pt := (x := 3); END_VAR
END_CONFIGURATION

FUNCTION Put
VAR_OUTPUT o : INT; END_VAR
    o := 1;
END_FUNCTION

FUNCTION F : INT
VAR_EXTERNAL CONSTANT G : Pt; END_VAR
VAR CONSTANT
    p : Pt := (x := 1);
    arr : ARRAY[0..1] OF INT := [1, 2];
END_VAR
    p.x := 5;
    G.x := 7;
    arr[0] := 5;
    Put(o => p.x);
    Put(o => arr[1]);
    F := p.x;
END_FUNCTION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0404] Error: write to a constant
        ,-[ file:///test0.st:19:5 ]
        |
     16 |     p : Pt := (x := 1);
        |     |
        |     `-- 'p' is declared CONSTANT here
        |
     19 |     p.x := 5;
        |     ^|^
        |      `--- cannot write to 'p.x' in constant 'p'
        |
        | Help: copy it into a variable to change the copy
        |
        | Note: a CONSTANT keeps the value it is declared with
    ----'
    [E0404] Error: write to a constant
        ,-[ file:///test0.st:20:5 ]
        |
     14 | VAR_EXTERNAL CONSTANT G : Pt; END_VAR
        |                       |
        |                       `-- 'G' is declared CONSTANT here
        |
     20 |     G.x := 7;
        |     ^|^
        |      `--- cannot write to 'G.x' in constant 'G'
        |
        | Help: copy it into a variable to change the copy
        |
        | Note: a CONSTANT keeps the value it is declared with
    ----'
    [E0404] Error: write to a constant
        ,-[ file:///test0.st:21:5 ]
        |
     17 |     arr : ARRAY[0..1] OF INT := [1, 2];
        |     ^|^
        |      `--- 'arr' is declared CONSTANT here
        |
     21 |     arr[0] := 5;
        |     ^^^|^^
        |        `---- cannot write to 'arr[0]' in constant 'arr'
        |
        | Help: copy it into a variable to change the copy
        |
        | Note: a CONSTANT keeps the value it is declared with
    ----'
    [E0404] Error: write to a constant
        ,-[ file:///test0.st:22:14 ]
        |
     16 |     p : Pt := (x := 1);
        |     |
        |     `-- 'p' is declared CONSTANT here
        |
     22 |     Put(o => p.x);
        |              ^|^
        |               `--- cannot write to 'p.x' in constant 'p'
        |
        | Help: copy it into a variable to change the copy
        |
        | Note: a CONSTANT keeps the value it is declared with
    ----'
    [E0404] Error: write to a constant
        ,-[ file:///test0.st:23:14 ]
        |
     17 |     arr : ARRAY[0..1] OF INT := [1, 2];
        |     ^|^
        |      `--- 'arr' is declared CONSTANT here
        |
     23 |     Put(o => arr[1]);
        |              ^^^|^^
        |                 `---- cannot write to 'arr[1]' in constant 'arr'
        |
        | Help: copy it into a variable to change the copy
        |
        | Note: a CONSTANT keeps the value it is declared with
    ----'
    ");
}

// A constant handed to a VAR_IN_OUT or to REF() changed through the
// parameter or the pointer: only a store into it was refused.
#[rstest]
fn invalid_constant_handed_to_in_out_or_ref(mut with_db: RootDatabase) {
    let source = r#"
TYPE Pt : STRUCT x : INT; END_STRUCT; END_TYPE

CONFIGURATION Cfg
VAR_GLOBAL CONSTANT LIMIT : INT := 10; END_VAR
END_CONFIGURATION

FUNCTION Bump
VAR_IN_OUT x : INT; END_VAR
    x := x + 1;
END_FUNCTION

FUNCTION_BLOCK FbBump
VAR_IN_OUT x : INT; END_VAR
    x := x + 1;
END_FUNCTION_BLOCK

PROGRAM P
VAR_EXTERNAL CONSTANT LIMIT : INT; END_VAR
VAR CONSTANT
    k : INT := 1;
    arr : ARRAY[0..1] OF INT := [1, 2];
    p : Pt := (x := 1);
END_VAR
VAR f : FbBump; r : REF_TO INT; END_VAR
    Bump(x := k);
    Bump(LIMIT);
    f(x := arr[0]);
    Bump(x := p.x);
    r := REF(k);
    r := REF(p.x);
END_PROGRAM
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0404] Error: write to a constant
        ,-[ file:///test0.st:26:15 ]
        |
     21 |     k : INT := 1;
        |     |
        |     `-- 'k' is declared CONSTANT here
        |
     26 |     Bump(x := k);
        |               |
        |               `-- cannot pass constant 'k' to a VAR_IN_OUT
        |
        | Help: pass it to a VAR_INPUT, or copy it into a variable and pass that
        |
        | Note: a VAR_IN_OUT could change it
    ----'
    [E0404] Error: write to a constant
        ,-[ file:///test0.st:27:10 ]
        |
     19 | VAR_EXTERNAL CONSTANT LIMIT : INT; END_VAR
        |                       ^^|^^
        |                         `---- 'LIMIT' is declared CONSTANT here
        |
     27 |     Bump(LIMIT);
        |          ^^|^^
        |            `---- cannot pass constant 'LIMIT' to a VAR_IN_OUT
        |
        | Help: pass it to a VAR_INPUT, or copy it into a variable and pass that
        |
        | Note: a VAR_IN_OUT could change it
    ----'
    [E0404] Error: write to a constant
        ,-[ file:///test0.st:28:12 ]
        |
     22 |     arr : ARRAY[0..1] OF INT := [1, 2];
        |     ^|^
        |      `--- 'arr' is declared CONSTANT here
        |
     28 |     f(x := arr[0]);
        |            ^^^|^^
        |               `---- cannot pass 'arr[0]' in constant 'arr' to a VAR_IN_OUT
        |
        | Help: pass it to a VAR_INPUT, or copy it into a variable and pass that
        |
        | Note: a VAR_IN_OUT could change it
    ----'
    [E0404] Error: write to a constant
        ,-[ file:///test0.st:29:15 ]
        |
     23 |     p : Pt := (x := 1);
        |     |
        |     `-- 'p' is declared CONSTANT here
        |
     29 |     Bump(x := p.x);
        |               ^|^
        |                `--- cannot pass 'p.x' in constant 'p' to a VAR_IN_OUT
        |
        | Help: pass it to a VAR_INPUT, or copy it into a variable and pass that
        |
        | Note: a VAR_IN_OUT could change it
    ----'
    [E0404] Error: write to a constant
        ,-[ file:///test0.st:30:14 ]
        |
     21 |     k : INT := 1;
        |     |
        |     `-- 'k' is declared CONSTANT here
        |
     30 |     r := REF(k);
        |              |
        |              `-- cannot take a reference to constant 'k'
        |
        | Help: copy it into a variable and take the reference of that
        |
        | Note: a reference could change it
    ----'
    [E0404] Error: write to a constant
        ,-[ file:///test0.st:31:14 ]
        |
     23 |     p : Pt := (x := 1);
        |     |
        |     `-- 'p' is declared CONSTANT here
        |
     31 |     r := REF(p.x);
        |              ^|^
        |               `--- cannot take a reference to 'p.x' in constant 'p'
        |
        | Help: copy it into a variable and take the reference of that
        |
        | Note: a reference could change it
    ----'
    ");
}

// A constant is read freely, and a VAR_INPUT takes a copy of it.
#[rstest]
fn valid_constant_read_or_copied(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Twice : INT
VAR_INPUT x : INT; END_VAR
    Twice := x * 2;
END_FUNCTION

PROGRAM P
VAR CONSTANT k : INT := 3; END_VAR
VAR copy : INT; r : REF_TO INT; END_VAR
    copy := k + Twice(x := k) + Twice(k);
    r := REF(copy);
END_PROGRAM
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// A FOR counter and a bit write store into the constant too.
#[rstest]
fn invalid_constant_as_for_counter_or_bit(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION F : INT
VAR CONSTANT
    k : INT := 1;
    MASK : WORD := 16#00FF;
END_VAR
    FOR k := 1 TO 3 DO
        F := F + 1;
    END_FOR;
    MASK.3 := TRUE;
END_FUNCTION
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0404] Error: write to a constant
       ,-[ file:///test0.st:7:9 ]
       |
     4 |     k : INT := 1;
       |     |
       |     `-- 'k' is declared CONSTANT here
       |
     7 |     FOR k := 1 TO 3 DO
       |         |
       |         `-- cannot write to constant 'k'
       |
       | Help: copy it into a variable to change the copy
       |
       | Note: a CONSTANT keeps the value it is declared with
    ---'
    [E0404] Error: write to a constant
        ,-[ file:///test0.st:10:5 ]
        |
      5 |     MASK : WORD := 16#00FF;
        |     ^^|^
        |       `--- 'MASK' is declared CONSTANT here
        |
     10 |     MASK.3 := TRUE;
        |     ^^^|^^
        |        `---- cannot write to 'MASK.3' in constant 'MASK'
        |
        | Help: copy it into a variable to change the copy
        |
        | Note: a CONSTANT keeps the value it is declared with
    ----'
    ");
}

// An instance changes when it runs, so it cannot be CONSTANT: its body and
// its methods ran, and changed it.
#[rstest]
fn invalid_constant_instance(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Fb
VAR n : INT; END_VAR
    n := n + 1;
END_FUNCTION_BLOCK

CLASS Counter
VAR n : INT; END_VAR
    METHOD Bump : INT
        n := n + 1;
        Bump := n;
    END_METHOD
END_CLASS

CONFIGURATION Cfg
VAR_GLOBAL CONSTANT shared : Fb; END_VAR
END_CONFIGURATION

PROGRAM P
VAR CONSTANT
    f : Fb;
    c : Counter;
    many : ARRAY[0..1] OF Fb;
END_VAR
    f();
END_PROGRAM
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0404] Error: write to a constant
        ,-[ file:///test0.st:21:5 ]
        |
     21 |     f : Fb;
        |     |
        |     `-- instance 'f' of 'Fb' cannot be CONSTANT
        |
        | Help: declare it in a VAR section without CONSTANT
        |
        | Note: an instance changes when it runs
    ----'
    [E0404] Error: write to a constant
        ,-[ file:///test0.st:22:5 ]
        |
     22 |     c : Counter;
        |     |
        |     `-- instance 'c' of 'Counter' cannot be CONSTANT
        |
        | Help: declare it in a VAR section without CONSTANT
        |
        | Note: an instance changes when it runs
    ----'
    [E0404] Error: write to a constant
        ,-[ file:///test0.st:23:5 ]
        |
     23 |     many : ARRAY[0..1] OF Fb;
        |     ^^|^
        |       `--- array 'many' of 'Fb' instances cannot be CONSTANT
        |
        | Help: declare it in a VAR section without CONSTANT
        |
        | Note: an instance changes when it runs
    ----'
    [E0404] Error: write to a constant
        ,-[ file:///test0.st:16:21 ]
        |
     16 | VAR_GLOBAL CONSTANT shared : Fb; END_VAR
        |                     ^^^|^^
        |                        `---- instance 'shared' of 'Fb' cannot be CONSTANT
        |
        | Help: declare it in a VAR section without CONSTANT
        |
        | Note: an instance changes when it runs
    ----'
    ");
}

// A reference to a constant is refused wherever it is taken: an input's
// default, a configuration global, a CONSTANT's own value.
#[rstest]
fn invalid_reference_to_a_constant_in_an_initial_value(mut with_db: RootDatabase) {
    let source = r#"
TYPE PInt : REF_TO INT; END_TYPE

CONFIGURATION Cfg
VAR_GLOBAL CONSTANT LIMIT : INT := 10; END_VAR
VAR_GLOBAL gp : PInt := REF(LIMIT); END_VAR
END_CONFIGURATION

FUNCTION Takes : INT
VAR_EXTERNAL CONSTANT LIMIT : INT; END_VAR
VAR_INPUT r : REF_TO INT := REF(LIMIT); END_VAR
    Takes := r^;
END_FUNCTION

PROGRAM P
VAR_EXTERNAL CONSTANT LIMIT : INT; END_VAR
VAR CONSTANT PK : PInt := REF(LIMIT); END_VAR
END_PROGRAM
"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0404] Error: write to a constant
        ,-[ file:///test0.st:11:33 ]
        |
     10 | VAR_EXTERNAL CONSTANT LIMIT : INT; END_VAR
        |                       ^^|^^
        |                         `---- 'LIMIT' is declared CONSTANT here
     11 | VAR_INPUT r : REF_TO INT := REF(LIMIT); END_VAR
        |                                 ^^|^^
        |                                   `---- cannot take a reference to constant 'LIMIT'
        |
        | Help: copy it into a variable and take the reference of that
        |
        | Note: a reference could change it
    ----'
    [E0404] Error: write to a constant
        ,-[ file:///test0.st:17:31 ]
        |
     16 | VAR_EXTERNAL CONSTANT LIMIT : INT; END_VAR
        |                       ^^|^^
        |                         `---- 'LIMIT' is declared CONSTANT here
     17 | VAR CONSTANT PK : PInt := REF(LIMIT); END_VAR
        |                               ^^|^^
        |                                 `---- cannot take a reference to constant 'LIMIT'
        |
        | Help: copy it into a variable and take the reference of that
        |
        | Note: a reference could change it
    ----'
    [E0404] Error: write to a constant
       ,-[ file:///test0.st:6:29 ]
       |
     5 | VAR_GLOBAL CONSTANT LIMIT : INT := 10; END_VAR
       |                     ^^|^^
       |                       `---- 'LIMIT' is declared CONSTANT here
     6 | VAR_GLOBAL gp : PInt := REF(LIMIT); END_VAR
       |                             ^^|^^
       |                               `---- cannot take a reference to constant 'LIMIT'
       |
       | Help: copy it into a variable and take the reference of that
       |
       | Note: a reference could change it
    ---'
    ");
}
