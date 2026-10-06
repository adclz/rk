// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `ARRAY[*] OF T`: a parameter whose bounds each call gives, one `*` per
//! dimension. It is a VAR_INPUT, VAR_OUTPUT or VAR_IN_OUT of a FUNCTION or a
//! METHOD, or a VAR_IN_OUT of a FUNCTION_BLOCK (E0509). A call binds an array
//! of the same element type and as many dimensions, whatever their bounds,
//! or a row of one, and binds it at every call. `ARRAY[*]` with no `OF` is an
//! array of any type and any number of dimensions, a VAR_INPUT or VAR_IN_OUT
//! of a FUNCTION: it is passed on, or has its bounds read, never indexed
//! (E0511).

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_diagnostics, with_db};

#[rstest]
fn valid_conformands(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Count : DINT
VAR_INPUT a : ARRAY[*]; END_VAR
    Count := 1;
END_FUNCTION

FUNCTION Pass : DINT
VAR_IN_OUT a : ARRAY[*]; END_VAR
    Pass := Count(a);
END_FUNCTION

FUNCTION First : INT
VAR_IN_OUT values : ARRAY[*] OF INT; END_VAR
    First := values[0];
END_FUNCTION

FUNCTION Pick : DINT
VAR_INPUT a : ARRAY[*] OF INT; END_VAR
    Pick := 1;
END_FUNCTION

FUNCTION Pick : DINT
VAR_INPUT a : ARRAY[*]; END_VAR
    Pick := 2;
END_FUNCTION

INTERFACE IScale
METHOD Scale
VAR_IN_OUT values : ARRAY[*] OF REAL; END_VAR
END_METHOD
END_INTERFACE

FUNCTION_BLOCK Doubler IMPLEMENTS IScale
METHOD PUBLIC Scale
VAR_IN_OUT values : ARRAY[*] OF REAL; END_VAR
    values[1] := values[1] * 2.0;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Base
METHOD PUBLIC Fill
VAR_IN_OUT values : ARRAY[0..2] OF INT; END_VAR
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Derived EXTENDS Base
METHOD PUBLIC OVERRIDE Fill
VAR_IN_OUT values : ARRAY[0..2] OF INT; END_VAR
    values[0] := 1;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION UseScale
VAR_INPUT s : IScale; END_VAR
VAR_IN_OUT values : ARRAY[*] OF REAL; END_VAR
    s.Scale(values);
END_FUNCTION

FUNCTION Main : DINT
VAR CONSTANT table : ARRAY[0..2] OF INT := [1, 2, 3]; END_VAR
VAR
    small : ARRAY[0..2] OF INT;
    grid : ARRAY[1..2, 0..2] OF INT;
    reals : ARRAY[1..4] OF REAL;
    d : Doubler;
END_VAR
    Main := Count(small) + Count(grid) + Count(reals) + Count(table) + Pass(grid);
    Main := First(small) + First(grid[1]) + Pick(small) + Pick(reals);
    UseScale(d, reals);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn invalid_conformand_not_allowed(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Fb
VAR_INPUT i : ARRAY[*] OF INT; END_VAR
VAR_OUTPUT o : ARRAY[*] OF INT; END_VAR
VAR_IN_OUT io : ARRAY[*] OF INT; any : ARRAY[*]; END_VAR
METHOD PUBLIC M
VAR_INPUT m : ARRAY[*]; END_VAR
END_METHOD
END_FUNCTION_BLOCK

FUNCTION F
VAR_INPUT i : ARRAY[*]; END_VAR
VAR_OUTPUT o : ARRAY[*]; END_VAR
END_FUNCTION

PROGRAM Main
VAR_INPUT i : ARRAY[*] OF INT; END_VAR
VAR_EXTERNAL g : ARRAY[*] OF INT; END_VAR
END_PROGRAM

CONFIGURATION Cfg
VAR_GLOBAL g : ARRAY[0..3] OF INT; END_VAR
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0509] Error: ARRAY[*] not allowed here
       ,-[ file:///test0.st:3:13 ]
       |
     3 | VAR_INPUT i : ARRAY[*] OF INT; END_VAR
       |             ^^^^^^^^|^^^^^^^^
       |                     `---------- a VAR_INPUT of a FUNCTION_BLOCK cannot be an ARRAY[*]
       |
       | Help: give it bounds, as in `ARRAY[0..9] OF INT`
       |
       | Note: an ARRAY[*] takes its bounds from each call: it is a parameter of a FUNCTION or a METHOD, or a VAR_IN_OUT of a FUNCTION_BLOCK
    ---'
    [E0509] Error: ARRAY[*] not allowed here
       ,-[ file:///test0.st:4:14 ]
       |
     4 | VAR_OUTPUT o : ARRAY[*] OF INT; END_VAR
       |              ^^^^^^^^|^^^^^^^^
       |                      `---------- a VAR_OUTPUT of a FUNCTION_BLOCK cannot be an ARRAY[*]
       |
       | Help: give it bounds, as in `ARRAY[0..9] OF INT`
       |
       | Note: an ARRAY[*] takes its bounds from each call: it is a parameter of a FUNCTION or a METHOD, or a VAR_IN_OUT of a FUNCTION_BLOCK
    ---'
    [E0509] Error: ARRAY[*] not allowed here
       ,-[ file:///test0.st:5:38 ]
       |
     5 | VAR_IN_OUT io : ARRAY[*] OF INT; any : ARRAY[*]; END_VAR
       |                                      ^^^^^|^^^^
       |                                           `------ a VAR_IN_OUT of a FUNCTION_BLOCK cannot be an ARRAY[*] of any type
       |
       | Help: give it an element type, as in `ARRAY[*] OF INT`
       |
       | Note: an ARRAY[*] of any type is a VAR_INPUT or a VAR_IN_OUT of a FUNCTION
    ---'
    [E0509] Error: ARRAY[*] not allowed here
       ,-[ file:///test0.st:7:13 ]
       |
     7 | VAR_INPUT m : ARRAY[*]; END_VAR
       |             ^^^^^|^^^^
       |                  `------ a parameter of a METHOD cannot be an ARRAY[*] of any type
       |
       | Help: give it an element type, as in `ARRAY[*] OF INT`
       |
       | Note: an ARRAY[*] of any type is a VAR_INPUT or a VAR_IN_OUT of a FUNCTION
    ---'
    [E0509] Error: ARRAY[*] not allowed here
        ,-[ file:///test0.st:13:14 ]
        |
     13 | VAR_OUTPUT o : ARRAY[*]; END_VAR
        |              ^^^^^|^^^^
        |                   `------ a VAR_OUTPUT cannot be an ARRAY[*] of any type
        |
        | Help: give it an element type, as in `ARRAY[*] OF INT`
        |
        | Note: an ARRAY[*] of any type is a VAR_INPUT or a VAR_IN_OUT of a FUNCTION
    ----'
    [E0509] Error: ARRAY[*] not allowed here
        ,-[ file:///test0.st:17:13 ]
        |
     17 | VAR_INPUT i : ARRAY[*] OF INT; END_VAR
        |             ^^^^^^^^|^^^^^^^^
        |                     `---------- a VAR_INPUT of a PROGRAM cannot be an ARRAY[*]
        |
        | Help: give it bounds, as in `ARRAY[0..9] OF INT`
        |
        | Note: an ARRAY[*] takes its bounds from each call: it is a parameter of a FUNCTION or a METHOD, or a VAR_IN_OUT of a FUNCTION_BLOCK
    ----'
    [E0509] Error: ARRAY[*] not allowed here
        ,-[ file:///test0.st:18:16 ]
        |
     18 | VAR_EXTERNAL g : ARRAY[*] OF INT; END_VAR
        |                ^^^^^^^^|^^^^^^^^
        |                        `---------- a VAR_EXTERNAL cannot be an ARRAY[*]
        |
        | Help: give it bounds, as in `ARRAY[0..9] OF INT`
        |
        | Note: an ARRAY[*] takes its bounds from each call: it is a parameter of a FUNCTION or a METHOD, or a VAR_IN_OUT of a FUNCTION_BLOCK
    ----'
    ");
}

#[rstest]
fn invalid_conformand_arguments(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Sum : DINT
VAR_IN_OUT values : ARRAY[*] OF INT; END_VAR
END_FUNCTION

FUNCTION Fixed : DINT
VAR_IN_OUT values : ARRAY[0..9] OF INT; END_VAR
END_FUNCTION

FUNCTION Caller : DINT
VAR_IN_OUT typed : ARRAY[*] OF INT; any : ARRAY[*]; END_VAR
VAR
    reals : ARRAY[0..9] OF REAL;
    grid : ARRAY[0..2, 0..2] OF INT;
    n : INT;
    copy : ARRAY[0..9] OF INT;
    far : ARRAY[3000000000..3000000001] OF INT;
END_VAR
    Caller := Sum(reals);
    Caller := Sum(grid);
    Caller := Sum(n);
    Caller := Sum(any);
    Caller := Sum(far);
    Caller := Fixed(typed);
    copy := typed;
    typed := copy;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:19:19 ]
        |
      3 | VAR_IN_OUT values : ARRAY[*] OF INT; END_VAR
        |            ^^^|^^
        |               `---- 'values' is declared here
        |
     19 |     Caller := Sum(reals);
        |                   ^^|^^
        |                     `---- expected 'ARRAY [*] OF INT', got 'ARRAY [0..9] OF REAL'
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:20:19 ]
        |
      3 | VAR_IN_OUT values : ARRAY[*] OF INT; END_VAR
        |            ^^^|^^
        |               `---- 'values' is declared here
        |
     20 |     Caller := Sum(grid);
        |                   ^^|^
        |                     `--- expected 'ARRAY [*] OF INT', got 'ARRAY [0..2, 0..2] OF INT'
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:21:19 ]
        |
      3 | VAR_IN_OUT values : ARRAY[*] OF INT; END_VAR
        |            ^^^|^^
        |               `---- 'values' is declared here
        |
     21 |     Caller := Sum(n);
        |                   |
        |                   `-- expected 'ARRAY [*] OF INT', got 'INT'
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:22:19 ]
        |
      3 | VAR_IN_OUT values : ARRAY[*] OF INT; END_VAR
        |            ^^^|^^
        |               `---- 'values' is declared here
        |
     22 |     Caller := Sum(any);
        |                   ^|^
        |                    `--- expected 'ARRAY [*] OF INT', got 'ARRAY [*]'
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:23:19 ]
        |
      3 | VAR_IN_OUT values : ARRAY[*] OF INT; END_VAR
        |            ^^^|^^
        |               `---- 'values' is declared here
        |
     23 |     Caller := Sum(far);
        |                   ^|^
        |                    `--- expected 'ARRAY [*] OF INT', got 'ARRAY [3000000000..3000000001] OF INT'
        |
        | Note: an ARRAY[*] takes arrays whose bounds fit a DINT
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:24:21 ]
        |
      7 | VAR_IN_OUT values : ARRAY[0..9] OF INT; END_VAR
        |            ^^^|^^
        |               `---- 'values' is declared here
        |
     24 |     Caller := Fixed(typed);
        |                     ^^|^^
        |                       `---- expected 'ARRAY [0..9] OF INT', got 'ARRAY [*] OF INT'
        |
        | Help: read and write it element by element
        |
        | Note: an ARRAY[*] has no bounds of its own to match another array's
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:25:13 ]
        |
     16 |     copy : ARRAY[0..9] OF INT;
        |     ^^|^
        |       `--- 'copy' is declared here
        |
     25 |     copy := typed;
        |             ^^|^^
        |               `---- expected 'ARRAY [0..9] OF INT', got 'ARRAY [*] OF INT'
        |
        | Help: read and write it element by element
        |
        | Note: an ARRAY[*] has no bounds of its own to match another array's
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:26:14 ]
        |
     11 | VAR_IN_OUT typed : ARRAY[*] OF INT; any : ARRAY[*]; END_VAR
        |            ^^|^^
        |              `---- 'typed' is declared here
        |
     26 |     typed := copy;
        |              ^^|^
        |                `--- expected 'ARRAY [*] OF INT', got 'ARRAY [0..9] OF INT'
        |
        | Help: read and write it element by element
        |
        | Note: an ARRAY[*] has no bounds of its own to match another array's
    ----'
    ");
}

#[rstest]
fn invalid_conformand_output_left_unbound(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Fill
VAR_OUTPUT values : ARRAY[*] OF INT; END_VAR
END_FUNCTION

FUNCTION Caller
    Fill();
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0802] Error: missing required parameter
       ,-[ file:///test0.st:7:5 ]
       |
     3 | VAR_OUTPUT values : ARRAY[*] OF INT; END_VAR
       |            ^^^|^^
       |               `---- parameter 'values' is declared here
       |
     7 |     Fill();
       |     ^^|^
       |       `--- call to 'Fill' is missing 1 required parameter: 'values'
       |
       | Note: an ARRAY[*] parameter has the bounds of the array the call binds to it
    ---'
    ");
}

#[rstest]
fn invalid_element_of_any_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION F : INT
VAR_IN_OUT a : ARRAY[*]; END_VAR
    F := a[1];
    a[2] := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0511] Error: element of an ARRAY[*] of any type
       ,-[ file:///test0.st:4:10 ]
       |
     3 | VAR_IN_OUT a : ARRAY[*]; END_VAR
       |            |
       |            `-- 'a' is declared here
     4 |     F := a[1];
       |          |
       |          `-- the elements of 'a' have no type
       |
       | Help: give it an element type, as in `ARRAY[*] OF INT`
       |
       | Note: an ARRAY[*] of any type is passed on or has its bounds read
    ---'
    [E0511] Error: element of an ARRAY[*] of any type
       ,-[ file:///test0.st:5:5 ]
       |
     3 | VAR_IN_OUT a : ARRAY[*]; END_VAR
       |            |
       |            `-- 'a' is declared here
       |
     5 |     a[2] := 0;
       |     |
       |     `-- the elements of 'a' have no type
       |
       | Help: give it an element type, as in `ARRAY[*] OF INT`
       |
       | Note: an ARRAY[*] of any type is passed on or has its bounds read
    ---'
    ");
}

#[rstest]
fn invalid_block_conformand_outside_its_body(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Buffer
VAR_IN_OUT values : ARRAY[*] OF INT; END_VAR
VAR first : REF_TO INT := REF(values[0]); END_VAR
METHOD PUBLIC Peek : INT
VAR i : DINT := values[1]; END_VAR
    Peek := values[i];
END_METHOD
    values[0] := 0;
END_FUNCTION_BLOCK

FUNCTION Caller : INT
VAR b : Buffer; data : ARRAY[1..3] OF INT; END_VAR
    b(values := data);
    Caller := b.values[1];
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0513] Error: ARRAY[*] read outside its block's body
       ,-[ file:///test0.st:4:31 ]
       |
     3 | VAR_IN_OUT values : ARRAY[*] OF INT; END_VAR
       |            ^^^|^^
       |               `---- 'values' is declared here
     4 | VAR first : REF_TO INT := REF(values[0]); END_VAR
       |                               ^^^|^^
       |                                  `---- 'values' has no bounds outside the body of 'Buffer'
       |
       | Help: pass it from the body to an ARRAY[*] parameter of the method
       |
       | Note: an ARRAY[*] VAR_IN_OUT has the bounds of what the call binds to it
    ---'
    [E0513] Error: ARRAY[*] read outside its block's body
       ,-[ file:///test0.st:6:17 ]
       |
     3 | VAR_IN_OUT values : ARRAY[*] OF INT; END_VAR
       |            ^^^|^^
       |               `---- 'values' is declared here
       |
     6 | VAR i : DINT := values[1]; END_VAR
       |                 ^^^|^^
       |                    `---- 'values' has no bounds outside the body of 'Buffer'
       |
       | Help: pass it from the body to an ARRAY[*] parameter of the method
       |
       | Note: an ARRAY[*] VAR_IN_OUT has the bounds of what the call binds to it
    ---'
    [E0513] Error: ARRAY[*] read outside its block's body
       ,-[ file:///test0.st:7:13 ]
       |
     3 | VAR_IN_OUT values : ARRAY[*] OF INT; END_VAR
       |            ^^^|^^
       |               `---- 'values' is declared here
       |
     7 |     Peek := values[i];
       |             ^^^|^^
       |                `---- 'values' has no bounds outside the body of 'Buffer'
       |
       | Help: pass it from the body to an ARRAY[*] parameter of the method
       |
       | Note: an ARRAY[*] VAR_IN_OUT has the bounds of what the call binds to it
    ---'
    [E0513] Error: ARRAY[*] read outside its block's body
        ,-[ file:///test0.st:15:17 ]
        |
      3 | VAR_IN_OUT values : ARRAY[*] OF INT; END_VAR
        |            ^^^|^^
        |               `---- 'values' is declared here
        |
     15 |     Caller := b.values[1];
        |                 ^^^|^^
        |                    `---- 'values' has no bounds outside the body of 'Buffer'
        |
        | Help: pass it from the body to an ARRAY[*] parameter of the method
        |
        | Note: an ARRAY[*] VAR_IN_OUT has the bounds of what the call binds to it
    ----'
    ");
}

#[rstest]
fn invalid_pragmas_on_a_conformand_function(mut with_db: RootDatabase) {
    let source = r#"
{extern 'env' 'sum'}
FUNCTION Imported : DINT
VAR_INPUT values : ARRAY[*] OF INT; END_VAR
END_FUNCTION

{export}
FUNCTION Exported : DINT
VAR_INPUT values : ARRAY[*] OF INT; END_VAR
END_FUNCTION

{test}
FUNCTION Tested
VAR_INPUT values : ARRAY[*]; END_VAR
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1502] Error: not representable on an extern FUNCTION
       ,-[ file:///test0.st:4:11 ]
       |
     4 | VAR_INPUT values : ARRAY[*] OF INT; END_VAR
       |           ^^^|^^
       |              `---- VAR_INPUT 'values' cannot cross a WASM import: an ARRAY[*] has the size of each call's array
       |
       | Note: an extern FUNCTION receives VAR_INPUT copies of a fixed size
    ---'
    [E1509] Error: FUNCTION not exportable
       ,-[ file:///test0.st:7:1 ]
       |
     7 | {export}
       | ^^^^|^^^
       |     `----- 'Exported' cannot be exported: it takes an ARRAY[*]
       |
       | Note: it is compiled once per array type it is called with, so there is no single function to export
    ---'
    [E1511] Error: test FUNCTION with an ARRAY[*] parameter
        ,-[ file:///test0.st:14:11 ]
        |
     14 | VAR_INPUT values : ARRAY[*]; END_VAR
        |           ^^^|^^
        |              `---- the {test} FUNCTION cannot take the ARRAY[*] 'values'
        |
        | Help: call a FUNCTION taking the ARRAY[*] from the test
        |
        | Note: the runner calls a test with no arguments, and an ARRAY[*] takes its bounds from the call
    ----'
    ");
}

#[rstest]
fn invalid_task_running_a_block_with_a_conformand(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Buffer
VAR_IN_OUT values : ARRAY[*] OF INT; END_VAR
END_FUNCTION_BLOCK

PROGRAM P
VAR b : Buffer; END_VAR
END_PROGRAM

CONFIGURATION Cfg
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : P(b WITH T);
    END_RESOURCE
END_CONFIGURATION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E1428] Error: program configuration element refused
        ,-[ file:///test0.st:13:31 ]
        |
     13 |         PROGRAM P1 WITH T : P(b WITH T);
        |                               |
        |                               `-- no task binds the ARRAY[*] 'values' of 'b'
        |
        | Help: call the function block from the program's body
        |
        | Note: an ARRAY[*] VAR_IN_OUT has the bounds of what a call binds to it
    ----'
    ");
}

/// A reference to a STRING element has the element's capacity: a
/// `REF_TO STRING` to a `STRING[4]` element wrote 80 bytes into 4. An element
/// of an `ARRAY[*] OF STRING` has the capacity of each call's array.
#[rstest]
fn invalid_reference_to_a_string_element(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION F
VAR_IN_OUT any : ARRAY[*] OF STRING; END_VAR
VAR
    names : ARRAY[0..1] OF STRING[4];
    grid : ARRAY[0..1, 0..1] OF STRING[4];
    r : REF_TO STRING;
    r4 : REF_TO STRING[4];
END_VAR
    r4 := REF(names[1]);
    r4 := REF(grid[1, 0]);
    r := REF(names[1]);
    r := REF(any[1]);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:12:10 ]
        |
      7 |     r : REF_TO STRING;
        |     |
        |     `-- 'r' is declared here
        |
     12 |     r := REF(names[1]);
        |          ^^^^^^|^^^^^^
        |                `-------- expected 'REF_TO STRING', got 'REF_TO STRING[4]'
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:13:10 ]
        |
      7 |     r : REF_TO STRING;
        |     |
        |     `-- 'r' is declared here
        |
     13 |     r := REF(any[1]);
        |          ^^^^^|^^^^^
        |               `------- expected 'REF_TO STRING', got a reference to an element of an ARRAY[*] OF STRING
        |
        | Note: an element of an ARRAY[*] OF STRING has the capacity of each call's array
    ----'
    ");
}

/// An `ARRAY[*]` is bound to a variable, or a row of one: a call result is
/// none (E0817), as a value is no VAR_IN_OUT argument (E0806).
#[rstest]
fn invalid_conformand_bound_to_a_value(mut with_db: RootDatabase) {
    let source = r#"
TYPE Four : ARRAY[0..3] OF INT; END_TYPE

FUNCTION Readings : Four
END_FUNCTION

FUNCTION Count : DINT
VAR_INPUT any : ARRAY[*]; typed : ARRAY[*] OF INT; END_VAR
END_FUNCTION

FUNCTION Clear : DINT
VAR_IN_OUT values : ARRAY[*] OF INT; END_VAR
END_FUNCTION

FUNCTION Demo : DINT
VAR a : Four; END_VAR
    Demo := Count(Readings(), a);
    Demo := Count(a, Readings());
    Demo := Clear(Readings());
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0817] Error: ARRAY[*] argument not a variable
        ,-[ file:///test0.st:17:19 ]
        |
      8 | VAR_INPUT any : ARRAY[*]; typed : ARRAY[*] OF INT; END_VAR
        |           ^|^
        |            `--- parameter 'any' is declared here
        |
     17 |     Demo := Count(Readings(), a);
        |                   ^^^^^|^^^^
        |                        `------ ARRAY[*] parameter 'any' of 'Count' requires a variable, not a value
        |
        | Help: store the value in a variable, and pass the variable
        |
        | Note: an ARRAY[*] is bound to a variable, or a row of one, and takes its bounds
    ----'
    [E0817] Error: ARRAY[*] argument not a variable
        ,-[ file:///test0.st:18:22 ]
        |
      8 | VAR_INPUT any : ARRAY[*]; typed : ARRAY[*] OF INT; END_VAR
        |                           ^^|^^
        |                             `---- parameter 'typed' is declared here
        |
     18 |     Demo := Count(a, Readings());
        |                      ^^^^^|^^^^
        |                           `------ ARRAY[*] parameter 'typed' of 'Count' requires a variable, not a value
        |
        | Help: store the value in a variable, and pass the variable
        |
        | Note: an ARRAY[*] is bound to a variable, or a row of one, and takes its bounds
    ----'
    [E0806] Error: VAR_IN_OUT argument not a variable
        ,-[ file:///test0.st:19:19 ]
        |
     12 | VAR_IN_OUT values : ARRAY[*] OF INT; END_VAR
        |            ^^^|^^
        |               `---- parameter 'values' is declared here
        |
     19 |     Demo := Clear(Readings());
        |                   ^^^^^|^^^^
        |                        `------ VAR_IN_OUT parameter 'values' of 'Clear' requires a variable, not a value
        |
        | Note: a literal, an expression or a call result has no address for a VAR_IN_OUT to bind
    ----'
    ");
}
