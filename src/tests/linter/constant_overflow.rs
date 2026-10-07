// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

/// Into a DINT CONSTANT, an assignment, typed operands, a CONSTANT operand.
/// An operation on a result that wrapped already is not reported again.
#[rstest]
fn operations_past_their_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION f : DINT
VAR CONSTANT
    N : DINT := 200 * 200;
    HALF : INT := 20000;
END_VAR
VAR
    x : DINT;
END_VAR
    x := 32767 + 1;
    x := INT#200 * INT#200;
    x := (HALF * 2) + 1;
    f := N;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "constant-overflow"), @r"
    [L0128] Warning: constant operation out of range
       ,-[ file:///test0.st:4:17 ]
       |
     4 |     N : DINT := 200 * 200;
       |                 ^^^^|^^^^
       |                     `------ the result, 40000, does not fit in INT
       |
       | Help: compute it at DINT, 'DINT#200 * 200'
       |
       | Note 1: INT holds -32768 to 32767: the program computes -25536
       |
       | Note 2: lint rule: constant-overflow
    ---'
    [L0128] Warning: constant operation out of range
        ,-[ file:///test0.st:10:10 ]
        |
     10 |     x := 32767 + 1;
        |          ^^^^|^^^^
        |              `------ the result, 32768, does not fit in INT
        |
        | Help: compute it at DINT, 'DINT#32767 + 1'
        |
        | Note 1: INT holds -32768 to 32767: the program computes -32768
        |
        | Note 2: lint rule: constant-overflow
    ----'
    [L0128] Warning: constant operation out of range
        ,-[ file:///test0.st:11:10 ]
        |
     11 |     x := INT#200 * INT#200;
        |          ^^^^^^^^|^^^^^^^^
        |                  `---------- the result, 40000, does not fit in INT
        |
        | Help: compute it at DINT, with an operand of that type
        |
        | Note 1: INT holds -32768 to 32767: the program computes -25536
        |
        | Note 2: lint rule: constant-overflow
    ----'
    [L0128] Warning: constant operation out of range
        ,-[ file:///test0.st:12:11 ]
        |
     12 |     x := (HALF * 2) + 1;
        |           ^^^^|^^^
        |               `----- the result, 40000, does not fit in INT
        |
        | Help: compute it at DINT, with an operand of that type
        |
        | Note 1: INT holds -32768 to 32767: the program computes -25536
        |
        | Note 2: lint rule: constant-overflow
    ----'
    ");
}

/// A STRING length and a bound wrap too, in a variable's declaration and in
/// a TYPE's.
#[rstest]
fn sizes_past_their_type(mut with_db: RootDatabase) {
    let source = r#"
TYPE Big : ARRAY[0..300 * 300] OF BYTE; END_TYPE

FUNCTION f : INT
VAR
    s : STRING[300 * 300];
END_VAR
    f := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "constant-overflow"), @r"
    [L0128] Warning: constant operation out of range
       ,-[ file:///test0.st:2:21 ]
       |
     2 | TYPE Big : ARRAY[0..300 * 300] OF BYTE; END_TYPE
       |                     ^^^^|^^^^
       |                         `------ the result, 90000, does not fit in INT
       |
       | Help: compute it at DINT, 'DINT#300 * 300'
       |
       | Note 1: INT holds -32768 to 32767: the program computes 24464
       |
       | Note 2: lint rule: constant-overflow
    ---'
    [L0128] Warning: constant operation out of range
       ,-[ file:///test0.st:6:16 ]
       |
     6 |     s : STRING[300 * 300];
       |                ^^^^|^^^^
       |                    `------ the result, 90000, does not fit in INT
       |
       | Help: compute it at DINT, 'DINT#300 * 300'
       |
       | Note 1: INT holds -32768 to 32767: the program computes 24464
       |
       | Note 2: lint rule: constant-overflow
    ---'
    ");
}

/// What fits is not reported: an operand typed wider, a literal whose minus
/// is part of it, a radix bit pattern, arithmetic within the range, a value
/// that only a variable holds.
#[rstest]
fn operations_within_their_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION f : DINT
VAR CONSTANT
    N : DINT := DINT#200 * 200;
    LOW : INT := -32768;
    MASK : INT := 16#FFFF;
END_VAR
VAR
    x : DINT;
    i : INT := 32767;
    s : STRING[100 * 3];
END_VAR
    x := 32766 + 1;
    x := LOW + 1;
    x := MASK;
    x := i + 1;
    f := N;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "constant-overflow"), @r"");
}
