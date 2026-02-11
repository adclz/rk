//! PROGRAM POU tests.

use crate::tests::{compile_to_wasm, execute_wasm, validate_wasm, with_db};
use rstest::*;

#[rstest]
fn test_simple_program(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM MyProgram
        VAR
            counter : INT := 0;
        END_VAR
            counter := counter + 1;
        END_PROGRAM
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    // Programs are void functions
    execute_wasm::<(), ()>(&wasm_bytes, "MyProgram", ());
}

#[rstest]
fn test_program_with_multiple_variables(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM MyProgram
        VAR
            x : INT := 10;
            y : INT := 20;
            sum : INT;
        END_VAR
            sum := x + y;
        END_PROGRAM
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    execute_wasm::<(), ()>(&wasm_bytes, "MyProgram", ());
}

#[rstest]
fn test_program_with_control_flow(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM MyProgram
        VAR
            x : INT := 5;
            result : INT;
        END_VAR
            IF x > 0 THEN
                result := x * 2;
            ELSE
                result := 0;
            END_IF
        END_PROGRAM
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    execute_wasm::<(), ()>(&wasm_bytes, "MyProgram", ());
}

#[rstest]
fn test_program_with_fb_instance(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Counter
        VAR
            count : INT := 0;
        END_VAR
        METHOD Increment : INT
            THIS.count := THIS.count + 1;
            Increment := THIS.count;
        END_METHOD
        END_FUNCTION_BLOCK

        PROGRAM MyProgram
        VAR
            myCounter : Counter;
            result : INT;
        END_VAR
            result := myCounter.Increment();
        END_PROGRAM
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    // Write WASM bytes to file for inspection with wasm2wat
    std::fs::write("test_program_with_fb_instance.wasm", &wasm_bytes)
        .expect("Failed to write WASM file");
    println!("\n✓ WASM bytes written to: test_program_with_fb_instance.wasm");
    println!("  Run: wasm2wat test_program_with_fb_instance.wasm -o test_program_with_fb_instance.wat\n");

    execute_wasm::<(), ()>(&wasm_bytes, "MyProgram", ());
}

#[rstest]
fn test_program_multiple_method_calls(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Accumulator
        VAR
            total : INT := 0;
        END_VAR
        METHOD Add : INT
        VAR_INPUT
            value : INT;
        END_VAR
            THIS.total := THIS.total + value;
            Add := THIS.total;
        END_METHOD
        METHOD GetTotal : INT
            GetTotal := THIS.total;
        END_METHOD
        END_FUNCTION_BLOCK

        PROGRAM MyProgram
        VAR
            acc : Accumulator;
            result1 : INT;
            result2 : INT;
            final_total : INT;
        END_VAR
            result1 := acc.Add(10);
            result2 := acc.Add(20);
            final_total := acc.GetTotal();
        END_PROGRAM
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    execute_wasm::<(), ()>(&wasm_bytes, "MyProgram", ());
}
