use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
#[case("ANY")]
#[case("ANY_INT")]
#[case("ANY_UNSIGNED")]
#[case("ANY_SIGNED")]
#[case("ANY_REAL")]
#[case("ANY_BIT")]
#[case("ANY_STRING")]
#[case("ANY_DATE")]
#[case("ANY_DURATION")]
fn valid_generic_type_cases(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(r#"
FUNCTION fn<GT: {value}, GY: {value}>

    VAR
        x: GT;
        y: GY;
    END_VAR

END_FUNCTION"#);

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("ANY_INT")]
#[case("ANY_UNSIGNED")]
#[case("ANY_SIGNED")]
#[case("ANY_REAL")]
#[case("ANY_BIT")]
#[case("ANY_STRING")]
#[case("ANY_DATE")]
#[case("ANY_DURATION")]
fn valid_generic_constraint_with_generic_cases(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(r#"
FUNCTION fn<GT: ANY + INTO<{value}>>

    VAR
        x: GT;
    END_VAR

END_FUNCTION"#);

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
#[case("BOOL")]
#[case("BYTE")]
#[case("WORD")]
#[case("DWORD")]
#[case("LWORD")]
#[case("SINT")]
#[case("INT")]
#[case("DINT")]
#[case("LINT")]
#[case("USINT")]
#[case("UINT")]
#[case("UDINT")]
#[case("ULINT")]
// todo: add more cases
fn valid_generic_constraint_with_elementary_cases(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(r#"
FUNCTION fn<GT: ANY + INTO<{value}>>

    VAR
        x: GT;
    END_VAR

END_FUNCTION"#);

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
fn invalid_generic_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn<GT: INT>

END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0310] Error: type mismatch
       ,-[ file:///test0.st:2:17 ]
       |
     2 | FUNCTION fn<GT: INT>
       |                 ^|^  
       |                  `--- generic 'GT' has invalid type 'INT'
    ---'
    ");
}

#[rstest]
fn invalid_generic_constraint(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn<GT : ANY_INT + INTO<ARRAY>>

END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0312] Error: type mismatch
       ,-[ file:///test0.st:2:33 ]
       |
     2 | FUNCTION fn<GT : ANY_INT + INTO<ARRAY>>
       |                                 ^^|^^  
       |                                   `---- generic 'GT' has invalid constraint 'ARRAY'
    ---'
    ");
}
