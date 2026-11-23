// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `VAR_CONSTANT`: the constants of a namespace, written at the top of a file
//! or in a NAMESPACE, which TYPEs and POUs name as they name its TYPEs.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_diagnostics, with_db};

/// A bound and a length name a constant of the file's top level: in a TYPE
/// and in a STRUCT element.
#[rstest]
fn valid_a_bound_names_a_constant_of_its_file(mut with_db: RootDatabase) {
    let source = r#"
VAR_CONSTANT
    ROWS : INT := 10;
    LEN : INT := 32;
END_VAR

TYPE Row : ARRAY[0..ROWS - 1] OF INT; END_TYPE
TYPE Name : STRING[LEN]; END_TYPE
TYPE Pair : STRUCT
    cells : ARRAY[0..ROWS] OF INT;
    label : STRING[LEN];
END_STRUCT; END_TYPE
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A bound names a constant of its namespace, or of a namespace around it.
#[rstest]
fn valid_a_bound_names_a_constant_of_its_namespace(mut with_db: RootDatabase) {
    let source = r#"
NAMESPACE Limits
    VAR_CONSTANT
        LOW : DINT := 2;
        HIGH : DINT := 6;
    END_VAR
    TYPE Span : ARRAY[LOW..HIGH + 2] OF BYTE; END_TYPE
    NAMESPACE Inner
        TYPE Wide : ARRAY[0..HIGH * 2] OF BYTE; END_TYPE
    END_NAMESPACE
END_NAMESPACE
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

/// A constant is a value: named where a type goes, it is refused (E0323),
/// as a FUNCTION is (E0316).
#[rstest]
fn invalid_a_constant_used_as_a_type(mut with_db: RootDatabase) {
    let source = r#"
VAR_CONSTANT
    ROWS : INT := 10;
END_VAR

FUNCTION_BLOCK Grid
VAR
    cells : ROWS;
END_VAR
END_FUNCTION_BLOCK
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0323] Error: constant used as a type
       ,-[ file:///test0.st:8:13 ]
       |
     3 |     ROWS : INT := 10;
       |     ^^|^
       |       `--- 'ROWS' is declared here
       |
     8 |     cells : ROWS;
       |             ^^|^
       |               `--- 'ROWS' is a constant, not a type
    ---'
    ");
}

/// A constant shares its namespace's names with its POUs and TYPEs: named
/// like a FUNCTION or a PROGRAM of the root, like a TYPE of a NAMESPACE, or
/// like another constant there in another file, it is a duplicate (E0102),
/// as two POUs are.
#[rstest]
fn invalid_a_constant_named_like_an_item_of_its_namespace(mut with_db: RootDatabase) {
    let first = r#"
VAR_CONSTANT
    Helper : INT := 1;
    Main : INT := 2;
END_VAR

FUNCTION Helper : INT
    Helper := 0;
END_FUNCTION

PROGRAM Main
END_PROGRAM

NAMESPACE Limits
    TYPE Row : ARRAY[0..2] OF INT; END_TYPE
    VAR_CONSTANT
        Row : INT := 3;
        ROWS : INT := 4;
    END_VAR
END_NAMESPACE
"#;
    let second = r#"
NAMESPACE Limits
    VAR_CONSTANT
        rows : INT := 5;
    END_VAR
END_NAMESPACE
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[first, second]), @r"
    [E0102] Error: duplicate name
       ,-[ file:///test0.st:7:10 ]
       |
     3 |     Helper : INT := 1;
       |     ^^^|^^
       |        `---- constant 'Helper' is already declared here
       |
     7 | FUNCTION Helper : INT
       |          ^^^|^^
       |             `---- FUNCTION 'Helper' has the name of a constant
    ---'
    [E0102] Error: duplicate name
        ,-[ file:///test0.st:11:9 ]
        |
      4 |     Main : INT := 2;
        |     ^^|^
        |       `--- constant 'Main' is already declared here
        |
     11 | PROGRAM Main
        |         ^^|^
        |           `--- PROGRAM 'Main' has the name of a constant
    ----'
    [E0102] Error: duplicate name
        ,-[ file:///test0.st:17:9 ]
        |
     15 |     TYPE Row : ARRAY[0..2] OF INT; END_TYPE
        |          ^|^
        |           `--- TYPE 'Row' is already declared here
        |
     17 |         Row : INT := 3;
        |         ^|^
        |          `--- constant 'Row' has the name of a TYPE
    ----'
    [E0102] Error: duplicate name
        ,-[ file:///test1.st:4:9 ]
        |
      4 |         rows : INT := 5;
        |         ^^|^
        |           `--- constant 'rows' is declared twice
        |
        |-[ file:///test0.st:18:9 ]
        |
     18 |         ROWS : INT := 4;
        |         ^^|^
        |           `--- constant 'ROWS' is already declared here
    ----'
    ");
}

/// A bound is the value of the constant it names: an index past it says so,
/// `ROWS - 1` being 9, `LOW..HIGH + 2` being 2..8, and `HIGH * 2` in a
/// namespace inside being 12.
#[rstest]
fn invalid_an_index_past_a_bound_shows_what_the_constants_are(mut with_db: RootDatabase) {
    let source = r#"
VAR_CONSTANT
    ROWS : INT := 10;
END_VAR

NAMESPACE Limits
    VAR_CONSTANT
        LOW : DINT := 2;
        HIGH : DINT := 6;
    END_VAR
    TYPE Span : ARRAY[LOW..HIGH + 2] OF BYTE; END_TYPE
    NAMESPACE Inner
        TYPE Wide : ARRAY[0..HIGH * 2] OF BYTE; END_TYPE
    END_NAMESPACE
END_NAMESPACE

TYPE Row : ARRAY[0..ROWS - 1] OF INT; END_TYPE

FUNCTION Past : INT
VAR
    r : Row;
    s : Limits.Span;
    w : Limits.Inner.Wide;
END_VAR
    r[10] := 1;
    s[9] := 1;
    w[13] := 1;
    Past := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0506] Error: constant index out of bounds
        ,-[ file:///test0.st:25:7 ]
        |
     25 |     r[10] := 1;
        |       ^|
        |        `-- index 10 is out of bounds (the dimension is declared 0..9)
    ----'
    [E0506] Error: constant index out of bounds
        ,-[ file:///test0.st:26:7 ]
        |
     26 |     s[9] := 1;
        |       |
        |       `-- index 9 is out of bounds (the dimension is declared 2..8)
    ----'
    [E0506] Error: constant index out of bounds
        ,-[ file:///test0.st:27:7 ]
        |
     27 |     w[13] := 1;
        |       ^|
        |        `-- index 13 is out of bounds (the dimension is declared 0..12)
    ----'
    ");
}

/// A constant whose value reads itself, directly or through another, is
/// refused where it is declared (E0408), and a bound naming it says
/// nothing more.
#[rstest]
fn invalid_a_constant_defined_by_its_own_value(mut with_db: RootDatabase) {
    let source = r#"
VAR_CONSTANT
    A : INT := A + 1;
END_VAR

NAMESPACE Limits
    VAR_CONSTANT
        B : INT := C * 2;
        C : INT := B;
    END_VAR
    TYPE Col : ARRAY[0..B] OF INT; END_TYPE
END_NAMESPACE

TYPE Row : ARRAY[0..A] OF INT; END_TYPE
TYPE Name : STRING[A]; END_TYPE
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0408] Error: constant defined by its own value
       ,-[ file:///test0.st:3:5 ]
       |
     3 |     A : INT := A + 1;
       |     |
       |     `-- 'A' is defined by its own value
       |
       | Note: a constant's value is computed before the program runs, from the constants it names
    ---'
    [E0408] Error: constant defined by its own value
       ,-[ file:///test0.st:8:9 ]
       |
     8 |         B : INT := C * 2;
       |         |
       |         `-- 'B' is defined by its own value, through 'C'
     9 |         C : INT := B;
       |         |
       |         `-- 'C' is declared here
       |
       | Note: a constant's value is computed before the program runs, from the constants it names
    ---'
    [E0408] Error: constant defined by its own value
       ,-[ file:///test0.st:9:9 ]
       |
     8 |         B : INT := C * 2;
       |         |
       |         `-- 'B' is declared here
     9 |         C : INT := B;
       |         |
       |         `-- 'C' is defined by its own value, through 'B'
       |
       | Note: a constant's value is computed before the program runs, from the constants it names
    ---'
    ");
}

/// A constant's value is computed before the program runs: one that calls
/// a FUNCTION is refused, as a TYPE's default would be.
#[rstest]
fn invalid_a_constant_whose_value_is_no_constant(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION Helper : INT
    Helper := 1;
END_FUNCTION

VAR_CONSTANT
    N : INT := Helper();
END_VAR
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0401] Error: initial value not constant
       ,-[ file:///test0.st:7:16 ]
       |
     7 |     N : INT := Helper();
       |                ^^^^|^^^
       |                    `----- this initial value must be a constant: it is fixed before the program runs
       |
       | Note: a call is not a constant
    ---'
    ");
}
