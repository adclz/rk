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
       |                     `--- invalid enum type 'BOOL'
       |
       | Note: only numeric integer types are allowed for ENUM
    ---'
    ");
}

#[rstest]
fn access_enum_variant_on_non_enum_type(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            List: INT (A, B, C);
        END_TYPE

        FUNCTION fn
            VAR 
                test: List
                typ: BOOL;
            END_VAR

            test := typ#A
        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0702] Error: invalid enum access
        ,-[ file:///test0.st:12:21 ]
        |
     12 |             test := typ#A
        |                     ^|^
        |                      `--- 'BOOL' is not an ENUM type
    ----'
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

/// A variant belongs to exactly one enum, and `Type::EnumVariant` carries
/// which — a foreign enum's variant is a type mismatch, not a value that
/// happens to share a name. (The coercion used to accept ANY variant into
/// ANY enum.)
#[rstest]
fn invalid_foreign_enum_variant(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Mode  : (Stop, Run); END_TYPE
        TYPE Color : (Red, Green); END_TYPE

        FUNCTION f : INT
        VAR m : Mode; END_VAR
            m := Color#Red;
        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:7:18 ]
       |
     2 |         TYPE Mode  : (Stop, Run); END_TYPE
       |                      ^^^^^|^^^^^
       |                           `------- type is defined by 'Mode' here
       |
     7 |             m := Color#Red;
       |                  ^^^^|^^^^
       |                      `------ expected 'Mode', got 'Color#Red'
    ---'
    ");
}

/// The CASE-label form of the same rule: a foreign enum's variant label is
/// refused where the label is checked against the selector, before any
/// duplicate-label question can arise. (This fixture used to live in the
/// duplicate-case LINTER tests, asserting the label merely was not a
/// duplicate — the coercion accepted any variant into any enum back then.)
#[rstest]
fn invalid_foreign_enum_variant_case_label(mut with_db: RootDatabase) {
    let source = r#"
        TYPE STATE: INT(A, B, C) END_TYPE
        TYPE STATE2: INT(A, B, C) END_TYPE

        FUNCTION test : INT
        VAR x : STATE; END_VAR
            CASE x OF
                STATE#A: test := 10;
                STATE2#A: test := 20;
            END_CASE;
        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0302] Error: type mismatch
       ,-[ file:///test0.st:9:17 ]
       |
     2 |         TYPE STATE: INT(A, B, C) END_TYPE
       |                     ^^^^^^|^^^^^
       |                           `------- type is defined by 'STATE' here
       |
     9 |                 STATE2#A: test := 20;
       |                 ^^^^|^^^
       |                     `----- can't compare 'STATE' with 'STATE2#A'
    ---'
    ");
}

// A declared variant value folds through the same evaluator as CASE labels
// and FOR steps: `1 + 1` is a constant. It used to pass the check and abort
// MIR, which only read bare literals.
#[rstest]
fn valid_enum_value_folding_expression(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Mode : (Idle := 1 + 1, Run) END_TYPE
        FUNCTION fn1 : INT
        VAR m : Mode; END_VAR
            m := Mode#Run;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn invalid_enum_value_not_constant(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR x : INT; END_VAR
        END_FUNCTION
        TYPE Mode : (Idle := fn1(), Run) END_TYPE
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0704] Error: invalid enum value
       ,-[ file:///test0.st:5:30 ]
       |
     5 |         TYPE Mode : (Idle := fn1(), Run) END_TYPE
       |                              ^^|^^
       |                                `---- an enum variant value must evaluate to a constant at compile time
    ---'
    ");
}
