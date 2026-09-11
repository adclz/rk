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
    [E0203] Error: no namespace item found
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
    [E0501] Error: invalid array bounds
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
    // ordering, so it is reported as an inferior upper bound (E0503) rather
    // than an unevaluatable value.
    let source = r#"
        TYPE
            List: ARRAY[0..-10] OF INT;
        END_TYPE
        "#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0503] Error: invalid array bounds
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
    [E0503] Error: invalid array bounds
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
    [E0509] Error: syntax
       ,-[ file:///test0.st:4:14 ]
       |
     4 |             A: ARRAY [*] OF INT;
       |              ^^^^^^^^^|^^^^^^^^
       |                       `---------- array conformands (ARRAY[*]) are not supported
    ---'
    ");
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
    [E0504] Error: invalid array access
       ,-[ file:///test0.st:7:15 ]
       |
     7 |             a[r] := 1;
       |               |
       |               `-- array index must be an integer, found REAL
    ---'
    [E0504] Error: invalid array access
       ,-[ file:///test0.st:8:15 ]
       |
     8 |             a[TRUE] := 2;
       |               ^^|^
       |                 `--- array index must be an integer, found BOOL
    ---'
    [E0306] Error: invalid literal
       ,-[ file:///test0.st:9:15 ]
       |
     9 |             a[1.5] := 3;
       |               ^|^
       |                `--- cannot infer '<float>' to 'DINT': invalid DINT literal
    ---'
    [E0306] Error: invalid literal
        ,-[ file:///test0.st:10:15 ]
        |
     10 |             a['x'] := 4;
        |               ^|^
        |                `--- cannot infer '<string>' to 'DINT': cannot use string literal as DINT
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

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r#"
    [E0201] Error: no item found in scope
       ,-[ file:///test0.st:6:15 ]
       |
     6 |             a[zz + 1] := 1;
       |               ^|
       |                `-- no item "zz" found in scope
    ---'
    "#);
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
    [E0506] Error: invalid array access
       ,-[ file:///test0.st:8:15 ]
       |
     8 |             a[5] := 1;
       |               |
       |               `-- index 5 is out of bounds (the dimension is declared 0..2)
    ---'
    [E0506] Error: invalid array access
       ,-[ file:///test0.st:9:15 ]
       |
     9 |             a[-1] := 2;
       |               ^|
       |                `-- index -1 is out of bounds (the dimension is declared 0..2)
    ---'
    [E0506] Error: invalid array access
        ,-[ file:///test0.st:10:15 ]
        |
     10 |             n[-3] := 3;
        |               ^|
        |                `-- index -3 is out of bounds (the dimension is declared -2..2)
    ----'
    [E0506] Error: invalid array access
        ,-[ file:///test0.st:11:18 ]
        |
     11 |             m[1][9] := 4;
        |                  |
        |                  `-- index 9 is out of bounds (the dimension is declared 1..3)
        |
        | Note: this error occurred in array dimension 2
    ----'
    [E0506] Error: invalid array access
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
    [E0502] Error: invalid array bounds
       ,-[ file:///test0.st:3:35 ]
       |
     3 |         VAR n : INT; a : ARRAY[0..n] OF INT; END_VAR
       |                                   |
       |                                   `-- invalid upper bound value for ARRAY
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
    [E0507] Error: invalid array access
       ,-[ file:///test0.st:4:49 ]
       |
     4 |         VAR a : ARRAY[0..K] OF INT := [1, 2, 3, 4]; END_VAR
       |                                                 |
       |                                                 `-- too many elements in array initializer (expected at most 3)
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
    [E0506] Error: invalid array access
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
