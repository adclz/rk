use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
#[case("ANY")]
#[case("ANY_MAGNITUDE")]
#[case("ANY_NUM")]
#[case("ANY_INT")]
#[case("ANY_UNSIGNED")]
#[case("ANY_SIGNED")]
#[case("ANY_REAL")]
#[case("ANY_BIT")]
#[case("ANY_CHARS")]
#[case("ANY_STRING")]
#[case("ANY_CHAR")]
#[case("ANY_DATE")]
#[case("ANY_DURATION")]
fn valid_generic_type_cases(mut with_db: RootDatabase, #[case] value: &str) {
    let source = format!(
        r#"
FUNCTION fn<GT: {value}, GY: {value}>

    VAR
        x: GT;
        y: GY;
    END_VAR

END_FUNCTION"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

#[rstest]
fn valid_generic_constraint_with_sibling_param(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn<GT: ANY_INT, GY: ANY_INT + INTO<GT>>

    VAR
        x: GT;
        y: GY;
    END_VAR

END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn valid_generic_constraint_with_multiple_params(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn<A: ANY_INT, B: ANY_INT + INTO<A>, C: ANY_INT + INTO<B>>

    VAR_INPUT
        x: A;
        y: B;
        z: C;
    END_VAR

END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
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
fn invalid_generic_constraint_array(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn<GT : ANY_INT + INTO<ARRAY>>

END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0312] Error: type mismatch
       ,-[ file:///test0.st:2:33 ]
       |
     2 | FUNCTION fn<GT : ANY_INT + INTO<ARRAY>>
       |                                 ^^|^^
       |                                   `---- INTO constraint on 'GT' must target a sibling generic parameter, got 'ARRAY'
    ---'
    ");
}

#[rstest]
fn invalid_generic_constraint_concrete_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn<GT: ANY_INT + INTO<INT>>

END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0312] Error: type mismatch
       ,-[ file:///test0.st:2:32 ]
       |
     2 | FUNCTION fn<GT: ANY_INT + INTO<INT>>
       |                                ^|^
       |                                 `--- INTO constraint on 'GT' must target a sibling generic parameter, got 'INT'
    ---'
    ");
}

#[rstest]
fn invalid_generic_constraint_any_group(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn<GT: ANY + INTO<ANY_INT>>

END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0312] Error: type mismatch
       ,-[ file:///test0.st:2:28 ]
       |
     2 | FUNCTION fn<GT: ANY + INTO<ANY_INT>>
       |                            ^^^|^^^
       |                               `----- INTO constraint on 'GT' must target a sibling generic parameter, got 'ANY_INT'
    ---'
    ");
}

#[rstest]
fn invalid_generic_constraint_self_referential(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn<GT: ANY_INT + INTO<GT>>

END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0312] Error: type mismatch
       ,-[ file:///test0.st:2:32 ]
       |
     2 | FUNCTION fn<GT: ANY_INT + INTO<GT>>
       |                                ^|
       |                                 `-- INTO constraint on 'GT' cannot reference itself
    ---'
    ");
}
