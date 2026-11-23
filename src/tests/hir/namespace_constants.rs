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
        LOW, HIGH : DINT := 4;
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
    [E0102] Error: duplicate POU
        ,-[ file:///test0.st:4:5 ]
        |
      4 |     Main : INT := 2;
        |     ^^|^
        |       `--- duplicate POU 'Main'
        |
     11 | PROGRAM Main
        |         ^^|^
        |           `--- POU 'Main' is also declared here
    ----'
    [E0102] Error: duplicate POU
       ,-[ file:///test0.st:7:10 ]
       |
     3 |     Helper : INT := 1;
       |     ^^^|^^
       |        `---- POU 'Helper' is already declared here
       |
     7 | FUNCTION Helper : INT
       |          ^^^|^^
       |             `---- duplicate POU 'Helper'
    ---'
    [E0102] Error: duplicate POU
        ,-[ file:///test0.st:11:9 ]
        |
      4 |     Main : INT := 2;
        |     ^^|^
        |       `--- POU 'Main' is also declared here
        |
     11 | PROGRAM Main
        |         ^^|^
        |           `--- duplicate POU 'Main'
    ----'
    [E0102] Error: duplicate POU
        ,-[ file:///test0.st:17:9 ]
        |
     15 |     TYPE Row : ARRAY[0..2] OF INT; END_TYPE
        |          ^|^
        |           `--- POU 'Row' is already declared here
        |
     17 |         Row : INT := 3;
        |         ^|^
        |          `--- duplicate POU 'Row'
    ----'
    [E0102] Error: duplicate POU
        ,-[ file:///test1.st:4:9 ]
        |
      4 |         rows : INT := 5;
        |         ^^|^
        |           `--- duplicate POU 'rows'
        |
        |-[ file:///test0.st:18:9 ]
        |
     18 |         ROWS : INT := 4;
        |         ^^|^
        |           `--- POU 'ROWS' is already declared here
    ----'
    ");
}
