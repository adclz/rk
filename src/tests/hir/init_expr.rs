// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::default::db::file::File;
use auto_lsp::lsp_types::DiagnosticSeverity;
use auto_lsp::tree_sitter::Range;
use db::RootDatabase;
use db::WorkspaceDataBase;
use hir::HirNodeInfo;
use hir::hir_ty::head::init_inference::infer_initialization;
use ide_diagnostic::{IdeDiagnostic, Related};
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::find_pou_with_name;
use crate::tests::utils::test_diagnostics;
use crate::tests::utils::test_snapshot;
use crate::tests::utils::with_db;

#[rstest]
fn unknown_struct_field(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Engine:
            STRUCT
                power : INT;
                oil : REAL;
            END_STRUCT
        END_TYPE

        FUNCTION StartEngine
            VAR
                // fuel is not a member of engine
                Base : Engine := (power := 100, fuel := 10.0);
            END_VAR

        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0202] Error: unknown field
        ,-[ file:///test0.st:12:49 ]
        |
     12 |                 Base : Engine := (power := 100, fuel := 10.0);
        |                                                 ^^^^^^|^^^^^
        |                                                       `------- 'Engine' has no field named 'fuel'
    ----'
    ");
}

#[rstest]
fn invalid_struct_value(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Engine:
            STRUCT
                power : INT;
                oil : REAL;
            END_STRUCT
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : Engine := (power := 10, fuel := 10.0);
            END_VAR

        END_FUNCTION

        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0202] Error: unknown field
        ,-[ file:///test0.st:11:48 ]
        |
     11 |                 Base : Engine := (power := 10, fuel := 10.0);
        |                                                ^^^^^^|^^^^^
        |                                                      `------- 'Engine' has no field named 'fuel'
    ----'
    ");
}

#[rstest]
fn invalid_array_value(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Engine: ARRAY[0..3] OF INT;
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : Engine := [3(10.5)];
            END_VAR

        END_FUNCTION

        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0308] Error: literal of the wrong kind
       ,-[ file:///test0.st:8:37 ]
       |
     8 |                 Base : Engine := [3(10.5)];
       |                                     ^^|^
       |                                       `--- invalid INT literal
       |
       | Note: INT takes a whole number, written like 42 or 16#2A
    ---'
    ");
}

#[rstest]
fn invalid_value_in_array_of_struct(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Engine: STRUCT
                Power: INT;
                Torque: INT;
            END_STRUCT;
            EngineArray: ARRAY[0..3] OF Engine;
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : EngineArray := [(Power := 10, Torque := 10.0)];
            END_VAR

        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0308] Error: literal of the wrong kind
        ,-[ file:///test0.st:12:64 ]
        |
     12 |                 Base : EngineArray := [(Power := 10, Torque := 10.0)];
        |                                                                ^^|^
        |                                                                  `--- invalid INT literal
        |
        | Note: INT takes a whole number, written like 42 or 16#2A
    ----'
    ");
}

#[rstest]
fn invalid_value_in_struct_with_array(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Engine: STRUCT
                Power: ARRAY[0..2] OF INT;
                Torque: INT;
            END_STRUCT;
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : Engine := (Power := [10, 5.3], Torque := 10);
            END_VAR

        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0308] Error: literal of the wrong kind
        ,-[ file:///test0.st:11:49 ]
        |
     11 |                 Base : Engine := (Power := [10, 5.3], Torque := 10);
        |                                                 ^|^
        |                                                  `--- invalid INT literal
        |
        | Note: INT takes a whole number, written like 42 or 16#2A
    ----'
    ");
}

#[rstest]
fn unexpected_struct_field(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Engine: ARRAY[0..3] OF INT;
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : Engine := [2(param1 := 0)];
            END_VAR

        END_FUNCTION

        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0403] Error: elementary type initialized with parentheses
       ,-[ file:///test0.st:8:37 ]
       |
     8 |                 Base : Engine := [2(param1 := 0)];
       |                                     ^^^^^|^^^^^
       |                                          `------- 'INT' is an elementary type and takes no '()' initializer
    ---'
    ");
}

