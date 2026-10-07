// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn assign_to_var_input(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR_INPUT
    x : INT;
END_VAR
    x := 42;
    test := x;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "input-assignment"), @r"
    [L0113] Warning: assignment to input variable
       ,-[ file:///test0.st:6:5 ]
       |
     4 |     x : INT;
       |     |
       |     `-- 'x' is declared here
       |
     6 |     x := 42;
       |     |
       |     `-- assignment to VAR_INPUT 'x'
       |
       | Note: lint rule: input-assignment
    ---'
    ");
}

#[rstest]
fn assign_to_var_input_in_fb(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
VAR_INPUT
    x : INT;
END_VAR
    x := 42;
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "input-assignment"), @r"
    [L0113] Warning: assignment to input variable
       ,-[ file:///test0.st:6:5 ]
       |
     4 |     x : INT;
       |     |
       |     `-- 'x' is declared here
       |
     6 |     x := 42;
       |     |
       |     `-- assignment to VAR_INPUT 'x'
       |
       | Note: lint rule: input-assignment
    ---'
    ");
}

#[rstest]
fn no_assign_to_var_input(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
VAR_INPUT
    x : INT;
END_VAR
VAR
    _y : INT;
END_VAR
    _y := x;
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "input-assignment"), @r"");
}

#[rstest]
fn assign_to_local_var(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    x := 42;
    test := x;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "input-assignment"), @r"");
}

#[rstest]
fn assign_to_var_input_in_if(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
VAR_INPUT
    x : INT;
END_VAR
    IF x > 0 THEN
        x := 0;
    END_IF;
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "input-assignment"), @r"
    [L0113] Warning: assignment to input variable
       ,-[ file:///test0.st:7:9 ]
       |
     4 |     x : INT;
       |     |
       |     `-- 'x' is declared here
       |
     7 |         x := 0;
       |         |
       |         `-- assignment to VAR_INPUT 'x'
       |
       | Note: lint rule: input-assignment
    ---'
    ");
}

/// Through an instance, `t.x := 1` sets another block's input, the way
/// `t(x := 1)` does: only a block writing its own input is reported, by its
/// name or through THIS.
#[rstest]
fn input_of_another_instance_not_flagged(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Child
VAR_INPUT
    x : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK Parent
VAR_INPUT
    own : INT;
END_VAR
VAR
    t : Child;
END_VAR
    t.x := 1;
    t();
    THIS.own := 2;
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "input-assignment"), @r"
    [L0113] Warning: assignment to input variable
        ,-[ file:///test0.st:17:5 ]
        |
     10 |     own : INT;
        |     ^|^
        |      `--- 'own' is declared here
        |
     17 |     THIS.own := 2;
        |     ^^^^|^^^
        |         `----- assignment to VAR_INPUT 'own'
        |
        | Note: lint rule: input-assignment
    ----'
    ");
}
