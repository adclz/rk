// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

/// A FOR counter and a sum written before every read: their values
/// between runs are never read.
#[rstest]
fn written_before_read_each_run(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Average
VAR_INPUT values : ARRAY[1..10] OF REAL; END_VAR
VAR_OUTPUT mean : REAL; END_VAR
VAR
    i : INT;
    sum : REAL;
END_VAR
    sum := 0.0;
    FOR i := 1 TO 10 DO
        sum := sum + values[i];
    END_FOR;
    mean := sum / 10.0;
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "var-could-be-temp"), @r"
    [L0317] Hint: VAR that could be a VAR_TEMP
       ,-[ file:///test0.st:6:5 ]
       |
     6 |     i : INT;
       |     |
       |     `-- 'i' is written before it is read each time the body runs
       |
       | Help: declare 'i' in a VAR_TEMP section
       |
       | Note 1: the value 'i' keeps from one call to the next is never read
       |
       | Note 2: lint rule: var-could-be-temp
    ---'
    [L0317] Hint: VAR that could be a VAR_TEMP
       ,-[ file:///test0.st:7:5 ]
       |
     7 |     sum : REAL;
       |     ^|^
       |      `--- 'sum' is written before it is read each time the body runs
       |
       | Help: declare 'sum' in a VAR_TEMP section
       |
       | Note 1: the value 'sum' keeps from one call to the next is never read
       |
       | Note 2: lint rule: var-could-be-temp
    ---'
    ");
}

/// What keeps a value between runs: a read on a path that did not write,
/// a write in one branch only, an element written, `REF()`, a method, a
/// read from outside, RETAIN.
#[rstest]
fn what_keeps_its_value(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Keeper
VAR_INPUT c : BOOL; END_VAR
VAR_OUTPUT q : INT; END_VAR
VAR
    count : INT;
    branch : INT;
    items : ARRAY[0..1] OF INT;
    referenced : INT;
    in_method : INT;
    outside : INT;
    p : REF_TO INT;
END_VAR
VAR RETAIN kept : INT; END_VAR
METHOD Peek : INT
    Peek := in_method;
END_METHOD
    count := count + 1;
    IF c THEN
        branch := 1;
    END_IF;
    q := branch;
    items[0] := 1;
    q := items[0];
    referenced := 1;
    IF p = NULL THEN
        p := REF(referenced);
    END_IF;
    in_method := 1;
    outside := 2;
    kept := 3;
    q := kept;
END_FUNCTION_BLOCK

PROGRAM Main
VAR k : Keeper; x : INT; END_VAR
    k(c := TRUE);
    x := x + k.outside;
END_PROGRAM
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "var-could-be-temp"), @r"");
}
