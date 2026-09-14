//! WASM execution tests (actually running the generated code).

use crate::tests::codegen::{compile_to_wasm, with_db};
use rstest::*;
use wasmtime::{Engine, Module, Store};

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

    let engine = crate::tests::codegen::test_engine();
    let module = Module::new(&engine, &wasm_bytes).expect("Failed to create module");
    let mut store = Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);

    let add_func = instance
        .get_typed_func::<(i32, i32), i32>(&mut store, "add")
        .expect("Failed to get function");

    let result = add_func
        .call(&mut store, (5, 3))
        .expect("Failed to call function");
    assert_eq!(result, 8, "5 + 3 should equal 8");
}

/// Regression: a function mixing scalar locals of different wasm value types
/// (i32 `INT` + i64 `LINT`) must emit valid wasm. `emit_function` used to
/// declare a phantom wasm local for the scalar return slot on top of the one
/// already produced from `func.locals`, shifting every subsequent local's *type
/// declaration* down by one relative to its MIR `local_index`. All-same-valtype
/// functions survived the shift; here `y : LINT` (i64) would land on an
/// i32-declared slot and the wasm validator would reject the module. No
/// generics or FBs involved — the minimal reproducer.
#[rstest]
fn test_execute_mixed_width_scalar_locals(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : LINT
        VAR
            x : INT := 1;
            y : LINT := 2;
            acc : LINT := 0;
        END_VAR
            y := y + 40;
            acc := x;
            acc := acc + y;
            test := acc;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let result: i64 = super::execute_wasm(&wasm_bytes, "test", ());
    assert_eq!(result, 43, "1 + (2 + 40) = 43 across i32 and i64 locals");
}

/// An integer literal in float context IS a float: inference resolves the 2
/// in `3.0 + 2` to REAL, and every consumer — including the no-cast-needed
/// check — believes it. `lower_literal` used to emit from the LEXEME instead,
/// so a float op consumed an i32 and the module failed wasm validation, from
/// a program `rk check` called clean. An execution test, not a check test,
/// because the check was never the layer that broke.
#[rstest]
fn test_execute_integer_literal_in_float_context(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : REAL
        VAR
            x : REAL := 3.0;
            r : REAL;
        END_VAR
            r := 3.0 + 2;
            r := r + x * 2;
            r := r - 1;
            test := r / 2;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let result: f32 = super::execute_wasm(&wasm_bytes, "test", ());
    assert_eq!(result, 5.0, "((3.0+2) + 3.0*2 - 1) / 2 = 5.0");
}

#[rstest]
/// A subrange-typed FOR control variable executes: `Small : INT (0..10)`
/// normalizes to INT everywhere the loop machinery looks. This was an ICE —
/// "FOR control variable must be elementary" — because subranges survived
/// normalization and the check saw `Type::SubRange`, not the base.
#[rstest]
fn test_execute_for_with_subrange_control_variable(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE
            Small : INT (0..10);
        END_TYPE

        FUNCTION test : INT
        VAR
            i : Small;
            acc : INT := 0;
        END_VAR
            FOR i := 1 TO 4 DO
                acc := acc + i;
            END_FOR
            test := acc;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm_bytes, "test", ());
    assert_eq!(result, 10, "1+2+3+4 over a subrange loop variable");
}

#[rstest]
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

    let engine = crate::tests::codegen::test_engine();
    let module = Module::new(&engine, &wasm_bytes).expect("Failed to create module");
    let mut store = Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);

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

    let engine = crate::tests::codegen::test_engine();
    let module = Module::new(&engine, &wasm_bytes).expect("Failed to create module");
    let mut store = Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);

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

    let engine = crate::tests::codegen::test_engine();
    let module = Module::new(&engine, &wasm_bytes).expect("Failed to create module");
    let mut store = Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);

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

    let engine = crate::tests::codegen::test_engine();
    let module = Module::new(&engine, &wasm_bytes).expect("Failed to create module");
    let mut store = Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);

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
    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);
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
    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);
    let func = instance
        .get_typed_func::<(), i32>(&mut store, "test_override")
        .unwrap();
    let result = func.call(&mut store, ()).unwrap();
    assert_eq!(result, 25, "5 + 20 = 25");
}

