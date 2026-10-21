// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

/// What the configuration runs reaches: its programs, what they call, the
/// types of their variables, what those extend and what their methods
/// call. An export and a test are roots too. The rest is unused.
#[rstest]
fn what_the_application_does_not_reach(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Called : INT
    Called := 1;
END_FUNCTION

FUNCTION FromMethod : INT
    FromMethod := 2;
END_FUNCTION

FUNCTION_BLOCK Base
END_FUNCTION_BLOCK

FUNCTION_BLOCK Motor EXTENDS Base
METHOD Start : INT
    Start := FromMethod();
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Valve
END_FUNCTION_BLOCK

TYPE Station : STRUCT v : Valve; END_STRUCT; END_TYPE

{export}
FUNCTION Exported : INT
    Exported := 3;
END_FUNCTION

{test}
FUNCTION t_called : BOOL
    t_called := Called() = 1;
END_FUNCTION

FUNCTION Forgotten : INT
    Forgotten := 4;
END_FUNCTION

FUNCTION_BLOCK Spare
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    m : Motor;
    s : Station;
    x : INT;
END_VAR
    x := Called();
    m();
END_PROGRAM

PROGRAM Unrun
END_PROGRAM

CONFIGURATION Plant
    RESOURCE Res ON CPU
        TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM M WITH Fast : Main;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unused-pou"), @r"
    [L0216] Info: unused POU
        ,-[ file:///test0.st:34:10 ]
        |
     34 | FUNCTION Forgotten : INT
        |          ^^^^|^^^^
        |              `------ 'Forgotten' is never used
        |
        | Help: remove 'Forgotten'
        |
        | Note 1: no program, {export} FUNCTION or {test} uses it, directly or through others
        |
        | Note 2: lint rule: unused-pou
    ----'
    [L0216] Info: unused POU
        ,-[ file:///test0.st:38:16 ]
        |
     38 | FUNCTION_BLOCK Spare
        |                ^^|^^
        |                  `---- 'Spare' is never used
        |
        | Help: remove 'Spare'
        |
        | Note 1: no program, {export} FUNCTION or {test} uses it, directly or through others
        |
        | Note 2: lint rule: unused-pou
    ----'
    [L0216] Info: unused POU
        ,-[ file:///test0.st:51:9 ]
        |
     51 | PROGRAM Unrun
        |         ^^|^^
        |           `---- 'Unrun' is never used
        |
        | Help: remove 'Unrun'
        |
        | Note 1: no program, {export} FUNCTION or {test} uses it, directly or through others
        |
        | Note 2: lint rule: unused-pou
    ----'
    ");
}

/// A workspace with no configuration is a library: its POUs are its
/// users'.
#[rstest]
fn a_library_reports_nothing(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Helper : INT
    Helper := 1;
END_FUNCTION

FUNCTION_BLOCK Unused
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "unused-pou"), @r"");
}