#[rstest]
fn unexpected_array(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Engine: INT;
        END_TYPE

        FUNCTION StartEngine
            VAR
                Base : Engine := [2];
            END_VAR

        END_FUNCTION

        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0508] Error: index into a non-array type
       ,-[ file:///test0.st:8:31 ]
       |
     8 |                 Base : Engine := [2];
       |                               ^^^|^^
       |                                  `---- cannot index into type 'Engine'
    ---'
    ");
}

/// Collects init expression diagnostics for a POU.
/// Each init expression gets a label showing its resolved type.
fn init_expr_diagnostics(
    db: &dyn WorkspaceDataBase,
    file: File,
    pou_name: &str,
) -> Vec<IdeDiagnostic> {
    let pou = match find_pou_with_name(db, file, pou_name) {
        Some(p) => p,
        None => return vec![],
    };

    let infer_result = infer_initialization(db, pou.get_scope_id(db));

    let mut entries: Vec<(Range, String)> = vec![];
    for (init_expr, typ) in &infer_result.init_expr_result.type_of_init_expr {
        let span = init_expr.get_span(db);
        entries.push((span, typ.kind().to_string()));
    }

    entries.sort_by_key(|(span, _)| span.start_byte);

    if entries.is_empty() {
        return vec![];
    }

    let (first_span, first_label) = &entries[0];
    let mut diag = ide_diagnostic::diag()
        .range(hir::denormalize(db, file, first_span).unwrap_or_default())
        .message(first_label.clone())
        .severity(DiagnosticSeverity::INFORMATION)
        .call();

    for (span, label) in &entries[1..] {
        diag.with_related(Related::new(label.clone(), file, *span));
    }

    vec![diag]
}

#[rstest]
fn walk_array_init_expression(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Engine: ARRAY[0..3] OF INT;
        END_TYPE

        FUNCTION fn
            VAR
                Base : Engine := [5];
            END_VAR

        END_FUNCTION
"#;

    assert_snapshot!(test_snapshot(&mut with_db, &[source], |db, file| {
        init_expr_diagnostics(db, file, "fn")
    }), @r"
    Info: DATATYPE
       ,-[ file:///test0.st:8:31 ]
       |
     8 |                 Base : Engine := [5];
       |                               ^^^|^^
       |                                  `---- DATATYPE
    ---'
    ");
}

