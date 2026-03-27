//! WASM execution tests (actually running the generated code).

use crate::tests::{compile_to_wasm, with_db};
use rstest::*;
use wasmtime::{Engine, Instance, Module, Store};

#[rstest]
fn test_execute_simple_arithmetic(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION add : INT
        VAR_INPUT
            a : INT;
            b : INT;
        END_VAR
            add := a + b;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm_bytes).expect("Failed to create module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("Failed to instantiate");

    let add_func = instance
        .get_typed_func::<(i32, i32), i32>(&mut store, "add")
        .expect("Failed to get function");

    let result = add_func
        .call(&mut store, (5, 3))
        .expect("Failed to call function");
    assert_eq!(result, 8, "5 + 3 should equal 8");
}

#[rstest]
fn test_execute_factorial(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION factorial : INT
        VAR_INPUT
            n : INT;
        END_VAR
            IF n <= 1 THEN
                factorial := 1;
            ELSE
                factorial := n * factorial(n - 1);
            END_IF;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm_bytes).expect("Failed to create module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("Failed to instantiate");

    let factorial_func = instance
        .get_typed_func::<i32, i32>(&mut store, "factorial")
        .expect("Failed to get function");

    let result = factorial_func
        .call(&mut store, 5)
        .expect("Failed to call function");
    assert_eq!(result, 120, "5! should equal 120");
}

#[rstest]
fn test_execute_chained_calls(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION double : INT
        VAR_INPUT
            x : INT;
        END_VAR
            double := x * 2;
        END_FUNCTION

        FUNCTION add_one : INT
        VAR_INPUT
            x : INT;
        END_VAR
            add_one := x + 1;
        END_FUNCTION

        FUNCTION process : INT
        VAR_INPUT
            x : INT;
        END_VAR
            process := double(add_one(x));
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm_bytes).expect("Failed to create module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("Failed to instantiate");

    let process_func = instance
        .get_typed_func::<i32, i32>(&mut store, "process")
        .expect("Failed to get function");

    let result = process_func
        .call(&mut store, 5)
        .expect("Failed to call function");
    assert_eq!(result, 12, "double(add_one(5)) = double(6) = 12");
}

#[rstest]
fn test_execute_implicit_cast_int_to_real(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION int_to_real : REAL
        VAR_INPUT
            x : INT;
        END_VAR
        VAR
            result : REAL;
        END_VAR
            result := x;  // Implicit cast INT -> REAL
            int_to_real := result;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm_bytes).expect("Failed to create module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("Failed to instantiate");

    let func = instance
        .get_typed_func::<i32, f32>(&mut store, "int_to_real")
        .expect("Failed to get function");

    let result = func.call(&mut store, 42).expect("Failed to call function");
    assert_eq!(result, 42.0, "INT 42 should cast to REAL 42.0");
}

#[rstest]
fn test_execute_implicit_cast_in_arithmetic(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION mixed_arithmetic : REAL
        VAR_INPUT
            x : INT;
        END_VAR
        VAR
            a : REAL;
            result : REAL;
        END_VAR
            a := 2.5;
            result := a * x;  // Implicit cast x from INT -> REAL
            mixed_arithmetic := result;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm_bytes).expect("Failed to create module");
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).expect("Failed to instantiate");

    let func = instance
        .get_typed_func::<i32, f32>(&mut store, "mixed_arithmetic")
        .expect("Failed to get function");

    let result = func.call(&mut store, 4).expect("Failed to call function");
    assert_eq!(result, 10.0, "2.5 * 4 should equal 10.0");
}

#[rstest]
fn test_default_int_param(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION add_default : INT
        VAR_INPUT
            a: INT;
            b: INT := 10;
        END_VAR
            add_default := a + b;
        END_FUNCTION

        FUNCTION test_default : INT
            test_default := add_default(a := 5);
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();
    let func = instance
        .get_typed_func::<(), i32>(&mut store, "test_default")
        .unwrap();
    let result = func.call(&mut store, ()).unwrap();
    assert_eq!(result, 15, "5 + default(10) = 15");
}

#[rstest]
fn test_default_param_override(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION add_default : INT
        VAR_INPUT
            a: INT;
            b: INT := 10;
        END_VAR
            add_default := a + b;
        END_FUNCTION

        FUNCTION test_override : INT
            test_override := add_default(a := 5, b := 20);
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = wasmtime::Instance::new(&mut store, &module, &[]).unwrap();
    let func = instance
        .get_typed_func::<(), i32>(&mut store, "test_override")
        .unwrap();
    let result = func.call(&mut store, ()).unwrap();
    assert_eq!(result, 25, "5 + 20 = 25");
}

