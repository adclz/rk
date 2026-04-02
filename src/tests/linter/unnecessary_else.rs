use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_lint_diagnostics, with_db};

#[rstest]
fn if_return_then_else(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    IF x > 0 THEN
        test := 1;
        RETURN;
    ELSE
        test := 0;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [W0113] Advice: unnecessary ELSE
        ,-[ file:///test0.st:6:5 ]
        |
      6 | ,->     IF x > 0 THEN
        : :
     11 | |->     END_IF;
        | |
        | `----------------- unnecessary ELSE branch: all preceding branches end with RETURN, EXIT, or CONTINUE
    ----'
    ");
}

#[rstest]
fn if_exit_then_else_in_loop(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    i : INT;
END_VAR
    FOR i := 0 TO 10 DO
        IF i > 5 THEN
            EXIT;
        ELSE
            test := i;
        END_IF;
    END_FOR;
END_FUNCTION
"#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [W0113] Advice: unnecessary ELSE
        ,-[ file:///test0.st:7:9 ]
        |
      7 | ,->         IF i > 5 THEN
        : :
     11 | |->         END_IF;
        | |
        | `--------------------- unnecessary ELSE branch: all preceding branches end with RETURN, EXIT, or CONTINUE
    ----'
    ");
}

#[rstest]
fn if_continue_then_else_in_loop(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    i : INT;
END_VAR
    FOR i := 0 TO 10 DO
        IF i > 5 THEN
            CONTINUE;
        ELSE
            test := i;
        END_IF;
    END_FOR;
END_FUNCTION
"#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [W0113] Advice: unnecessary ELSE
        ,-[ file:///test0.st:7:9 ]
        |
      7 | ,->         IF i > 5 THEN
        : :
     11 | |->         END_IF;
        | |
        | `--------------------- unnecessary ELSE branch: all preceding branches end with RETURN, EXIT, or CONTINUE
    ----'
    ");
}

#[rstest]
fn no_warning_without_exit(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    IF x > 0 THEN
        test := 1;
    ELSE
        test := 0;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn no_warning_without_else(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    IF x > 0 THEN
        RETURN;
    END_IF;
    test := 0;
END_FUNCTION
"#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"");
}

#[rstest]
fn elsif_all_exit_then_else(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    IF x > 10 THEN
        test := 2;
        RETURN;
    ELSIF x > 0 THEN
        test := 1;
        RETURN;
    ELSE
        test := 0;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"
    [W0113] Advice: unnecessary ELSE
        ,-[ file:///test0.st:6:5 ]
        |
      6 | ,->     IF x > 10 THEN
        : :
     14 | |->     END_IF;
        | |
        | `----------------- unnecessary ELSE branch: all preceding branches end with RETURN, EXIT, or CONTINUE
    ----'
    ");
}

#[rstest]
fn no_warning_elsif_does_not_exit(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    x : INT;
END_VAR
    IF x > 10 THEN
        test := 2;
        RETURN;
    ELSIF x > 0 THEN
        test := 1;
    ELSE
        test := 0;
    END_IF;
END_FUNCTION
"#;
    assert_snapshot!(test_lint_diagnostics(&mut with_db, &[source]), @r"");
}
