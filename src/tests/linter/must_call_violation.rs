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
METHOD PUBLIC reset
    done := FALSE;
END_METHOD
    done := start;
END_FUNCTION_BLOCK
"#;

/// Read and never called: in a PROGRAM, as a block's member, as an array,
/// and with only a method called, which does not run the body.
#[rstest]
fn instances_never_called(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Station
VAR d : Delay; END_VAR
VAR_OUTPUT ready : BOOL; END_VAR
    ready := d.done;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    d : Delay;
    many : ARRAY[0..3] OF Delay;
    reset_only : Delay;
    lamp : BOOL;
END_VAR
    lamp := d.done OR many[1].done;
    reset_only.reset();
END_PROGRAM
"#;
    assert_snapshot!(
        test_single_lint(&mut with_db, &[DELAY, source], "must-call-violation"),
        @r"
    [L0006] Warning: {must_call} instance never called
       ,-[ file:///test1.st:3:5 ]
       |
     3 | VAR d : Delay; END_VAR
       |     |
       |     `-- 'd' is never called
       |
       |-[ file:///test0.st:2:1 ]
       |
     2 | {must_call}
       | ^^^^^|^^^^^
       |      `------- 'Delay' is marked {must_call} here
       |
       | Help: call 'd' at every scan
       |
       | Note 1: the outputs of a 'Delay' change only when it is called
       |
       | Note 2: lint rule: must-call-violation
    ---'
    [L0006] Warning: {must_call} instance never called
        ,-[ file:///test1.st:10:5 ]
        |
     10 |     d : Delay;
        |     |
        |     `-- 'd' is never called
        |
        |-[ file:///test0.st:2:1 ]
        |
      2 | {must_call}
        | ^^^^^|^^^^^
        |      `------- 'Delay' is marked {must_call} here
        |
        | Help: call 'd' at every scan
        |
        | Note 1: the outputs of a 'Delay' change only when it is called
        |
        | Note 2: lint rule: must-call-violation
    ----'
    [L0006] Warning: {must_call} instance never called
        ,-[ file:///test1.st:11:5 ]
        |
     11 |     many : ARRAY[0..3] OF Delay;
        |     ^^|^
        |       `--- 'many' is never called
        |
        |-[ file:///test0.st:2:1 ]
        |
      2 | {must_call}
        | ^^^^^|^^^^^
        |      `------- 'Delay' is marked {must_call} here
        |
        | Help: call 'many' at every scan
        |
        | Note 1: the outputs of a 'Delay' change only when it is called
        |
        | Note 2: lint rule: must-call-violation
    ----'
    [L0006] Warning: {must_call} instance never called
        ,-[ file:///test1.st:12:5 ]
        |
     12 |     reset_only : Delay;
        |     ^^^^^|^^^^
        |          `------ 'reset_only' is never called
        |
        |-[ file:///test0.st:2:1 ]
        |
      2 | {must_call}
        | ^^^^^|^^^^^
        |      `------- 'Delay' is marked {must_call} here
        |
        | Help: call 'reset_only' at every scan
        |
        | Note 1: the outputs of a 'Delay' change only when it is called
        |
        | Note 2: lint rule: must-call-violation
    ----'
    "
    );
}

/// Called in the body or in a method of the owner, an element of an array
/// called, handed on to a VAR_IN_OUT or to `REF()`: each is called, or its
/// holder's to call. A block without the pragma is not judged.
#[rstest]
fn instances_called_or_handed_on(mut with_db: RootDatabase) {
    let source = r#"
TYPE PDelay : REF_TO Delay; END_TYPE

FUNCTION_BLOCK Plain
VAR_OUTPUT q : BOOL; END_VAR
END_FUNCTION_BLOCK

FUNCTION run_it
VAR_IN_OUT d : Delay; END_VAR
    d(start := TRUE);
END_FUNCTION

FUNCTION_BLOCK Station
VAR a : Delay; b : Delay; END_VAR
METHOD PUBLIC step
    b(start := TRUE);
END_METHOD
    a(start := TRUE);
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    many : ARRAY[0..3] OF Delay;
    lent : Delay;
    pointed : Delay;
    p : PDelay;
    plain : Plain;
    i : INT;
    lamp : BOOL;
END_VAR
    many[i](start := TRUE);
    run_it(d := lent);
    p := REF(pointed);
    lamp := plain.q;
END_PROGRAM
"#;
    assert_snapshot!(
        test_single_lint(&mut with_db, &[DELAY, source], "must-call-violation"),
        @r""
    );
}

/// A member the block never calls itself, called from outside it through a
/// path, `s.inner()`: it runs.
#[rstest]
fn member_called_from_outside_its_block(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Station
VAR_OUTPUT ready : BOOL; END_VAR
VAR inner : Delay; END_VAR
    ready := inner.done;
END_FUNCTION_BLOCK

PROGRAM Main
VAR s : Station; END_VAR
    s.inner(start := TRUE);
    s();
END_PROGRAM
"#;
    assert_snapshot!(
        test_single_lint(&mut with_db, &[DELAY, source], "must-call-violation"),
        @r""
    );
}
