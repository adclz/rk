// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn simple_self_assignment(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    x := x;
    test := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "self-assignment"), @r"
    [L0104] Warning: self-assignment
       ,-[ file:///test0.st:6:5 ]
       |
     6 |     x := x;
       |     ^^^|^^
       |        `---- variable 'x' is assigned to itself
       |
       | Note: lint rule: self-assignment
    ---'
    ");
}

#[rstest]
fn no_warning_different_variables(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
    y : INT;
END_VAR
    x := y;
    test := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "self-assignment"), @r"");
}

#[rstest]
fn no_warning_expression_rhs(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    x := x + 1;
    test := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "self-assignment"), @r"");
}

#[rstest]
fn self_assignment_in_if_body(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
    flag : BOOL;
END_VAR
    IF flag THEN
        x := x;
    END_IF;
    test := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "self-assignment"), @r"
    [L0104] Warning: self-assignment
       ,-[ file:///test0.st:8:9 ]
       |
     8 |         x := x;
       |         ^^^|^^
       |            `---- variable 'x' is assigned to itself
       |
       | Note: lint rule: self-assignment
    ---'
    ");
}

#[rstest]
fn self_assignment_in_fb(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFb
VAR
    counter : INT;
END_VAR
    counter := counter;
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "self-assignment"), @r"
    [L0104] Warning: self-assignment
       ,-[ file:///test0.st:6:5 ]
       |
     6 |     counter := counter;
       |     ^^^^^^^^^|^^^^^^^^
       |              `---------- variable 'counter' is assigned to itself
       |
       | Note: lint rule: self-assignment
    ---'
    ");
}

/// Each link of a chain is an assignment: `b := b` within it is one, and a
/// chain of other places is none.
#[rstest]
fn self_assignment_in_a_chain(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR a, b : INT; END_VAR
    a := b := b := 1;
    a := b := 1;
    test := a;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "self-assignment"), @r"
    [L0104] Warning: self-assignment
       ,-[ file:///test0.st:4:10 ]
       |
     4 |     a := b := b := 1;
       |          ^^^|^^
       |             `---- variable 'b' is assigned to itself
       |
       | Note: lint rule: self-assignment
    ---'
    ");
}

/// One variable is not yet one place: an element copied to another, a bit
/// to another, is no self-assignment, in a chain as alone.
#[rstest]
fn no_warning_for_another_part_of_the_variable(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR arr : ARRAY[0..3] OF INT; w : WORD; i, j, a : INT; END_VAR
    arr[0] := arr[1];
    arr[i] := arr[j];
    w.%X3 := w.%X4;
    a := arr[1] := arr[2] := 0;
    test := a;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "self-assignment"), @r"");
}

/// The same part, written alike but for spaces and case, is a
/// self-assignment.
#[rstest]
fn self_assignment_of_one_element(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR arr : ARRAY[0..3] OF INT; i : INT; END_VAR
    arr[i] := ARR[ I ];
    test := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "self-assignment"), @r"
    [L0104] Warning: self-assignment
       ,-[ file:///test0.st:4:5 ]
       |
     4 |     arr[i] := ARR[ I ];
       |     ^^^^^^^^^|^^^^^^^^
       |              `---------- variable 'arr' is assigned to itself
       |
       | Note: lint rule: self-assignment
    ---'
    ");
}
