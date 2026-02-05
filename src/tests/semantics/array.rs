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
    [E0210] Error: no namespace item found
       ,-[ file:///test0.st:4:25 ]
       |
     4 |                 input : something;
       |                         ^^^^|^^^^  
       |                             `------ no item found for path 'something'
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
    [E0601] Error: invalid array bounds
       ,-[ file:///test0.st:3:25 ]
       |
     3 |             List: ARRAY[-1..10] OF INT;
       |                         ^|  
       |                          `-- invalid lower bound value for ARRAY
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
    [E0602] Error: invalid array bounds
       ,-[ file:///test0.st:3:28 ]
       |
     3 |             List: ARRAY[0..-10] OF INT;
       |                            ^|^  
       |                             `--- invalid upper bound value for ARRAY
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
    [E0603] Error: invalid array bounds
       ,-[ file:///test0.st:3:29 ]
       |
     3 |             List: ARRAY[10..1] OF INT;
       |                             |  
       |                             `-- upper bound value must be greater than lower bound value
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
    [E0601] Error: invalid array bounds
       ,-[ file:///test0.st:3:31 ]
       |
     3 |             List: ARRAY[0..3, -2..1] OF INT;
       |                               ^|  
       |                                `-- invalid lower bound value for ARRAY
    ---'
    ");
}
