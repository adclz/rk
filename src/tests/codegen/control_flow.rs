//! Control flow execution tests - IF, CASE, FOR, WHILE, REPEAT.

use crate::tests::codegen::{compile_to_wasm, with_db};
use rstest::*;

#[rstest]
fn test_if_else(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION abs_value : INT
        VAR_INPUT
            x : INT;
        END_VAR
            IF x < 0 THEN
                abs_value := -x;
            ELSE
                abs_value := x;
            END_IF;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);

    let abs_value = instance
        .get_typed_func::<i32, i32>(&mut store, "abs_value")
        .expect("Failed to get function");

    assert_eq!(abs_value.call(&mut store, -5).unwrap(), 5);
    assert_eq!(abs_value.call(&mut store, 3).unwrap(), 3);
    assert_eq!(abs_value.call(&mut store, 0).unwrap(), 0);
}

#[rstest]
fn test_nested_if(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION classify : INT
        VAR_INPUT
            x : INT;
        END_VAR
            IF x > 0 THEN
                IF x > 10 THEN
                    classify := 2;  // Large positive
                ELSE
                    classify := 1;  // Small positive
                END_IF;
            ELSIF x < 0 THEN
                classify := -1;     // Negative
            ELSE
                classify := 0;      // Zero
            END_IF;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);

    let classify = instance
        .get_typed_func::<i32, i32>(&mut store, "classify")
        .expect("Failed to get function");

    assert_eq!(
        classify.call(&mut store, 15).unwrap(),
        2,
        "15 is large positive"
    );
    assert_eq!(
        classify.call(&mut store, 5).unwrap(),
        1,
        "5 is small positive"
    );
    assert_eq!(classify.call(&mut store, -3).unwrap(), -1, "-3 is negative");
    assert_eq!(classify.call(&mut store, 0).unwrap(), 0, "0 is zero");
}

#[rstest]
fn test_case_statement(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION day_type : INT
        VAR_INPUT
            day : INT;
        END_VAR
            CASE day OF
                1, 2, 3, 4, 5:
                    day_type := 1;  // Weekday
                6, 7:
                    day_type := 0;  // Weekend
            ELSE
                day_type := -1;     // Invalid
            END_CASE;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);

    let day_type = instance
        .get_typed_func::<i32, i32>(&mut store, "day_type")
        .expect("Failed to get function");

    assert_eq!(
        day_type.call(&mut store, 1).unwrap(),
        1,
        "Monday is weekday"
    );
    assert_eq!(
        day_type.call(&mut store, 5).unwrap(),
        1,
        "Friday is weekday"
    );
    assert_eq!(
        day_type.call(&mut store, 6).unwrap(),
        0,
        "Saturday is weekend"
    );
    assert_eq!(
        day_type.call(&mut store, 7).unwrap(),
        0,
        "Sunday is weekend"
    );
    assert_eq!(day_type.call(&mut store, 0).unwrap(), -1, "0 is invalid");
    assert_eq!(day_type.call(&mut store, 8).unwrap(), -1, "8 is invalid");
}

#[rstest]
fn test_for_loop(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION sum_to_n : INT
        VAR_INPUT
            n : INT;
        END_VAR
        VAR
            sum : INT;
            i : INT;
        END_VAR
            sum := 0;
            FOR i := 1 TO n DO
                sum := sum + i;
            END_FOR;
            sum_to_n := sum;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);

    let sum_to_n = instance
        .get_typed_func::<i32, i32>(&mut store, "sum_to_n")
        .expect("Failed to get function");

    assert_eq!(sum_to_n.call(&mut store, 5).unwrap(), 15, "1+2+3+4+5 = 15");
    assert_eq!(
        sum_to_n.call(&mut store, 10).unwrap(),
        55,
        "Sum to 10 is 55"
    );
}

#[rstest]
fn test_while_loop(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION power_of_two : INT
        VAR_INPUT
            n : INT;
        END_VAR
        VAR
            result : INT;
            i : INT;
        END_VAR
            result := 1;
            i := 0;
            WHILE i < n DO
                result := result * 2;
                i := i + 1;
            END_WHILE;
            power_of_two := result;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);

    let power_of_two = instance
        .get_typed_func::<i32, i32>(&mut store, "power_of_two")
        .expect("Failed to get function");

    assert_eq!(power_of_two.call(&mut store, 0).unwrap(), 1, "2^0 = 1");
    assert_eq!(power_of_two.call(&mut store, 3).unwrap(), 8, "2^3 = 8");
    assert_eq!(power_of_two.call(&mut store, 5).unwrap(), 32, "2^5 = 32");
}

