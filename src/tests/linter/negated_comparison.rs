use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn not_eq(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR x : INT; y : INT; END_VAR
    test := NOT (x = y);
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "negated-comparison"), @r"
    [L0111] Info: negated comparison
       ,-[ file:///test0.st:4:13 ]
       |
     4 |     test := NOT (x = y);
       |             ^^^^^|^|^^^
       |                  `------- NOT with '=' can be simplified to '<>'
       |                    |
       |                    `----- replace 'NOT (x = y)' with 'x <> y'
       |
       | Note: lint rule: negated-comparison
    ---'
    ");
}

#[rstest]
fn not_ne(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR x : INT; y : INT; END_VAR
    test := NOT (x <> y);
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "negated-comparison"), @r"
    [L0111] Info: negated comparison
       ,-[ file:///test0.st:4:13 ]
       |
     4 |     test := NOT (x <> y);
       |             ^^^^^^|^|^^^
       |                   `------- NOT with '<>' can be simplified to '='
       |                     |
       |                     `----- replace 'NOT (x <> y)' with 'x = y'
       |
       | Note: lint rule: negated-comparison
    ---'
    ");
}

#[rstest]
fn not_lt(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR x : INT; y : INT; END_VAR
    test := NOT (x < y);
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "negated-comparison"), @r"
    [L0111] Info: negated comparison
       ,-[ file:///test0.st:4:13 ]
       |
     4 |     test := NOT (x < y);
       |             ^^^^^|^|^^^
       |                  `------- NOT with '<' can be simplified to '>='
       |                    |
       |                    `----- replace 'NOT (x < y)' with 'x >= y'
       |
       | Note: lint rule: negated-comparison
    ---'
    ");
}

#[rstest]
fn not_gt(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR x : INT; y : INT; END_VAR
    test := NOT (x > y);
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "negated-comparison"), @r"
    [L0111] Info: negated comparison
       ,-[ file:///test0.st:4:13 ]
       |
     4 |     test := NOT (x > y);
       |             ^^^^^|^|^^^
       |                  `------- NOT with '>' can be simplified to '<='
       |                    |
       |                    `----- replace 'NOT (x > y)' with 'x <= y'
       |
       | Note: lint rule: negated-comparison
    ---'
    ");
}

#[rstest]
fn not_le(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR x : INT; y : INT; END_VAR
    test := NOT (x <= y);
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "negated-comparison"), @r"
    [L0111] Info: negated comparison
       ,-[ file:///test0.st:4:13 ]
       |
     4 |     test := NOT (x <= y);
       |             ^^^^^^|^|^^^
       |                   `------- NOT with '<=' can be simplified to '>'
       |                     |
       |                     `----- replace 'NOT (x <= y)' with 'x > y'
       |
       | Note: lint rule: negated-comparison
    ---'
    ");
}

#[rstest]
fn not_ge(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : BOOL
VAR x : INT; y : INT; END_VAR
    test := NOT (x >= y);
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "negated-comparison"), @r"
    [L0111] Info: negated comparison
       ,-[ file:///test0.st:4:13 ]
       |
     4 |     test := NOT (x >= y);
       |             ^^^^^^|^|^^^
       |                   `------- NOT with '>=' can be simplified to '<'
       |                     |
       |                     `----- replace 'NOT (x >= y)' with 'x < y'
       |
       | Note: lint rule: negated-comparison
    ---'
    ");
}

#[rstest]
fn if_condition(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; y : INT; END_VAR
    IF NOT (x > y) THEN
        test := 1;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "negated-comparison"), @r"
    [L0111] Info: negated comparison
       ,-[ file:///test0.st:4:8 ]
       |
     4 |     IF NOT (x > y) THEN
       |        ^^^^^|^|^^^
       |             `------- NOT with '>' can be simplified to '<='
       |               |
       |               `----- replace 'NOT (x > y)' with 'x <= y'
       |
       | Note: lint rule: negated-comparison
    ---'
    ");
}

#[rstest]
fn for_condition(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR x : INT; y : INT; END_VAR
    FOR x := NOT (x > y) TO 1 BY 1 DO
        test := 1;
    END_FOR;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "negated-comparison"), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:4:14 ]
       |
     3 | VAR x : INT; y : INT; END_VAR
       |     |
       |     `-- type is declared by variable 'x' here
     4 |     FOR x := NOT (x > y) TO 1 BY 1 DO
       |              ^^^^^|^^^^^
       |                   `------- expected 'INT', got 'BOOL'
       |                   |
       |                   `------- consider explicitly casting with 'BOOL_TO_INT(NOT (x > y))'
       |
       | Help: insert explicit cast 'BOOL_TO_INT(NOT (x > y))'
    ---'
    [L0111] Info: negated comparison
       ,-[ file:///test0.st:4:14 ]
       |
     4 |     FOR x := NOT (x > y) TO 1 BY 1 DO
       |              ^^^^^|^|^^^
       |                   `------- NOT with '>' can be simplified to '<='
       |                     |
       |                     `----- replace 'NOT (x > y)' with 'x <= y'
       |
       | Note: lint rule: negated-comparison
    ---'
    ");
}
