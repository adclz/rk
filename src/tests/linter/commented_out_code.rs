// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

/// Statements in a line comment, in two consecutive ones read as one, and
/// in a block comment.
#[rstest]
fn code_in_a_comment(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM Line
VAR count : INT; END_VAR
    count := count + 1;
    // count := 0;
    // Reset();

    (* IF count > 10 THEN
        count := 0;
    END_IF; *)
END_PROGRAM
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "commented-out-code"), @r"
    [L0318] Hint: code left in a comment
       ,-[ file:///test0.st:5:5 ]
       |
     5 | ,->     // count := 0;
     6 | |->     // Reset();
       | |
       | `--------------------- this comment contains code
       |
       |     Help: delete the commented-out code
       |
       |     Note 1: code in a comment is easily put back by mistake
       |
       |     Note 2: lint rule: commented-out-code
    ---'
    [L0318] Hint: code left in a comment
        ,-[ file:///test0.st:8:5 ]
        |
      8 | ,->     (* IF count > 10 THEN
        : :
     10 | |->     END_IF; *)
        | |
        | `-------------------- this comment contains code
        |
        |     Help: delete the commented-out code
        |
        |     Note 1: code in a comment is easily put back by mistake
        |
        |     Note 2: lint rule: commented-out-code
    ----'
    ");
}

/// Prose is no code, nor is a name or an expression without a statement.
#[rstest]
fn prose_is_not_code(mut with_db: RootDatabase) {
    let source = r#"
// The line counts its parts; it resets at 10.
PROGRAM Line
VAR count : INT; END_VAR
    // count goes up by one
    count := count + 1; (* see count *)
    // TODO
END_PROGRAM
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "commented-out-code"), @r"");
}
