use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::test_diagnostics;
use crate::tests::utils::with_db;

#[rstest]
fn valid_if_then(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        flag : BOOL;
    END_VAR

    IF flag THEN
        x := 1;
    END_IF;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_if_then_else(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        flag : BOOL;
    END_VAR

    IF flag THEN
        x := 1;
    ELSE
        x := 2;
    END_IF;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_if_elsif_else(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        a : BOOL;
        b : BOOL;
    END_VAR

    IF a THEN
        x := 1;
    ELSIF b THEN
        x := 2;
    ELSE
        x := 3;
    END_IF;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_multiple_elsif(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        a : BOOL;
        b : BOOL;
        c : BOOL;
    END_VAR

    IF a THEN
        x := 1;
    ELSIF b THEN
        x := 2;
    ELSIF c THEN
        x := 3;
    ELSE
        x := 4;
    END_IF;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn invalid_if_condition_not_bool(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
    END_VAR

    IF x THEN
        x := 1;
    END_IF;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:7:8 ]
       |
     7 |     IF x THEN
       |        |
       |        `-- expected 'BOOL', got 'INT'
    ---'
    ");
}

#[rstest]
fn invalid_elsif_condition_not_bool(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        flag : BOOL;
    END_VAR

    IF flag THEN
        x := 1;
    ELSIF x THEN
        x := 2;
    END_IF;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:10:11 ]
        |
     10 |     ELSIF x THEN
        |           |
        |           `-- expected 'BOOL', got 'INT'
    ----'
    ");
}

#[rstest]
fn invalid_multiple_elsif_conditions_not_bool(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        y : REAL;
        flag : BOOL;
    END_VAR

    IF flag THEN
        x := 1;
    ELSIF x THEN
        x := 2;
    ELSIF y THEN
        x := 3;
    END_IF;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:11:11 ]
        |
     11 |     ELSIF x THEN
        |           |
        |           `-- expected 'BOOL', got 'INT'
    ----'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:13:11 ]
        |
     13 |     ELSIF y THEN
        |           |
        |           `-- expected 'BOOL', got 'REAL'
    ----'
    ");
}

#[rstest]
fn invalid_if_and_elsif_conditions_not_bool(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        y : REAL;
    END_VAR

    IF x THEN
        x := 1;
    ELSIF y THEN
        x := 2;
    END_IF;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
       ,-[ file:///test0.st:8:8 ]
       |
     8 |     IF x THEN
       |        |
       |        `-- expected 'BOOL', got 'INT'
    ---'
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:10:11 ]
        |
     10 |     ELSIF y THEN
        |           |
        |           `-- expected 'BOOL', got 'REAL'
    ----'
    ");
}

#[rstest]
fn valid_if_condition_with_comparison(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
    END_VAR

    IF x > 5 THEN
        x := 1;
    ELSIF x < 0 THEN
        x := 0;
    END_IF;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn invalid_type_mismatch_in_elsif_body(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        flag : BOOL;
    END_VAR

    IF flag THEN
        x := 1;
    ELSIF NOT flag THEN
        x := 'hello';
    END_IF;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0306] Error: invalid literal
        ,-[ file:///test0.st:11:14 ]
        |
      4 |         x : INT;
        |         |
        |         `-- type is declared by variable 'x' here
        |
     11 |         x := 'hello';
        |              ^^^|^^^
        |                 `----- cannot infer '<string>' to 'INT': cannot use string literal as INT
    ----'
    ");
}

#[rstest]
fn valid_nested_if_elsif(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        a : BOOL;
        b : BOOL;
    END_VAR

    IF a THEN
        IF b THEN
            x := 1;
        ELSIF NOT b THEN
            x := 2;
        END_IF;
    ELSIF NOT a THEN
        x := 3;
    END_IF;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn valid_elsif_with_string_condition(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION_BLOCK fb1
    VAR
        x : INT;
        s : STRING;
    END_VAR

    IF TRUE THEN
        x := 1;
    ELSIF s THEN
        x := 2;
    END_IF;

END_FUNCTION_BLOCK"#;

    assert_snapshot!(test_diagnostics(&mut with_db, &[source]), @r"
    [E0301] Error: type mismatch
        ,-[ file:///test0.st:10:11 ]
        |
     10 |     ELSIF s THEN
        |           |
        |           `-- expected 'BOOL', got 'STRING'
    ----'
    ");
}
