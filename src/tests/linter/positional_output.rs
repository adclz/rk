// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! L0314: a positional argument in the place of a VAR_OUTPUT receives the
//! output, which nothing at the call site says.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

/// A FUNCTION's output declared first, a block's and a METHOD's, in a
/// statement and inside an expression. The fix names the output.
#[rstest]
fn a_positional_argument_receiving_an_output_is_reported(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION first : INT
VAR_OUTPUT o : INT; END_VAR
VAR_INPUT a : INT; END_VAR
    o := a * 10;
    first := 0;
END_FUNCTION

FUNCTION_BLOCK B
VAR_INPUT i : INT; END_VAR
VAR_OUTPUT q : INT; END_VAR
    METHOD m : INT
    VAR_INPUT a : INT; END_VAR
    VAR_OUTPUT o : INT; END_VAR
        o := a;
        m := a;
    END_METHOD
    q := i;
END_FUNCTION_BLOCK

PROGRAM Main
VAR y, z, w, r : INT; b : B; END_VAR
    r := first(y, 4) + 1;
    b(3, z);
    r := b.m(2, w);
END_PROGRAM
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "positional-output"), @r"
    [L0314] Hint: positional output
        ,-[ file:///test0.st:23:16 ]
        |
      3 | VAR_OUTPUT o : INT; END_VAR
        |            |
        |            `-- 'o' is declared here
        |
     23 |     r := first(y, 4) + 1;
        |                |
        |                `-- 'y' receives the output 'o' of 'first'
        |
        | Help: write o => y
        |
        | Note 1: a positional list gives every parameter in declaration order, outputs included
        |
        | Note 2: lint rule: positional-output
    ----'
    [L0314] Hint: positional output
        ,-[ file:///test0.st:24:10 ]
        |
     11 | VAR_OUTPUT q : INT; END_VAR
        |            |
        |            `-- 'q' is declared here
        |
     24 |     b(3, z);
        |          |
        |          `-- 'z' receives the output 'q' of 'b'
        |
        | Help: write q => z
        |
        | Note 1: a positional list gives every parameter in declaration order, outputs included
        |
        | Note 2: lint rule: positional-output
    ----'
    [L0314] Hint: positional output
        ,-[ file:///test0.st:25:17 ]
        |
     14 |     VAR_OUTPUT o : INT; END_VAR
        |                |
        |                `-- 'o' is declared here
        |
     25 |     r := b.m(2, w);
        |                 |
        |                 `-- 'w' receives the output 'o' of 'b.m'
        |
        | Help: write o => w
        |
        | Note 1: a positional list gives every parameter in declaration order, outputs included
        |
        | Note 2: lint rule: positional-output
    ----'
    ");
}

/// Bound by name, or an input in its place: nothing to report.
#[rstest]
fn named_outputs_and_positional_inputs_are_not_reported(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION g : INT
VAR_INPUT a, b : INT; END_VAR
VAR_OUTPUT o : INT; END_VAR
    o := a + b;
    g := a;
END_FUNCTION

PROGRAM Main
VAR y, r : INT; END_VAR
    r := g(1, 2);
    r := g(1, 2, o => y);
END_PROGRAM
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "positional-output"), @r"");
}
