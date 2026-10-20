// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

/// A variable, a field, a type, a POU called, a parameter and an enum
/// variant, each written in another case than its declaration.
#[rstest]
fn a_name_in_another_case(mut with_db: RootDatabase) {
    let source = r#"
TYPE Color : (Red, Green); END_TYPE
TYPE Drive : STRUCT Speed : INT; END_STRUCT; END_TYPE

FUNCTION StartMotor : INT
VAR_INPUT Level : INT; END_VAR
    StartMotor := Level;
END_FUNCTION

PROGRAM Line
VAR
    MotorSpeed : INT;
    d : drive;
    c : Color;
END_VAR
    motorspeed := startmotor(level := 1);
    d.speed := MotorSpeed;
    c := Color#RED;
END_PROGRAM
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "name-case"), @r"
    [L0315] Hint: name written in another case
        ,-[ file:///test0.st:13:9 ]
        |
      3 | TYPE Drive : STRUCT Speed : INT; END_STRUCT; END_TYPE
        |      ^^|^^
        |        `---- 'Drive' is declared here
        |
     13 |     d : drive;
        |         ^^|^^
        |           `---- 'drive' is declared as 'Drive'
        |
        | Help: write 'Drive'
        |
        | Note 1: a name reads more easily in the case of its declaration
        |
        | Note 2: lint rule: name-case
    ----'
    [L0315] Hint: name written in another case
        ,-[ file:///test0.st:16:5 ]
        |
     12 |     MotorSpeed : INT;
        |     ^^^^^|^^^^
        |          `------ 'MotorSpeed' is declared here
        |
     16 |     motorspeed := startmotor(level := 1);
        |     ^^^^^|^^^^
        |          `------ 'motorspeed' is declared as 'MotorSpeed'
        |
        | Help: write 'MotorSpeed'
        |
        | Note 1: a name reads more easily in the case of its declaration
        |
        | Note 2: lint rule: name-case
    ----'
    [L0315] Hint: name written in another case
        ,-[ file:///test0.st:16:19 ]
        |
      5 | FUNCTION StartMotor : INT
        |          ^^^^^|^^^^
        |               `------ 'StartMotor' is declared here
        |
     16 |     motorspeed := startmotor(level := 1);
        |                   ^^^^^|^^^^
        |                        `------ 'startmotor' is declared as 'StartMotor'
        |
        | Help: write 'StartMotor'
        |
        | Note 1: a name reads more easily in the case of its declaration
        |
        | Note 2: lint rule: name-case
    ----'
    [L0315] Hint: name written in another case
        ,-[ file:///test0.st:16:30 ]
        |
      6 | VAR_INPUT Level : INT; END_VAR
        |           ^^|^^
        |             `---- 'Level' is declared here
        |
     16 |     motorspeed := startmotor(level := 1);
        |                              ^^|^^
        |                                `---- 'level' is declared as 'Level'
        |
        | Help: write 'Level'
        |
        | Note 1: a name reads more easily in the case of its declaration
        |
        | Note 2: lint rule: name-case
    ----'
    [L0315] Hint: name written in another case
        ,-[ file:///test0.st:17:7 ]
        |
      3 | TYPE Drive : STRUCT Speed : INT; END_STRUCT; END_TYPE
        |                     ^^|^^
        |                       `---- 'Speed' is declared here
        |
     17 |     d.speed := MotorSpeed;
        |       ^^|^^
        |         `---- 'speed' is declared as 'Speed'
        |
        | Help: write 'Speed'
        |
        | Note 1: a name reads more easily in the case of its declaration
        |
        | Note 2: lint rule: name-case
    ----'
    [L0315] Hint: name written in another case
        ,-[ file:///test0.st:18:16 ]
        |
      2 | TYPE Color : (Red, Green); END_TYPE
        |               ^|^
        |                `--- 'Red' is declared here
        |
     18 |     c := Color#RED;
        |                ^|^
        |                 `--- 'RED' is declared as 'Red'
        |
        | Help: write 'Red'
        |
        | Note 1: a name reads more easily in the case of its declaration
        |
        | Note 2: lint rule: name-case
    ----'
    ");
}

/// Every name in the case of its declaration.
#[rstest]
fn names_as_declared(mut with_db: RootDatabase) {
    let source = r#"
TYPE Color : (Red, Green); END_TYPE

FUNCTION StartMotor : INT
VAR_INPUT Level : INT; END_VAR
    StartMotor := Level;
END_FUNCTION

PROGRAM Line
VAR
    MotorSpeed : INT;
    c : Color;
END_VAR
    MotorSpeed := StartMotor(Level := 1);
    c := Color#Red;
END_PROGRAM
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "name-case"), @r"");
}
