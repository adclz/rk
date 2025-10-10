use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn invalid_enum_type(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            List: BOOL (A, B, C);
        END_TYPE
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:3:13 ]
       |
     3 |             List: BOOL (A, B, C);
       |             ^^|^  
       |               `--- invalid enum type 'BOOL'
       | 
       | Note: only numeric integer types are allowed for ENUM
    ---'
    ");
}

#[rstest]
fn type_mismatch_enum_variant_decl(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            List: UINT (A, B := -5, C);
        END_TYPE
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
       ,-[ file:///test0.st:3:28 ]
       |
     3 |             List: UINT (A, B := -5, C);
       |             ^^|^  ^^|^     |  
       |               `--------------- 'List' is declared here
       |                     |      |  
       |                     `--------- type defined here
       |                            |  
       |                            `-- invalid value for enum variant 'B': literal can not be negative
    ---'
    ");
}

#[rstest]
fn unknown_enum_variant(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            List: UINT (A, B, C);
        END_TYPE

        FUNCTION fb1
            VAR
                test: List;
            END_VAR

            test := List#D; // D is not a valid enum variant

        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    Error: 
        ,-[ file:///test0.st:11:21 ]
        |
      3 |             List: UINT (A, B, C);
        |             ^^|^^^^^^^^^|^^^^^^^  
        |               `------------------- 'List' is declared here
        |                         |         
        |                         `--------- type defined here
        | 
     11 |             test := List#D; // D is not a valid enum variant
        |                     ^^^|^^  
        |                        `---- invalid assignment: ENUM 'List' has no variant named 'D'
    ----'
    ");
}
