use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_lint_diagnostics, with_db};

#[rstest]
fn case_with_else_no_warning(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR
            x : INT;
        END_VAR
            CASE x OF
                1: test := 1;
                2: test := 2;
            ELSE
                test := 0;
            END_CASE;
        END_FUNCTION
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn case_without_else_warning(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR
            x : INT;
        END_VAR
            CASE x OF
                1: test := 1;
                2: test := 2;
            END_CASE;
        END_FUNCTION
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [W0106] Warning: CASE without ELSE
       ,-[ file:///test0.st:6:13 ]
       |
     6 | ,->             CASE x OF
       : :   
     9 | |->             END_CASE;
       | |                           
       | `--------------------------- CASE statement has no ELSE branch
    ---'
    ");
}

#[rstest]
fn nested_case_inner_missing_else(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR
            x : INT;
            y : INT;
        END_VAR
            CASE x OF
                1:
                    CASE y OF
                        10: test := 10;
                    END_CASE;
                2: test := 2;
            ELSE
                test := 0;
            END_CASE;
        END_FUNCTION
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [W0106] Warning: CASE without ELSE
        ,-[ file:///test0.st:9:21 ]
        |
      9 | ,->                     CASE y OF
        : :   
     11 | |->                     END_CASE;
        | |                                   
        | `----------------------------------- CASE statement has no ELSE branch
    ----'
    ");
}

#[rstest]
fn case_in_program(mut with_db: RootDatabase) {
    let source = r#"
        PROGRAM prog1
        VAR
            x : INT;
            result : INT;
        END_VAR
            CASE x OF
                1: result := 1;
            END_CASE;
        END_PROGRAM
    "#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [W0106] Warning: CASE without ELSE
       ,-[ file:///test0.st:7:13 ]
       |
     7 | ,->             CASE x OF
       : :   
     9 | |->             END_CASE;
       | |                           
       | `--------------------------- CASE statement has no ELSE branch
    ---'
    ");
}
