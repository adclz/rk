use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

// ── Valid: ALL ANY_* types are accepted as type specs ────────────────────

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
#[case("ANY_ELEMENTARY")]
fn valid_any_type_specs(mut with_db: RootDatabase, #[case] any_type: &str) {
    let source = format!(
        r#"
FUNCTION fn1

    VAR
        x: {any_type};
        y: {any_type};
    END_VAR

END_FUNCTION"#
    );

    insta::allow_duplicates! { assert_snapshot!(test_diagnostics(&mut with_db, &[&source]), @""); }
}

// ── Valid: ANY_* as return type ──────────────────────────────────────────

#[rstest]
fn valid_any_num_return_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : ANY_NUM
    VAR_INPUT
        x: INTO(fn1);
    END_VAR
    fn1 := x;
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// ── Valid: INTO(ref) spec on a variable ──────────────────────────────────

#[rstest]
fn valid_into_spec(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1
    VAR_INPUT
        value: ANY;
        target: INTO(value);
    END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// ── Valid: INTO(ref) with ANY_INT constraint ─────────────────────────────

#[rstest]
fn valid_into_spec_with_constraint(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1
    VAR_INPUT
        value: ANY_INT;
        target: INTO(value);
    END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// ── Valid: multiple params with INTO(ref) ────────────────────────────────

#[rstest]
fn valid_into_spec_multiple_params(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1
    VAR_INPUT
        a: ANY_SIGNED;
        b: INTO(a);
        c: INTO(a);
    END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}


#[rstest]
fn invalid_into_spec_multiple_params(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1
    VAR
        a: ANY_SIGNED;
        b: ANY_SIGNED;
    END_VAR

    a := b
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

// ── INTO(ref) error cases ───────────────────────────────────────────────

#[rstest]
fn invalid_into_ref_not_found(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1
    VAR_INPUT
        x: INTO(nonexistent);
    END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0231] Error: INTO reference not found
       ,-[ file:///test0.st:4:12 ]
       |
     4 |         x: INTO(nonexistent);
       |            ^^^^^^^^|^^^^^^^^
       |                    `---------- INTO reference 'nonexistent' not found in scope
    ---'
    ");
}

#[rstest]
fn invalid_into_ref_not_elementary(mut with_db: RootDatabase) {
    let source = r#"
TYPE MyStruct : STRUCT
    field: INT;
END_STRUCT
END_TYPE

FUNCTION fn1 : INT
    VAR_INPUT
        s: MyStruct;
        x: INTO(s);
    END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0232] Error: INTO reference must be an ANY type
        ,-[ file:///test0.st:10:12 ]
        |
     10 |         x: INTO(s);
        |            ^^^|^^^
        |               `----- INTO reference 's' must have an ANY type, got 'MyStruct'
        |
        | Note: only variables with ANY types (e.g. ANY_INT, ANY_REAL, ANY_BIT) can be used as INTO references
    ----'
    ");
}

#[rstest]
fn invalid_into_ref_concrete_type(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1 : INT
    VAR_INPUT
        a: INT;
        b: INTO(a);
    END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0232] Error: INTO reference must be an ANY type
       ,-[ file:///test0.st:5:12 ]
       |
     5 |         b: INTO(a);
       |            ^^^|^^^
       |               `----- INTO reference 'a' must have an ANY type, got 'INT'
       |
       | Note: only variables with ANY types (e.g. ANY_INT, ANY_REAL, ANY_BIT) can be used as INTO references
    ---'
    ");
}

#[rstest]
fn invalid_into_ref_not_elementary_suggests_fn(mut with_db: RootDatabase) {
    let source = r#"
TYPE MyStruct : STRUCT
    field: INT;
END_STRUCT
END_TYPE

FUNCTION fn1 : ANY_NUM
    VAR_INPUT
        s: MyStruct;
        x: INTO(s);
    END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0232] Error: INTO reference must be an ANY type
        ,-[ file:///test0.st:10:12 ]
        |
     10 |         x: INTO(s);
        |            ^^^|^^^
        |               `----- INTO reference 's' must have an ANY type, got 'MyStruct'
        |
        | Note 1: only variables with ANY types (e.g. ANY_INT, ANY_REAL, ANY_BIT) can be used as INTO references
        |
        | Note 2: you may also use INTO(fn1) to reference the return type 'ANY_NUM'
    ----'
    ");
}

#[rstest]
fn invalid_into_ref_callable_no_return(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION fn1
    VAR_INPUT
        x: INTO(fn1);
    END_VAR
END_FUNCTION"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0232] Error: INTO reference must be an ANY type
       ,-[ file:///test0.st:4:12 ]
       |
     4 |         x: INTO(fn1);
       |            ^^^^|^^^^
       |                `------ INTO reference 'fn1' must have an ANY type, got '{unknown}'
       |
       | Note: only variables with ANY types (e.g. ANY_INT, ANY_REAL, ANY_BIT) can be used as INTO references
    ---'
    ");
}
