use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn invalid_start_value(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        I: INT;
        O: BOOL;
    END_VAR

    FOR I := O TO 10 DO

    END_FOR;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:8:14 ]
       |
     4 |         I: INT;
       |         |
       |         `-- type is declared by variable 'I' here
       |
     8 |     FOR I := O TO 10 DO
       |              |
       |              `-- expected 'INT', got 'BOOL'
       |              |
       |              `-- consider explicitly casting with 'BOOL_TO_INT(O)'
       |
       | Help: insert explicit cast 'BOOL_TO_INT(O)'
    ---'
    ");
}

#[rstest]
fn invalid_stop_value(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        I: INT;
        O: BOOL;
    END_VAR

    FOR I := 10 TO O DO

    END_FOR;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0302] Error: type mismatch
       ,-[ file:///test0.st:8:20 ]
       |
     4 |         I: INT;
       |         |
       |         `-- type is declared by variable 'I' here
       |
     8 |     FOR I := 10 TO O DO
       |                    |
       |                    `-- can't compare 'INT' with 'BOOL'
       |                    |
       |                    `-- consider explicitly casting with 'BOOL_TO_INT(O)'
       |
       | Help: insert explicit cast 'BOOL_TO_INT(O)'
    ---'
    ");
}

#[rstest]
fn invalid_step_value(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        I: INT;
        O: BOOL;
    END_VAR

    FOR I := 0 TO 10 BY O DO

    END_FOR;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0302] Error: type mismatch
       ,-[ file:///test0.st:8:25 ]
       |
     4 |         I: INT;
       |         |
       |         `-- type is declared by variable 'I' here
       |
     8 |     FOR I := 0 TO 10 BY O DO
       |                         |
       |                         `-- can't compare 'INT' with 'BOOL'
       |                         |
       |                         `-- consider explicitly casting with 'BOOL_TO_INT(O)'
       |
       | Help: insert explicit cast 'BOOL_TO_INT(O)'
    ---'
    ");
}

#[rstest]
fn while_condition_is_not_a_bool(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        I: INT;
        O: BOOL;
    END_VAR

    WHILE I DO

    END_WHILE;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:8:11 ]
       |
     8 |     WHILE I DO
       |           |
       |           `-- expected 'BOOL', got 'INT'
    ---'
    ");
}

#[rstest]
fn repeat_condition_is_not_a_bool(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        I: INT;
        O: BOOL;
    END_VAR

    REPEAT O := TRUE;
        UNTIL I
    END_REPEAT;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:9:15 ]
       |
     9 |         UNTIL I
       |               |
       |               `-- expected 'BOOL', got 'INT'
    ---'
    ");
}
