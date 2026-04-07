use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn empty_branch(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; END_VAR
    CASE x OF
        1:
        2: test := 20;
    END_CASE;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "empty-case-branch"), @r"
    [L0130] Hint: empty CASE branch
       ,-[ file:///test0.st:5:9 ]
       |
     5 |         1:
       |         |
       |         `-- CASE branch has no statements
       |
       | Note: lint rule: empty-case-branch
    ---'
    ");
}

#[rstest]
fn all_branches_non_empty_no_warning(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; END_VAR
    CASE x OF
        1: test := 10;
        2: test := 20;
    END_CASE;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "empty-case-branch"), @r"");
}

#[rstest]
fn multiple_empty_branches(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; END_VAR
    CASE x OF
        1:
        2:
        3: test := 30;
    END_CASE;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "empty-case-branch"), @r"
    [L0130] Hint: empty CASE branch
       ,-[ file:///test0.st:5:9 ]
       |
     5 |         1:
       |         |
       |         `-- CASE branch has no statements
       |
       | Note: lint rule: empty-case-branch
    ---'
    [L0130] Hint: empty CASE branch
       ,-[ file:///test0.st:6:9 ]
       |
     6 |         2:
       |         |
       |         `-- CASE branch has no statements
       |
       | Note: lint rule: empty-case-branch
    ---'
    ");
}
