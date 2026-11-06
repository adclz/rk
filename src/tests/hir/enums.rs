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

/// A type with named values takes its base's values, as the base would: no other
/// type, and no value that does not fit. An enum with no base holds its names only,
/// and two types with named values stay two types.
#[rstest]
#[case::a_plain_enum_from_a_number("c := 1;", "[E0301]", "expected 'Plain', got 'INT'")]
#[case::a_plain_enum_as_a_number("i := c;", "[E0301]", "expected 'INT', got 'Plain'")]
#[case::a_plain_enum_from_a_variable("c := i;", "[E0301]", "expected 'Plain', got 'INT'")]
#[case::a_plain_enum_against_a_number(
    "b := c = 1;",
    "[E0302]",
    "cannot compare 'Plain' with 'INT'"
)]
#[case::a_plain_name_in_a_calculation(
    "i := Plain#One + 1;",
    "[E0305]",
    "cannot be applied to type 'Plain#One'"
)]
#[case::values_with_no_base_from_a_number("st := 5;", "[E0301]", "expected 'Stepped', got 'INT'")]
#[case::values_with_no_base_as_a_number("i := st;", "[E0301]", "expected 'INT', got 'Stepped'")]
#[case::another_type("m := p;", "[E0301]", "expected 'Light', got 'Paint'")]
#[case::a_name_of_another_type("m := Paint#Blue;", "[E0301]", "expected 'Light', got 'Paint#Blue'")]
#[case::against_another_type("b := m = p;", "[E0302]", "cannot compare 'Light' with 'Paint'")]
#[case::against_a_name_of_another_type(
    "b := m = Paint#Blue;",
    "[E0302]",
    "cannot compare 'Light' with 'Paint#Blue'"
)]
#[case::a_string("m := 'x';", "[E0308]", "cannot use string literal as INT")]
#[case::a_real_literal("m := 1.5;", "[E0308]", "invalid INT literal")]
#[case::a_bool("m := TRUE;", "[E0301]", "expected 'Light', got 'BOOL'")]
#[case::as_a_bool("b := m;", "[E0301]", "expected 'BOOL', got 'Light'")]
#[case::a_real_variable("m := x;", "[E0301]", "expected 'Light', got 'REAL'")]
#[case::past_its_base("s := 300;", "[E0306]", "does not fit in SINT")]
#[case::a_wider_variable("s := i;", "[E0301]", "expected 'Small', got 'INT'")]
#[case::negative_on_an_unsigned_base("rt := -1;", "[E0307]", "UINT is unsigned")]
#[case::an_in_out_of_the_base("inout_int(m);", "[E0301]", "expected 'INT', got 'Light'")]
#[case::an_in_out_from_the_base("inout_light(i);", "[E0301]", "expected 'Light', got 'INT'")]
#[case::a_reference_to_the_base(
    "ri := REF(m);",
    "[E0301]",
    "expected 'REF_TO INT', got 'REF_TO Light'"
)]
#[case::a_reference_from_the_base(
    "r := REF(i);",
    "[E0301]",
    "expected 'REF_TO Light', got 'REF_TO INT'"
)]
#[case::raised_to_a_power("x := m ** 2;", "[E0305]", "cannot be applied to type 'Light'")]
#[case::a_label_of_another_type(
    "CASE m OF Paint#Blue: i := 1; END_CASE;",
    "[E0302]",
    "cannot compare 'Light' with 'Paint#Blue'"
)]
#[case::a_plain_enum_as_a_subscript(
    "list[c] := 1;",
    "[E0504]",
    "the index is 'Plain', not an integer"
)]
#[case::a_plain_enum_as_a_counter(
    "FOR c := 1 TO 3 DO i := 1; END_FOR;",
    "[E1206]",
    "'c' is 'Plain', not an integer"
)]
#[case::not_on_a_number_base("b := NOT m;", "[E0301]", "expected 'BOOL', got 'INT'")]
#[case::not_on_a_plain_enum("b := NOT c;", "[E0301]", "expected 'BOOL', got 'Plain'")]
#[case::a_part_of_a_plain_enum("b := c.0;", "[E1431]", "type 'Plain' has no parts to access")]
#[case::a_part_past_its_base("b := m.16;", "[E1429]", "out of range for type 'INT'")]
#[case::a_literal_between_two_types("i := both(5);", "[E0809]", "call to 'both' is ambiguous")]
#[case::a_variable_between_two_types("i := both(i);", "[E0809]", "call to 'both' is ambiguous")]
fn invalid_what_named_values_do_not_take(
    mut with_db: RootDatabase,
    #[case] body: &str,
    #[case] code: &str,
    #[case] message: &str,
) {
    let source = format!(
        r#"TYPE
    Light : INT (Red := 1, Amber := 2);
    Paint : INT (Rouge := 1, Blue := 4);
    Small : SINT (Lo := 1, Hi := 2);
    Plain : (One, Two);
    Stepped : (Five := 5, Six);
    Rate : UINT (Slow := 60000, Fast := 2);
END_TYPE

FUNCTION inout_int
VAR_IN_OUT v : INT; END_VAR
    v := 1;
END_FUNCTION

FUNCTION inout_light
VAR_IN_OUT v : Light; END_VAR
    v := 1;
END_FUNCTION

FUNCTION both : INT
VAR_INPUT v : Light; END_VAR
    both := 1;
END_FUNCTION

FUNCTION both : INT
VAR_INPUT v : Paint; END_VAR
    both := 2;
END_FUNCTION

FUNCTION F
VAR
    m : Light;
    p : Paint;
    s : Small;
    c : Plain;
    st : Stepped;
    rt : Rate;
    i : INT;
    x : REAL;
    b : BOOL;
    list : ARRAY[0..4] OF INT;
    r : REF_TO Light;
    ri : REF_TO INT;
END_VAR
    {body}
END_FUNCTION
"#
    );
    let report = test_diagnostics(&mut with_db, &[&source]);
    assert!(report.contains(code), "{report}");
    assert!(report.contains(message), "{report}");
}

