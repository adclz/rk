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
fn negative_lower_bound_is_valid(mut with_db: RootDatabase) {
    // IEC 61131-3 allows a negative lower bound. This was rejected while array
    // bounds were folded as UNSIGNED, so `-1` failed to evaluate at all.
    let source = r#"
        TYPE
            List: ARRAY[-1..10] OF INT;
        END_TYPE
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn non_constant_array_bound_is_rejected(mut with_db: RootDatabase) {
    // A bound that is not a compile-time constant still cannot be folded.
    let source = r#"
        FUNCTION fn1 : INT
        VAR x : INT; arr : ARRAY[x..10] OF INT; END_VAR
        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0601] Error: invalid array bounds
       ,-[ file:///test0.st:3:34 ]
       |
     3 |         VAR x : INT; arr : ARRAY[x..10] OF INT; END_VAR
       |                                  |
       |                                  `-- invalid lower bound value for ARRAY
    ---'
    ");
}

#[rstest]
fn upper_bound_below_lower_bound_is_rejected(mut with_db: RootDatabase) {
    // `0..-10` now FOLDS (both are valid constants); what is wrong is the
    // ordering, so it is reported as an inferior upper bound (E0603) rather
    // than an unevaluatable value.
    let source = r#"
        TYPE
            List: ARRAY[0..-10] OF INT;
        END_TYPE
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0603] Error: invalid array bounds
       ,-[ file:///test0.st:3:28 ]
       |
     3 |             List: ARRAY[0..-10] OF INT;
       |                            ^|^
       |                             `--- upper bound value must be greater than lower bound value
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
fn nested_array_negative_bound_is_valid(mut with_db: RootDatabase) {
    // A negative bound in ANY dimension of a multi-dimensional array is valid.
    let source = r#"
        TYPE
            List: ARRAY[0..3, -2..1] OF INT;
        END_TYPE
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn array_conformand_not_supported(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1
        VAR_INPUT
            A: ARRAY [*] OF INT;
        END_VAR

        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0036] Error: syntax
       ,-[ file:///test0.st:4:14 ]
       |
     4 |             A: ARRAY [*] OF INT;
       |              ^^^^^^^^^|^^^^^^^^
       |                       `---------- array conformands (ARRAY[*]) are not supported
    ---'
    ");
}
