use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn if_not_with_else(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    flag : BOOL;
END_VAR
    IF NOT flag THEN
        test := 0;
    ELSE
        test := 1;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "negated-condition"), @r"
    [L0115] Info: negated condition
       ,-[ file:///test0.st:6:8 ]
       |
     6 |     IF NOT flag THEN
       |        ^^^^|^^^
       |            `----- remove NOT and swap the THEN and ELSE bodies
     7 |         test := 0;
       |         ^^^^|^^^^
       |             `------ swap this
       |
     9 |         test := 1;
       |         ^^^^|^^^^
       |             `------ with this
       |
       | Note: lint rule: negated-condition
    ---'
    ");
}

#[rstest]
fn no_warning_without_else(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    flag : BOOL;
END_VAR
    IF NOT flag THEN
        test := 0;
    END_IF;
    test := 1;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "negated-condition"), @r"");
}

#[rstest]
fn no_warning_positive_condition(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    flag : BOOL;
END_VAR
    IF flag THEN
        test := 1;
    ELSE
        test := 0;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "negated-condition"), @r"");
}

#[rstest]
fn no_warning_with_elsif(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    a : BOOL;
    b : BOOL;
END_VAR
    IF NOT a THEN
        test := 0;
    ELSIF b THEN
        test := 1;
    ELSE
        test := 2;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "negated-condition"), @r"");
}

#[rstest]
fn no_warning_comparison(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    IF x <> 0 THEN
        test := 1;
    ELSE
        test := 0;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "negated-condition"), @r"");
}
