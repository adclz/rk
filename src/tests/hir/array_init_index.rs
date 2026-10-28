// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! An array initializer's element at its index, `[3] := 7`.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

/// Every form the index takes: a literal, one below zero in an array that
/// starts there, a named CONSTANT, one subscript per dimension; and every
/// value: a scalar, a STRING, a struct, an element that is itself an array.
#[rstest]
fn valid_an_element_at_its_index(mut with_db: RootDatabase) {
    let source = r#"
        TYPE
            Pt : STRUCT x : INT; y : INT; END_STRUCT;
            Row : ARRAY[0..1] OF INT;
            Grid : ARRAY[0..1, 0..2] OF INT := [[1, 2] := 7];
        END_TYPE

        FUNCTION_BLOCK fb
        VAR CONSTANT LAST : INT := 9; END_VAR
        VAR
            a : ARRAY[0..9] OF INT := [[3] := 7, [LAST] := 1];
            b : ARRAY[-5..5] OF INT := [[-2] := 7, [LAST - 4] := 2];
            g : Grid := [[0, 2] := 3, [1, 0] := 4];
            s : ARRAY[0..3] OF Pt := [[2] := (x := 1, y := 2)];
            r : ARRAY[0..3] OF Row := [[1] := [5, 6]];
            t : ARRAY[0..2] OF STRING := [[1] := 'one'];
        END_VAR
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// What follows an element at its index starts after it: a value, a repeat
/// group, and in the flat form of a two-dimensional array the next row. In
/// a two-dimensional array the index is the whole subscript, among rows
/// too, and inside a row it counts the row's dimensions.
#[rstest]
fn valid_what_follows_an_index_starts_after_it(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Grid : ARRAY[0..1, 0..2] OF INT; END_TYPE

        FUNCTION_BLOCK fb
        VAR
            a : ARRAY[0..3] OF INT := [[1] := 4, 5, 1()];
            b : ARRAY[0..5] OF INT := [[1] := 4, 2(0), [5] := 1];
            g : Grid := [[0, 1] := 9, [1, 0] := 8, 7];
            h : Grid := [[1, 2, 3], [[1] := 9]];
            i : Grid := [[1, 2, 3], [1, 0] := 9, [1, 2] := 8];
            j : Grid := [[0, 2] := 1, 2];
        END_VAR
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// Only the `:=` after the `]` tells an index from a row of one element:
/// `[[1], [2]]` is two rows and `[[1] := [2]]` one element at its index.
#[rstest]
fn valid_an_index_is_told_from_a_row_of_one(mut with_db: RootDatabase) {
    let source = r#"
        TYPE One : ARRAY[0..0] OF INT; END_TYPE

        FUNCTION_BLOCK fb
        VAR
            rows : ARRAY[0..2] OF One := [[1], [2]];
            idx : ARRAY[0..2] OF One := [[1] := [2]];
        END_VAR
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn invalid_an_index_outside_the_bounds(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK fb
        VAR
            a : ARRAY[0..9] OF INT := [[10] := 7];
            b : ARRAY[1..9] OF INT := [[0] := 7];
            g : ARRAY[0..1, 0..2] OF INT := [[0, 3] := 7];
        END_VAR
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0506] Error: constant index out of bounds
       ,-[ file:///test0.st:4:41 ]
       |
     4 |             a : ARRAY[0..9] OF INT := [[10] := 7];
       |                                         ^|
       |                                          `-- index 10 is out of bounds (the dimension is declared 0..9)
    ---'
    [E0506] Error: constant index out of bounds
       ,-[ file:///test0.st:5:41 ]
       |
     5 |             b : ARRAY[1..9] OF INT := [[0] := 7];
       |                                         |
       |                                         `-- index 0 is out of bounds (the dimension is declared 1..9)
    ---'
    [E0506] Error: constant index out of bounds
       ,-[ file:///test0.st:6:50 ]
       |
     6 |             g : ARRAY[0..1, 0..2] OF INT := [[0, 3] := 7];
       |                                                  |
       |                                                  `-- index 3 is out of bounds (the dimension is declared 0..2)
       |
       | Note: this error occurred in array dimension 2
    ---'
    ");
}

/// The index names every dimension: one short of it is E0510, whether the
/// value is a scalar or a row, and one past it is E0508.
#[rstest]
fn invalid_an_index_short_of_or_past_the_dimensions(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK fb
        VAR
            g : ARRAY[0..1, 0..2] OF INT := [[1] := 7];
            h : ARRAY[0..1, 0..2] OF INT := [[1] := [4, 5, 6]];
            a : ARRAY[0..9] OF INT := [[1, 2] := 7];
            k : ARRAY[0..1, 0..2] OF INT := [[0, 1, 2] := 5];
        END_VAR
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0510] Error: incomplete subscript
       ,-[ file:///test0.st:4:47 ]
       |
     4 |             g : ARRAY[0..1, 0..2] OF INT := [[1] := 7];
       |                                               |
       |                                               `-- this subscript names 1 of the array's 2 dimensions
       |
       | Help: subscript every dimension, `m[i, j]` or `m[i][j]`, or declare an array of an array type for rows
       |
       | Note: a multi-dimensional array is indexed in all its dimensions
    ---'
    [E0510] Error: incomplete subscript
       ,-[ file:///test0.st:5:47 ]
       |
     5 |             h : ARRAY[0..1, 0..2] OF INT := [[1] := [4, 5, 6]];
       |                                               |
       |                                               `-- this subscript names 1 of the array's 2 dimensions
       |
       | Help: subscript every dimension, `m[i, j]` or `m[i][j]`, or declare an array of an array type for rows
       |
       | Note: a multi-dimensional array is indexed in all its dimensions
    ---'
    [E0508] Error: index into a non-array type
       ,-[ file:///test0.st:6:40 ]
       |
     6 |             a : ARRAY[0..9] OF INT := [[1, 2] := 7];
       |                                        ^^^^^|^^^^^
       |                                             `------- cannot index into type 'INT'
    ---'
    [E0508] Error: index into a non-array type
       ,-[ file:///test0.st:7:46 ]
       |
     7 |             k : ARRAY[0..1, 0..2] OF INT := [[0, 1, 2] := 5];
       |                                              ^^^^^^^|^^^^^^
       |                                                     `-------- cannot index into type 'INT'
    ---'
    ");
}

#[rstest]
fn invalid_an_index_that_is_not_a_constant_integer(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK fb
        VAR
            n : INT;
            a : ARRAY[0..9] OF INT := [[n] := 7];
            b : ARRAY[0..9] OF INT := [[1.5] := 7];
            c : ARRAY[0..9] OF INT := [[TRUE] := 7];
        END_VAR
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0514] Error: initializer index not constant
       ,-[ file:///test0.st:5:41 ]
       |
     5 |             a : ARRAY[0..9] OF INT := [[n] := 7];
       |                                         |
       |                                         `-- the index is not a constant
       |
       | Help: write a literal or a named CONSTANT
       |
       | Note: an element's index in an initializer is resolved before the program runs
    ---'
    [E0308] Error: literal of the wrong kind
       ,-[ file:///test0.st:6:41 ]
       |
     6 |             b : ARRAY[0..9] OF INT := [[1.5] := 7];
       |                                         ^|^
       |                                          `--- invalid DINT literal
       |
       | Note: DINT takes a whole number, written like 42 or 16#2A
    ---'
    [E0504] Error: array index not an integer
       ,-[ file:///test0.st:7:41 ]
       |
     7 |             c : ARRAY[0..9] OF INT := [[TRUE] := 7];
       |                                         ^^|^
       |                                           `--- the index is 'BOOL', not an integer
    ---'
    ");
}

/// A cell written twice: by two indexes, by a value and then an index back
/// over it, by an index onto what followed an earlier one, and in a row of
/// a two-dimensional array.
#[rstest]
fn invalid_an_element_written_twice(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK fb
        VAR
            a : ARRAY[0..9] OF INT := [[3] := 7, [3] := 8];
            b : ARRAY[0..9] OF INT := [1, 2, [0] := 9];
            c : ARRAY[0..9] OF INT := [[1] := 5, 6, [2] := 7];
            g : ARRAY[0..1, 0..2] OF INT := [[1, 2, 3], [[2] := 9, [2] := 8]];
        END_VAR
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0116] Error: duplicate element in an initializer
       ,-[ file:///test0.st:4:50 ]
       |
     4 |             a : ARRAY[0..9] OF INT := [[3] := 7, [3] := 8];
       |                                        ^^^^|^^^  ^^^^|^^^
       |                                            `--------------- element [3] is already initialized here
       |                                                      |
       |                                                      `----- duplicate element [3] in initializer expression
    ---'
    [E0116] Error: duplicate element in an initializer
       ,-[ file:///test0.st:5:46 ]
       |
     5 |             b : ARRAY[0..9] OF INT := [1, 2, [0] := 9];
       |                                        |     ^^^^|^^^
       |                                        `--------------- element [0] is already initialized here
       |                                                  |
       |                                                  `----- duplicate element [0] in initializer expression
    ---'
    [E0116] Error: duplicate element in an initializer
       ,-[ file:///test0.st:6:53 ]
       |
     6 |             c : ARRAY[0..9] OF INT := [[1] := 5, 6, [2] := 7];
       |                                                  |  ^^^^|^^^
       |                                                  `------------ element [2] is already initialized here
       |                                                         |
       |                                                         `----- duplicate element [2] in initializer expression
    ---'
    [E0116] Error: duplicate element in an initializer
       ,-[ file:///test0.st:7:68 ]
       |
     7 |             g : ARRAY[0..1, 0..2] OF INT := [[1, 2, 3], [[2] := 9, [2] := 8]];
       |                                                          ^^^^|^^^  ^^^^|^^^
       |                                                              `--------------- element [1, 2] is already initialized here
       |                                                                        |
       |                                                                        `----- duplicate element [1, 2] in initializer expression
    ---'
    ");
}

/// The count goes on from the index: too many after it is E0507 as before,
/// and a bracket for a scalar element is E0508 as before.
#[rstest]
fn invalid_what_follows_an_index_is_counted(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK fb
        VAR
            a : ARRAY[0..9] OF INT := [[8] := 1, 2, 3];
            b : ARRAY[0..9] OF INT := [[1] := [2]];
        END_VAR
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0507] Error: too many elements in array initializer
       ,-[ file:///test0.st:4:53 ]
       |
     4 |             a : ARRAY[0..9] OF INT := [[8] := 1, 2, 3];
       |                                                     |
       |                                                     `-- the initializer has more elements than the array's 10
    ---'
    [E0508] Error: index into a non-array type
       ,-[ file:///test0.st:5:47 ]
       |
     5 |             b : ARRAY[0..9] OF INT := [[1] := [2]];
       |                                               ^|^
       |                                                `--- cannot index into type 'ARRAY [0..9] OF INT'
    ---'
    ");
}

/// Only an array bracket takes an element at its index: not a repeat group,
/// which would write the cell once per repeat, and not a struct initializer.
/// The value is one element's, so a repeat group is not one either.
#[rstest]
fn invalid_an_index_outside_an_array_bracket(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Pt : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION_BLOCK fb
        VAR
            a : ARRAY[0..9] OF INT := [2([3] := 7)];
            p : Pt := ([1] := 2);
            b : ARRAY[0..9] OF INT := [[1] := 2(7)];
        END_VAR
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0508] Error: index into a non-array type
       ,-[ file:///test0.st:6:42 ]
       |
     6 |             a : ARRAY[0..9] OF INT := [2([3] := 7)];
       |                                          ^|^
       |                                           `--- cannot index into type 'ARRAY [0..9] OF INT'
    ---'
    [E0001] Error: syntax error
       ,-[ file:///test0.st:6:46 ]
       |
     6 |             a : ARRAY[0..9] OF INT := [2([3] := 7)];
       |                                              ^^|^
       |                                                `--- unexpected token(s): ':= 7'
    ---'
    [E0508] Error: index into a non-array type
       ,-[ file:///test0.st:7:24 ]
       |
     7 |             p : Pt := ([1] := 2);
       |                        ^|^
       |                         `--- cannot index into type 'Pt'
    ---'
    [E0001] Error: syntax error
       ,-[ file:///test0.st:7:28 ]
       |
     7 |             p : Pt := ([1] := 2);
       |                            ^^|^
       |                              `--- unexpected token(s): ':= 2'
    ---'
    [E0001] Error: syntax error
       ,-[ file:///test0.st:8:47 ]
       |
     8 |             b : ARRAY[0..9] OF INT := [[1] := 2(7)];
       |                                               |
       |                                               `-- unexpected token(s): '2'
    ---'
    ");
}

/// The value is checked as any element's is.
#[rstest]
fn invalid_the_value_at_an_index(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK fb
        VAR
            n : INT;
            a : ARRAY[0..9] OF INT := [[1] := 'x'];
            b : ARRAY[0..9] OF INT := [[1] := n];
            c : ARRAY[0..9] OF INT (0..5) := [[1] := 9];
        END_VAR
        END_FUNCTION_BLOCK
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0308] Error: literal of the wrong kind
       ,-[ file:///test0.st:5:47 ]
       |
     5 |             a : ARRAY[0..9] OF INT := [[1] := 'x'];
       |                                               ^|^
       |                                                `--- cannot use string literal as INT
    ---'
    [E0401] Error: initial value not constant
       ,-[ file:///test0.st:6:47 ]
       |
     6 |             b : ARRAY[0..9] OF INT := [[1] := n];
       |                                               |
       |                                               `-- this initial value must be a constant: it is fixed before the program runs
       |
       | Help: declare it CONSTANT if its value never changes
       |
       | Note: 'n' is not CONSTANT
    ---'
    [E0702] Error: value outside the subrange
       ,-[ file:///test0.st:7:54 ]
       |
     7 |             c : ARRAY[0..9] OF INT (0..5) := [[1] := 9];
       |                                                      |
       |                                                      `-- value 9 is outside the subrange 0..5
       |
       | Note: the declared range only admits values from 0 to 5
    ---'
    ");
}
