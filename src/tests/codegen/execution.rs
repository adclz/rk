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

    let engine = Engine::default();
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

    let engine = Engine::default();
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

    let engine = Engine::default();
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

    let engine = Engine::default();
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
    let engine = wasmtime::Engine::default();
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
    let engine = wasmtime::Engine::default();
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
    let wasm = super::compile_to_wasm_checked(&mut with_db, source);
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
    let wasm = super::compile_to_wasm_checked(&mut with_db, source);
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
    let wasm = super::compile_to_wasm_checked(&mut with_db, source);
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
    let wasm = super::compile_to_wasm_checked(&mut with_db, source);
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
    let engine = Engine::default();
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
    let engine = Engine::default();
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
    let engine = Engine::default();
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
    let engine = Engine::default();
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
    let engine = Engine::default();
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

/// Sanity-check the date/time integer encoding end-to-end. The cast emitter
/// is responsible for the unit conversions documented in `stdlib/Convert.st`;
/// these round-trips would silently misbehave if the i32/i64 split or the
/// `* / 1_000_000` (and friends) scaling regressed.
#[rstest]
fn test_execute_datetime_round_trip(mut with_db: db::RootDatabase) {
    let source = r#"
        // TIME (i32 ms) -> LTIME (i64 ns): widening with * 1_000_000
        FUNCTION time_to_ltime : LTIME
        VAR_INPUT t : TIME; END_VAR
            time_to_ltime := t;
        END_FUNCTION

        // LTIME (i64 ns) -> TIME (i32 ms): explicit narrow with / 1_000_000
        FUNCTION ltime_to_time_inline : TIME
        VAR_INPUT lt : LTIME; END_VAR
        VAR result : TIME; END_VAR
            {wasm 'cast' (params lt) (result result)}
            ltime_to_time_inline := result;
        END_FUNCTION

        // DT (i32 secs) -> DATE (i32 days): div by 86400
        FUNCTION dt_to_date_inline : DATE
        VAR_INPUT d : DATE_AND_TIME; END_VAR
        VAR result : DATE; END_VAR
            {wasm 'cast' (params d) (result result)}
            dt_to_date_inline := result;
        END_FUNCTION

        // DT -> TOD (ms-of-day): (secs % 86400) * 1000
        FUNCTION dt_to_tod_inline : TOD
        VAR_INPUT d : DATE_AND_TIME; END_VAR
        VAR result : TOD; END_VAR
            {wasm 'cast' (params d) (result result)}
            dt_to_tod_inline := result;
        END_FUNCTION

        // LDT (i64 ns) -> LTOD (i64 ns-of-day): mod by 86_400_000_000_000
        FUNCTION ldt_to_ltod_inline : LTOD
        VAR_INPUT l : LDATE_AND_TIME; END_VAR
        VAR result : LTOD; END_VAR
            {wasm 'cast' (params l) (result result)}
            ldt_to_ltod_inline := result;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let engine = Engine::default();
    let module = Module::new(&engine, &wasm_bytes).expect("Failed to create module");
    let mut store = Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);

    // 5 ms -> 5_000_000 ns
    let f = instance
        .get_typed_func::<i32, i64>(&mut store, "time_to_ltime")
        .unwrap();
    assert_eq!(f.call(&mut store, 5).unwrap(), 5_000_000);

    // 1500 ns -> 0 ms (truncating). 5_000_000 ns -> 5 ms.
    let f = instance
        .get_typed_func::<i64, i32>(&mut store, "ltime_to_time_inline")
        .unwrap();
    assert_eq!(f.call(&mut store, 1500).unwrap(), 0);
    assert_eq!(f.call(&mut store, 5_000_000).unwrap(), 5);

    // 86_399 secs (just under one day) -> 0 days. 86_400 secs -> 1 day.
    // 1973-01-01 = 1096 days from 1970-01-01 (1970 + 1971 + 1972 leap = 365+365+366).
    let f = instance
        .get_typed_func::<i32, i32>(&mut store, "dt_to_date_inline")
        .unwrap();
    assert_eq!(f.call(&mut store, 86_399).unwrap(), 0);
    assert_eq!(f.call(&mut store, 86_400).unwrap(), 1);
    assert_eq!(f.call(&mut store, 1_096 * 86_400).unwrap(), 1_096);

    // DT 12:34:56 (= 12*3600 + 34*60 + 56 = 45_296 secs into the day)
    // -> TOD 45_296_000 ms-of-day.
    let f = instance
        .get_typed_func::<i32, i32>(&mut store, "dt_to_tod_inline")
        .unwrap();
    let secs_into_day = 12 * 3600 + 34 * 60 + 56;
    assert_eq!(
        f.call(&mut store, secs_into_day).unwrap(),
        secs_into_day * 1_000
    );
    // 1 day + 1 sec since epoch -> 1 sec-of-day -> 1000 ms-of-day.
    assert_eq!(f.call(&mut store, 86_400 + 1).unwrap(), 1_000);

    // LDT 1 day + 500 ns -> LTOD 500 ns.
    let f = instance
        .get_typed_func::<i64, i64>(&mut store, "ldt_to_ltod_inline")
        .unwrap();
    assert_eq!(f.call(&mut store, 86_400_000_000_000 + 500).unwrap(), 500);
    assert_eq!(f.call(&mut store, 86_400_000_000_000 * 2 + 1).unwrap(), 1);
}