/// Names of two enums are no more comparable than the enums.
#[rstest]
fn invalid_two_names_of_two_enums(mut with_db: RootDatabase) {
    let source = r#"
TYPE
    Color : (Red, Green);
    Size : (Big, Small);
END_TYPE

FUNCTION F
VAR b : BOOL; END_VAR
    b := Color#Red = Size#Big;
END_FUNCTION
"#;
    let report = test_diagnostics(&mut with_db, &[source]);
    assert!(report.contains("[E0302]"), "{report}");
    assert!(
        report.contains("cannot compare 'Color#Red' with 'Size#Big'"),
        "{report}"
    );
}

/// A name folds as long as its value does: a value that names its own type, or a
/// type naming it back, is no constant. An enum with no base type has no number to
/// fold, and a folded name is held to the bounds it meets.
#[rstest]
#[case::two_values_naming_each_other(
    "TYPE Loop : INT (A := Loop#B, B := Loop#A); END_TYPE",
    "[E0604]"
)]
#[case::two_types_naming_each_other(
    "TYPE One : INT (A := Two#B); Two : INT (B := One#A); END_TYPE",
    "[E0604]"
)]
#[case::a_value_naming_itself("TYPE Self : INT (A := Self#A + 1); END_TYPE", "[E0604]")]
#[case::a_plain_enum_as_a_bound(
    "TYPE Plain : (One, Two); END_TYPE
     FUNCTION F VAR a : ARRAY[Plain#One..Plain#Two] OF INT; END_VAR END_FUNCTION",
    "[E0501]"
)]
#[case::values_with_no_base_as_a_bound(
    "TYPE Stepped : (Five := 5, Six); END_TYPE
     FUNCTION F VAR a : ARRAY[Stepped#Five..Stepped#Six] OF INT; END_VAR END_FUNCTION",
    "[E0501]"
)]
#[case::values_with_no_base_calculated(
    "TYPE Stepped : (Five := 5, Six); END_TYPE
     FUNCTION F VAR CONSTANT k : INT := Stepped#Five + 1; END_VAR END_FUNCTION",
    "[E0305]"
)]
#[case::values_with_no_base_as_a_length(
    "TYPE Stepped : (Five := 5, Six); END_TYPE
     FUNCTION F VAR s : STRING[Stepped#Six]; END_VAR END_FUNCTION",
    "[E0315]"
)]
#[case::a_range_of_plain_labels(
    "TYPE Plain : (One, Two); END_TYPE
     FUNCTION F VAR c : Plain; i : INT; END_VAR CASE c OF Plain#One..Plain#Two: i := 1; END_CASE; END_FUNCTION",
    "[E1205]"
)]
#[case::past_an_arrays_bounds(
    "TYPE Light : INT (Red := 1, Green := 3); END_TYPE
     FUNCTION F VAR a : ARRAY[0..1] OF INT; END_VAR a[Light#Green] := 7; END_FUNCTION",
    "[E0506]"
)]
#[case::past_a_subrange_it_bounds(
    "TYPE Light : INT (Red := 1, Green := 3); END_TYPE
     FUNCTION F VAR r : INT (Light#Red..Light#Green); END_VAR r := 5; END_FUNCTION",
    "[E0702]"
)]
fn invalid_what_does_not_fold(mut with_db: RootDatabase, #[case] source: &str, #[case] code: &str) {
    let report = test_diagnostics(&mut with_db, &[source]);
    assert!(report.contains(code), "{report}");
}
