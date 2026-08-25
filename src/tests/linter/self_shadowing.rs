use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn fb_variable_same_name(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFB
VAR
    MyFB : INT;
END_VAR
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "self-shadowing"), @r"
    [L0315] Warning: variable shadows its own POU
       ,-[ file:///test0.st:4:5 ]
       |
     2 | FUNCTION_BLOCK MyFB
       |                ^^|^
       |                  `--- FUNCTION_BLOCK 'MyFB' is declared here
       |
     4 |     MyFB : INT;
       |     ^^|^
       |       `--- variable 'MyFB' has the same name as its declaring FUNCTION_BLOCK
       |
       | Note: lint rule: self-shadowing
    ---'
    ");
}

#[rstest]
fn method_variable_same_name(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFB
    METHOD doWork : INT
    VAR
        doWork : INT;
    END_VAR
        doWork := 1;
    END_METHOD
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "self-shadowing"), @r"
    [E0117] Error: duplicate definitions
       ,-[ file:///test0.st:5:9 ]
       |
     5 |         doWork : INT;
       |         ^^^|^^
       |            `---- variable 'doWork' is the METHOD's return value
    ---'
    [L0315] Warning: variable shadows its own POU
       ,-[ file:///test0.st:5:9 ]
       |
     3 |     METHOD doWork : INT
       |            ^^^|^^
       |               `---- METHOD 'doWork' is declared here
       |
     5 |         doWork : INT;
       |         ^^^|^^
       |            `---- variable 'doWork' has the same name as its declaring METHOD
       |
       | Note: lint rule: self-shadowing
    ---'
    ");
}

#[rstest]
fn function_return_not_flagged(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION myFunc : INT
    myFunc := 42;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "self-shadowing"), @r"");
}

#[rstest]
fn no_shadowing(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFB
VAR
    x : INT;
END_VAR
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "self-shadowing"), @r"");
}

#[rstest]
fn program_variable_same_name(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM Main
VAR
    Main : INT;
END_VAR
END_PROGRAM
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "self-shadowing"), @r"
    [L0315] Warning: variable shadows its own POU
       ,-[ file:///test0.st:4:5 ]
       |
     2 | PROGRAM Main
       |         ^^|^
       |           `--- PROGRAM 'Main' is declared here
       |
     4 |     Main : INT;
       |     ^^|^
       |       `--- variable 'Main' has the same name as its declaring PROGRAM
       |
       | Note: lint rule: self-shadowing
    ---'
    ");
}
