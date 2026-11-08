// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn fb_field_mutation(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFB
VAR
    x : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION caller : INT
VAR
    fb : MyFB;
END_VAR
    fb.x := 42;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "external-mutation"), @r"
    [L0117] Warning: external instance mutation
        ,-[ file:///test0.st:12:5 ]
        |
     12 |     fb.x := 42;
        |     ^^|^
        |       `--- 'fb.x' is written from outside its instance
        |
        | Note: lint rule: external-mutation
    ----'
    ");
}

#[rstest]
fn class_field_mutation(mut with_db: RootDatabase) {
    let source = r#"
CLASS MyClass
VAR PUBLIC
    x : INT;
END_VAR
END_CLASS

FUNCTION caller : INT
VAR
    obj : MyClass;
END_VAR
    obj.x := 42;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "external-mutation"), @r"
    [L0117] Warning: external instance mutation
        ,-[ file:///test0.st:12:5 ]
        |
     12 |     obj.x := 42;
        |     ^^|^^
        |       `---- 'obj.x' is written from outside its instance
        |
        | Note: lint rule: external-mutation
    ----'
    ");
}

#[rstest]
fn no_mutation_simple_var(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    x := 42;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "external-mutation"), @r"");
}

#[rstest]
fn fb_call_not_flagged(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFB
VAR_INPUT
    x : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION caller : INT
VAR
    fb : MyFB;
END_VAR
    fb(x := 42);
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "external-mutation"), @r"");
}

#[rstest]
fn this_field_not_flagged(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Parent
VAR
    child : Child;
END_VAR
    METHOD doWork : INT
        THIS.child.x := 42;
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Child
VAR
    x : INT;
END_VAR
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "external-mutation"), @r"");
}

/// An input is the caller's to set: written before the call, it is passed
/// the way `fb(x := 42)` passes it. An output written from outside is still
/// reported.
#[rstest]
fn input_written_from_outside_not_flagged(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFB
VAR_INPUT
    x : INT;
END_VAR
VAR_OUTPUT
    y : INT;
END_VAR
    y := x;
END_FUNCTION_BLOCK

FUNCTION caller : INT
VAR
    fb : MyFB;
END_VAR
    fb.x := 42;
    fb();
    fb.y := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "external-mutation"), @r"
    [L0117] Warning: external instance mutation
        ,-[ file:///test0.st:18:5 ]
        |
     18 |     fb.y := 0;
        |     ^^|^
        |       `--- 'fb.y' is written from outside its instance
        |
        | Note: lint rule: external-mutation
    ----'
    ");
}

/// The variable a `__CATCH` names is written the message, as an
/// assignment would: the output of another instance is reported, and its
/// input, which is what a caller writes, is not.
#[rstest]
fn a_catch_writes_its_variable(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Counter
VAR_INPUT label : STRING; END_VAR
VAR_OUTPUT last : STRING; END_VAR
END_FUNCTION_BLOCK

FUNCTION caller : INT
VAR
    c : Counter;
END_VAR
    __TRY
        caller := 1;
    __CATCH(c.last)
        caller := 2;
    __END_TRY;
    __TRY
        caller := 3;
    __CATCH(c.label)
        caller := 4;
    __END_TRY;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "external-mutation"), @r"
    [L0117] Warning: external instance mutation
        ,-[ file:///test0.st:13:13 ]
        |
     13 |     __CATCH(c.last)
        |             ^^^|^^
        |                `---- 'c.last' is written from outside its instance
        |
        | Note: lint rule: external-mutation
    ----'
    ");
}

