use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_lint_diagnostics, with_db};

#[rstest]
fn function_call_discards_return_value(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION add : INT
        VAR_INPUT
            a : INT;
            b : INT;
        END_VAR
            add := a + b;
        END_FUNCTION

        FUNCTION test : INT
        VAR
            x : INT;
        END_VAR
            add(a := 1, b := 2);
            test := x;
        END_FUNCTION
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn return_value_assigned(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION add : INT
        VAR_INPUT
            a : INT;
            b : INT;
        END_VAR
            add := a + b;
        END_FUNCTION

        FUNCTION test : INT
        VAR
            x : INT;
        END_VAR
            x := add(a := 1, b := 2);
            test := x;
        END_FUNCTION
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn no_return_type_no_warning(mut with_db: RootDatabase) {
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
fn function_block_call_no_warning(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Counter
        VAR
            count : INT;
        END_VAR
            count := count + 1;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR
            c : Counter;
        END_VAR
            c();
            test := 0;
        END_FUNCTION
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"");
}
