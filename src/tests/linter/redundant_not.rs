use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn not_not_variable(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR
    x : BOOL;
END_VAR
    test := NOT NOT x;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "redundant-not"), @r"
    [L0108] Info: redundant NOT
       ,-[ file:///test0.st:6:13 ]
       |
     6 |     test := NOT NOT x;
       |             ^^^^|^^^^
       |                 `------ double negation: 'NOT NOT x' can be simplified to 'x'
       |
       | Note: lint rule: redundant-not
    ---'
    ");
}

#[rstest]
fn not_not_in_condition(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    flag : BOOL;
END_VAR
    IF NOT NOT flag THEN
        test := 1;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "redundant-not"), @r"
    [L0108] Info: redundant NOT
       ,-[ file:///test0.st:6:8 ]
       |
     6 |     IF NOT NOT flag THEN
       |        ^^^^^^|^^^^^
       |              `------- double negation: 'NOT NOT x' can be simplified to 'x'
       |
       | Note: lint rule: redundant-not
    ---'
    ");
}

#[rstest]
fn single_not_no_warning(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR
    x : BOOL;
END_VAR
    test := NOT x;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "redundant-not"), @r"");
}

#[rstest]
fn not_not_nested_in_expression(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR
    x : BOOL;
    y : BOOL;
END_VAR
    test := y AND NOT NOT x;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "redundant-not"), @r"
    [L0108] Info: redundant NOT
       ,-[ file:///test0.st:7:19 ]
       |
     7 |     test := y AND NOT NOT x;
       |                   ^^^^|^^^^
       |                       `------ double negation: 'NOT NOT x' can be simplified to 'x'
       |
       | Note: lint rule: redundant-not
    ---'
    ");
}
