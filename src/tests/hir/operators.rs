use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

// A comparison reads one value on each side, and a STRUCT, an ARRAY or an
// instance has none. These passed the check, then stopped the build with an
// internal error.
#[rstest]
fn invalid_comparison_of_aggregates(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Pt : STRUCT x : INT; END_STRUCT; END_TYPE
        FUNCTION_BLOCK Fb END_FUNCTION_BLOCK

        PROGRAM P
        VAR
            p : Pt; q : Pt;
            a : ARRAY[0..1] OF INT; b : ARRAY[0..1] OF INT;
            f : Fb; g : Fb;
            r : BOOL;
        END_VAR
            r := p = q;
            r := a <> b;
            r := f < g;
        END_PROGRAM
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0305] Error: type mismatch
        ,-[ file:///test0.st:12:18 ]
        |
      2 |         TYPE Pt : STRUCT x : INT; END_STRUCT; END_TYPE
        |                   ^^^^^^^^^^^^^|^^^^^^^^^^^^
        |                                `-------------- type is defined by 'Pt' here
        |
     12 |             r := p = q;
        |                  ^^|^^
        |                    `---- operator '=' cannot be applied to type 'Pt'
    ----'
    [E0305] Error: type mismatch
        ,-[ file:///test0.st:13:18 ]
        |
      8 |             a : ARRAY[0..1] OF INT; b : ARRAY[0..1] OF INT;
        |             |
        |             `-- type is declared by variable 'a' here
        |
     13 |             r := a <> b;
        |                  ^^^|^^
        |                     `---- operator '<>' cannot be applied to type 'ARRAY [0..1] OF INT'
    ----'
    [E0305] Error: type mismatch
        ,-[ file:///test0.st:14:18 ]
        |
      3 |         FUNCTION_BLOCK Fb END_FUNCTION_BLOCK
        |                        ^|
        |                         `-- FUNCTION_BLOCK 'Fb' is defined here
        |
     14 |             r := f < g;
        |                  ^^|^^
        |                    `---- operator '<' cannot be applied to type 'Fb'
    ----'
    ");
}

// Two interfaces are compared no more than two instances: it passed the
// check, then stopped the build with an internal error.
#[rstest]
fn invalid_comparison_of_interfaces(mut with_db: RootDatabase) {
    let source = r#"
        INTERFACE I
            METHOD M : INT END_METHOD
        END_INTERFACE

        FUNCTION Same : BOOL
        VAR_IN_OUT i1 : I; i2 : I; END_VAR
            Same := i1 = i2;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0305] Error: type mismatch
       ,-[ file:///test0.st:8:21 ]
       |
     2 |         INTERFACE I
       |                   |
       |                   `-- INTERFACE 'I' is defined here
       |
     8 |             Same := i1 = i2;
       |                     ^^^|^^^
       |                        `----- operator '=' cannot be applied to type 'I'
    ---'
    ");
}

// AND, OR and XOR combine bits, and a float has none to combine. It used to
// build an invalid module.
#[rstest]
fn invalid_logic_on_floats(mut with_db: RootDatabase) {
    let source = r#"
        FUNCTION F : REAL
        VAR a : REAL; b : LREAL; END_VAR
            F := a AND a;
            b := b XOR b;
        END_FUNCTION
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0305] Error: type mismatch
       ,-[ file:///test0.st:4:18 ]
       |
     4 |             F := a AND a;
       |                  ^^^|^^^
       |                     `----- operator 'AND' cannot be applied to type 'REAL'
    ---'
    [E0305] Error: type mismatch
       ,-[ file:///test0.st:5:18 ]
       |
     5 |             b := b XOR b;
       |                  ^^^|^^^
       |                     `----- operator 'XOR' cannot be applied to type 'LREAL'
    ---'
    ");
}

// A sign takes what binary `-` takes. On a BOOL or an enum it made a value
// outside the type; on a STRING or an ARRAY, an internal error.
#[rstest]
fn invalid_sign_on_a_type_without_arithmetic(mut with_db: RootDatabase) {
    let source = r#"
        TYPE Color : (Red, Green); END_TYPE

        PROGRAM P
        VAR
            b : BOOL; e : Color; s : STRING; c : CHAR; d : DATE;
            a : ARRAY[0..1] OF INT;
        END_VAR
            b := -b;
            e := -e;
            s := -s;
            c := -c;
            d := -d;
            a := -a;
        END_PROGRAM
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0305] Error: type mismatch
       ,-[ file:///test0.st:9:18 ]
       |
     6 |             b : BOOL; e : Color; s : STRING; c : CHAR; d : DATE;
       |             |
       |             `-- type is declared by variable 'b' here
       |
     9 |             b := -b;
       |                  ^|
       |                   `-- operator '-' cannot be applied to type 'BOOL'
    ---'
    [E0305] Error: type mismatch
        ,-[ file:///test0.st:10:18 ]
        |
      2 |         TYPE Color : (Red, Green); END_TYPE
        |                      ^^^^^^|^^^^^
        |                            `------- type is defined by 'Color' here
        |
     10 |             e := -e;
        |                  ^|
        |                   `-- operator '-' cannot be applied to type 'Color'
    ----'
    [E0305] Error: type mismatch
        ,-[ file:///test0.st:11:18 ]
        |
      6 |             b : BOOL; e : Color; s : STRING; c : CHAR; d : DATE;
        |                                  |
        |                                  `-- type is declared by variable 's' here
        |
     11 |             s := -s;
        |                  ^|
        |                   `-- operator '-' cannot be applied to type 'STRING'
    ----'
    [E0305] Error: type mismatch
        ,-[ file:///test0.st:12:18 ]
        |
      6 |             b : BOOL; e : Color; s : STRING; c : CHAR; d : DATE;
        |                                              |
        |                                              `-- type is declared by variable 'c' here
        |
     12 |             c := -c;
        |                  ^|
        |                   `-- operator '-' cannot be applied to type 'CHAR'
    ----'
    [E0305] Error: type mismatch
        ,-[ file:///test0.st:13:18 ]
        |
      6 |             b : BOOL; e : Color; s : STRING; c : CHAR; d : DATE;
        |                                                        |
        |                                                        `-- type is declared by variable 'd' here
        |
     13 |             d := -d;
        |                  ^|
        |                   `-- operator '-' cannot be applied to type 'DATE'
    ----'
    [E0305] Error: type mismatch
        ,-[ file:///test0.st:14:18 ]
        |
      7 |             a : ARRAY[0..1] OF INT;
        |             |
        |             `-- type is declared by variable 'a' here
        |
     14 |             a := -a;
        |                  ^|
        |                   `-- operator '-' cannot be applied to type 'ARRAY [0..1] OF INT'
    ----'
    ");
}

// Numbers, bit strings and durations keep their sign, as they keep binary
// `-`.
#[rstest]
fn valid_sign_on_numbers_bit_strings_and_durations(mut with_db: RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR i : INT; u : UINT; r : REAL; w : WORD; t : TIME; END_VAR
            i := -i;
            u := -u;
            r := -r;
            w := -w;
            t := -t;
        END_PROGRAM
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

// An untyped literal under a sign or NOT takes its type from the context,
// and the operator is checked against that type there.
#[rstest]
fn invalid_sign_or_not_given_a_type_without_it(mut with_db: RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR b : BOOL; i : INT; END_VAR
            b := -(1);
            i := NOT 16#0F;
        END_PROGRAM
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0305] Error: type mismatch
       ,-[ file:///test0.st:4:18 ]
       |
     4 |             b := -(1);
       |                  ^^|^
       |                    `--- operator '-' cannot be applied to type 'BOOL'
    ---'
    [E0305] Error: type mismatch
       ,-[ file:///test0.st:5:18 ]
       |
     5 |             i := NOT 16#0F;
       |                  ^^^^|^^^^
       |                      `------ operator 'NOT' cannot be applied to type 'INT'
    ---'
    ");
}

// NOT on an untyped literal takes the bit string its context gives it: the
// mask idiom `w AND NOT 16#0F0F` was refused, the literal forced to BOOL.
#[rstest]
fn valid_not_on_an_untyped_literal(mut with_db: RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR w : WORD; b : BYTE; x : BOOL; END_VAR
            w := w AND NOT 16#0F0F;
            b := NOT 16#0F;
            x := NOT 1;
        END_PROGRAM
    "#;
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}
