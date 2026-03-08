use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn valid_case_int(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        y : INT;
    END_VAR

    CASE x OF
        1: y := 10;
        2: y := 20;
        3: y := 30;
    ELSE
        y := 0;
    END_CASE;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_case_with_subrange(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        y : INT;
    END_VAR

    CASE x OF
        1..5: y := 10;
        6..10: y := 20;
    ELSE
        y := 0;
    END_CASE;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_case_with_multiple_labels(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        y : INT;
    END_VAR

    CASE x OF
        1, 2, 3: y := 10;
        4, 5, 6: y := 20;
    ELSE
        y := 0;
    END_CASE;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn invalid_case_label_type_mismatch(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        y : INT;
    END_VAR

    CASE x OF
        'hello': y := 10;
    ELSE
        y := 0;
    END_CASE;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0302] Error: type mismatch
       ,-[ file:///test0.st:9:9 ]
       |
     4 |         x : INT;
       |         |
       |         `-- type is declared by variable 'x' here
       |
     9 |         'hello': y := 10;
       |         ^^^|^^^
       |            `----- can't compare 'INT' with 'STRING'
    ---'
    ");
}

#[rstest]
fn valid_case_string_condition(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : STRING;
        y : INT;
    END_VAR

    CASE x OF
        'a': y := 1;
    ELSE
        y := 0;
    END_CASE;

END_FUNCTION_BLOCK"#;

    // NOTE: IEC 61131-3 only allows ordinal types for CASE conditions,
    // but we don't validate this yet — STRING passes without error.
    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @"");
}

#[rstest]
fn invalid_case_subrange_type_mismatch(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        y : INT;
    END_VAR

    CASE x OF
        'a'..'z': y := 10;
    ELSE
        y := 0;
    END_CASE;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0302] Error: type mismatch
       ,-[ file:///test0.st:9:9 ]
       |
     4 |         x : INT;
       |         |
       |         `-- type is declared by variable 'x' here
       |
     9 |         'a'..'z': y := 10;
       |         ^|^
       |          `--- can't compare 'INT' with 'STRING'
    ---'
    [E0302] Error: type mismatch
       ,-[ file:///test0.st:9:14 ]
       |
     4 |         x : INT;
       |         |
       |         `-- type is declared by variable 'x' here
       |
     9 |         'a'..'z': y := 10;
       |              ^|^
       |               `--- can't compare 'INT' with 'STRING'
    ---'
    ");
}

#[rstest]
fn valid_case_with_body_type_check(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        y : INT;
    END_VAR

    CASE x OF
        1: y := 10;
        2: y := 'bad';
    ELSE
        y := 0;
    END_CASE;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:10:17 ]
        |
      5 |         y : INT;
        |         |
        |         `-- type is declared by variable 'y' here
        |
     10 |         2: y := 'bad';
        |                 ^^|^^
        |                   `---- expected 'INT', got 'STRING'
    ----'
    ");
}

#[rstest]
fn valid_case_nested(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        y : INT;
        z : INT;
    END_VAR

    CASE x OF
        1:
            CASE y OF
                10: z := 100;
                20: z := 200;
            ELSE
                z := 0;
            END_CASE;
        2: z := 2;
    ELSE
        z := -1;
    END_CASE;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_case_mixed_labels_and_subranges(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        y : INT;
    END_VAR

    CASE x OF
        1, 2, 10..20: y := 10;
        3: y := 30;
    ELSE
        y := 0;
    END_CASE;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_case_with_variable_label(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        y : INT;
        LIMIT : INT := 100;
    END_VAR

    CASE x OF
        LIMIT: y := 1;
    ELSE
        y := 0;
    END_CASE;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn invalid_case_else_body_type_mismatch(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        y : INT;
    END_VAR

    CASE x OF
        1: y := 10;
    ELSE
        y := 'wrong';
    END_CASE;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:11:14 ]
        |
      5 |         y : INT;
        |         |
        |         `-- type is declared by variable 'y' here
        |
     11 |         y := 'wrong';
        |              ^^^|^^^
        |                 `----- expected 'INT', got 'STRING'
    ----'
    ");
}
