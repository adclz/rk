// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

/// An address read, written, passed and bound in a body.
#[rstest]
fn an_address_in_a_body(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Pulse
VAR_INPUT x : WORD; END_VAR
VAR_OUTPUT q : BOOL; END_VAR
    q := x > 0;
END_FUNCTION_BLOCK

PROGRAM Mixer
VAR
    level : WORD;
    p : Pulse;
END_VAR
    %QX0.0 := %IX0.1;
    level := %IW2;
    p(x := %MW4, q => %QX0.1);
END_PROGRAM
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "address-in-body"), @r"
    [L0132] Warning: address used in a body
        ,-[ file:///test0.st:13:15 ]
        |
     13 |     %QX0.0 := %IX0.1;
        |               ^^^|^^
        |                  `---- the address '%IX0.1' is used directly
        |
        | Help: declare a variable at it, 'name AT %IX0.1 : BOOL;', and use its name
        |
        | Note 1: a variable declared at the address names what is wired there, and is the one place to change when the wiring moves
        |
        | Note 2: lint rule: address-in-body
    ----'
    [L0132] Warning: address used in a body
        ,-[ file:///test0.st:13:5 ]
        |
     13 |     %QX0.0 := %IX0.1;
        |     ^^^|^^
        |        `---- the address '%QX0.0' is used directly
        |
        | Help: declare a variable at it, 'name AT %QX0.0 : BOOL;', and use its name
        |
        | Note 1: a variable declared at the address names what is wired there, and is the one place to change when the wiring moves
        |
        | Note 2: lint rule: address-in-body
    ----'
    [L0132] Warning: address used in a body
        ,-[ file:///test0.st:14:14 ]
        |
     14 |     level := %IW2;
        |              ^^|^
        |                `--- the address '%IW2' is used directly
        |
        | Help: declare a variable at it, 'name AT %IW2 : WORD;', and use its name
        |
        | Note 1: a variable declared at the address names what is wired there, and is the one place to change when the wiring moves
        |
        | Note 2: lint rule: address-in-body
    ----'
    [L0132] Warning: address used in a body
        ,-[ file:///test0.st:15:12 ]
        |
     15 |     p(x := %MW4, q => %QX0.1);
        |            ^^|^
        |              `--- the address '%MW4' is used directly
        |
        | Help: declare a variable at it, 'name AT %MW4 : WORD;', and use its name
        |
        | Note 1: a variable declared at the address names what is wired there, and is the one place to change when the wiring moves
        |
        | Note 2: lint rule: address-in-body
    ----'
    [L0132] Warning: address used in a body
        ,-[ file:///test0.st:15:23 ]
        |
     15 |     p(x := %MW4, q => %QX0.1);
        |                       ^^^|^^
        |                          `---- the address '%QX0.1' is used directly
        |
        | Help: declare a variable at it, 'name AT %QX0.1 : BOOL;', and use its name
        |
        | Note 1: a variable declared at the address names what is wired there, and is the one place to change when the wiring moves
        |
        | Note 2: lint rule: address-in-body
    ----'
    ");
}

/// A variable declared at an address, the configuration's connections of a
/// program instance and a VAR_CONFIG are no body.
#[rstest]
fn declarations_and_the_configuration(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Motor
VAR
    run AT %Q* : BOOL;
END_VAR
    run := TRUE;
END_FUNCTION_BLOCK

PROGRAM Line
VAR_INPUT pulse : BOOL; END_VAR
VAR
    lamp AT %QX0.2 : BOOL;
    m : Motor;
END_VAR
    lamp := pulse;
    m();
END_PROGRAM

CONFIGURATION Plant
    VAR_CONFIG
        Main.L1.m.run AT %QX0.3 : BOOL;
    END_VAR
    RESOURCE Main ON CPU
        TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM L1 WITH Fast : Line(pulse := %IX0.0);
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "address-in-body"), @r"");
}
