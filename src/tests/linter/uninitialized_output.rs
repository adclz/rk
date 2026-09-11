use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn output_never_assigned(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR_OUTPUT
    result : INT;
END_VAR
    test := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "uninitialized-output"), @r"
    [L0206] Info: uninitialized output
       ,-[ file:///test0.st:4:5 ]
       |
     4 |     result : INT;
       |     ^^^|^^
       |        `---- 1 VAR_OUTPUT is never assigned in the body: 'result'
       |        |
       |        `---- 'result' declared here
       |
       | Note: lint rule: uninitialized-output
    ---'
    ");
}

#[rstest]
fn output_assigned(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR_OUTPUT
    result : INT;
END_VAR
    result := 42;
    test := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "uninitialized-output"), @r"");
}

#[rstest]
fn output_assigned_in_if(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
VAR_OUTPUT
    result : INT;
END_VAR
    IF x > 0 THEN
        result := 1;
    END_IF;
    test := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "uninitialized-output"), @r"");
}

#[rstest]
fn output_with_initializer_no_warning(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR_OUTPUT
    result : INT := 0;
END_VAR
    test := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "uninitialized-output"), @r"");
}

#[rstest]
fn fb_output_never_assigned(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFb
VAR_OUTPUT
    done : BOOL;
    value : INT;
END_VAR
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "uninitialized-output"), @r"
    [L0206] Info: uninitialized output
       ,-[ file:///test0.st:4:5 ]
       |
     4 |     done : BOOL;
       |     ^^|^
       |       `--- 2 VAR_OUTPUT are never assigned in the body: 'done', 'value'
       |       |
       |       `--- 'done' declared here
     5 |     value : INT;
       |     ^^|^^
       |       `---- 'value' declared here
       |
       | Note: lint rule: uninitialized-output
    ---'
    ");
}