#[rstest]
fn struct_fields(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Engine:
            STRUCT
                power : INT;
                oil : REAL;
            END_STRUCT
        END_TYPE

        FUNCTION fn
            VAR
                Base : Engine := (power := 100, oil := 10.0);
            END_VAR

        END_FUNCTION
"#;

    assert_snapshot!(test_snapshot(&mut with_db, &[source], |db, file| {
        init_expr_diagnostics(db, file, "fn")
    }), @r"
    Info: DATATYPE
        ,-[ file:///test0.st:11:31 ]
        |
     11 |                 Base : Engine := (power := 100, oil := 10.0);
        |                               ^^^^^^^^^^|^^^^|^^^^^^^|^^^^^^
        |                                         `--------------------- STRUCT_ELEMENT
        |                                              |       |
        |                                              `---------------- DATATYPE
        |                                                      |
        |                                                      `-------- STRUCT_ELEMENT
    ----'
    ");
}

// ─────────────────────────────────────────────────────────────────────────────
// Init-expr edge cases, against this codebase's IEC semantics: partial init is
// allowed (only a max check), and a repetition `x(y)` fills x*y positions. These
// assert what INFERENCE does — a diagnostic, or none for accepted-by-design init.
// ─────────────────────────────────────────────────────────────────────────────

/// CORRECT: a struct init missing a field is partial init (the field defaults) —
/// there is no completeness/minimum check, so it is accepted by design.
#[rstest]
fn partial_struct_init_accepted(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Engine: STRUCT power : INT; oil : REAL; END_STRUCT END_TYPE
        FUNCTION Test
            VAR e : Engine := (power := 100); END_VAR
        END_FUNCTION
        "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// CORRECT: an empty initializer is full partial init (all default). No
/// under-fill check, accepted by design.
#[rstest]
fn empty_array_init_accepted(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION Test
            VAR a : ARRAY[0..3] OF INT := []; END_VAR
        END_FUNCTION
        "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A digit-separator in a repeat count `[1_0(7)]` (count = 10) is accepted by
/// inference (the count is read via `as_u64`, which strips `_`).
#[rstest]
fn underscore_repeat_count_typechecks(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION Test
            VAR a : ARRAY[0..9] OF INT := [1_0(7)]; END_VAR
        END_FUNCTION
        "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// Nested repetition `[2(3(5))]` = 2×(3×5) = 6 positions, exactly filling
/// `ARRAY[0..5]`: inference accepts it (no diagnostic).
#[rstest]
fn nested_repetition_typechecks(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION Test
            VAR a : ARRAY[0..5] OF INT := [2(3(5))]; END_VAR
        END_FUNCTION
        "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// LIMITATION: a radix repetition count `[16#4(0)]` is not accepted — the grammar
/// constrains the count to a decimal literal, so `16#4` is a syntax error. A
/// clearer "count must be a decimal literal" diagnostic would help.
#[rstest]
fn radix_repeat_count_rejected(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION Test
            VAR a : ARRAY[0..3] OF INT := [16#4(0)]; END_VAR
        END_FUNCTION
        "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0001] Error: syntax error
       ,-[ file:///test0.st:3:44 ]
       |
     3 |             VAR a : ARRAY[0..3] OF INT := [16#4(0)]; END_VAR
       |                                            ^^|^
       |                                              `--- unexpected token(s): '16#4'
    ---'
    ");
}

/// LIMITATION: a named-constant count `[FOO(0)]` is not
/// accepted — the count must be a literal, so it mis-parses as a function call in
/// init (E0402). A clearer diagnostic would help.
/// TODO: we might accept this in a next version
#[rstest]
fn named_constant_repeat_count_rejected(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION Test
            VAR a : ARRAY[0..3] OF INT := [FOO(0)]; END_VAR
        END_FUNCTION
        "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0402] Error: function call in a repeat count
       ,-[ file:///test0.st:3:44 ]
       |
     3 |             VAR a : ARRAY[0..3] OF INT := [FOO(0)]; END_VAR
       |                                            ^^^|^^
       |                                               `---- the repeat count is a call, not a constant
    ---'
    ");
}

/// `n()` repeats no value: n elements keep their default. The grammar wanted
/// a value inside, and said `no item ""`. The empty repetition still counts
/// its elements against the array's bounds.
#[rstest]
fn empty_repetition_counts_its_elements(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION Test
            VAR
                a : ARRAY[0..3] OF INT := [1, 2(), 4];
                b : ARRAY[0..3] OF INT := [1, 4()];
            END_VAR
        END_FUNCTION
        "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0507] Error: too many elements in array initializer
       ,-[ file:///test0.st:5:47 ]
       |
     5 |                 b : ARRAY[0..3] OF INT := [1, 4()];
       |                                               ^|^
       |                                                `--- the initializer has more elements than the array's 4
    ---'
    ");
}

/// A STRUCT's values may come without a name: each sets the next field that no
/// value names, in the order the fields are declared, as a call's positional
/// arguments fill the parameters no argument names. `()` keeps a field's
/// default. Each place a start value goes takes them.
#[rstest]
fn valid_values_without_a_name_fill_the_fields_in_order(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Point : STRUCT x : INT := 7; y : INT; END_STRUCT;
            Named : STRUCT id : INT; name : STRING[8]; END_STRUCT;
            Holder : STRUCT p : Point := (3, 4); arr : ARRAY[0..1] OF INT; n : INT; END_STRUCT;
            Origin : Point := (1, 2);
        END_TYPE
        VAR_CONSTANT CORNER : Point := (5, 6); END_VAR
        CONFIGURATION C VAR_GLOBAL g : Point := (8, 9); END_VAR END_CONFIGURATION

        FUNCTION_BLOCK Takes
            VAR_INPUT p : Point := (10, 11); END_VAR
        END_FUNCTION_BLOCK

        FUNCTION F
            VAR
                a : Point := (1, 2);
                b : Point := (1, y := 2);
                c : Point := (y := 2, 1);
                d : Point := ((), 4);
                e : Named := (3, 'pump');
                h : Holder := ((x := 1), [4, 5], 6);
            END_VAR
        END_FUNCTION
        "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// Values without a name nest: an array's rows, a member's value, an element
/// at its index, a repetition's value, a struct's in another's place.
#[rstest]
fn valid_values_without_a_name_nest(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Point : STRUCT x : INT; y : INT; END_STRUCT;
            Holder : STRUCT p : Point; n : INT; END_STRUCT;
        END_TYPE

        FUNCTION F
            VAR
                rows : ARRAY[0..1] OF Point := [(1, 2), (3, 4)];
                member : Holder := (p := (1, 2), n := 3);
                indexed : ARRAY[0..3] OF Point := [[2] := (5, 6)];
                repeated : ARRAY[0..3] OF Point := [2((7, 8)), 2((y := 1, 9))];
                kept : ARRAY[0..0] OF Point := [((), 4)];
                nested : ARRAY[0..0] OF Holder := [((1, 2), 3)];
            END_VAR
        END_FUNCTION
        "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A value without a name after the last field no value names has no field
/// to go to.
#[rstest]
fn invalid_more_values_than_fields(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION F
            VAR
                p : Point := (1, 2, 3);
                q : Point := (y := 2, 1, 3);
            END_VAR
        END_FUNCTION
        "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0410] Error: more values than fields
       ,-[ file:///test0.st:6:37 ]
       |
     2 |         TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE
       |              ^^|^^
       |                `---- 'Point' is declared here
       |
     6 |                 p : Point := (1, 2, 3);
       |                                     |
       |                                     `-- no field of 'Point' is left for this value
       |
       | Note: a value without a name sets the next field that no value names
    ---'
    [E0410] Error: more values than fields
       ,-[ file:///test0.st:7:42 ]
       |
     2 |         TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE
       |              ^^|^^
       |                `---- 'Point' is declared here
       |
     7 |                 q : Point := (y := 2, 1, 3);
       |                                          |
       |                                          `-- no field of 'Point' is left for this value
       |
       | Note: a value without a name sets the next field that no value names
    ---'
    ");
}

/// An instance's members are named: a value without a name was typed against
/// the instance itself and placed nowhere, `((k := 5))` passing with its 5
/// dropped.
#[rstest]
fn invalid_a_value_without_a_member_name(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Drive
            VAR_INPUT speed : INT; ramp : INT; END_VAR
        END_FUNCTION_BLOCK

        FUNCTION F
            VAR
                d : Drive := (speed := 100, 5);
            END_VAR
        END_FUNCTION
        "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0409] Error: value without a member name
       ,-[ file:///test0.st:8:45 ]
       |
     2 |         FUNCTION_BLOCK Drive
       |                        ^^|^^
       |                          `---- FUNCTION_BLOCK 'Drive' is declared here
       |
     8 |                 d : Drive := (speed := 100, 5);
       |                                             |
       |                                             `-- this value names no member of 'Drive'
       |
       | Help: write the member's name before the value: <member> := <value>
       |
       | Note: an instance's initializer names each member it sets
    ---'
    ");
}

/// The types each case below initializes.
const PLACED: &str = r#"
    TYPE PAIR : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE
    TYPE NAMED : STRUCT id : INT; name : STRING[8]; END_STRUCT; END_TYPE
    TYPE HOLDER : STRUCT p : PAIR; n : INT; END_STRUCT; END_TYPE
    TYPE ROW : STRUCT arr : ARRAY[0..3] OF INT; n : INT; END_STRUCT; END_TYPE
    FUNCTION_BLOCK FB VAR_INPUT k : INT; END_VAR END_FUNCTION_BLOCK
    VAR_CONSTANT ORIGIN : PAIR := (x := 4, y := 9); END_VAR
"#;

/// A value without a name is the value of a field, checked as a named one is,
/// or refused where it has none, at the top of a start value or in an array's
/// row and a member's value. Each of the first five passed, and the program
/// ran without it.
#[rstest]
#[case::doubled_parentheses("FUNCTION F VAR p : PAIR := ((x := 5)); END_VAR END_FUNCTION", "E0403")]
#[case::beside_a_named_value(
    "FUNCTION F VAR p : PAIR := ((x := 5), y := 1); END_VAR END_FUNCTION",
    "E0403"
)]
#[case::a_repetition(
    "FUNCTION F VAR p : PAIR := (2((x := 5))); END_VAR END_FUNCTION",
    "E0515"
)]
#[case::a_named_repetition(
    "FUNCTION F VAR p : PAIR := (x := 2(5)); END_VAR END_FUNCTION",
    "E0515"
)]
#[case::a_repetition_of_rows_for_an_array(
    "FUNCTION F VAR r : ROW := (arr := 2([1, 2])); END_VAR END_FUNCTION",
    "E0515"
)]
#[case::a_repetition_for_an_array(
    "FUNCTION F VAR r : ROW := (arr := 2(7)); END_VAR END_FUNCTION",
    "E0515"
)]
#[case::a_repetition_for_an_array_by_place(
    "FUNCTION F VAR r : ROW := (2([1, 2]), 3); END_VAR END_FUNCTION",
    "E0515"
)]
#[case::a_repetition_of_an_instance_s_value_by_place(
    "FUNCTION F VAR r : ARRAY[0..1] OF FB := [2(k := 1, 5)]; END_VAR END_FUNCTION",
    "E0409"
)]
#[case::an_instance("FUNCTION F VAR b : FB := ((k := 5)); END_VAR END_FUNCTION", "E0409")]
#[case::a_value_of_the_struct(
    "FUNCTION F VAR p : PAIR := (ORIGIN, y := 1); END_VAR END_FUNCTION",
    "E0301"
)]
#[case::a_bracket("FUNCTION F VAR p : PAIR := ([1, 2]); END_VAR END_FUNCTION", "E0508")]
#[case::a_long_string(
    "FUNCTION F VAR n : NAMED := (1, 'longer than 8'); END_VAR END_FUNCTION",
    "E0314"
)]
#[case::past_the_last_field("FUNCTION F VAR p : PAIR := (1, 2, 3); END_VAR END_FUNCTION", "E0410")]
#[case::one_value_alone("FUNCTION F VAR p : PAIR := (5); END_VAR END_FUNCTION", "E0301")]
#[case::a_row_past_the_last_field(
    "FUNCTION F VAR r : ARRAY[0..0] OF PAIR := [(1, 2, 3)]; END_VAR END_FUNCTION",
    "E0410"
)]
#[case::a_member_s_value_past_the_last_field(
    "FUNCTION F VAR h : HOLDER := (p := (1, 2, 3)); END_VAR END_FUNCTION",
    "E0410"
)]
#[case::a_row_of_instances(
    "FUNCTION F VAR r : ARRAY[0..0] OF FB := [(k := 1, 2)]; END_VAR END_FUNCTION",
    "E0409"
)]
#[case::a_row_of_one_value(
    "FUNCTION F VAR r : ARRAY[0..0] OF PAIR := [(5)]; END_VAR END_FUNCTION",
    "E0301"
)]
#[case::a_long_string_in_a_row(
    "FUNCTION F VAR r : ARRAY[0..0] OF NAMED := [(1, 'longer than 8')]; END_VAR END_FUNCTION",
    "E0314"
)]
fn a_value_without_a_name_is_a_field_s_value(
    mut with_db: RootDatabase,
    #[case] source: &str,
    #[case] code: &str,
) {
    let report = test_diagnostics(&mut with_db, &[PLACED, source]);
    let codes: Vec<&str> = report
        .lines()
        .filter_map(|line| line.strip_prefix('[')?.split_once(']'))
        .map(|(code, _)| code)
        .collect();
    assert_eq!(codes, [code], "{report}");
}

/// A repetition writes elements of an array, in the brackets of its value.
/// Given as a field's whole value, it was walked as the array's elements and
/// placed nowhere: `arr` stayed 0. In brackets, it is the array's value.
#[rstest]
fn invalid_a_repetition_outside_an_array_s_brackets(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Recipe : STRUCT steps : ARRAY[0..3] OF INT; END_STRUCT; END_TYPE

        FUNCTION F
            VAR
                a : Recipe := (steps := 2([1, 2]));
                b : Recipe := (steps := [2(1, 2)]);
            END_VAR
        END_FUNCTION
        "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0515] Error: repetition outside an array's brackets
       ,-[ file:///test0.st:6:41 ]
       |
     6 |                 a : Recipe := (steps := 2([1, 2]));
       |                                         ^^^^|^^^^
       |                                             `------ this repetition is outside an array's brackets
       |
       | Help: write the array's value in brackets: [2(...)]
       |
       | Note: a repetition writes elements of an array, in its brackets
    ---'
    ");
}

/// An initializer of a type that does not resolve is placed nowhere, and its
/// type's name is the one error: nothing else is said about it, by name or
/// by place, nested or in an array.
#[rstest]
fn invalid_an_initializer_of_an_unknown_type(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Holder : STRUCT p : Nowhere; n : INT; END_STRUCT; END_TYPE

        FUNCTION F
            VAR
                a : Nowhere := (1, 2);
                b : ARRAY[0..1] OF Nowhere := [(x := 1), (2, 3)];
                c : Holder := ((1, 2), 3);
            END_VAR
        END_FUNCTION
        "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0203] Error: unknown name in a namespace
       ,-[ file:///test0.st:2:34 ]
       |
     2 |         TYPE Holder : STRUCT p : Nowhere; n : INT; END_STRUCT; END_TYPE
       |                                  ^^^|^^^
       |                                     `----- no item found for path 'Nowhere'
    ---'
    [E0203] Error: unknown name in a namespace
       ,-[ file:///test0.st:6:21 ]
       |
     6 |                 a : Nowhere := (1, 2);
       |                     ^^^|^^^
       |                        `----- no item found for path 'Nowhere'
    ---'
    [E0203] Error: unknown name in a namespace
       ,-[ file:///test0.st:7:36 ]
       |
     7 |                 b : ARRAY[0..1] OF Nowhere := [(x := 1), (2, 3)];
       |                                    ^^^|^^^
       |                                       `----- no item found for path 'Nowhere'
    ---'
    ");
}

/// Values that name a member make one element, which a repetition repeats:
/// `[3(PT := T#1s)]` gives each instance its PT, as `[3((PT := T#1s))]` does,
/// values without a name following the named ones as in parentheses.
#[rstest]
fn valid_a_repetition_of_member_values(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Timer VAR_INPUT PT : TIME; END_VAR END_FUNCTION_BLOCK
        TYPE
            Point : STRUCT x : INT; y : INT; END_STRUCT;
            Timers : ARRAY[1..3] OF Timer := [3(PT := T#1s)];
        END_TYPE

        FUNCTION F
            VAR
                a : ARRAY[0..1] OF Point := [2(x := 5)];
                b : ARRAY[0..3] OF Point := [2(x := 1, 6), 2(y := 2)];
            END_VAR
        END_FUNCTION
        "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A member's value straight in an array's brackets belongs to no element:
/// it set none and was placed nowhere. In parentheses, it is an element.
#[rstest]
fn invalid_a_member_value_outside_an_element_s_parentheses(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION F
            VAR
                a : ARRAY[0..1] OF Point := [x := 5];
                b : ARRAY[0..1] OF Point := [(x := 5)];
            END_VAR
        END_FUNCTION
        "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0516] Error: member value outside an element's parentheses
       ,-[ file:///test0.st:6:46 ]
       |
     6 |                 a : ARRAY[0..1] OF Point := [x := 5];
       |                                              ^^^|^^
       |                                                 `---- this member's value is outside an element's parentheses
       |
       | Help: write the element in parentheses: (x := ...)
       |
       | Note: an element of an array is one value, its members in parentheses
    ---'
    ");
}
