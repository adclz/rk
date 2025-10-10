use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

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

#[rstest]
fn invalid_subrange_value_type(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Range: UINT (0..5);
        END_TYPE

        FUNCTION fb1
            VAR
                test: Range;
            END_VAR

            test :=  -1 // -1 should not be allowed here (UINT)

        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
        ,-[ file:///test0.st:11:22 ]
        |
      3 |             Range: UINT (0..5);
        |             ^^|^^  ^^|^  
        |               `---------- 'Range' is declared here
        |                      |   
        |                      `--- type defined here
        | 
     11 |             test :=  -1 // -1 should not be allowed here (UINT)
        |                      ^|  
        |                       `-- invalid assignment: literal can not be negative
    ----'
    ");
}

#[rstest]
fn out_fo_bounds_subrange_value(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Range: UINT (0..5);
        END_TYPE

        FUNCTION fb1
            VAR
                test: Range;
            END_VAR

            test :=  6 // 6 should not be allowed here (UINT (0..5))

        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
        ,-[ file:///test0.st:11:22 ]
        |
      3 |             Range: UINT (0..5);
        |             ^^|^^^^^^^^|^^^^^^  
        |               `----------------- 'Range' is declared here
        |                        |        
        |                        `-------- type defined here
        | 
     11 |             test :=  6 // 6 should not be allowed here (UINT (0..5))
        |                      |  
        |                      `-- invalid assignment: value 6 is out of bounds for SUBRANGE Range (expected between 0 and 5)
    ----'
    ");
}
