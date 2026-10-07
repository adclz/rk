// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

/// `=` and `<>` on REAL or LREAL values, a REAL against an INT included:
/// the comparison runs at REAL.
#[rstest]
fn real_compared_exactly(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR
    x : REAL;
    y : LREAL;
    n : INT;
END_VAR
    test := x = 0.3;
    test := y <> x;
    test := x = n;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "float-equality"), @r"
    [L0124] Warning: exact comparison of REAL values
       ,-[ file:///test0.st:8:13 ]
       |
     8 |     test := x = 0.3;
       |             ^^^|^^^
       |                `----- REAL values compared with '='
       |
       | Help: compare with a tolerance if one is acceptable, 'ABS(x - 0.3) <= tolerance'
       |
       | Note 1: a REAL holds the nearest binary fraction, so 1.1 + 2.2 = 3.3 is FALSE
       |
       | Note 2: lint rule: float-equality
    ---'
    [L0124] Warning: exact comparison of REAL values
       ,-[ file:///test0.st:9:13 ]
       |
     9 |     test := y <> x;
       |             ^^^|^^
       |                `---- LREAL values compared with '<>'
       |
       | Help: compare with a tolerance if one is acceptable, 'ABS(y - x) > tolerance'
       |
       | Note 1: a REAL holds the nearest binary fraction, so 1.1 + 2.2 = 3.3 is FALSE
       |
       | Note 2: lint rule: float-equality
    ---'
    [L0124] Warning: exact comparison of REAL values
        ,-[ file:///test0.st:10:13 ]
        |
     10 |     test := x = n;
        |             ^^|^^
        |               `---- REAL values compared with '='
        |
        | Help: compare with a tolerance if one is acceptable, 'ABS(x - n) <= tolerance'
        |
        | Note 1: a REAL holds the nearest binary fraction, so 1.1 + 2.2 = 3.3 is FALSE
        |
        | Note 2: lint rule: float-equality
    ----'
    ");
}

/// An ordering is not an equality, zero is exact, `x <> x` is the NaN test,
/// and integers compare exactly.
#[rstest]
fn exact_comparisons_not_flagged(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR
    x : REAL;
    y : REAL;
    a : INT;
    b : DINT;
END_VAR
    test := x < y;
    test := x >= 1.5;
    test := x <> 0.0;
    test := 0 = x;
    test := x <> x;
    test := a = b;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "float-equality"), @r"");
}

/// The help rewrites the comparison from its own operands, a difference on
/// the right in parentheses.
#[rstest]
fn tolerance_keeps_the_operands(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR
    x : REAL;
    a : REAL;
    b : REAL;
END_VAR
    test := x = a - b;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "float-equality"), @r"
    [L0124] Warning: exact comparison of REAL values
       ,-[ file:///test0.st:8:13 ]
       |
     8 |     test := x = a - b;
       |             ^^^^|^^^^
       |                 `------ REAL values compared with '='
       |
       | Help: compare with a tolerance if one is acceptable, 'ABS(x - (a - b)) <= tolerance'
       |
       | Note 1: a REAL holds the nearest binary fraction, so 1.1 + 2.2 = 3.3 is FALSE
       |
       | Note 2: lint rule: float-equality
    ---'
    ");
}