/// Formal (named) arguments may appear in any order at the call site — the
/// emitted args must be bound by parameter name, not call-site position.
#[rstest]
fn test_named_args_out_of_order(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION sub2 : INT
        VAR_INPUT
            a : INT;
            b : INT;
        END_VAR
            sub2 := a - b;
        END_FUNCTION

        FUNCTION test : INT
            test := sub2(b := 3, a := 10);
        END_FUNCTION
    "#;
    let wasm = super::compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 7, "a(10) - b(3) = 7");
}

/// A defaulted parameter declared *before* a provided named argument must
/// still be synthesized at the call site, in declaration order.
#[rstest]
fn test_default_param_before_named(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION sub_default : INT
        VAR_INPUT
            a : INT := 10;
            b : INT;
        END_VAR
            sub_default := a - b;
        END_FUNCTION

        FUNCTION test : INT
            test := sub_default(b := 4);
        END_FUNCTION
    "#;
    let wasm = super::compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 6, "default(10) - 4 = 6");
}

/// METHOD calls fill omitted defaulted VAR_INPUTs exactly like functions —
/// the default is synthesized after the implicit `this` receiver.
#[rstest]
fn test_method_omitted_default_param(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Fb
            METHOD PUBLIC add2 : INT
            VAR_INPUT
                a : INT;
                b : INT := 10;
            END_VAR
                add2 := a + b;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR fb : Fb; END_VAR
            test := fb.add2(a := 5);
        END_FUNCTION
    "#;
    let wasm = super::compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 15, "5 + default(10) = 15");
}

/// Mixed formal + positional args on an FB invocation: a positional arg binds
/// to the first input *not* claimed by a named one, regardless of call order.
#[rstest]
fn test_fb_mixed_named_then_positional(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK diff
        VAR_INPUT
            a : INT;
            b : INT;
        END_VAR
        VAR_OUTPUT
            o : INT;
        END_VAR
            o := a - b;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR inst : diff; END_VAR
            inst(b := 3, 10);
            test := inst.o;
        END_FUNCTION
    "#;
    let wasm = super::compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 7, "a(10) - b(3) = 7");
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
    let engine = crate::tests::codegen::test_engine();
    let module = Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);
    let func = instance
        .get_typed_func::<(), i32>(&mut store, "test")
        .unwrap();
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
    let engine = crate::tests::codegen::test_engine();
    let module = Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);
    let func = instance
        .get_typed_func::<(), i32>(&mut store, "test")
        .unwrap();
    let result = func.call(&mut store, ()).unwrap();
    assert_eq!(
        result, 1,
        "Q should be TRUE: NOT FALSE AND NOT FALSE = TRUE"
    );
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
    let engine = crate::tests::codegen::test_engine();
    let module = Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);
    let func = instance
        .get_typed_func::<(), i32>(&mut store, "test")
        .unwrap();
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
    let engine = crate::tests::codegen::test_engine();
    let module = Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);
    let func = instance
        .get_typed_func::<(), i32>(&mut store, "test")
        .unwrap();
    let result = func.call(&mut store, ()).unwrap();
    assert_eq!(
        result, 1,
        "CV should be 1 after first rising edge (nested R_TRIG)"
    );
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

    let _wasm_bytes = compile_to_wasm(&mut with_db, source);
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
    let engine = crate::tests::codegen::test_engine();
    let module = Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);
    let func = instance
        .get_typed_func::<(), i32>(&mut store, "test")
        .unwrap();
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

