// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

/// Each bare operand of another boolean operator: AND under OR, AND under
/// XOR, XOR under OR, `&` as AND.
#[rstest]
fn an_operand_of_another_operator(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Mixed : BOOL
VAR_INPUT a, b, c, d : BOOL; END_VAR
    Mixed := a AND b OR c AND d;
    Mixed := a OR b AND c;
    Mixed := a AND b XOR c;
    Mixed := a XOR b OR c;
    Mixed := a & b OR c;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "mixed-boolean-operators"), @r"
    [L0314] Hint: boolean operators mixed without parentheses
       ,-[ file:///test0.st:4:14 ]
       |
     4 |     Mixed := a AND b OR c AND d;
       |              ^^^|^^^
       |                 `----- 'a AND b' is an operand of OR without parentheses
       |
       | Help: write '(a AND b)' to show the order
       |
       | Note 1: AND binds tighter than XOR, and XOR tighter than OR
       |
       | Note 2: lint rule: mixed-boolean-operators
    ---'
    [L0314] Hint: boolean operators mixed without parentheses
       ,-[ file:///test0.st:4:25 ]
       |
     4 |     Mixed := a AND b OR c AND d;
       |                         ^^^|^^^
       |                            `----- 'c AND d' is an operand of OR without parentheses
       |
       | Help: write '(c AND d)' to show the order
       |
       | Note 1: AND binds tighter than XOR, and XOR tighter than OR
       |
       | Note 2: lint rule: mixed-boolean-operators
    ---'
    [L0314] Hint: boolean operators mixed without parentheses
       ,-[ file:///test0.st:5:19 ]
       |
     5 |     Mixed := a OR b AND c;
       |                   ^^^|^^^
       |                      `----- 'b AND c' is an operand of OR without parentheses
       |
       | Help: write '(b AND c)' to show the order
       |
       | Note 1: AND binds tighter than XOR, and XOR tighter than OR
       |
       | Note 2: lint rule: mixed-boolean-operators
    ---'
    [L0314] Hint: boolean operators mixed without parentheses
       ,-[ file:///test0.st:6:14 ]
       |
     6 |     Mixed := a AND b XOR c;
       |              ^^^|^^^
       |                 `----- 'a AND b' is an operand of XOR without parentheses
       |
       | Help: write '(a AND b)' to show the order
       |
       | Note 1: AND binds tighter than XOR, and XOR tighter than OR
       |
       | Note 2: lint rule: mixed-boolean-operators
    ---'
    [L0314] Hint: boolean operators mixed without parentheses
       ,-[ file:///test0.st:7:14 ]
       |
     7 |     Mixed := a XOR b OR c;
       |              ^^^|^^^
       |                 `----- 'a XOR b' is an operand of OR without parentheses
       |
       | Help: write '(a XOR b)' to show the order
       |
       | Note 1: AND binds tighter than XOR, and XOR tighter than OR
       |
       | Note 2: lint rule: mixed-boolean-operators
    ---'
    [L0314] Hint: boolean operators mixed without parentheses
       ,-[ file:///test0.st:8:14 ]
       |
     8 |     Mixed := a & b OR c;
       |              ^^|^^
       |                `---- 'a & b' is an operand of OR without parentheses
       |
       | Help: write '(a & b)' to show the order
       |
       | Note 1: AND binds tighter than XOR, and XOR tighter than OR
       |
       | Note 2: lint rule: mixed-boolean-operators
    ---'
    ");
}

/// One operator repeated, parentheses written, and comparisons under AND,
/// which bind tighter and read as they are.
#[rstest]
fn one_operator_or_parentheses(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Clear : BOOL
VAR_INPUT a, b, c : BOOL; x, y : INT; END_VAR
    Clear := a OR b OR c;
    Clear := (a AND b) OR c;
    Clear := a AND (b OR c);
    Clear := x = 1 AND y = 2;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "mixed-boolean-operators"), @r"");
}