/// A VAR_IN_OUT the block keeps as a member is an input written back, and
/// the caller sets it the way it sets an input: `fb.io := 42; fb();`. One
/// bound by reference, a STRING, is written through the last call's
/// binding, and is reported.
#[rstest]
fn in_out_member_written_from_outside_not_flagged(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFB
VAR_IN_OUT
    io : INT;
    s : STRING;
END_VAR
    io := io + 1;
    s := 'x';
END_FUNCTION_BLOCK

FUNCTION caller : INT
VAR
    fb : MyFB;
    t : STRING;
END_VAR
    fb.io := 42;
    fb(s := t);
    fb.s := 'y';
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "external-mutation"), @r"
    [L0117] Warning: external instance mutation
        ,-[ file:///test0.st:18:5 ]
        |
     18 |     fb.s := 'y';
        |     ^^|^
        |       `--- 'fb.s' is written from outside its instance
        |
        | Note: lint rule: external-mutation
    ----'
    ");
}

/// A chain writes each of its places: a member of another instance among
/// them is written from outside.
#[rstest]
fn fb_field_mutation_by_a_chain(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFB
VAR
    x : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION caller : INT
VAR
    fb : MyFB;
    y : INT;
END_VAR
    fb.x := y := 42;
    caller := y;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "external-mutation"), @r"
    [L0117] Warning: external instance mutation
        ,-[ file:///test0.st:13:5 ]
        |
     13 |     fb.x := y := 42;
        |     ^^|^
        |       `--- 'fb.x' is written from outside its instance
        |
        | Note: lint rule: external-mutation
    ----'
    ");
}

/// A part of a member is the member written: an element, a field, an input
/// of an instance the block holds, a member of an element or of the instance
/// a reference points at. A part of an input is the caller's to set, and
/// what a member points at is not the instance.
#[rstest]
fn a_part_of_a_member_written_from_outside(mut with_db: RootDatabase) {
    let source = r#"
TYPE Pair : STRUCT a : INT; b : INT; END_STRUCT; END_TYPE

FUNCTION_BLOCK Inner
VAR_INPUT x : INT; END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK MyFB
VAR_INPUT inp : Pair; list : ARRAY[0..2] OF INT; END_VAR
VAR
    x : ARRAY[0..2] OF INT;
    s : Pair;
    inner : Inner;
    r : REF_TO Pair;
END_VAR
END_FUNCTION_BLOCK

FUNCTION caller : INT
VAR
    fb : MyFB;
    fbs : ARRAY[0..1] OF MyFB;
    p : REF_TO MyFB;
    target : Pair;
END_VAR
    p := REF(fb);
    fb.x[0] := 1;
    fb.s.a := 1;
    fb.inner.x := 1;
    fbs[1].s.a := 1;
    p^.s.b := 1;
    fb.inp.a := 1;    // a part of an input
    fb.list[1] := 1;  // a part of an input
    fb.r^.a := 1;     // what a member points at
    target.a := 1;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "external-mutation"), @r"
    [L0117] Warning: external instance mutation
        ,-[ file:///test0.st:26:5 ]
        |
     26 |     fb.x[0] := 1;
        |     ^^^|^^^
        |        `----- 'fb.x[0]' is written from outside its instance
        |
        | Note: lint rule: external-mutation
    ----'
    [L0117] Warning: external instance mutation
        ,-[ file:///test0.st:27:5 ]
        |
     27 |     fb.s.a := 1;
        |     ^^^|^^
        |        `---- 'fb.s.a' is written from outside its instance
        |
        | Note: lint rule: external-mutation
    ----'
    [L0117] Warning: external instance mutation
        ,-[ file:///test0.st:28:5 ]
        |
     28 |     fb.inner.x := 1;
        |     ^^^^^|^^^^
        |          `------ 'fb.inner.x' is written from outside its instance
        |
        | Note: lint rule: external-mutation
    ----'
    [L0117] Warning: external instance mutation
        ,-[ file:///test0.st:29:5 ]
        |
     29 |     fbs[1].s.a := 1;
        |     ^^^^^|^^^^
        |          `------ 'fbs[1].s.a' is written from outside its instance
        |
        | Note: lint rule: external-mutation
    ----'
    [L0117] Warning: external instance mutation
        ,-[ file:///test0.st:30:5 ]
        |
     30 |     p^.s.b := 1;
        |     ^^^|^^
        |        `---- 'p^.s.b' is written from outside its instance
        |
        | Note: lint rule: external-mutation
    ----'
    ");
}