{extern 'assert' 'fail'}
FUNCTION __ASSERT_FAIL
VAR_INPUT message: STRING; END_VAR
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

    // The codegen now wraps `{test}` functions in `try_table`, so even
    // a core-wasm test needs `wasm_exceptions(true)` to load.
    let engine = {
        let mut c = wasmtime::Config::new();
        c.wasm_exceptions(true);
        Engine::new(&c).unwrap()
    };
    let module = Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = Store::new(&engine, ());
    let memory = wasmtime::Memory::new(&mut store, wasmtime::MemoryType::new(1, None)).unwrap();
    let linker = {
        let mut l = wasmtime::Linker::new(&engine);
        l.define(&store, "env", "memory", memory).unwrap();
        l.func_wrap(
            "assert",
            "fail",
            |_ptr: i32, _len: i32| -> Result<(), wasmtime::Error> {
                Err(wasmtime::Error::msg("assertion failed"))
            },
        )
        .unwrap();
        l
    };
    let instance = linker.instantiate(&mut store, &module).unwrap();
    // `{test}` functions are now codegen-wrapped in a `try_table` and
    // return an i32 pointer to the canonical-ABI `result<unit, string>`
    // area. On success the discriminant byte at that address is 0.
    let func = instance
        .get_typed_func::<(), i32>(&mut store, "test_ctu")
        .unwrap();
    let result_area = func.call(&mut store, ()).unwrap();
    let mem = instance.get_memory(&mut store, "memory").unwrap();
    let mut disc = [0u8; 1];
    mem.read(&store, result_area as usize, &mut disc).unwrap();
    assert_eq!(disc[0], 0, "test_ctu should pass (Ok discriminant)");
}


/// Sub-width arithmetic wraps at the IEC type width: results
/// are re-normalized into the 8/16-bit domain after each op instead of
/// escaping into the i32 lane. `USINT 255 + 1` used to evaluate to 256 — a
/// value the type cannot hold — and the escape persisted through stores and
/// comparisons.
#[rstest]
fn test_execute_subwidth_arithmetic_wraps(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION usint_add : USINT
        VAR_INPUT a : USINT; b : USINT; END_VAR
            usint_add := a + b;
        END_FUNCTION

        FUNCTION int_add : INT
        VAR_INPUT a : INT; b : INT; END_VAR
            int_add := a + b;
        END_FUNCTION

        FUNCTION sint_div : SINT
        VAR_INPUT a : SINT; b : SINT; END_VAR
            sint_div := a / b;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "usint_add", (255i32, 1i32));
    assert_eq!(r, 0, "USINT 255 + 1 wraps to 0");
    let r: i32 = super::execute_wasm(&wasm, "int_add", (32767i32, 1i32));
    assert_eq!(r, -32768, "INT 32767 + 1 wraps to -32768");
    let r: i32 = super::execute_wasm(&wasm, "sint_div", (-128i32, -1i32));
    assert_eq!(r, -128, "SINT -128 / -1 wraps to -128");
}

/// Narrowing casts truncate to the target width and re-extend — the value
/// must never escape the target's domain (INT_TO_SINT-style conversions are
/// lowered through the same Cast path as language-level narrowing).
#[rstest]
fn test_execute_subwidth_cast_normalizes(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION to_sint : SINT
        VAR_INPUT a : INT; END_VAR
        VAR s : SINT; END_VAR
            s := INT_TO_SINT(a);
            to_sint := s;
        END_FUNCTION

        FUNCTION INT_TO_SINT : SINT
        VAR_INPUT IN : INT; END_VAR
            {wasm 'nop' (params IN) (result INT_TO_SINT)}
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "to_sint", (200i32,));
    assert_eq!(r, -56, "INT_TO_SINT(200) truncates to the 8-bit two's-complement -56");
}

/// Binary operators compute in the common WIDER type of both operands
/// regardless of operand order (IEC 6.6.1.6). `INT * REAL` used to be typed
/// INT (left-anchored), which rejected the expression outright; the reversed
/// order computed correctly. Both orders must now produce the REAL result.
#[rstest]
fn test_execute_widened_arithmetic_is_commutative(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION int_times_real : REAL
        VAR_INPUT i : INT; r : REAL; END_VAR
            int_times_real := i * r;
        END_FUNCTION

        FUNCTION real_times_int : REAL
        VAR_INPUT i : INT; r : REAL; END_VAR
            real_times_int := r * i;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    // 3 * 0.5: INT-domain math would truncate to 0 (or 1); REAL math gives 1.5.
    let a: f32 = super::execute_wasm(&wasm, "int_times_real", (3i32, 0.5f32));
    assert_eq!(a, 1.5, "INT * REAL computes in REAL");
    let b: f32 = super::execute_wasm(&wasm, "real_times_int", (3i32, 0.5f32));
    assert_eq!(b, 1.5, "REAL * INT computes in REAL");
}

