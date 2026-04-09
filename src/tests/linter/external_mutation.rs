use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn fb_field_mutation(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFB
VAR
    x : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION caller : INT
VAR
    fb : MyFB;
END_VAR
    fb.x := 42;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "external-mutation"), @r"
    [L0317] Warning: external instance mutation
        ,-[ file:///test0.st:12:5 ]
        |
     12 |     fb.x := 42;
        |     ^^|^
        |       `--- direct mutation of 'fb.x' - instances should own their data
        |
        | Note: lint rule: external-mutation
    ----'
    ");
}

#[rstest]
fn class_field_mutation(mut with_db: RootDatabase) {
    let source = r#"
CLASS MyClass
VAR PUBLIC
    x : INT;
END_VAR
END_CLASS

FUNCTION caller : INT
VAR
    obj : MyClass;
END_VAR
    obj.x := 42;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "external-mutation"), @r"
    [L0317] Warning: external instance mutation
        ,-[ file:///test0.st:12:5 ]
        |
     12 |     obj.x := 42;
        |     ^^|^^
        |       `---- direct mutation of 'obj.x' - instances should own their data
        |
        | Note: lint rule: external-mutation
    ----'
    ");
}

#[rstest]
fn no_mutation_simple_var(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    x := 42;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "external-mutation"), @r"");
}

#[rstest]
fn fb_call_not_flagged(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFB
VAR_INPUT
    x : INT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION caller : INT
VAR
    fb : MyFB;
END_VAR
    fb(x := 42);
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "external-mutation"), @r"");
}

#[rstest]
fn this_field_not_flagged(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK Parent
VAR
    child : Child;
END_VAR
    METHOD doWork : INT
        THIS.child.x := 42;
    END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Child
VAR
    x : INT;
END_VAR
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "external-mutation"), @r"");
}