#[rstest]
fn test_fb_bool_output(mut with_db: db::RootDatabase) {
    let source = r#"
FUNCTION_BLOCK MyFB
VAR_INPUT x: BOOL; END_VAR
VAR_OUTPUT y: BOOL; END_VAR
    y := x;
END_FUNCTION_BLOCK

FUNCTION test : INT
VAR fb : MyFB; END_VAR
    fb(x := TRUE);
    IF fb.y THEN
        test := 1;
    ELSE
        test := 0;
    END_IF;
END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).unwrap();
    let func = instance.get_typed_func::<(), i32>(&mut store, "test").unwrap();
    let result = func.call(&mut store, ()).unwrap();
    assert_eq!(result, 1, "fb.y should be TRUE (1) after fb(x := TRUE)");
}

#[rstest]
fn test_fb_bool_not_logic(mut with_db: db::RootDatabase) {
    // Mimics F_TRIG: Q := NOT CLK AND NOT M
    let source = r#"
FUNCTION_BLOCK TestFB
VAR_INPUT CLK: BOOL; END_VAR
VAR_OUTPUT Q: BOOL; END_VAR
VAR M: BOOL; END_VAR
    Q := NOT CLK AND NOT M;
    M := NOT CLK;
END_FUNCTION_BLOCK

FUNCTION test : INT
VAR fb : TestFB; END_VAR
    // First call: CLK=FALSE, M=FALSE (default)
    // Q = NOT FALSE AND NOT FALSE = TRUE
    fb(CLK := FALSE);
    IF fb.Q THEN
        test := 1;
    ELSE
        test := 0;
    END_IF;
END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).unwrap();
    let func = instance.get_typed_func::<(), i32>(&mut store, "test").unwrap();
    let result = func.call(&mut store, ()).unwrap();
    assert_eq!(result, 1, "Q should be TRUE: NOT FALSE AND NOT FALSE = TRUE");
}

#[rstest]
fn test_fb_counter_pattern(mut with_db: db::RootDatabase) {
    // Mimics CTU: count up on rising edge of CU
    let source = r#"
FUNCTION_BLOCK CTU
VAR_INPUT
    CU: BOOL;
    R: BOOL;
    PV: INT;
END_VAR
VAR_OUTPUT
    Q: BOOL;
    CV: INT;
END_VAR
VAR
    CU_EDGE: BOOL;
END_VAR
    IF R THEN
        CV := 0;
    ELSIF CU AND NOT CU_EDGE THEN
        CV := CV + 1;
    END_IF;
    Q := CV >= PV;
    CU_EDGE := CU;
END_FUNCTION_BLOCK

FUNCTION test : INT
VAR counter : CTU; END_VAR
    // First call: CU rising edge, should count to 1
    counter(CU := TRUE, R := FALSE, PV := 5);
    test := counter.CV;
END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).unwrap();
    let func = instance.get_typed_func::<(), i32>(&mut store, "test").unwrap();
    let result = func.call(&mut store, ()).unwrap();
    assert_eq!(result, 1, "CV should be 1 after first rising edge");
}

#[rstest]
fn test_nested_fb_bool(mut with_db: db::RootDatabase) {
    let source = r#"
FUNCTION_BLOCK R_TRIG
VAR_INPUT CLK: BOOL; END_VAR
VAR_OUTPUT Q: BOOL; END_VAR
VAR M: BOOL; END_VAR
    Q := CLK AND NOT M;
    M := CLK;
END_FUNCTION_BLOCK

FUNCTION_BLOCK CTU
VAR_INPUT CU: BOOL; R: BOOL; PV: INT; END_VAR
VAR_OUTPUT Q: BOOL; CV: INT; END_VAR
VAR CU_T: R_TRIG; END_VAR
    CU_T(CU);
    IF R THEN
        CV := 0;
    ELSIF CU_T.Q AND (CV < PV) THEN
        CV := CV + 1;
    END_IF;
    Q := (CV >= PV);
END_FUNCTION_BLOCK

FUNCTION test : INT
VAR counter : CTU; END_VAR
    counter(CU := TRUE, R := FALSE, PV := 3);
    test := counter.CV;
END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).unwrap();
    let func = instance.get_typed_func::<(), i32>(&mut store, "test").unwrap();
    let result = func.call(&mut store, ()).unwrap();
    assert_eq!(result, 1, "CV should be 1 after first rising edge (nested R_TRIG)");
}