/// The type a comparison's operands are compared at is inference's decision:
/// HIR records the join it computed with the IEC widening lattice, and codegen
/// consumes it. MIR used to re-derive that join with its own ad-hoc rule
/// (float wins, then 64-bit, then signed), which disagreed with the lattice —
/// `BYTE` vs `WORD` yielded `UDInt` where HIR says `WORD` — so a comparison
/// could execute at a different type than the one type-checking accepted.
#[rstest]
fn mixed_type_comparisons_use_the_inferred_join(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR
            b : BYTE := 16#FF;
            w : WORD := 16#00FF;
            i : INT := -1;
            d : DINT := -1;
            r : REAL := 255.0;
            score : INT;
        END_VAR
            IF b = w THEN score := score + 1; END_IF;      (* bit strings, differing widths *)
            IF NOT (b < w) THEN score := score + 10; END_IF;
            IF i = d THEN score := score + 100; END_IF;    (* signed, differing widths *)
            IF r > i THEN score := score + 1000; END_IF;   (* int vs float *)
            test := score;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 1111, "every mixed-width comparison holds");
}

/// A subrange behaves as its base type at runtime — arithmetic, comparison and
/// round-tripping through a function all operate on the underlying integer.
#[rstest]
fn subrange_behaves_as_its_base_type(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Pct : INT (0..100); END_TYPE

        FUNCTION scale : INT
        VAR_INPUT p : Pct; END_VAR
            scale := p * 2;
        END_FUNCTION

        FUNCTION test : INT
        VAR
            inline_sr : INT (0..100) := 30;
            aliased   : Pct;
        END_VAR
            aliased := 20;
            inline_sr := inline_sr + 5;        (* 35 *)
            test := scale(p := aliased) + inline_sr;   (* 40 + 35 *)
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 75, "subrange arithmetic runs on the base type");
}

/// A negative subrange bound is legal and addresses correctly.
#[rstest]
fn subrange_with_negative_bounds(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR i : INT (-4095..4095); END_VAR
            i := -4000;
            i := i + 5;
            test := i;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, -3995);
}

/// The rest of the sub-width invariant's surface: bitwise NOT, unary minus,
/// subtraction underflow and multiplication overflow all stay inside the
/// 8/16-bit domain. Verified fixed earlier (the wrap pins lived only in the
/// STDLIB's own test suite, which `cargo nextest` never runs) — pinned here
/// so a codegen regression fails in CI, not in a field diagnosis.
#[rstest]
fn test_execute_subwidth_not_neg_and_more_wraps(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION byte_not : BYTE
        VAR_INPUT a : BYTE; END_VAR
            byte_not := NOT a;
        END_FUNCTION

        FUNCTION word_not : WORD
        VAR_INPUT a : WORD; END_VAR
            word_not := NOT a;
        END_FUNCTION

        FUNCTION sint_neg : SINT
        VAR_INPUT a : SINT; END_VAR
            sint_neg := -a;
        END_FUNCTION

        FUNCTION usint_sub : USINT
        VAR_INPUT a : USINT; b : USINT; END_VAR
            usint_sub := a - b;
        END_FUNCTION

        FUNCTION usint_mul : USINT
        VAR_INPUT a : USINT; b : USINT; END_VAR
            usint_mul := a * b;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "byte_not", (0i32,));
    assert_eq!(r, 0xFF, "NOT BYTE#0 is 0xFF, not the i32 lane's 0xFFFFFFFF");
    let r: i32 = super::execute_wasm(&wasm, "word_not", (0x00FFi32,));
    assert_eq!(r, 0xFF00, "NOT WORD#00FF stays 16-bit");
    let r: i32 = super::execute_wasm(&wasm, "sint_neg", (-128i32,));
    assert_eq!(r, -128, "-(SINT#-128) wraps to -128, the two's-complement edge");
    let r: i32 = super::execute_wasm(&wasm, "usint_sub", (0i32, 1i32));
    assert_eq!(r, 255, "USINT 0 - 1 wraps to 255");
    let r: i32 = super::execute_wasm(&wasm, "usint_mul", (16i32, 16i32));
    assert_eq!(r, 0, "USINT 16 * 16 wraps to 0");
}

