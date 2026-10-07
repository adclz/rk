// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

/// The counter the condition reads is never incremented; a REPEAT waits for
/// a flag nothing sets; an EXIT that only leaves a nested FOR leaves the
/// WHILE running.
#[rstest]
fn condition_never_changes(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR i : INT; j : INT; x : INT; done : BOOL; END_VAR
    WHILE i < 10 DO
        x := x + 1;
    END_WHILE;
    REPEAT
        x := x + 1;
    UNTIL done
    END_REPEAT;
    WHILE i < 10 AND x > 0 DO
        FOR j := 0 TO 3 DO
            EXIT;
        END_FOR;
    END_WHILE;
    test := x;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "endless-loop"), @r"
    [L0126] Warning: loop that never ends
       ,-[ file:///test0.st:4:11 ]
       |
     4 |     WHILE i < 10 DO
       |           ^^^|^^
       |              `---- the loop never changes 'i'
       |
       | Help: change 'i' in the loop, or leave it with EXIT
       |
       | Note 1: its condition never changes, so once the loop starts it never ends
       |
       | Note 2: lint rule: endless-loop
    ---'
    [L0126] Warning: loop that never ends
       ,-[ file:///test0.st:9:11 ]
       |
     9 |     UNTIL done
       |           ^^|^
       |             `--- the loop never changes 'done'
       |
       | Help: change 'done' in the loop, or leave it with EXIT
       |
       | Note 1: its condition never changes, so once the loop starts it never ends
       |
       | Note 2: lint rule: endless-loop
    ---'
    [L0126] Warning: loop that never ends
        ,-[ file:///test0.st:11:11 ]
        |
     11 |     WHILE i < 10 AND x > 0 DO
        |           ^^^^^^^^|^^^^^^^
        |                   `--------- the loop changes none of 'i', 'x'
        |
        | Help: change one of them in the loop, or leave it with EXIT
        |
        | Note 1: its condition never changes, so once the loop starts it never ends
        |
        | Note 2: lint rule: endless-loop
    ----'
    ");
}

/// An input does not change during a scan: waiting for one in a loop never
/// ends, a located one included.
#[rstest]
fn waiting_for_an_input(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Door
VAR_INPUT closed : BOOL; END_VAR
    WHILE NOT closed DO
    END_WHILE;
END_FUNCTION_BLOCK

PROGRAM Main
VAR start AT %IX0.0 : BOOL; END_VAR
    WHILE NOT start DO
    END_WHILE;
END_PROGRAM
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "endless-loop"), @r"
    [L0126] Warning: loop that never ends
       ,-[ file:///test0.st:4:11 ]
       |
     4 |     WHILE NOT closed DO
       |           ^^^^^|^^^^
       |                `------ the loop never changes 'closed'
       |
       | Help: change 'closed' in the loop, or leave it with EXIT
       |
       | Note 1: its condition never changes, so once the loop starts it never ends
       |
       | Note 2: lint rule: endless-loop
    ---'
    [L0126] Warning: loop that never ends
        ,-[ file:///test0.st:10:11 ]
        |
     10 |     WHILE NOT start DO
        |           ^^^^|^^^^
        |               `------ the loop never changes 'start'
        |
        | Help: change 'start' in the loop, or leave it with EXIT
        |
        | Note 1: its condition never changes, so once the loop starts it never ends
        |
        | Note 2: lint rule: endless-loop
    ----'
    ");
}

/// A call cannot reach a FUNCTION's own local: the loop still never ends.
#[rstest]
fn a_call_cannot_change_a_local(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION log
VAR_INPUT v : INT; END_VAR
END_FUNCTION

FUNCTION test : INT
VAR i : INT; END_VAR
    WHILE i < 10 DO
        log(v := i);
    END_WHILE;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "endless-loop"), @r"
    [L0126] Warning: loop that never ends
       ,-[ file:///test0.st:8:11 ]
       |
     8 |     WHILE i < 10 DO
       |           ^^^|^^
       |              `---- the loop never changes 'i'
       |
       | Help: change 'i' in the loop, or leave it with EXIT
       |
       | Note 1: its condition never changes, so once the loop starts it never ends
       |
       | Note 2: lint rule: endless-loop
    ---'
    ");
}

/// Each loop here can end: its condition's variable is written, counted by
/// a FOR, passed to a VAR_IN_OUT, written through a reference, or within
/// reach of a call; or the loop leaves with EXIT or RETURN; or its condition
/// calls a function.
#[rstest]
fn loops_that_can_end(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION bump
VAR_IN_OUT io : INT; END_VAR
    io := io + 1;
END_FUNCTION

FUNCTION ready : BOOL
END_FUNCTION

FUNCTION_BLOCK Stepper
VAR done : BOOL; n : INT; END_VAR
METHOD step
    n := n + 1;
    done := n > 10;
END_METHOD
    WHILE NOT done DO
        step();
    END_WHILE;
END_FUNCTION_BLOCK

FUNCTION test : INT
VAR i : INT; j : INT; x : INT; p : REF_TO INT; END_VAR
    WHILE i < 10 DO
        i := i + 1;
    END_WHILE;
    WHILE j < 3 DO
        FOR j := 0 TO 5 DO
            x := x + 1;
        END_FOR;
    END_WHILE;
    WHILE i < 20 DO
        bump(io := i);
    END_WHILE;
    p := REF(i);
    WHILE i < 30 DO
        p^ := p^ + 1;
    END_WHILE;
    WHILE x < 10 DO
        IF i > 5 THEN
            EXIT;
        END_IF;
    END_WHILE;
    REPEAT
        RETURN;
    UNTIL x > 3
    END_REPEAT;
    WHILE NOT ready() DO
        x := x + 1;
    END_WHILE;
    test := x;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "endless-loop"), @r"");
}
