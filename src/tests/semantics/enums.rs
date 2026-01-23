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
    [E0701] Error: invalid enum type
       ,-[ file:///test0.st:3:19 ]
       |
     3 |             List: BOOL (A, B, C);
       |                   ^^|^  
       |                     `--- I=invalid enum type 'BOOL'
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
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:3:33 ]
       |
     3 |             List: UINT (A, B := -5, C);
       |                                 ^|  
       |                                  `-- cannot infer '<integer>' to 'UINT': literal can not be negative
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
    [E0703] Error: invalid enum access
        ,-[ file:///test0.st:11:26 ]
        |
     11 |             test := List#D; // D is not a valid enum variant
        |                          |  
        |                          `-- ENUM has no variant named 'D'
    ----'
    ");
}