/// An UNCAUGHT `__RAISE` faults the scan with its own message. The payload
/// always travelled with the exception — `(ptr, len)` on the
/// `$rk_exception` tag — but the scan reported only wasmtime's opaque
/// "thrown Wasm exception" until it started reading the pending exception's
/// fields. A plain trap (no pending exception) must pass through untouched.
#[rstest]
fn an_uncaught_raise_names_its_fault(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR n : DINT; END_VAR
            n := n + 1;
            IF n = 2 THEN
                __RAISE('motor overheated');
            END_IF;
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = crate::tests::codegen::compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = runtime::Plc::load(&wasm, runtime::Config::default()).expect("load");
    plc.scan().expect("first scan is fine");
    let err = plc.scan().expect_err("second scan raises");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("uncaught IEC exception: motor overheated"),
        "the fault carries the RAISE payload, got: {msg}"
    );
    // The scan after a fault runs normally again (n keeps counting past 2).
    plc.scan().expect("the PLC is not wedged after a fault");
}

/// Float literals lower through the same HIR accessors the checker validated
/// them with. A raw `text.parse()` in MIR rejected `1_000.5` — IEC allows
/// underscores in literals, HIR strips them — so valid code ICE'd at
/// compile. Covers the typed (REAL#), untyped-inferred, and LREAL forms.
#[rstest]
fn float_literals_with_underscores_execute(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : DINT
        VAR r : REAL; l : LREAL; ok : DINT; END_VAR
            r := 1_000.5;
            l := 2_500_000.25;
            IF r = 1000.5 THEN ok := ok + 1; END_IF;
            IF l = LREAL#2500000.25 THEN ok := ok + 10; END_IF;
            r := REAL#1_5.5;
            IF r = 15.5 THEN ok := ok + 100; END_IF;
            test := ok;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 111, "all three literal forms parse to the same values");
}

/// Binary operands evaluate LEFT-TO-RIGHT — a declared choice, since IEC
/// leaves operand order to the implementation. Each call bumps the shared
/// counter through VAR_IN_OUT, so the order is observable: 1 then 2 gives
/// 12; right-to-left would give 21.
#[rstest]
fn test_operands_evaluate_left_to_right(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION bump : INT
        VAR_IN_OUT c : INT; END_VAR
            c := c + 1;
            bump := c;
        END_FUNCTION
        FUNCTION test : INT
        VAR n : INT; END_VAR
            test := bump(c := n) * 10 + bump(c := n);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 12, "left operand first: 1*10 + 2");
}

/// Named arguments evaluate in the callee's DECLARATION order, not the order
/// they are written at the call site - the same order they are passed in.
/// `a` is declared first, so its expression runs first even written second:
/// a = 1, b = 2 gives 12; written-order evaluation would give 21.
#[rstest]
fn test_args_evaluate_in_declaration_order(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION bump : INT
        VAR_IN_OUT c : INT; END_VAR
            c := c + 1;
            bump := c;
        END_FUNCTION
        FUNCTION f : INT
        VAR_INPUT a : INT; b : INT; END_VAR
            f := a * 10 + b;
        END_FUNCTION
        FUNCTION test : INT
        VAR n : INT; END_VAR
            test := f(b := bump(c := n), a := bump(c := n));
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(r, 12, "a evaluated first: declaration order, not written order");
}

/// An unsigned literal past the SIGNED maximum of its width is an ordinary
/// value of that type, and must survive lowering: wasm has one integer type
/// per width, and the operations — not the constant — pick the interpretation.
/// Reading it as `i32`/`i64` fails to parse, so every UDINT above 2147483647
/// and every ULINT above 9223372036854775807 passed `check` and then died in
/// lowering with "number too large to fit in target type".
#[rstest]
fn test_execute_unsigned_literal_above_the_signed_max(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : UDINT
        VAR n : UDINT; END_VAR
            n := 4294967295;
            run := n;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(
        r as u32, 4294967295,
        "UDINT max round-trips through the i32 lane"
    );
}

/// The same on the 64-bit lane.
#[rstest]
fn test_execute_unsigned_64bit_literal_above_the_signed_max(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : ULINT
        VAR n : ULINT; END_VAR
            n := 18446744073709551615;
            run := n;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i64 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(
        r as u64, 18446744073709551615,
        "ULINT max round-trips through the i64 lane"
    );
}
