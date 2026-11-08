// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

// A bound computes at its type like any expression: `200 * 200` multiplies
// two INTs, -25536, and so does a CONSTANT holding it. Both used to fold in
// 64 bits to 40000. `DINT#200 * 200` is 40000.
#[rstest]
fn invalid_bound_wraps_at_its_type(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR CONSTANT N : INT := 200 * 200; END_VAR
        VAR
            a : ARRAY[0..200 * 200] OF BYTE;
            b : ARRAY[0..N] OF BYTE;
            c : ARRAY[0..DINT#200 * 200] OF BYTE;
        END_VAR
            fn1 := 1;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0503] Error: upper bound below the lower bound
       ,-[ file:///test0.st:5:26 ]
       |
     5 |             a : ARRAY[0..200 * 200] OF BYTE;
       |                          ^^^^|^^^^
       |                              `------ the upper bound -25536 is below the lower bound 0
    ---'
    [E0503] Error: upper bound below the lower bound
       ,-[ file:///test0.st:6:26 ]
       |
     6 |             b : ARRAY[0..N] OF BYTE;
       |                          |
       |                          `-- the upper bound -25536 is below the lower bound 0
    ---'
    ");
}

// A constant subscript is the index the program computes: `K + 1` on a SINT
// K = 127 is -128, outside the array, where the folded 128 was inside it.
#[rstest]
fn invalid_subscript_wraps_at_its_type(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR CONSTANT K : SINT := 127; END_VAR
        VAR a : ARRAY[0..200] OF INT; END_VAR
            fn1 := a[K + 1];
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0506] Error: constant index out of bounds
       ,-[ file:///test0.st:5:22 ]
       |
     5 |             fn1 := a[K + 1];
       |                      ^^|^^
       |                        `---- index -128 is out of bounds (the dimension is declared 0..200)
    ---'
    ");
}

// A STRING length counts bytes: a negative one, written or wrapped, sizes
// nothing. It used to be accepted.
#[rstest]
fn invalid_negative_string_length(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR
            s : STRING[-5];
            t : STRING[200 * 200];
        END_VAR
            fn1 := 1;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0320] Error: negative STRING length
       ,-[ file:///test0.st:4:24 ]
       |
     4 |             s : STRING[-5];
       |                        ^|
       |                         `-- the length is -5: it cannot be negative
    ---'
    [E0320] Error: negative STRING length
       ,-[ file:///test0.st:5:24 ]
       |
     5 |             t : STRING[200 * 200];
       |                        ^^^^|^^^^
       |                            `------ the length is -25536: it cannot be negative
    ---'
    ");
}

// A bit operation is checked as the value it folds to: a subscript past the
// bounds, an upper bound below the lower. An untyped literal has no width, so
// `NOT` flips every bit of the number: `NOT 16#FFF0` is -65521, a negative
// length. None of them folded: the bounds and the length were refused as not
// constant, and the subscript ran past the array.
#[rstest]
fn invalid_a_bit_operation_is_checked_as_its_value(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION fn1 : INT
        VAR
            a : ARRAY[0..9] OF INT;
            b : ARRAY[1..(16#0F AND 16#F0)] OF INT;
            s : STRING[NOT 16#FFF0];
        END_VAR
            a[16#F0 OR 16#0F] := 1;
            fn1 := 1;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0503] Error: upper bound below the lower bound
       ,-[ file:///test0.st:5:26 ]
       |
     5 |             b : ARRAY[1..(16#0F AND 16#F0)] OF INT;
       |                          ^^^^^^^^|^^^^^^^^
       |                                  `---------- the upper bound 0 is below the lower bound 1
    ---'
    [E0320] Error: negative STRING length
       ,-[ file:///test0.st:6:24 ]
       |
     6 |             s : STRING[NOT 16#FFF0];
       |                        ^^^^^|^^^^^
       |                             `------- the length is -65521: it cannot be negative
    ---'
    [E0506] Error: constant index out of bounds
       ,-[ file:///test0.st:8:15 ]
       |
     8 |             a[16#F0 OR 16#0F] := 1;
       |               ^^^^^^^|^^^^^^
       |                      `-------- index 255 is out of bounds (the dimension is declared 0..9)
    ---'
    ");
}

// The check of a REAL initial value walks each CONSTANT once, however often it
// is named. Each of these names the two before it: walked again at each name,
// 20 of them took 1 s to check, and these 60 would not have finished. Checked
// only: lowering the value is another matter.
#[test]
fn a_real_constant_named_twice_is_walked_once() {
    let chain: String = (2..=60)
        .map(|k| format!("f{k} : REAL := f{} + f{};\n", k - 1, k - 2))
        .collect();
    let source = format!(
        "FUNCTION_BLOCK Fb
         VAR CONSTANT f0 : REAL := 0.5; f1 : REAL := 1.5; {chain} END_VAR
         VAR m : REAL := f60; END_VAR
         END_FUNCTION_BLOCK"
    );
    let report = crate::tests::utils::within(std::time::Duration::from_secs(30), move || {
        crate::tests::utils::test_snapshot(&mut RootDatabase::default(), &[&source], |db, file| {
            hir::check::check_file(
                db,
                &hir::hir_ty::calls::CallGraph::of_files(db, [file]),
                file,
            )
        })
    });
    assert_snapshot!(report, @"");
}

/// BOOL logic folds over BOOL literals and CONSTANTs only: a comparison, a
/// variable and a CONSTANT that names itself through another are no
/// constant, and the cycle ends.
#[rstest]
fn invalid_bool_logic_that_does_not_fold(mut with_db: RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR CONSTANT a : BOOL := b; b : BOOL := a; END_VAR
        VAR
            i : INT;
            x : BOOL := a AND TRUE;
            y : BOOL := TRUE AND (1 < 2);
            z : BOOL := TRUE AND (i > 0);
        END_VAR
        END_PROGRAM
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0401] Error: initial value not constant
       ,-[ file:///test0.st:3:34 ]
       |
     3 |         VAR CONSTANT a : BOOL := b; b : BOOL := a; END_VAR
       |                                  |
       |                                  `-- this initial value must be a constant: it is fixed before the program runs
       |
       | Note: 'b' is CONSTANT, but its own value does not fold (a reference cycle, or a non-constant initializer)
    ---'
    [E0401] Error: initial value not constant
       ,-[ file:///test0.st:3:49 ]
       |
     3 |         VAR CONSTANT a : BOOL := b; b : BOOL := a; END_VAR
       |                                                 |
       |                                                 `-- this initial value must be a constant: it is fixed before the program runs
       |
       | Note: 'a' is CONSTANT, but its own value does not fold (a reference cycle, or a non-constant initializer)
    ---'
    [E0401] Error: initial value not constant
       ,-[ file:///test0.st:6:25 ]
       |
     6 |             x : BOOL := a AND TRUE;
       |                         ^^^^^|^^^^
       |                              `------ this initial value must be a constant: it is fixed before the program runs
       |
       | Note: 'a' is CONSTANT, but its own value does not fold (a reference cycle, or a non-constant initializer)
    ---'
    [E0401] Error: initial value not constant
       ,-[ file:///test0.st:7:25 ]
       |
     7 |             y : BOOL := TRUE AND (1 < 2);
       |                         ^^^^^^^^|^^^^^^^
       |                                 `--------- this initial value must be a constant: it is fixed before the program runs
    ---'
    [E0401] Error: initial value not constant
       ,-[ file:///test0.st:8:25 ]
       |
     8 |             z : BOOL := TRUE AND (i > 0);
       |                         ^^^^^^^^|^^^^^^^
       |                                 `--------- this initial value must be a constant: it is fixed before the program runs
       |
       | Help: declare it CONSTANT if its value never changes
       |
       | Note: 'i' is not CONSTANT
    ---'
    ");
}
