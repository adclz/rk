//! Array execution tests - actually running WASM to verify array operations.

use crate::tests::codegen::{compile_to_wasm, compile_to_wasm_checked, with_db};
use rstest::*;

#[rstest]
fn test_array_write_and_read(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_array : INT
        VAR
            arr : ARRAY[0..4] OF INT;
        END_VAR
            arr[0] := 10;
            arr[1] := 20;
            arr[2] := 30;
            test_array := arr[1];
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);

    let test_array = instance
        .get_typed_func::<(), i32>(&mut store, "test_array")
        .expect("Failed to get function");

    let result = test_array.call(&mut store, ()).unwrap();
    assert_eq!(result, 20, "Should read value 20 from arr[1]");
}

#[rstest]
fn test_array_sum(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION sum_array : INT
        VAR
            arr : ARRAY[0..4] OF INT;
            sum : INT;
            i : INT;
        END_VAR
            arr[0] := 1;
            arr[1] := 2;
            arr[2] := 3;
            arr[3] := 4;
            arr[4] := 5;

            sum := 0;
            FOR i := 0 TO 4 DO
                sum := sum + arr[i];
            END_FOR;

            sum_array := sum;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);

    let sum_array = instance
        .get_typed_func::<(), i32>(&mut store, "sum_array")
        .expect("Failed to get function");

    let result = sum_array.call(&mut store, ()).unwrap();
    assert_eq!(result, 15, "Sum of 1+2+3+4+5 should be 15");
}

#[rstest]
fn test_2d_array_access(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_2d : INT
        VAR
            matrix : ARRAY[0..2, 0..2] OF INT;
        END_VAR
            matrix[0, 0] := 1;
            matrix[0, 1] := 2;
            matrix[1, 0] := 3;
            matrix[1, 1] := 4;

            test_2d := matrix[1, 1];
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);

    let test_2d = instance
        .get_typed_func::<(), i32>(&mut store, "test_2d")
        .expect("Failed to get function");

    let result = test_2d.call(&mut store, ()).unwrap();
    assert_eq!(result, 4, "matrix[1,1] should be 4");
}

#[rstest]
fn test_array_with_non_zero_base(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_offset : INT
        VAR
            arr : ARRAY[10..14] OF INT;
        END_VAR
            arr[10] := 100;
            arr[11] := 200;
            arr[12] := 300;

            test_offset := arr[11];
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);

    let test_offset = instance
        .get_typed_func::<(), i32>(&mut store, "test_offset")
        .expect("Failed to get function");

    let result = test_offset.call(&mut store, ()).unwrap();
    assert_eq!(result, 200, "arr[11] should be 200");
}

#[rstest]
fn test_array_of_real(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_real_arr : REAL
        VAR
            arr : ARRAY[0..2] OF REAL;
        END_VAR
            arr[0] := 1.5;
            arr[1] := 2.5;
            arr[2] := 3.5;
            test_real_arr := arr[0] + arr[1] + arr[2];
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);
    let func = instance
        .get_typed_func::<(), f32>(&mut store, "test_real_arr")
        .unwrap();
    let result = func.call(&mut store, ()).unwrap();
    assert!(
        (result - 7.5).abs() < 0.001,
        "Sum should be 7.5, got {}",
        result
    );
}

#[rstest]
fn test_array_of_struct(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point :
            STRUCT
                x : INT;
                y : INT;
            END_STRUCT;
        END_TYPE

        FUNCTION test_struct_arr : INT
        VAR
            pts : ARRAY[0..2] OF Point;
        END_VAR
            pts[0].x := 10;
            pts[0].y := 20;
            pts[1].x := 30;
            pts[1].y := 40;
            test_struct_arr := pts[0].x + pts[1].y;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);
    let func = instance
        .get_typed_func::<(), i32>(&mut store, "test_struct_arr")
        .unwrap();
    let result = func.call(&mut store, ()).unwrap();
    assert_eq!(result, 50, "pts[0].x + pts[1].y = 10 + 40 = 50");
}

#[rstest]
fn test_array_passed_to_function(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION sum_first_two : INT
        VAR_INPUT
            a : INT;
            b : INT;
        END_VAR
            sum_first_two := a + b;
        END_FUNCTION

        FUNCTION test_arr_call : INT
        VAR
            arr : ARRAY[0..4] OF INT;
        END_VAR
            arr[0] := 100;
            arr[1] := 200;
            test_arr_call := sum_first_two(a := arr[0], b := arr[1]);
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);
    let func = instance
        .get_typed_func::<(), i32>(&mut store, "test_arr_call")
        .unwrap();
    let result = func.call(&mut store, ()).unwrap();
    assert_eq!(result, 300, "arr[0] + arr[1] = 100 + 200 = 300");
}

#[rstest]
fn test_array_in_for_loop_with_computation(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_arr_compute : INT
        VAR
            arr : ARRAY[0..9] OF INT;
            i : INT;
            sum : INT;
        END_VAR
            FOR i := 0 TO 9 DO
                arr[i] := i * i;
            END_FOR;
            sum := 0;
            FOR i := 0 TO 9 DO
                sum := sum + arr[i];
            END_FOR;
            test_arr_compute := sum;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);
    let func = instance
        .get_typed_func::<(), i32>(&mut store, "test_arr_compute")
        .unwrap();
    let result = func.call(&mut store, ()).unwrap();
    // Sum of squares 0..9 = 0+1+4+9+16+25+36+49+64+81 = 285
    assert_eq!(result, 285, "Sum of squares 0..9 should be 285");
}

