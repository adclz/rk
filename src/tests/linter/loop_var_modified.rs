use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use crate::tests::utils::{test_single_lint, with_db};

#[rstest]
fn simple_modification(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    i : INT;
END_VAR
    FOR i := 0 TO 10 DO
        i := i + 2;
    END_FOR;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "loop-var-modified"), @r"
    [L0308] Warning: loop variable modified in body
       ,-[ file:///test0.st:7:9 ]
       |
     6 |     FOR i := 0 TO 10 DO
       |         |
       |         `-- 'i' is used here as control variable
     7 |         i := i + 2;
       |         |
       |         `-- loop variable 'i' is modified inside the loop body
       |
       | Note: lint rule: loop-var-modified
    ---'
    ");
}

#[rstest]
fn modification_in_if(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    i : INT;
END_VAR
    FOR i := 0 TO 10 DO
        IF i > 5 THEN
            i := 10;
        END_IF;
    END_FOR;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "loop-var-modified"), @r"
    [L0308] Warning: loop variable modified in body
       ,-[ file:///test0.st:8:13 ]
       |
     6 |     FOR i := 0 TO 10 DO
       |         |
       |         `-- 'i' is used here as control variable
       |
     8 |             i := 10;
       |             |
       |             `-- loop variable 'i' is modified inside the loop body
       |
       | Note: lint rule: loop-var-modified
    ---'
    ");
}

#[rstest]
fn no_modification(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    i : INT;
    x : INT;
END_VAR
    FOR i := 0 TO 10 DO
        x := i * 2;
    END_FOR;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "loop-var-modified"), @r"");
}

#[rstest]
fn different_var_in_nested_for(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    i : INT;
    j : INT;
END_VAR
    FOR i := 0 TO 10 DO
        FOR j := 0 TO 5 DO
            j := j + 1;
        END_FOR;
    END_FOR;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "loop-var-modified"), @r"
    [L0308] Warning: loop variable modified in body
       ,-[ file:///test0.st:9:13 ]
       |
     8 |         FOR j := 0 TO 5 DO
       |             |
       |             `-- 'j' is used here as control variable
     9 |             j := j + 1;
       |             |
       |             `-- loop variable 'j' is modified inside the loop body
       |
       | Note: lint rule: loop-var-modified
    ---'
    ");
}

#[rstest]
fn outer_var_modified_in_nested_for(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    i : INT;
    j : INT;
END_VAR
    FOR i := 0 TO 10 DO
        FOR j := 0 TO 5 DO
            i := 99;
        END_FOR;
    END_FOR;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "loop-var-modified"), @r"
    [L0308] Warning: loop variable modified in body
       ,-[ file:///test0.st:9:13 ]
       |
     7 |     FOR i := 0 TO 10 DO
       |         |
       |         `-- 'i' is used here as control variable
       |
     9 |             i := 99;
       |             |
       |             `-- loop variable 'i' is modified inside the loop body
       |
       | Note: lint rule: loop-var-modified
    ---'
    ");
}

#[rstest]
fn modification_inside_while_within_for(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    i : INT;
    done : BOOL;
END_VAR
    FOR i := 0 TO 10 DO
        WHILE NOT done DO
            i := i + 1;
        END_WHILE;
    END_FOR;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "loop-var-modified"), @r"
    [L0308] Warning: loop variable modified in body
       ,-[ file:///test0.st:9:13 ]
       |
     7 |     FOR i := 0 TO 10 DO
       |         |
       |         `-- 'i' is used here as control variable
       |
     9 |             i := i + 1;
       |             |
       |             `-- loop variable 'i' is modified inside the loop body
       |
       | Note: lint rule: loop-var-modified
    ---'
    ");
}

#[rstest]
fn modification_inside_repeat_within_for(mut with_db: RootDatabase) {
    let source = r#"
FUNCTION test : INT
VAR
    i : INT;
END_VAR
    FOR i := 0 TO 10 DO
        REPEAT
            i := i + 1;
        UNTIL i > 5
        END_REPEAT;
    END_FOR;
END_FUNCTION
"#;
    assert_snapshot!(test_single_lint(&mut with_db, &[source], "loop-var-modified"), @r"
    [L0308] Warning: loop variable modified in body
       ,-[ file:///test0.st:8:13 ]
       |
     6 |     FOR i := 0 TO 10 DO
       |         |
       |         `-- 'i' is used here as control variable
       |
     8 |             i := i + 1;
       |             |
       |             `-- loop variable 'i' is modified inside the loop body
       |
       | Note: lint rule: loop-var-modified
    ---'
    ");
}
