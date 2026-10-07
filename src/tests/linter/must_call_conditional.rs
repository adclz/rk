// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

const DELAY: &str = r#"
{must_call}
FUNCTION_BLOCK Delay
VAR_INPUT start : BOOL; END_VAR
VAR_OUTPUT done : BOOL; END_VAR
    done := start;
END_FUNCTION_BLOCK
"#;

/// Called only in an IF, only in the steps of a CASE, only in a loop.
#[rstest]
fn called_only_in_a_branch(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM Main
VAR
    in_if : Delay;
    shared : Delay;
    looped : Delay;
    auto_mode : BOOL;
    step : INT;
    n : INT;
END_VAR
    IF auto_mode THEN
        in_if(start := TRUE);
    END_IF;
    CASE step OF
        1: shared(start := TRUE);
        2: shared(start := TRUE);
    END_CASE;
    WHILE n < 3 DO
        looped(start := TRUE);
        n := n + 1;
    END_WHILE;
END_PROGRAM
"#;
    assert_snapshot!(
        test_single_lint(&mut with_db, &[DELAY, source], "must-call-conditional"),
        @r"
    [L0007] Warning: {must_call} instance called only in a branch
        ,-[ file:///test1.st:12:9 ]
        |
     12 |         in_if(start := TRUE);
        |         ^^|^^
        |           `---- 'in_if' is called only inside an IF
        |
        |-[ file:///test0.st:2:1 ]
        |
      2 | {must_call}
        | ^^^^^|^^^^^
        |      `------- 'Delay' is marked {must_call} here
        |
        | Help: call 'in_if' at every scan, outside the IF, and pass the condition as an input
        |
        | Note 1: a scan that skips the call leaves 'in_if' as it was
        |
        | Note 2: lint rule: must-call-conditional
    ----'
    [L0007] Warning: {must_call} instance called only in a branch
        ,-[ file:///test1.st:15:12 ]
        |
     15 |         1: shared(start := TRUE);
        |            ^^^|^^
        |               `---- 'shared' is called only inside a CASE
        |
        |-[ file:///test0.st:2:1 ]
        |
      2 | {must_call}
        | ^^^^^|^^^^^
        |      `------- 'Delay' is marked {must_call} here
        |
        | Help: call 'shared' at every scan, outside the CASE, and pass the condition as an input
        |
        | Note 1: a scan that skips the call leaves 'shared' as it was
        |
        | Note 2: lint rule: must-call-conditional
    ----'
    [L0007] Warning: {must_call} instance called only in a branch
        ,-[ file:///test1.st:19:9 ]
        |
     19 |         looped(start := TRUE);
        |         ^^^|^^
        |            `---- 'looped' is called only inside a loop
        |
        |-[ file:///test0.st:2:1 ]
        |
      2 | {must_call}
        | ^^^^^|^^^^^
        |      `------- 'Delay' is marked {must_call} here
        |
        | Help: call 'looped' at every scan, outside the loop, and pass the condition as an input
        |
        | Note 1: a scan that skips the call leaves 'looped' as it was
        |
        | Note 2: lint rule: must-call-conditional
    ----'
    "
    );
}

/// Called at the top of the body, once there among calls in branches, in a
/// method, handed on in a branch, or an array a FOR walks: each runs at
/// every scan, or at times the body does not show.
#[rstest]
fn called_at_every_scan_or_elsewhere(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION run_it
VAR_IN_OUT d : Delay; END_VAR
    d(start := TRUE);
END_FUNCTION

FUNCTION_BLOCK Station
VAR by_method : Delay; END_VAR
METHOD PUBLIC step
    by_method(start := TRUE);
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    plain : Delay;
    mixed : Delay;
    lent : Delay;
    many : ARRAY[0..3] OF Delay;
    i : INT;
    go : BOOL;
END_VAR
    plain(start := go);
    mixed(start := go);
    IF go THEN
        mixed(start := TRUE);
        run_it(d := lent);
    END_IF;
    FOR i := 0 TO 3 DO
        many[i](start := go);
    END_FOR;
END_PROGRAM
"#;
    assert_snapshot!(
        test_single_lint(&mut with_db, &[DELAY, source], "must-call-conditional"),
        @r""
    );
}
