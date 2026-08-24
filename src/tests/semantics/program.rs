use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn assign_mismatch_type(mut with_db: RootDatabase) {
    let source = r#"
PROGRAM prog
    VAR
        test: INT;
    END_VAR

    test := ULINT#5;

END_PROGRAM"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:7:13 ]
       |
     4 |         test: INT;
       |         ^^|^
       |           `--- type is declared by variable 'test' here
       |
     7 |     test := ULINT#5;
       |             ^^^|^^^
       |                `----- expected 'INT', got 'ULINT'
       |                |
       |                `----- consider explicitly casting with 'ULINT_TO_INT(ULINT#5)'
       |
       | Help: insert explicit cast 'ULINT_TO_INT(ULINT#5)'
    ---'
    ");
}
