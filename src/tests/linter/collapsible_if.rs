use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn simple_collapsible(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    a : BOOL;
    b : BOOL;
END_VAR
    IF a THEN
        IF b THEN
            test := 1;
        END_IF;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "collapsible-if"), @r"
    [L0312] Hint: collapsible IF statements
        ,-[ file:///test0.st:7:5 ]
        |
      7 | ,->     IF a THEN
        | |          |
        | |          `-- ...can be merged here: IF 'a AND b' THEN
      8 | |           IF b THEN
        | |              |
        | |              `-- the condition of this IF statement...
        : :
     11 | |->     END_IF;
        | |
        | `----------------- IF statements can be collapsed into a single one
        |
        |     Note: lint rule: collapsible-if
    ----'
    ");
}

#[rstest]
fn outer_has_else(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    a : BOOL;
    b : BOOL;
END_VAR
    IF a THEN
        IF b THEN
            test := 1;
        END_IF;
    ELSE
        test := 0;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "collapsible-if"), @r"");
}

#[rstest]
fn inner_has_else(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    a : BOOL;
    b : BOOL;
END_VAR
    IF a THEN
        IF b THEN
            test := 1;
        ELSE
            test := 0;
        END_IF;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "collapsible-if"), @r"");
}

#[rstest]
fn multiple_statements_in_then(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    a : BOOL;
    b : BOOL;
    x : INT;
END_VAR
    IF a THEN
        x := 1;
        IF b THEN
            test := 1;
        END_IF;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "collapsible-if"), @r"");
}

#[rstest]
fn not_an_if_inside(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    a : BOOL;
END_VAR
    IF a THEN
        test := 1;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "collapsible-if"), @r"");
}
