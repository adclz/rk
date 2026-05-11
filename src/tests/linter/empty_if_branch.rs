use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn empty_then(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : BOOL;
END_VAR
    IF x THEN
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "empty-if-branch"), @r"
    [L0211] Hint: empty IF branch
       ,-[ file:///test0.st:6:8 ]
       |
     6 |     IF x THEN
       |        |
       |        `-- IF branch has no statements
       |
       | Note: lint rule: empty-if-branch
    ---'
    ");
}

#[rstest]
fn empty_elsif(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    IF x = 1 THEN
        test := 1;
    ELSIF x = 2 THEN
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "empty-if-branch"), @r"
    [L0211] Hint: empty IF branch
       ,-[ file:///test0.st:8:11 ]
       |
     8 |     ELSIF x = 2 THEN
       |           ^^|^^
       |             `---- ELSIF branch has no statements
       |
       | Note: lint rule: empty-if-branch
    ---'
    ");
}

#[rstest]
fn non_empty_branches(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : BOOL;
END_VAR
    IF x THEN
        test := 1;
    ELSE
        test := 0;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "empty-if-branch"), @r"");
}