/// A negative lower bound is valid IEC and must address correctly at runtime:
/// element `arr[-2]` sits at offset 0, and indexing subtracts the (negative)
/// lower bound. Array bounds were folded as UNSIGNED, so a negative bound never
/// evaluated and the declaration was rejected outright.
#[rstest]
fn negative_lower_bound_array_addresses_correctly(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR arr : ARRAY[-2..2] OF INT; i : INT; END_VAR
            FOR i := -2 TO 2 DO
                arr[i] := i * 10;
            END_FOR;
            (* -20 + 0 + 20 = 0 proves both ends address distinctly; add a
               middle element so a collapsed layout cannot also give 0 *)
            test := arr[-2] + arr[2] + arr[-1] + 100;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 90, "-20 + 20 + (-10) + 100");
}

/// Typed integer literals as bounds (`INT#1..INT#3`) fold like untyped ones.
/// Validation and lowering used different literal matchers, so this form was
/// accepted by one and rejected by the other.
#[rstest]
fn typed_literal_array_bounds(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR arr : ARRAY[INT#1..INT#3] OF INT; END_VAR
            arr[1] := 7;
            arr[3] := 9;
            test := arr[1] + arr[3];
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 16, "typed-literal bounds address correctly");
}

/// Regression: an array subscript containing a call used to ICE.
///
/// `emit_load` and `emit_addr_of` emitted the index expression with a freshly
/// defaulted (empty) `fn_indices` map, so any call inside a subscript resolved
/// against no functions at all and tripped the unknown-callee panic in
/// `emit_call`. No function block or array-of-instance needed — a plain
/// `a[idx()]` was enough.
#[rstest]
fn array_subscript_containing_a_call(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION idx : INT
            idx := 1;
        END_FUNCTION

        FUNCTION run : INT
        VAR
            a : ARRAY[0..2] OF INT;
        END_VAR
            a[idx()] := 7;
            run := a[idx()];
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(result, 7, "the subscript's call resolves on both load and store");
}

/// An out-of-bounds subscript is DENIED at runtime: the access raises an IEC
/// exception ("array index out of bounds") and the scan faults, instead of
/// computing a neighbour's address. Before this check, `a[i]` with `i = 4` on
/// an `ARRAY[0..2]` silently overwrote whatever came next — observed running
/// corrupted for hundreds of scans.
#[rstest]
fn an_out_of_bounds_write_faults_instead_of_corrupting(mut with_db: db::RootDatabase) {
    let source = r#"
        PROGRAM P
        VAR
            n : DINT;
            a : ARRAY[0..2] OF DINT;
            guard : DINT := 777;
        END_VAR
            n := n + 1;
            IF n = 3 THEN
                a[n + 1] := 99;   (* 4 is past the end *)
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
    plc.run(2).expect("in-bounds scans run fine");
    let err = plc.scan().expect_err("the third scan goes out of bounds");
    // The check raises through `$rk_exception` with the message as its
    // payload, and the scan extracts it — the fault names itself instead of
    // reading "thrown Wasm exception".
    assert!(
        format!("{err:#}").contains("array index out of bounds"),
        "the fault carries the bounds message, got: {err:#}"
    );
}

/// Reads are checked by the same wrap — garbage is also a wrong answer.
#[rstest]
fn an_out_of_bounds_read_faults_too(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : DINT
        VAR
            a : ARRAY[0..2] OF DINT;
            i : DINT;
        END_VAR
            i := 5;
            run := a[i];
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);
    let f = instance
        .get_typed_func::<(), i32>(&mut store, "run")
        .unwrap();
    assert!(f.call(&mut store, ()).is_err(), "a[5] on [0..2] must fault");
}

/// Both boundary subscripts are IN bounds — the check must not be off by one,
/// and lower bounds (including negative ones) are honoured per dimension.
#[rstest]
fn boundary_subscripts_do_not_fault(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : DINT
        VAR
            a : ARRAY[-2..2] OF DINT;
            lo : DINT;
            hi : DINT;
        END_VAR
            lo := -2;
            hi := 2;
            a[lo] := 10;
            a[hi] := 32;
            run := a[lo] * 100 + a[hi];
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(result, 1032, "-2 and 2 are both legal on ARRAY[-2..2]");
}

/// Multi-dimensional bounds are PER DIMENSION: `m[1][9]` on
/// `ARRAY[1..3, 1..3]` is out of bounds even though its flat offset lands
/// inside the array's total byte range — a flat-only check would let it
/// alias `m[3][?]`.
#[rstest]
fn per_dimension_bounds_not_flat_bounds(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : DINT
        VAR
            m : ARRAY[1..3, 1..3] OF DINT;
            j : DINT;
        END_VAR
            j := 9;
            run := m[1][j];
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);
    let f = instance
        .get_typed_func::<(), i32>(&mut store, "run")
        .unwrap();
    assert!(
        f.call(&mut store, ()).is_err(),
        "dimension 2's bound is 3; 9 must fault even though 1*36 < sizeof(m)"
    );
}

/// An array dimensioned by a CONSTANT compiles and runs — the OSCAT idiom.
/// `ARRAY[0..K]` was refused at the declaration (E0602) when the bound was
/// anything but a bare literal.
#[rstest]
fn constant_bounded_array_runs(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR CONSTANT K : INT := 3; END_VAR
        VAR a : ARRAY[0..K] OF INT; i : INT; s : INT; END_VAR
            FOR i := 0 TO K DO
                a[i] := i * 10;
            END_FOR;
            FOR i := 0 TO K DO
                s := s + a[i];
            END_FOR;
            test := s;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm_checked(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(r, 60, "four elements, 0 + 10 + 20 + 30");
}
