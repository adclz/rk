use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

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
