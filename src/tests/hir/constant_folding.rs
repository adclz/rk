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
    [E0503] Error: invalid array bounds
       ,-[ file:///test0.st:5:26 ]
       |
     5 |             a : ARRAY[0..200 * 200] OF BYTE;
       |                          ^^^^|^^^^
       |                              `------ the upper bound -25536 is below the lower bound 0
    ---'
    [E0503] Error: invalid array bounds
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
    [E0506] Error: invalid array access
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
    [E0320] Error: length is negative
       ,-[ file:///test0.st:4:24 ]
       |
     4 |             s : STRING[-5];
       |                        ^|
       |                         `-- a STRING length cannot be negative, and this one is -5
    ---'
    [E0320] Error: length is negative
       ,-[ file:///test0.st:5:24 ]
       |
     5 |             t : STRING[200 * 200];
       |                        ^^^^|^^^^
       |                            `------ a STRING length cannot be negative, and this one is -25536
    ---'
    ");
}
