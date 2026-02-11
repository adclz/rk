//! Control flow code generation tests.

use crate::tests::{compile_to_wasm, validate_wasm, with_db};
use rstest::*;

#[rstest]
fn test_simple_if(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_if : INT
        VAR_INPUT
            condition : BOOL;
        END_VAR
        VAR
            result : INT;
        END_VAR
            result := 0;
            IF condition THEN
                result := 1;
            END_IF;
            test_if := result;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_if_else(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_if_else : INT
        VAR_INPUT
            condition : BOOL;
        END_VAR
        VAR
            result : INT;
        END_VAR
            IF condition THEN
                result := 1;
            ELSE
                result := 2;
            END_IF;
            test_if_else := result;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_if_elsif_else(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_if_elsif_else : INT
        VAR_INPUT
            value : INT;
        END_VAR
        VAR
            result : INT;
        END_VAR
            IF value < 0 THEN
                result := -1;
            ELSIF value > 0 THEN
                result := 1;
            ELSE
                result := 0;
            END_IF;
            test_if_elsif_else := result;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_multiple_elsif(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_multiple_elsif : INT
        VAR_INPUT
            value : INT;
        END_VAR
        VAR
            result : INT;
        END_VAR
            IF value = 1 THEN
                result := 10;
            ELSIF value = 2 THEN
                result := 20;
            ELSIF value = 3 THEN
                result := 30;
            ELSE
                result := 0;
            END_IF;
            test_multiple_elsif := result;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_nested_if(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_nested_if : INT
        VAR_INPUT
            a : BOOL;
            b : BOOL;
        END_VAR
        VAR
            result : INT;
        END_VAR
            result := 0;
            IF a THEN
                IF b THEN
                    result := 1;
                ELSE
                    result := 2;
                END_IF;
            ELSE
                result := 3;
            END_IF;
            test_nested_if := result;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_if_with_early_return(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_early_return : INT
        VAR_INPUT
            condition : BOOL;
        END_VAR
            IF condition THEN
                test_early_return := 1;
                RETURN;
            END_IF;
            test_early_return := 2;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_for_loop(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_for : INT
        VAR
            i : INT;
            sum : INT;
        END_VAR
            sum := 0;
            FOR i := 1 TO 10 DO
                sum := sum + i;
            END_FOR;
            test_for := sum;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_for_loop_with_step(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_for_step : INT
        VAR
            i : INT;
            sum : INT;
        END_VAR
            sum := 0;
            FOR i := 0 TO 10 BY 2 DO
                sum := sum + i;
            END_FOR;
            test_for_step := sum;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_while_loop(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_while : INT
        VAR
            i : INT;
            sum : INT;
        END_VAR
            i := 1;
            sum := 0;
            WHILE i <= 10 DO
                sum := sum + i;
                i := i + 1;
            END_WHILE;
            test_while := sum;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_repeat_loop(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_repeat : INT
        VAR
            i : INT;
            sum : INT;
        END_VAR
            i := 1;
            sum := 0;
            REPEAT
                sum := sum + i;
                i := i + 1;
            UNTIL i > 10
            END_REPEAT;
            test_repeat := sum;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_loop_with_exit(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_exit : INT
        VAR
            i : INT;
            sum : INT;
        END_VAR
            i := 1;
            sum := 0;
            WHILE i <= 100 DO
                sum := sum + i;
                i := i + 1;
                IF i > 10 THEN
                    EXIT;
                END_IF;
            END_WHILE;
            test_exit := sum;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_loop_with_continue(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_continue : INT
        VAR
            i : INT;
            sum : INT;
        END_VAR
            sum := 0;
            FOR i := 1 TO 10 DO
                IF i MOD 2 = 0 THEN
                    CONTINUE;
                END_IF;
                sum := sum + i;
            END_FOR;
            test_continue := sum;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_nested_loops(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_nested : INT
        VAR
            i : INT;
            j : INT;
            sum : INT;
        END_VAR
            sum := 0;
            FOR i := 1 TO 3 DO
                FOR j := 1 TO 3 DO
                    sum := sum + i * j;
                END_FOR;
            END_FOR;
            test_nested := sum;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}
