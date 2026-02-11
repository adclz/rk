//! CASE statement code generation tests.

use crate::tests::{compile_to_wasm, validate_wasm, with_db};
use rstest::*;

#[rstest]
fn test_case_statement(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_case : INT
        VAR_INPUT
            value : INT;
        END_VAR
        VAR
            result : INT;
        END_VAR
            CASE value OF
                1: result := 10;
                2: result := 20;
                3: result := 30;
            ELSE
                result := 0;
            END_CASE;
            test_case := result;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_case_with_multiple_values(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_case_multi : INT
        VAR_INPUT
            value : INT;
        END_VAR
        VAR
            result : INT;
        END_VAR
            CASE value OF
                1, 2, 3: result := 100;
                4, 5: result := 200;
                6: result := 300;
            ELSE
                result := 0;
            END_CASE;
            test_case_multi := result;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_case_without_else(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_case_no_else : INT
        VAR_INPUT
            value : INT;
        END_VAR
        VAR
            result : INT;
        END_VAR
            result := 0;
            CASE value OF
                1: result := 10;
                2: result := 20;
                3: result := 30;
            END_CASE;
            test_case_no_else := result;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_case_with_ranges(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_case_ranges : INT
        VAR_INPUT
            value : INT;
        END_VAR
        VAR
            result : INT;
        END_VAR
            CASE value OF
                1..10: result := 1;
                11..20: result := 2;
                21..30: result := 3;
            ELSE
                result := 0;
            END_CASE;
            test_case_ranges := result;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_simple_nested_case(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_simple : INT
        VAR_INPUT
            x : INT;
        END_VAR
            CASE x OF
                1:
                    CASE x OF
                        1: test_simple := 1;
                    END_CASE;
            END_CASE;
            test_simple := 0;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}

#[rstest]
fn test_nested_case(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_nested_case : INT
        VAR_INPUT
            a : INT;
            b : INT;
        END_VAR
        VAR
            result : INT;
        END_VAR
            CASE a OF
                1:
                    CASE b OF
                        1: result := 11;
                        2: result := 12;
                    ELSE
                        result := 10;
                    END_CASE;
                2:
                    CASE b OF
                        1: result := 21;
                        2: result := 22;
                    ELSE
                        result := 20;
                    END_CASE;
            ELSE
                result := 0;
            END_CASE;
            test_nested_case := result;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
}
