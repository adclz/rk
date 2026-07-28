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
    [E0609] Error: invalid array access
       ,-[ file:///test0.st:7:15 ]
       |
     7 |             a[r] := 1;
       |               |
       |               `-- array index must be an integer, found REAL
    ---'
    [E0609] Error: invalid array access
       ,-[ file:///test0.st:8:15 ]
       |
     8 |             a[TRUE] := 2;
       |               ^^|^
       |                 `--- array index must be an integer, found BOOL
    ---'
    [E0309] Error: invalid literal
       ,-[ file:///test0.st:9:15 ]
       |
     9 |             a[1.5] := 3;
       |               ^|^
       |                `--- cannot infer '<float>' to 'DINT': invalid DINT literal
    ---'
    [E0609] Error: invalid array access
        ,-[ file:///test0.st:10:15 ]
        |
     10 |             a['x'] := 4;
        |               ^|^
        |                `--- array index must be an integer, found STRING
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
    [E0204] Error: no item found in scope
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
