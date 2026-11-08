// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

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
    [E0203] Error: unknown name in a namespace
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
    [E0501] Error: invalid array lower bound
       ,-[ file:///test0.st:3:34 ]
       |
     3 |         VAR x : INT; arr : ARRAY[x..10] OF INT; END_VAR
       |                                  |
       |                                  `-- the lower bound is not an integer constant
    ---'
    ");
}

#[rstest]
fn upper_bound_below_lower_bound_is_rejected(mut with_db: RootDatabase) {
    // `0..-10` now FOLDS (both are valid constants); what is wrong is the
    // ordering, so it is reported as an inferior upper bound (E0503) rather
    // than an unevaluatable value.
    let source = r#"
        TYPE
            List: ARRAY[0..-10] OF INT;
        END_TYPE
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0503] Error: upper bound below the lower bound
       ,-[ file:///test0.st:3:28 ]
       |
     3 |             List: ARRAY[0..-10] OF INT;
       |                            ^|^
       |                             `--- the upper bound -10 is below the lower bound 0
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
    [E0503] Error: upper bound below the lower bound
       ,-[ file:///test0.st:3:29 ]
       |
     3 |             List: ARRAY[10..1] OF INT;
       |                             |
       |                             `-- the upper bound 1 is below the lower bound 10
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

/// A subscript is an ordinary expression — arithmetic, calls, nesting. These
/// used to type as `Never` (the path walk never descended into them) and MIR
/// refused to lower any compound subscript at all.
#[rstest]
fn valid_compound_subscript_expressions(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION idx : DINT
            idx := 1;
        END_FUNCTION

        FUNCTION fn1 : DINT
        VAR
            a : ARRAY[0..9] OF DINT;
            b : ARRAY[0..9] OF DINT;
            n : DINT;
            i : INT;
        END_VAR
            a[n + 1] := 1;
            a[i * 2] := 2;
            a[n + i] := 3;
            a[idx()] := 4;
            a[b[n]] := 5;
            a[(n + 1) * 2] := 6;
            fn1 := a[n - 1];
        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn invalid_non_integer_subscript(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : DINT
        VAR
            a : ARRAY[0..9] OF DINT;
            r : REAL;
        END_VAR
            a[r] := 1;
            a[TRUE] := 2;
            a[1.5] := 3;
            a['x'] := 4;
        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0504] Error: array index not an integer
       ,-[ file:///test0.st:7:15 ]
       |
     7 |             a[r] := 1;
       |               |
       |               `-- the index is 'REAL', not an integer
    ---'
    [E0504] Error: array index not an integer
       ,-[ file:///test0.st:8:15 ]
       |
     8 |             a[TRUE] := 2;
       |               ^^|^
       |                 `--- the index is 'BOOL', not an integer
    ---'
    [E0308] Error: literal of the wrong kind
       ,-[ file:///test0.st:9:15 ]
       |
     9 |             a[1.5] := 3;
       |               ^|^
       |                `--- invalid DINT literal
       |
       | Note: DINT takes a whole number, written like 42 or 16#2A
    ---'
    [E0308] Error: literal of the wrong kind
        ,-[ file:///test0.st:10:15 ]
        |
     10 |             a['x'] := 4;
        |               ^|^
        |                `--- cannot use string literal as DINT
    ----'
    ");
}

/// A name that does not resolve inside a subscript must be reported — before
/// subscripts were inferred at all, `a[zz + 1]` passed `check` silently and
/// then failed in MIR lowering.
#[rstest]
fn invalid_unresolved_name_in_subscript(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : DINT
        VAR
            a : ARRAY[0..9] OF DINT;
        END_VAR
            a[zz + 1] := 1;
        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0201] Error: unknown name
       ,-[ file:///test0.st:6:15 ]
       |
     6 |             a[zz + 1] := 1;
       |               ^|
       |                `-- no item 'zz' found in scope
    ---'
    ");
}

/// Compound subscripts reach body inference through every path-resolution
/// entry, not just plain variable targets: an FB-call target
/// (`fbs[n + 1]()`) resolves through the call machinery, not the
/// assignment-target walk, and must type its subscripts all the same.
#[rstest]
fn valid_compound_subscripts_on_call_paths(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Inner
        VAR_INPUT x : DINT; END_VAR
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Outer
        VAR
            fbs : ARRAY[0..3] OF Inner;
            n : DINT;
        END_VAR
            fbs[n + 1](x := 2);
            fbs[n * 2].x := 3;
        END_FUNCTION_BLOCK
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A CONSTANT subscript outside the declared bounds is provable at compile
/// time — rejected here (E0506) instead of deferred to the runtime bounds
/// check. Per dimension, negative bounds respected, both subscript
/// spellings; boundary values stay silent.
#[rstest]
fn invalid_constant_subscript_out_of_bounds(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR
            a : ARRAY[0..2] OF INT;
            n : ARRAY[-2..2] OF INT;
            m : ARRAY[1..3, 1..3] OF INT;
        END_VAR
            a[5] := 1;
            a[-1] := 2;
            n[-3] := 3;
            m[1][9] := 4;
            m[9][1] := 5;
        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0506] Error: constant index out of bounds
       ,-[ file:///test0.st:8:15 ]
       |
     8 |             a[5] := 1;
       |               |
       |               `-- index 5 is out of bounds (the dimension is declared 0..2)
    ---'
    [E0506] Error: constant index out of bounds
       ,-[ file:///test0.st:9:15 ]
       |
     9 |             a[-1] := 2;
       |               ^|
       |                `-- index -1 is out of bounds (the dimension is declared 0..2)
    ---'
    [E0506] Error: constant index out of bounds
        ,-[ file:///test0.st:10:15 ]
        |
     10 |             n[-3] := 3;
        |               ^|
        |                `-- index -3 is out of bounds (the dimension is declared -2..2)
    ----'
    [E0506] Error: constant index out of bounds
        ,-[ file:///test0.st:11:18 ]
        |
     11 |             m[1][9] := 4;
        |                  |
        |                  `-- index 9 is out of bounds (the dimension is declared 1..3)
        |
        | Note: this error occurred in array dimension 2
    ----'
    [E0506] Error: constant index out of bounds
        ,-[ file:///test0.st:12:15 ]
        |
     12 |             m[9][1] := 5;
        |               |
        |               `-- index 9 is out of bounds (the dimension is declared 1..3)
    ----'
    ");
}

/// In-bounds constants — including both boundaries and negative lower
/// bounds — are silent, and a non-constant subscript is never judged here.
/// A constant subscript is checked against the dimension it indexes: a
/// chained bracket continues a multi-dimensional array, `t[1][2][3]`, or
/// indexes the element it reached from its first dimension, `r[1][2]` of an
/// array of rows. Each was checked against another dimension than its own.
#[rstest]
fn valid_constant_subscripts_down_a_chain(mut with_db: RootDatabase) {
    let source = r#"
        TYPE RowT : ARRAY[0..2] OF INT; END_TYPE
        TYPE Mat : ARRAY[0..1, 0..2] OF INT; END_TYPE
        FUNCTION f : INT
        VAR
            t : ARRAY[0..1, 0..2, 0..3] OF INT;
            r : ARRAY[0..1] OF RowT;
            x : ARRAY[0..1] OF Mat;
        END_VAR
            t[1][2][3] := 1;
            r[1][2] := 2;
            x[1][0][2] := 3;
            f := t[1, 2][3] + r[1][2] + x[1][1][2];
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// Past a row's own bounds, a constant subscript of the row is refused with
/// them, the dimension named the row's.
#[rstest]
fn invalid_constant_subscript_out_of_a_rows_bounds(mut with_db: RootDatabase) {
    let source = r#"
        TYPE RowT : ARRAY[0..2] OF INT; END_TYPE
        FUNCTION f : INT
        VAR r : ARRAY[0..1] OF RowT; END_VAR
            r[1][3] := 2;
            f := 1;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0506] Error: constant index out of bounds
       ,-[ file:///test0.st:5:18 ]
       |
     5 |             r[1][3] := 2;
       |                  |
       |                  `-- index 3 is out of bounds (the dimension is declared 0..2)
    ---'
    ");
}

/// A path ending in a subscript that leaves dimensions of a multi-dimensional
/// array is refused (E0510): `m[1]` of an `ARRAY[0..1, 0..2]` was typed as the
/// whole array, so `h.m[1] := m2` copied a whole matrix from row 1 on, over
/// what follows it. A further bracket still indexes it.
#[rstest]
fn invalid_incomplete_subscript(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Holder : STRUCT
            m : ARRAY[0..1, 0..2] OF INT;
            guard : ARRAY[0..2] OF INT;
        END_STRUCT; END_TYPE
        FUNCTION f : INT
        VAR
            h : Holder;
            m2 : ARRAY[0..1, 0..2] OF INT;
            row : ARRAY[0..2] OF INT;
        END_VAR
            h.m[1] := m2;
            row := h.m[1];
            f := h.m[1][2];
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0510] Error: incomplete subscript
        ,-[ file:///test0.st:12:17 ]
        |
     12 |             h.m[1] := m2;
        |                 |
        |                 `-- this subscript names 1 of the array's 2 dimensions
        |
        | Help: subscript every dimension, `m[i, j]` or `m[i][j]`, or declare an array of an array type for rows
        |
        | Note: a multi-dimensional array is indexed in all its dimensions
    ----'
    [E0510] Error: incomplete subscript
        ,-[ file:///test0.st:13:24 ]
        |
     13 |             row := h.m[1];
        |                        |
        |                        `-- this subscript names 1 of the array's 2 dimensions
        |
        | Help: subscript every dimension, `m[i, j]` or `m[i][j]`, or declare an array of an array type for rows
        |
        | Note: a multi-dimensional array is indexed in all its dimensions
    ----'
    ");
}

#[rstest]
fn valid_boundary_subscripts_are_silent(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR
            a : ARRAY[0..2] OF INT;
            n : ARRAY[-2..2] OF INT;
            i : INT;
        END_VAR
            a[0] := 1;
            a[2] := 2;
            n[-2] := 3;
            n[2] := 4;
            a[i] := 5;
        END_FUNCTION
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

// Bounds are constant EXPRESSIONS, not literals: a CONSTANT variable and
// folding arithmetic are legal for arrays and subranges alike.
#[rstest]
fn valid_constant_and_folding_bounds(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR CONSTANT K : INT := 3; END_VAR
        VAR
            a : ARRAY[0..K] OF INT;
            b : ARRAY[0..2 + 2] OF INT;
            x : INT (0..K);
            y : INT (0..2 + 2);
        END_VAR
            a[0] := 1;
            fn1 := b[0] + x + y;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn invalid_array_bound_not_constant(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR n : INT; a : ARRAY[0..n] OF INT; END_VAR
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0502] Error: invalid array upper bound
       ,-[ file:///test0.st:3:35 ]
       |
     3 |         VAR n : INT; a : ARRAY[0..n] OF INT; END_VAR
       |                                   |
       |                                   `-- the upper bound is not an integer constant
    ---'
    ");
}

// The initializer-length check reads the FOLDED bounds, so a CONSTANT-bounded
// array still rejects an oversized initializer.
#[rstest]
fn invalid_too_many_elements_with_constant_bound(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR CONSTANT K : INT := 2; END_VAR
        VAR a : ARRAY[0..K] OF INT := [1, 2, 3, 4]; END_VAR
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0507] Error: too many elements in array initializer
       ,-[ file:///test0.st:4:49 ]
       |
     4 |         VAR a : ARRAY[0..K] OF INT := [1, 2, 3, 4]; END_VAR
       |                                                 |
       |                                                 `-- the initializer has more elements than the array's 3
    ---'
    ");
}

// The compile-time subscript check folds both sides: a CONSTANT subscript
// against a CONSTANT bound is still caught before the runtime guard.
#[rstest]
fn invalid_constant_subscript_out_of_constant_bound(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR CONSTANT K : INT := 2; END_VAR
        VAR a : ARRAY[0..K] OF INT; END_VAR
            fn1 := a[K + 1];
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0506] Error: constant index out of bounds
       ,-[ file:///test0.st:5:22 ]
       |
     5 |             fn1 := a[K + 1];
       |                      ^^|^^
       |                        `---- index 3 is out of bounds (the dimension is declared 0..2)
    ---'
    ");
}

// Two separately declared ARRAY[-1..1] types are the same type. `as_range`
// refused every negative bound, so these were never mutually assignable.
#[rstest]
fn valid_negative_bounds_are_assignable(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION consume : INT
        VAR_INPUT a : ARRAY[-1..1] OF INT; END_VAR
            consume := a[-1] + a[1];
        END_FUNCTION
        FUNCTION test : INT
        VAR b : ARRAY[-1..1] OF INT; END_VAR
            b[-1] := 1;
            b[1] := 2;
            test := consume(a := b);
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

/// A STRING element's capacity is its size, so arrays of `STRING[4]` and of
/// `STRING` are two types: copied one into the other, the elements moved at
/// the wrong stride.
#[rstest]
fn arrays_of_strings_of_two_capacities_differ(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION TakeWide : INT VAR_INPUT a : ARRAY[0..1] OF STRING; END_VAR TakeWide := 0; END_FUNCTION
FUNCTION f : INT
VAR a4 : ARRAY[0..1] OF STRING[4]; a80 : ARRAY[0..1] OF STRING; b4 : ARRAY[0..1] OF STRING[4]; END_VAR
    a80 := a4;
    a4 := b4;
    f := TakeWide(a4);
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:5:12 ]
       |
     4 | VAR a4 : ARRAY[0..1] OF STRING[4]; a80 : ARRAY[0..1] OF STRING; b4 : ARRAY[0..1] OF STRING[4]; END_VAR
       |                                    ^|^
       |                                     `--- 'a80' is declared here
     5 |     a80 := a4;
       |            ^|
       |             `-- expected 'ARRAY [0..1] OF STRING', got 'ARRAY [0..1] OF STRING[4]'
    ---'
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:7:19 ]
       |
     2 | FUNCTION TakeWide : INT VAR_INPUT a : ARRAY[0..1] OF STRING; END_VAR TakeWide := 0; END_FUNCTION
       |                                   |
       |                                   `-- 'a' is declared here
       |
     7 |     f := TakeWide(a4);
       |                   ^|
       |                    `-- expected 'ARRAY [0..1] OF STRING', got 'ARRAY [0..1] OF STRING[4]'
    ---'
    ");
}

/// A repetition past the array's end is E0507, and nothing more: the
/// flattened leaves the lowering reads are made only for an initializer
/// the walk accepted. Expanded count by count, this one took the checker
/// down at two gigabytes.
#[rstest]
fn invalid_repeat_count_beyond_the_array(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
VAR
    x : ARRAY[0..2] OF INT := [15532559262904483838(0)];
    y : ARRAY[0..2] OF INT := [4(1)];
END_VAR
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0507] Error: too many elements in array initializer
       ,-[ file:///test0.st:4:32 ]
       |
     4 |     x : ARRAY[0..2] OF INT := [15532559262904483838(0)];
       |                                ^^^^^^^^^^^|^^^^^^^^^^^
       |                                           `------------- the initializer has more elements than the array's 3
    ---'
    [E0507] Error: too many elements in array initializer
       ,-[ file:///test0.st:5:32 ]
       |
     5 |     y : ARRAY[0..2] OF INT := [4(1)];
       |                                ^^|^
       |                                  `--- the initializer has more elements than the array's 3
    ---'
    ");
}

/// A STRUCT element's array is checked as a variable's: its bounds were
/// not, and `ARRAY[5..1]` inside a STRUCT went unreported.
#[rstest]
fn invalid_reversed_bound_in_a_struct_element(mut with_db: RootDatabase) {
    let source = r#"
TYPE S : STRUCT
    a : ARRAY[5..1] OF INT;
END_STRUCT
END_TYPE
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0503] Error: upper bound below the lower bound
       ,-[ file:///test0.st:3:18 ]
       |
     3 |     a : ARRAY[5..1] OF INT;
       |                  |
       |                  `-- the upper bound 1 is below the lower bound 5
    ---'
    ");
}

/// A report names an index's type as it was declared, at each place an index is
/// checked. Normalized, an enum read 'ENUM', a struct 'STRUCT' and an alias its target.
#[rstest]
#[case::an_enum("", "a[c] := 1;", "[E0504]", "the index is 'Color', not an integer")]
#[case::a_struct("", "a[s] := 1;", "[E0504]", "the index is 'Pt', not an integer")]
#[case::an_alias("", "a[r] := 1;", "[E0504]", "the index is 'Ratio', not an integer")]
#[case::an_array_type("", "a[g] := 1;", "[E0504]", "the index is 'Grid', not an integer")]
#[case::a_name(
    "",
    "a[Color#Red] := 1;",
    "[E0504]",
    "the index is 'Color#Red', not an integer"
)]
#[case::a_bit_string_base("", "a[fl] := 1;", "[E0504]", "the index is 'Flags', not an integer")]
#[case::a_pack_by_an_enum("", "F := ...v[c];", "[E0819]", "the index is 'Color', not an integer")]
#[case::a_pack_by_a_struct("", "F := ...v[s];", "[E0819]", "the index is 'Pt', not an integer")]
#[case::a_pack_by_an_alias("", "F := ...v[r];", "[E0819]", "the index is 'Ratio', not an integer")]
#[case::a_pack_by_a_name(
    "",
    "F := ...v[Color#Red];",
    "[E0819]",
    "the index is 'Color#Red', not an integer"
)]
#[case::an_initial_by_an_enum(
    "b : ARRAY[0..3] OF INT := [[kc] := 7];",
    "",
    "[E0504]",
    "the index is 'Color', not an integer"
)]
#[case::an_initial_by_a_struct(
    "b : ARRAY[0..3] OF INT := [[ks] := 7];",
    "",
    "[E0504]",
    "the index is 'Pt', not an integer"
)]
#[case::an_initial_by_an_alias(
    "b : ARRAY[0..3] OF INT := [[kr] := 7];",
    "",
    "[E0504]",
    "the index is 'Ratio', not an integer"
)]
#[case::an_initial_by_a_name(
    "b : ARRAY[0..3] OF INT := [[Color#Red] := 7];",
    "",
    "[E0504]",
    "the index is 'Color#Red', not an integer"
)]
// An integer to each of them: what is wrong with this one is that it is no constant.
#[case::an_initial_by_a_number_base(
    "b : ARRAY[0..3] OF INT := [[m] := 7];",
    "",
    "[E0514]",
    "the index is not a constant"
)]
fn invalid_an_index_is_named_as_declared(
    mut with_db: RootDatabase,
    #[case] decl: &str,
    #[case] body: &str,
    #[case] code: &str,
    #[case] message: &str,
) {
    let source = format!(
        r#"TYPE
    Color : (Red, Green);
    Pt : STRUCT x : INT; END_STRUCT;
    Ratio : REAL;
    Grid : ARRAY[0..1] OF INT;
    Flags : WORD (FA := 1);
    Light : INT (Stop := 1);
END_TYPE

FUNCTION F : INT
VAR_INPUT v : INT...; END_VAR
VAR CONSTANT
    kc : Color := Color#Red;
    ks : Pt := (x := 1);
    kr : Ratio := 1.0;
END_VAR
VAR
    a : ARRAY[0..3] OF INT;
    c : Color;
    s : Pt;
    r : Ratio;
    g : Grid;
    fl : Flags;
    m : Light;
    {decl}
END_VAR
    {body}
END_FUNCTION
"#
    );
    let report = test_diagnostics(&mut with_db, &[&source]);
    assert!(report.contains(code), "{report}");
    assert!(report.contains(message), "{report}");
}

