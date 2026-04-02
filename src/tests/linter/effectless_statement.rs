use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_lint_diagnostics, with_db};

#[rstest]
fn bare_variable_reference(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR
            x : INT;
        END_VAR
            x;
            test := 0;
        END_FUNCTION
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0105] Warning: effectless statement
       ,-[ file:///test0.st:6:13 ]
       |
     6 |             x;
       |             |
       |             `-- statement has no effect
    ---'
    ");
}

#[rstest]
fn bare_literal(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR
            x : INT;
        END_VAR
            42;
            test := x;
        END_FUNCTION
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [E0050] Error: syntax
       ,-[ file:///test0.st:6:13 ]
       |
     6 |             42;
       |             ^|
       |              `-- Unexpected token(s): '42'
    ---'
    ");
}

#[rstest]
fn assignment_no_warning(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR
            x : INT;
        END_VAR
            x := 42;
            test := x;
        END_FUNCTION
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn function_call_no_warning(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK do_something
        VAR_INPUT
            a : INT;
        END_VAR
        VAR
            local : INT;
        END_VAR
            local := a + 1;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR
            fb : do_something;
        END_VAR
            fb(a := 1);
            test := 0;
        END_FUNCTION
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn effectless_in_program(mut with_db: RootDatabase) {
    let source = r#"
        PROGRAM prog1
        VAR
            x : INT;
        END_VAR
            x;
        END_PROGRAM
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0105] Warning: effectless statement
       ,-[ file:///test0.st:6:13 ]
       |
     6 |             x;
       |             |
       |             `-- statement has no effect
    ---'
    ");
}

#[rstest]
fn effectless_in_function_block(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK fb1
        VAR
            x : INT;
        END_VAR
            x;
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [L0105] Warning: effectless statement
       ,-[ file:///test0.st:6:13 ]
       |
     6 |             x;
       |             |
       |             `-- statement has no effect
    ---'
    ");
}