#[rstest]
fn debug_nested_fb_wasm(mut with_db: db::RootDatabase) {
    let source = r#"
FUNCTION_BLOCK R_TRIG
VAR_INPUT CLK: BOOL; END_VAR
VAR_OUTPUT Q: BOOL; END_VAR
VAR M: BOOL; END_VAR
    Q := CLK AND NOT M;
    M := CLK;
END_FUNCTION_BLOCK

FUNCTION_BLOCK CTU
VAR_INPUT CU: BOOL; R: BOOL; PV: INT; END_VAR
VAR_OUTPUT Q: BOOL; CV: INT; END_VAR
VAR CU_T: R_TRIG; END_VAR
    CU_T(CU);
    IF R THEN
        CV := 0;
    ELSIF CU_T.Q AND (CV < PV) THEN
        CV := CV + 1;
    END_IF;
    Q := (CV >= PV);
END_FUNCTION_BLOCK

FUNCTION test : INT
VAR counter : CTU; END_VAR
    counter(CU := TRUE, R := FALSE, PV := 3);
    test := counter.CV;
END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    std::fs::write("/tmp/nested_fb.wasm", &wasm_bytes).unwrap();
    eprintln!("Wrote /tmp/nested_fb.wasm ({} bytes)", wasm_bytes.len());
}


#[rstest]
fn test_nested_fb_counter_two_edges(mut with_db: db::RootDatabase) {
    let source = r#"
FUNCTION_BLOCK R_TRIG
VAR_INPUT CLK: BOOL; END_VAR
VAR_OUTPUT Q: BOOL; END_VAR
VAR M: BOOL; END_VAR
    Q := CLK AND NOT M;
    M := CLK;
END_FUNCTION_BLOCK

FUNCTION_BLOCK CTU
VAR_INPUT CU: BOOL; R: BOOL; PV: INT; END_VAR
VAR_OUTPUT Q: BOOL; CV: INT; END_VAR
VAR CU_T: R_TRIG; END_VAR
    CU_T(CU);
    IF R THEN
        CV := 0;
    ELSIF CU_T.Q AND (CV < PV) THEN
        CV := CV + 1;
    END_IF;
    Q := (CV >= PV);
END_FUNCTION_BLOCK

FUNCTION test : INT
VAR counter : CTU; END_VAR
    counter(CU := TRUE, R := FALSE, PV := 10);   // Edge 1 → CV=1
    counter(CU := FALSE, R := FALSE, PV := 10);  // No edge
    counter(CU := TRUE, R := FALSE, PV := 10);   // Edge 2 → CV=2
    test := counter.CV;
END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = Store::new(&engine, ());
    let instance = Instance::new(&mut store, &module, &[]).unwrap();
    let func = instance.get_typed_func::<(), i32>(&mut store, "test").unwrap();
    let result = func.call(&mut store, ()).unwrap();
    assert_eq!(result, 2, "CV should be 2 after two rising edges");
}

#[rstest]
fn test_nested_fb_counter_with_assert(mut with_db: db::RootDatabase) {
    let source = r#"
FUNCTION_BLOCK R_TRIG
VAR_INPUT CLK: BOOL; END_VAR
VAR_OUTPUT Q: BOOL; END_VAR
VAR M: BOOL; END_VAR
    Q := CLK AND NOT M;
    M := CLK;
END_FUNCTION_BLOCK

FUNCTION_BLOCK CTU
VAR_INPUT CU: BOOL; R: BOOL; PV: INT; END_VAR
VAR_OUTPUT Q: BOOL; CV: INT; END_VAR
VAR CU_T: R_TRIG; END_VAR
    CU_T(CU);
    IF R THEN
        CV := 0;
    ELSIF CU_T.Q AND (CV < PV) THEN
        CV := CV + 1;
    END_IF;
    Q := (CV >= PV);
END_FUNCTION_BLOCK

FUNCTION __ASSERT_FAIL
VAR_INPUT message: STRING; END_VAR
    {extern 'assert' 'fail' (params message)}
END_FUNCTION

FUNCTION ASSERT
VAR_INPUT value: BOOL; message: STRING := ''; END_VAR
    IF NOT value THEN
        __ASSERT_FAIL(message);
    END_IF;
END_FUNCTION

{test}
FUNCTION test_ctu
VAR counter: CTU; END_VAR
    counter(CU := TRUE, R := FALSE, PV := 3);
    ASSERT(counter.CV = 1, 'CV should be 1');
END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm_bytes).unwrap();
    let linker = {
        let mut l = wasmtime::Linker::new(&engine);
        l.func_wrap("assert", "fail", |_ptr: i32, _len: i32| -> Result<(), wasmtime::Error> {
            Err(wasmtime::Error::msg("assertion failed"))
        }).unwrap();
        l
    };
    let mut store = Store::new(&engine, ());
    let instance = linker.instantiate(&mut store, &module).unwrap();
    let func = instance.get_typed_func::<(), ()>(&mut store, "test_ctu").unwrap();
    func.call(&mut store, ()).unwrap();
}
