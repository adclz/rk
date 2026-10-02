// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

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
    [E0601] Error: ENUM base type not an integer
       ,-[ file:///test0.st:3:19 ]
       |
     3 |             List: BOOL (A, B, C);
       |                   ^^|^
       |                     `--- 'BOOL' is not an integer type
       |
       | Note: an ENUM is stored in an integer type
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
    [E0602] Error: not an ENUM type
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
    [E0307] Error: negative literal for an unsigned type
       ,-[ file:///test0.st:3:33 ]
       |
     3 |             List: UINT (A, B := -5, C);
       |                                 ^|
       |                                  `-- the value is negative and UINT is unsigned
       |
       | Help: use INT, or drop the sign
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
    [E0603] Error: unknown ENUM variant
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
       |              ^^|^
       |                `--- 'Mode' is declared here
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
    [E0302] Error: types not comparable
       ,-[ file:///test0.st:9:17 ]
       |
     2 |         TYPE STATE: INT(A, B, C) END_TYPE
       |              ^^|^^
       |                `---- 'STATE' is declared here
       |
     9 |                 STATE2#A: test := 20;
       |                 ^^^^|^^^
       |                     `----- cannot compare 'STATE' with 'STATE2#A'
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
    [E0604] Error: ENUM variant value not constant
       ,-[ file:///test0.st:5:30 ]
       |
     5 |         TYPE Mode : (Idle := fn1(), Run) END_TYPE
       |                              ^^|^^
       |                                `---- the value is not a compile-time constant
    ---'
    ");
}

/// A TYPE's enum default is written qualified, like every enum value, and
/// the type may name itself: nothing typed a begin path in a TYPE-scoped
/// initializer before, so `Color#Green` here, and even `Shade : Color :=
/// Color#Green` in a second type, was E0602 on '{unknown}'.
#[rstest]
fn valid_enum_type_default_is_qualified(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Color : (Red, Green, Blue) := Color#Green;
            Shade : Color := Color#Blue;
        END_TYPE
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// The bare spelling stays refused in that position too: the once-per-type
/// rule sees a name it cannot fold.
#[rstest]
fn invalid_enum_type_default_bare_variant(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Color : (Red, Green, Blue) := Green;
        END_TYPE
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0401] Error: initial value not constant
       ,-[ file:///test0.st:3:43 ]
       |
     3 |             Color : (Red, Green, Blue) := Green;
       |                                           ^^|^^
       |                                             `---- this initial value must be a constant: it is fixed before the program runs
    ---'
    ");
}

/// An enum alias defaulting to another enum's variant is a type error.
#[rstest]
fn invalid_enum_type_default_of_another_enum(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Color : (Red, Green, Blue);
            Other : (Red, Black);
            Shade : Color := Other#Red;
        END_TYPE
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:5:27 ]
       |
     3 |             Color : (Red, Green, Blue);
       |             ^^|^^
       |               `---- 'Color' is declared here
       |
     5 |             Shade : Color := Other#Red;
       |                           ^^^^^^|^^^^^
       |                                 `------- expected 'Color', got 'Other#Red'
    ---'
    ");
}

// Every value fits the storage, a continued one too, and DINT when none is
// written. `B` wrapped to -128 and read back as another variant. Only the
// first value past the edge is reported.
#[rstest]
fn invalid_value_past_the_storage(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Code : SINT (A := 127, B, C);
            Wide : (P := 3000000000);
        END_TYPE
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0605] Error: ENUM value outside its storage type
       ,-[ file:///test0.st:3:36 ]
       |
     3 |             Code : SINT (A := 127, B, C);
       |                                    |
       |                                    `-- 'B' is 128, out of range for SINT
       |
       | Note: a variant without a value is one more than the one before
    ---'
    [E0605] Error: ENUM value outside its storage type
       ,-[ file:///test0.st:4:21 ]
       |
     4 |             Wide : (P := 3000000000);
       |                     |
       |                     `-- 'P' is 3000000000, out of range for DINT
    ---'
    ");
}
