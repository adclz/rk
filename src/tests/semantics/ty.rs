use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn unknown_type(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK fb1
            VAR
                input : something;
            END_VAR

        END_FUNCTION_BLOCK
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:4:25 ]
       |
     4 |                 input : something;
       |                         ^^^^|^^^^  
       |                             `------ unknown item 'something'
    ---'
    ");
}

#[rstest]
fn invalid_lower_bound_in_array(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            List: ARRAY[-1..10] OF INT;
        END_TYPE
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:3:25 ]
       |
     3 |             List: ARRAY[-1..10] OF INT;
       |                         ^|  
       |                          `-- Invalid lower bound value for ARRAY
    ---'
    ");
}

#[rstest]
fn invalid_upper_bound_in_array(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            List: ARRAY[0..-10] OF INT;
        END_TYPE
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:3:28 ]
       |
     3 |             List: ARRAY[0..-10] OF INT;
       |                            ^|^  
       |                             `--- Invalid upper bound value for ARRAY
    ---'
    ");
}

#[rstest]
fn inferior_upper_bound_in_array(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            List: ARRAY[10..1] OF INT;
        END_TYPE
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:3:29 ]
       |
     3 |             List: ARRAY[10..1] OF INT;
       |                             |  
       |                             `-- Upper bound value must be greater than lower bound value
    ---'
    ");
}

#[rstest]
fn nested_array_invalid_bound(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            List: ARRAY[0..3, -2..1] OF INT;
        END_TYPE
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:3:31 ]
       |
     3 |             List: ARRAY[0..3, -2..1] OF INT;
       |                               ^|  
       |                                `-- Invalid lower bound value for ARRAY
    ---'
    ");
}



#[rstest]
fn invalid_subrange_type(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Range: BOOL (0..5);
        END_TYPE
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:3:13 ]
       |
     3 |             Range: BOOL (0..5);
       |             ^^|^^  
       |               `---- invalid subrange type 'BOOL'
       | 
       | Note: only numeric integer types are allowed for SUBRANGE
    ---'
    ");
}

#[rstest]
fn invalid_start_value(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Range: UINT (-10..0);
        END_TYPE
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:3:26 ]
       |
     3 |             Range: UINT (-10..0);
       |             ^^|^^  ^^|^  ^|^  
       |               `--------------- 'Range' is declared here
       |                      |    |   
       |                      `-------- type defined here
       |                           |   
       |                           `--- invalid start value for subrange: literal can not be negative
    ---'
    ");
}

#[rstest]
fn invalid_end_value(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Range: UINT (0..-5);
        END_TYPE
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:3:29 ]
       |
     3 |             Range: UINT (0..-5);
       |             ^^|^^  ^^|^     ^|  
       |               `----------------- 'Range' is declared here
       |                      |       |  
       |                      `---------- type defined here
       |                              |  
       |                              `-- invalid end value for subrange: literal can not be negative
    ---'
    ");
}
