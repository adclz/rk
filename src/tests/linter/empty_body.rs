use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn empty_function(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : INT
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "empty-body"), @r"
    [L0125] Advice: empty body
       ,-[ file:///test0.st:2:1 ]
       |
     2 | ,-> FUNCTION fn1 : INT
     3 | |-> END_FUNCTION
       | |
       | `------------------ FUNCTION 'fn1' has an empty body
       | |
       | |   Note: lint rule: empty-body
    ---'
    ");
}

#[rstest]
fn empty_function_block(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "empty-body"), @r"
    [L0125] Advice: empty body
       ,-[ file:///test0.st:2:1 ]
       |
     2 | ,-> FUNCTION_BLOCK fb1
     3 | |-> END_FUNCTION_BLOCK
       | |
       | `------------------------ FUNCTION_BLOCK 'fb1' has an empty body
       | |
       | |   Note: lint rule: empty-body
    ---'
    ");
}

#[rstest]
fn empty_method(mut with_db: RootDatabase) {
    let source = r#"
CLASS c1
    METHOD m1 : INT
    END_METHOD
END_CLASS
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "empty-body"), @r"
    [L0125] Advice: empty body
       ,-[ file:///test0.st:3:5 ]
       |
     3 | ,->     METHOD m1 : INT
     4 | |->     END_METHOD
       | |
       | `-------------------- METHOD 'm1' has an empty body
       |
       |     Note: lint rule: empty-body
    ---'
    ");
}

#[rstest]
fn non_empty_function(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : INT
    fn1 := 42;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "empty-body"), @r"");
}

#[rstest]
fn extern_function_no_warning(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : INT
VAR_INPUT _x : INT; END_VAR
    {extern 'mod' 'fn' (params _x) (result fn1)}
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "empty-body"), @"");
}

#[rstest]
fn function_with_vars_only(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : INT
VAR
    _x : INT;
END_VAR
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "empty-body"), @r"
    [L0125] Advice: empty body
       ,-[ file:///test0.st:2:1 ]
       |
     2 | ,-> FUNCTION fn1 : INT
       : :
     6 | |-> END_FUNCTION
       | |
       | `------------------ FUNCTION 'fn1' has an empty body
       | |
       | |   Note: lint rule: empty-body
    ---'
    ");
}
