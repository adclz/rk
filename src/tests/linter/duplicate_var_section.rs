use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn duplicate_var_in_function(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR
            x : INT;
        END_VAR
        VAR
            y : INT;
        END_VAR
            fn1 := x + y;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "duplicate-var-section"), @r"
    [L0103] Info: duplicate variable section
       ,-[ file:///test0.st:6:9 ]
       |
     6 | ,->         VAR
       : :
     8 | |->         END_VAR
       | |
       | `--------------------- duplicate VAR section
       |
       |     Note 1: merge this section with the existing one above
       |
       |     Note 2: lint rule: duplicate-var-section
    ---'
    ");
}

#[rstest]
fn duplicate_var_input_in_function(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR_INPUT
            x : INT;
        END_VAR
        VAR_INPUT
            y : INT;
        END_VAR
            fn1 := x + y;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "duplicate-var-section"), @r"
    [L0103] Info: duplicate variable section
       ,-[ file:///test0.st:6:9 ]
       |
     6 | ,->         VAR_INPUT
       : :
     8 | |->         END_VAR
       | |
       | `--------------------- duplicate VAR_INPUT section
       |
       |     Note 1: merge this section with the existing one above
       |
       |     Note 2: lint rule: duplicate-var-section
    ---'
    ");
}

#[rstest]
fn duplicate_var_in_function_block(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK fb1
        VAR
            x : INT;
        END_VAR
        VAR
            y : INT;
        END_VAR
            x := y;
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "duplicate-var-section"), @r"
    [L0103] Info: duplicate variable section
       ,-[ file:///test0.st:6:9 ]
       |
     6 | ,->         VAR
       : :
     8 | |->         END_VAR
       | |
       | `--------------------- duplicate VAR section
       |
       |     Note 1: merge this section with the existing one above
       |
       |     Note 2: lint rule: duplicate-var-section
    ---'
    ");
}

#[rstest]
fn duplicate_var_input_in_function_block(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK fb1
        VAR_INPUT
            x : INT;
        END_VAR
        VAR_INPUT
            y : INT;
        END_VAR
            x := y;
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "duplicate-var-section"), @r"
    [L0103] Info: duplicate variable section
       ,-[ file:///test0.st:6:9 ]
       |
     6 | ,->         VAR_INPUT
       : :
     8 | |->         END_VAR
       | |
       | `--------------------- duplicate VAR_INPUT section
       |
       |     Note 1: merge this section with the existing one above
       |
       |     Note 2: lint rule: duplicate-var-section
    ---'
    ");
}

#[rstest]
fn duplicate_var_in_program(mut with_db: RootDatabase) {
    let source = r#"
        PROGRAM prog1
        VAR
            x : INT;
        END_VAR
        VAR
            y : INT;
        END_VAR
            x := y;
        END_PROGRAM
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "duplicate-var-section"), @r"
    [L0103] Info: duplicate variable section
       ,-[ file:///test0.st:6:9 ]
       |
     6 | ,->         VAR
       : :
     8 | |->         END_VAR
       | |
       | `--------------------- duplicate VAR section
       |
       |     Note 1: merge this section with the existing one above
       |
       |     Note 2: lint rule: duplicate-var-section
    ---'
    ");
}

#[rstest]
fn duplicate_var_in_method(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK fb1
        METHOD m1 : INT
        VAR
            x : INT;
        END_VAR
        VAR
            y : INT;
        END_VAR
            m1 := x + y;
        END_METHOD
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "duplicate-var-section"), @r"
    [L0103] Info: duplicate variable section
       ,-[ file:///test0.st:7:9 ]
       |
     7 | ,->         VAR
       : :
     9 | |->         END_VAR
       | |
       | `--------------------- duplicate VAR section
       |
       |     Note 1: merge this section with the existing one above
       |
       |     Note 2: lint rule: duplicate-var-section
    ---'
    ");
}

#[rstest]
fn different_var_sections_no_warning(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR_INPUT
            x : INT;
        END_VAR
        VAR_OUTPUT
            y : INT;
        END_VAR
        VAR
            z : INT;
        END_VAR
            z := x;
            y := z;
            fn1 := z;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "duplicate-var-section"), @r"");
}

#[rstest]
fn single_var_section_no_warning(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR
            x : INT;
        END_VAR
            fn1 := x;
        END_FUNCTION
    "#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "duplicate-var-section"), @r"");
}