#[rstest]
fn test_repeat_loop(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION find_divisor : INT
        VAR_INPUT
            n : INT;
        END_VAR
        VAR
            i : INT;
        END_VAR
            i := 2;
            REPEAT
                IF n MOD i = 0 THEN
                    find_divisor := i;
                    RETURN;
                END_IF;
                i := i + 1;
            UNTIL i > n
            END_REPEAT;
            find_divisor := n;  // Prime or 1
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);

    let find_divisor = instance
        .get_typed_func::<i32, i32>(&mut store, "find_divisor")
        .expect("Failed to get function");

    assert_eq!(
        find_divisor.call(&mut store, 15).unwrap(),
        3,
        "15 divisible by 3"
    );
    assert_eq!(find_divisor.call(&mut store, 7).unwrap(), 7, "7 is prime");
    assert_eq!(
        find_divisor.call(&mut store, 12).unwrap(),
        2,
        "12 divisible by 2"
    );
}

/// Descending FOR: a constant negative `BY` flips the exit comparison.
/// Before the fix the loop compared ascending (`ctrl > end`) regardless of
/// direction, so `BY -1` exited before its first iteration.
#[rstest]
fn test_for_descending(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION count_down : INT
        VAR n : INT; i : INT; END_VAR
            FOR i := 10 TO 1 BY -1 DO
                n := n + 1;
            END_FOR;
            count_down := n;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "count_down", ());
    assert_eq!(r, 10, "descending FOR runs all 10 iterations");
}

/// Descending FOR with a step that overshoots the bound: exits on the first
/// counter value strictly below `end`.
#[rstest]
fn test_for_descending_by_two(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION count_down2 : INT
        VAR n : INT; i : INT; END_VAR
            FOR i := 10 TO 1 BY -2 DO
                n := n + 1;
            END_FOR;
            count_down2 := n;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "count_down2", ());
    assert_eq!(r, 5, "10,8,6,4,2 then 0 < 1 exits");
}

/// Ascending FOR whose range is empty must not iterate (guards that the
/// descending fix didn't disturb the ascending exit test).
#[rstest]
fn test_for_empty_ascending_range(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION no_iters : INT
        VAR n : INT; i : INT; END_VAR
            FOR i := 5 TO 1 DO
                n := n + 1;
            END_FOR;
            no_iters := n;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "no_iters", ());
    assert_eq!(r, 0, "start > end with positive step never enters the body");
}

/// LINT (i64-lane) control variable. Before the fix the bound check and
/// increment hardcoded i32 ops, producing a module that failed wasm
/// validation outright.
#[rstest]
fn test_for_lint_control_var(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION lint_sum : LINT
        VAR n : LINT; i : LINT; END_VAR
            FOR i := LINT#1 TO LINT#3 DO
                n := n + i;
            END_FOR;
            lint_sum := n;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i64 = super::execute_wasm(&wasm, "lint_sum", ());
    assert_eq!(r, 6, "i64 FOR loop sums 1+2+3");
}

/// Descending i64 FOR with bounds only representable beyond i32.
#[rstest]
fn test_for_lint_descending_beyond_i32(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION lint_down : LINT
        VAR n : LINT; i : LINT; END_VAR
            FOR i := LINT#5000000002 TO LINT#5000000000 BY LINT#-1 DO
                n := n + 1;
            END_FOR;
            lint_down := n;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i64 = super::execute_wasm(&wasm, "lint_down", ());
    assert_eq!(r, 3, "descending i64 FOR with bounds beyond i32 range");
}

/// Sub-width control variable: the counter increment normalizes like any
/// other arithmetic, so the counter never escapes its type's domain. (An
/// upper bound at the type MAX wraps before the exit check and never
/// terminates — other toolchains-documented behavior — so bounds here stay below it.)
#[rstest]
fn test_for_subwidth_counter_stays_in_domain(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION usint_counter : USINT
        VAR i : USINT; n : USINT; END_VAR
            FOR i := 250 TO 254 DO
                n := n + 1;
            END_FOR;
            IF n <> 5 THEN
                usint_counter := 0;
            ELSE
                usint_counter := i;
            END_IF;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "usint_counter", ());
    assert_eq!(r, 255, "5 iterations and the counter exits in-domain at 255");
}