/// What an element is made of is checked, under a reference and an `ARRAY[*]` too.
/// These passed `rk check` and built, with the length or the range ignored.
#[rstest]
fn invalid_what_an_element_is_made_of(mut with_db: RootDatabase) {
    let source = r#"
TYPE
    Alias : ARRAY[0..1] OF STRING[-1];
    Holder : STRUCT member : ARRAY[0..1] OF INT (5..1); END_STRUCT;
END_TYPE

FUNCTION F
VAR_IN_OUT conformand : ARRAY[*] OF STRING[-1]; END_VAR
VAR
    n : INT;
    range : ARRAY[0..1] OF INT (5..1);
    length : ARRAY[0..1] OF STRING[-1];
    not_constant : ARRAY[0..1] OF STRING[n];
    reference : REF_TO ARRAY[5..1] OF INT;
    referenced_element : REF_TO ARRAY[0..1] OF STRING[-1];
END_VAR
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0320] Error: negative STRING length
       ,-[ file:///test0.st:3:35 ]
       |
     3 |     Alias : ARRAY[0..1] OF STRING[-1];
       |                                   ^|
       |                                    `-- the length is -1: it cannot be negative
    ---'
    [E0705] Error: subrange bounds reversed
       ,-[ file:///test0.st:4:53 ]
       |
     4 |     Holder : STRUCT member : ARRAY[0..1] OF INT (5..1); END_STRUCT;
       |                                                     |
       |                                                     `-- the upper bound 1 is below the lower bound 5
    ---'
    [E0320] Error: negative STRING length
       ,-[ file:///test0.st:8:44 ]
       |
     8 | VAR_IN_OUT conformand : ARRAY[*] OF STRING[-1]; END_VAR
       |                                            ^|
       |                                             `-- the length is -1: it cannot be negative
    ---'
    [E0705] Error: subrange bounds reversed
        ,-[ file:///test0.st:11:36 ]
        |
     11 |     range : ARRAY[0..1] OF INT (5..1);
        |                                    |
        |                                    `-- the upper bound 1 is below the lower bound 5
    ----'
    [E0320] Error: negative STRING length
        ,-[ file:///test0.st:12:36 ]
        |
     12 |     length : ARRAY[0..1] OF STRING[-1];
        |                                    ^|
        |                                     `-- the length is -1: it cannot be negative
    ----'
    [E0315] Error: STRING length not constant
        ,-[ file:///test0.st:13:42 ]
        |
     13 |     not_constant : ARRAY[0..1] OF STRING[n];
        |                                          |
        |                                          `-- the length is not a compile-time constant
    ----'
    [E0503] Error: upper bound below the lower bound
        ,-[ file:///test0.st:14:33 ]
        |
     14 |     reference : REF_TO ARRAY[5..1] OF INT;
        |                                 |
        |                                 `-- the upper bound 1 is below the lower bound 5
    ----'
    [E0320] Error: negative STRING length
        ,-[ file:///test0.st:15:55 ]
        |
     15 |     referenced_element : REF_TO ARRAY[0..1] OF STRING[-1];
        |                                                       ^|
        |                                                        `-- the length is -1: it cannot be negative
    ----'
    ");
}

/// A field an element lacks is missing from the element, wherever the initializer
/// reaches it. Named after the array, it offered no field to correct it to.
#[rstest]
fn invalid_a_field_an_element_lacks(mut with_db: RootDatabase) {
    let source = r#"
TYPE
    Engine : STRUCT fuel : INT; END_STRUCT;
    Holder : STRUCT rows : ARRAY[0..1] OF Engine := [(fuell := 1)]; END_STRUCT;
END_TYPE

FUNCTION_BLOCK Motor
VAR_INPUT rpm : INT; END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK F
VAR
    list : ARRAY[0..1] OF Engine := [(fuell := 1)];
    matrix : ARRAY[0..1, 0..1] OF Engine := [[(fuell := 1)]];
    again : ARRAY[0..1] OF Engine := [2((fuell := 1))];
    index : ARRAY[0..1] OF Engine := [[1] := (fuell := 1)];
    motors : ARRAY[0..1] OF Motor := [(rpmm := 1)];
    held : Holder := (rows := [(fuell := 1)]);
    inner : STRUCT rows : ARRAY[0..1] OF Engine; END_STRUCT := (rows := [(fuell := 1)]);
    right : ARRAY[0..1] OF Engine := [(fuel := 1)];
END_VAR
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0202] Error: unknown field
       ,-[ file:///test0.st:4:55 ]
       |
     4 |     Holder : STRUCT rows : ARRAY[0..1] OF Engine := [(fuell := 1)]; END_STRUCT;
       |                                                       ^^^^^|^^^^
       |                                                            `------ 'Engine' has no field named 'fuell'
       |
       | Note: 'Engine' has a field with a similar name:
       |       - fuel
    ---'
    [E0202] Error: unknown field
        ,-[ file:///test0.st:13:39 ]
        |
     13 |     list : ARRAY[0..1] OF Engine := [(fuell := 1)];
        |                                       ^^^^^|^^^^
        |                                            `------ 'Engine' has no field named 'fuell'
        |
        | Note: 'Engine' has a field with a similar name:
        |       - fuel
    ----'
    [E0202] Error: unknown field
        ,-[ file:///test0.st:14:48 ]
        |
     14 |     matrix : ARRAY[0..1, 0..1] OF Engine := [[(fuell := 1)]];
        |                                                ^^^^^|^^^^
        |                                                     `------ 'Engine' has no field named 'fuell'
        |
        | Note: 'Engine' has a field with a similar name:
        |       - fuel
    ----'
    [E0202] Error: unknown field
        ,-[ file:///test0.st:15:42 ]
        |
     15 |     again : ARRAY[0..1] OF Engine := [2((fuell := 1))];
        |                                          ^^^^^|^^^^
        |                                               `------ 'Engine' has no field named 'fuell'
        |
        | Note: 'Engine' has a field with a similar name:
        |       - fuel
    ----'
    [E0202] Error: unknown field
        ,-[ file:///test0.st:16:47 ]
        |
     16 |     index : ARRAY[0..1] OF Engine := [[1] := (fuell := 1)];
        |                                               ^^^^^|^^^^
        |                                                    `------ 'Engine' has no field named 'fuell'
        |
        | Note: 'Engine' has a field with a similar name:
        |       - fuel
    ----'
    [E0202] Error: unknown field
        ,-[ file:///test0.st:17:40 ]
        |
     17 |     motors : ARRAY[0..1] OF Motor := [(rpmm := 1)];
        |                                        ^^^^|^^^^
        |                                            `------ 'Motor' has no field named 'rpmm'
        |
        | Note: 'Motor' has a field with a similar name:
        |       - rpm
    ----'
    [E0202] Error: unknown field
        ,-[ file:///test0.st:18:33 ]
        |
     18 |     held : Holder := (rows := [(fuell := 1)]);
        |                                 ^^^^^|^^^^
        |                                      `------ 'Engine' has no field named 'fuell'
        |
        | Note: 'Engine' has a field with a similar name:
        |       - fuel
    ----'
    [E0202] Error: unknown field
        ,-[ file:///test0.st:19:75 ]
        |
     19 |     inner : STRUCT rows : ARRAY[0..1] OF Engine; END_STRUCT := (rows := [(fuell := 1)]);
        |                                                                           ^^^^^|^^^^
        |                                                                                `------ 'Engine' has no field named 'fuell'
        |
        | Note: 'Engine' has a field with a similar name:
        |       - fuel
    ----'
    ");
}
