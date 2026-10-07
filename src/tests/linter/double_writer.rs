// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

/// Two programs writing one `%Q` global, one of them through a FUNCTION it
/// calls.
#[rstest]
fn one_output_two_programs(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION close_valve
VAR_EXTERNAL valve : BOOL; END_VAR
    valve := FALSE;
END_FUNCTION

PROGRAM Filling
VAR_EXTERNAL valve : BOOL; END_VAR
    valve := TRUE;
END_PROGRAM

PROGRAM Draining
    close_valve();
END_PROGRAM

CONFIGURATION Plant
VAR_GLOBAL valve AT %QX0.0 : BOOL; END_VAR
    RESOURCE Main ON CPU
        TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM Fill WITH Fast : Filling;
        PROGRAM Drain WITH Fast : Draining;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "double-writer"), @r"
    [L0129] Warning: output written by several program instances
        ,-[ file:///test0.st:21:9 ]
        |
      4 |     valve := FALSE;
        |     ^^|^^
        |       `---- 'Drain' writes it here
        |
      9 |     valve := TRUE;
        |     ^^|^^
        |       `---- 'Fill' writes it here
        |
     21 |         PROGRAM Drain WITH Fast : Draining;
        |         ^^^^^^^^^^^^^^^^^|^^^^^^^^^^^^^^^^
        |                          `------------------ the output 'valve' at %QX0.0 is written by 2 program instances, 'Fill' and 'Drain'
        |
        | Help: write the output in one program, and send it the others' requests
        |
        | Note 1: at each scan the last instance to run sets the output, and the other writes are lost
        |
        | Note 2: lint rule: double-writer
    ----'
    ");
}

/// A program with a located output, instantiated twice: both instances
/// write the one address.
#[rstest]
fn located_output_in_a_program_run_twice(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM Pump
VAR motor AT %QX0.1 : BOOL; run : BOOL; END_VAR
    motor := run;
END_PROGRAM

CONFIGURATION Plant
    RESOURCE Main ON CPU
        TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH Fast : Pump;
        PROGRAM P2 WITH Fast : Pump;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "double-writer"), @r"
    [L0129] Warning: output written by several program instances
        ,-[ file:///test0.st:11:9 ]
        |
      4 |     motor := run;
        |     ^^|^^
        |       `---- 'P1' writes it here
        |       |
        |       `---- 'P2' writes it here
        |
     11 |         PROGRAM P2 WITH Fast : Pump;
        |         ^^^^^^^^^^^^^|^^^^^^^^^^^^^
        |                      `--------------- the output 'motor' at %QX0.1 is written by 2 program instances, 'P1' and 'P2'
        |
        | Help: write the output in one program, and send it the others' requests
        |
        | Note 1: at each scan the last instance to run sets the output, and the other writes are lost
        |
        | Note 2: lint rule: double-writer
    ----'
    ");
}

/// One writer per output is the rule: an output written by one program and
/// read by the other, a `%M` marker two programs share as a handshake, an
/// output written twice by one program.
#[rstest]
fn one_writer_per_output(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM Producer
VAR_EXTERNAL lamp : BOOL; request : BOOL; END_VAR
    lamp := TRUE;
    IF request THEN
        lamp := FALSE;
    END_IF;
    request := TRUE;
END_PROGRAM

PROGRAM Consumer
VAR_EXTERNAL lamp : BOOL; request : BOOL; END_VAR
VAR seen : BOOL; END_VAR
    seen := lamp;
    request := FALSE;
END_PROGRAM

CONFIGURATION Plant
VAR_GLOBAL
    lamp AT %QX0.2 : BOOL;
    request AT %MX0.0 : BOOL;
END_VAR
    RESOURCE Main ON CPU
        TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM A WITH Fast : Producer;
        PROGRAM B WITH Fast : Consumer;
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "double-writer"), @r"");
}
