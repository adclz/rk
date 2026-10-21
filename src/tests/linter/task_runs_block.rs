// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

/// An instance a task runs is reported, a program a task runs is not.
#[rstest]
fn a_task_running_an_instance(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Counter
VAR_OUTPUT n : INT; END_VAR
    n := n + 1;
END_FUNCTION_BLOCK

PROGRAM Line
VAR fb1, fb2 : Counter; seen : INT; END_VAR
    seen := fb1.n + fb2.n;
    fb2();
END_PROGRAM

CONFIGURATION Plant
    RESOURCE Res ON CPU
        TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
        TASK Slow(INTERVAL := T#20ms, PRIORITY := 2);
        PROGRAM L WITH Slow : Line(fb1 WITH Fast);
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "task-runs-block"), @r"
    [L0316] Hint: task running a FUNCTION_BLOCK instance
        ,-[ file:///test0.st:17:36 ]
        |
     17 |         PROGRAM L WITH Slow : Line(fb1 WITH Fast);
        |                                    ^|^
        |                                     `--- the task 'Fast' runs the instance 'fb1' instead of a PROGRAM
        |
        | Help: call 'fb1' from a PROGRAM that 'Fast' runs
        |
        | Note 1: the instance then runs at the task's rate, not when its PROGRAM runs
        |
        | Note 2: lint rule: task-runs-block
    ----'
    ");
}
