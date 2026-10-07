// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! E0322: storage past what a module addresses, whose sizes and addresses are
//! 32 bits, reported where its size first passes them.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

/// An array, a dimension longer than 32 bits, a STRING: each at its own
/// declaration. A variable of a TYPE already reported, an array of it, and
/// a STRUCT holding a part already reported are not reported again.
#[rstest]
fn invalid_storage_reported_where_it_passes(mut with_db: RootDatabase) {
    let source = r#"
TYPE Big : ARRAY[0..4294967296] OF BYTE; END_TYPE

TYPE Long : STRING[5000000000]; END_TYPE

TYPE Holder : STRUCT
    inner : ARRAY[0..599999999] OF LINT;
    flag : BOOL;
END_STRUCT
END_TYPE

FUNCTION f : INT
VAR
    x : Big;
    y : ARRAY[0..1] OF Big;
    h : Holder;
    s : Long;
END_VAR
    f := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0322] Error: storage larger than 4 GiB
       ,-[ file:///test0.st:2:12 ]
       |
     2 | TYPE Big : ARRAY[0..4294967296] OF BYTE; END_TYPE
       |            ^^^^^^^^^^^^^^|^^^^^^^^^^^^^
       |                          `--------------- this array takes 17179869188 bytes
       |
       | Note: a module's sizes and addresses are 32 bits: 4294967295 bytes at most
    ---'
    [E0322] Error: storage larger than 4 GiB
       ,-[ file:///test0.st:4:13 ]
       |
     4 | TYPE Long : STRING[5000000000]; END_TYPE
       |             ^^^^^^^^^|^^^^^^^^
       |                      `---------- this STRING takes 5000000004 bytes
       |
       | Note: a module's sizes and addresses are 32 bits: 4294967295 bytes at most
    ---'
    [E0322] Error: storage larger than 4 GiB
       ,-[ file:///test0.st:7:13 ]
       |
     7 |     inner : ARRAY[0..599999999] OF LINT;
       |             ^^^^^^^^^^^^^|^^^^^^^^^^^^^
       |                          `--------------- this array takes 4800000000 bytes
       |
       | Note: a module's sizes and addresses are 32 bits: 4294967295 bytes at most
    ---'
    ");
}

/// Parts that fit, too large together: a STRUCT, an FB, a CLASS, a PROGRAM
/// and an array of instances, each reported once.
#[rstest]
fn invalid_storage_too_large_together(mut with_db: RootDatabase) {
    let source = r#"
TYPE Pair : STRUCT
    a : ARRAY[0..299999999] OF LINT;
    b : ARRAY[0..299999999] OF LINT;
END_STRUCT
END_TYPE

FUNCTION_BLOCK Twin
VAR
    a : ARRAY[0..299999999] OF LINT;
    b : ARRAY[0..299999999] OF LINT;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK Small
VAR
    a : ARRAY[0..99999999] OF LINT;
END_VAR
END_FUNCTION_BLOCK

CLASS Duo
VAR
    a : ARRAY[0..299999999] OF LINT;
    b : ARRAY[0..299999999] OF LINT;
END_VAR
END_CLASS

PROGRAM Main
VAR
    p : ARRAY[0..299999999] OF LINT;
    q : ARRAY[0..299999999] OF LINT;
END_VAR
END_PROGRAM

PROGRAM Fleet
VAR
    many : ARRAY[0..9] OF Small;
END_VAR
END_PROGRAM
"#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0322] Error: storage larger than 4 GiB
       ,-[ file:///test0.st:2:6 ]
       |
     2 | TYPE Pair : STRUCT
       |      ^^|^
       |        `--- this STRUCT takes 4800000000 bytes
       |
       | Note: a module's sizes and addresses are 32 bits: 4294967295 bytes at most
    ---'
    [E0322] Error: storage larger than 4 GiB
       ,-[ file:///test0.st:8:16 ]
       |
     8 | FUNCTION_BLOCK Twin
       |                ^^|^
       |                  `--- an instance of 'Twin' takes 4800000000 bytes
       |
       | Note: a module's sizes and addresses are 32 bits: 4294967295 bytes at most
    ---'
    [E0322] Error: storage larger than 4 GiB
        ,-[ file:///test0.st:21:7 ]
        |
     21 | CLASS Duo
        |       ^|^
        |        `--- an instance of 'Duo' takes 4800000000 bytes
        |
        | Note: a module's sizes and addresses are 32 bits: 4294967295 bytes at most
    ----'
    [E0322] Error: storage larger than 4 GiB
        ,-[ file:///test0.st:28:9 ]
        |
     28 | PROGRAM Main
        |         ^^|^
        |           `--- the PROGRAM 'Main' takes 4800000000 bytes
        |
        | Note: a module's sizes and addresses are 32 bits: 4294967295 bytes at most
    ----'
    [E0322] Error: storage larger than 4 GiB
        ,-[ file:///test0.st:37:12 ]
        |
     37 |     many : ARRAY[0..9] OF Small;
        |            ^^^^^^^^^^|^^^^^^^^^
        |                      `----------- this array takes 8000000000 bytes
        |
        | Note: a module's sizes and addresses are 32 bits: 4294967295 bytes at most
    ----'
    ");
}
