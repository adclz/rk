// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! `ARRAY[*]` parameters: each call binds an array of its own bounds, and
//! the function is compiled once per array type it is called with, its
//! bounds constants in each copy. `LOWER_BOUND` and `UPPER_BOUND` read them.

use crate::tests::codegen::{execute_wasm, with_db};
use rstest::*;

/// `source` compiled after `USING Std.Arrays;`, the stdlib's array functions
/// loaded and lowered with it, as `rk` builds.
fn compile(db: &mut db::RootDatabase, source: &str) -> Vec<u8> {
    crate::tests::codegen::compile_with_libraries(
        db,
        &[include_str!("../../../stdlib/Arrays.st")],
        &format!("USING Std.Arrays;\n{source}"),
    )
}

/// One function sums arrays of three different bounds, a negative lower
/// bound among them.
#[rstest]
fn a_function_takes_arrays_of_any_bounds(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Sum : DINT
        VAR_IN_OUT values : ARRAY[*] OF INT; END_VAR
        VAR i : DINT; END_VAR
            FOR i := LOWER_BOUND(values, 1) TO UPPER_BOUND(values, 1) DO
                Sum := Sum + values[i];
            END_FOR;
        END_FUNCTION

        FUNCTION run : DINT
        VAR
            three : ARRAY[0..2] OF INT := [1, 2, 3];
            five : ARRAY[1..5] OF INT := [10, 20, 30, 40, 50];
            around : ARRAY[-2..2] OF INT := [100, 200, 300, 400, 500];
        END_VAR
            run := Sum(three) + Sum(values := five) + Sum(around);
        END_FUNCTION
    "#;
    let result: i32 = execute_wasm(&compile(&mut with_db, source), "run", ());
    assert_eq!(result, 6 + 150 + 1500);
}

/// Each dimension has its bounds, read by its number.
#[rstest]
fn the_bounds_of_each_dimension(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Trace : DINT
        VAR_IN_OUT m : ARRAY[*, *] OF DINT; END_VAR
        VAR i : DINT; END_VAR
            FOR i := LOWER_BOUND(ARR := m, DIM := 1) TO UPPER_BOUND(ARR := m, DIM := 1) DO
                Trace := Trace + m[i, i - LOWER_BOUND(m, 1) + LOWER_BOUND(m, 2)];
            END_FOR;
        END_FUNCTION

        FUNCTION Shape : DINT
        VAR_IN_OUT m : ARRAY[*, *] OF DINT; END_VAR
            Shape := LOWER_BOUND(m, 1) * 1000 + UPPER_BOUND(m, 1) * 100
                + LOWER_BOUND(m, 2) * 10 + UPPER_BOUND(m, 2);
        END_FUNCTION

        FUNCTION run : DINT
        VAR
            m : ARRAY[1..3, 4..6] OF DINT := [1, 0, 0, 0, 2, 0, 0, 0, 3];
        END_VAR
            run := Shape(m) * 10 + Trace(m);
        END_FUNCTION
    "#;
    let result: i32 = execute_wasm(&compile(&mut with_db, source), "run", ());
    assert_eq!(result, 1346 * 10 + 6);
}

/// A VAR_INPUT is the function's own copy; a VAR_OUTPUT is written into the
/// array it is bound to.
#[rstest]
fn inputs_are_copies_and_outputs_are_written(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Doubled
        VAR_INPUT source : ARRAY[*] OF DINT; END_VAR
        VAR_OUTPUT target : ARRAY[*] OF DINT; END_VAR
        VAR i : DINT; END_VAR
            FOR i := LOWER_BOUND(source, 1) TO UPPER_BOUND(source, 1) DO
                source[i] := source[i] * 2;
                target[i] := source[i];
            END_FOR;
        END_FUNCTION

        FUNCTION run : DINT
        VAR
            a : ARRAY[1..3] OF DINT := [1, 2, 3];
            b : ARRAY[1..3] OF DINT;
        END_VAR
            Doubled(source := a, target => b);
            run := a[1] * 100000 + a[3] * 10000 + b[1] * 100 + b[3];
        END_FUNCTION
    "#;
    let result: i32 = execute_wasm(&compile(&mut with_db, source), "run", ());
    assert_eq!(
        result,
        100000 + 30000 + 200 + 6,
        "a is unchanged, b doubled"
    );
}

/// A parameter passed on to another `ARRAY[*]` keeps the bounds of the array
/// the first call bound.
#[rstest]
fn a_parameter_passed_on_keeps_its_bounds(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Count : DINT
        VAR_IN_OUT values : ARRAY[*] OF INT; END_VAR
            Count := UPPER_BOUND(values, 1) - LOWER_BOUND(values, 1) + 1;
        END_FUNCTION

        FUNCTION Twice : DINT
        VAR_IN_OUT values : ARRAY[*] OF INT; END_VAR
            Twice := Count(values) * 2;
        END_FUNCTION

        FUNCTION run : DINT
        VAR
            a : ARRAY[0..3] OF INT;
            b : ARRAY[5..14] OF INT;
        END_VAR
            run := Twice(a) * 100 + Twice(b);
        END_FUNCTION
    "#;
    let result: i32 = execute_wasm(&compile(&mut with_db, source), "run", ());
    assert_eq!(result, 8 * 100 + 20);
}

/// A METHOD takes one as a FUNCTION does.
#[rstest]
fn a_method_takes_an_array_of_any_bounds(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Stats
        VAR_OUTPUT largest : DINT; END_VAR
        METHOD PUBLIC Scan
        VAR_IN_OUT values : ARRAY[*] OF DINT; END_VAR
        VAR i : DINT; END_VAR
            largest := values[LOWER_BOUND(values, 1)];
            FOR i := LOWER_BOUND(values, 1) TO UPPER_BOUND(values, 1) DO
                IF values[i] > largest THEN largest := values[i]; END_IF;
            END_FOR;
        END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION run : DINT
        VAR
            s : Stats;
            a : ARRAY[0..2] OF DINT := [4, 9, 2];
            b : ARRAY[1..4] OF DINT := [7, 1, 30, 5];
        END_VAR
            s.Scan(a);
            run := s.largest * 100;
            s.Scan(values := b);
            run := run + s.largest;
        END_FUNCTION
    "#;
    let result: i32 = execute_wasm(&compile(&mut with_db, source), "run", ());
    assert_eq!(result, 930);
}

/// An element STRING keeps the capacity the caller declared: a write into a
/// `STRING[4]` element stops at four bytes.
#[rstest]
fn string_elements_keep_their_capacity(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Fill
        VAR_IN_OUT names : ARRAY[*] OF STRING; END_VAR
        VAR i : DINT; END_VAR
            FOR i := LOWER_BOUND(names, 1) TO UPPER_BOUND(names, 1) DO
                names[i] := 'abcdefgh';
            END_FOR;
        END_FUNCTION

        FUNCTION run : DINT
        VAR
            short : ARRAY[0..1] OF STRING[4];
            long : ARRAY[0..1] OF STRING[20];
            guard : DINT := 77;
        END_VAR
            Fill(short);
            Fill(long);
            IF short[1] = 'abcd' THEN run := 100; END_IF;
            IF long[0] = 'abcdefgh' THEN run := run + 10; END_IF;
            run := run + guard / 77;
        END_FUNCTION
    "#;
    let result: i32 = execute_wasm(&compile(&mut with_db, source), "run", ());
    assert_eq!(result, 111, "four bytes in STRING[4], eight in STRING[20]");
}

/// A subscript outside the bounds of the array the call bound faults, as on
/// any array.
#[rstest]
fn a_subscript_past_the_bound_array_faults(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Item : INT
        VAR_IN_OUT values : ARRAY[*] OF INT; END_VAR
        VAR_INPUT i : DINT; END_VAR
            Item := values[i];
        END_FUNCTION

        FUNCTION run : DINT
        VAR a : ARRAY[1..3] OF INT := [1, 2, 3]; END_VAR
            run := Item(a, 2) + Item(a, 4);
        END_FUNCTION
    "#;
    let wasm = compile(&mut with_db, source);
    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let (instance, memory) = super::instantiate_returning_memory(&mut store, &module);
    let f = instance
        .get_typed_func::<(), i32>(&mut store, "run")
        .unwrap();
    let err = f.call(&mut store, ()).expect_err("a[4] on [1..3]");
    let msg = super::fault_message(&mut store, memory, err);
    assert!(msg.contains("array index out of bounds"), "got: {msg}");
}

/// A FUNCTION_BLOCK's body runs as its copy for the array each call binds:
/// one instance, called with two arrays of different bounds.
#[rstest]
fn a_block_body_runs_for_the_array_each_call_binds(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Sum : DINT
        VAR_IN_OUT values : ARRAY[*] OF INT; END_VAR
        VAR i : DINT; END_VAR
            FOR i := LOWER_BOUND(values, 1) TO UPPER_BOUND(values, 1) DO
                Sum := Sum + values[i];
            END_FOR;
        END_FUNCTION

        FUNCTION_BLOCK Totalizer
        VAR_IN_OUT values : ARRAY[*] OF INT; END_VAR
        VAR_OUTPUT total : DINT; count : DINT; END_VAR
            total := Sum(values);
            count := UPPER_BOUND(values, 1) - LOWER_BOUND(values, 1) + 1;
            values[LOWER_BOUND(values, 1)] := 0;
        END_FUNCTION_BLOCK

        FUNCTION run : DINT
        VAR
            t : Totalizer;
            a : ARRAY[1..3] OF INT := [1, 2, 3];
            b : ARRAY[-1..2] OF INT := [10, 20, 30, 40];
        END_VAR
            t(values := a);
            run := t.total * 10 + t.count;
            t(values := b);
            run := run * 1000 + t.total * 10 + t.count;
            run := run * 10 + a[1] + b[-1];
        END_FUNCTION
    "#;
    let result: i32 = execute_wasm(&compile(&mut with_db, source), "run", ());
    assert_eq!(
        result,
        ((6 * 10 + 3) * 1000 + 100 * 10 + 4) * 10,
        "each call saw its own bounds, and both first elements were reset"
    );
}

/// `SUPER()` runs the base's body in the same call, bound alike.
#[rstest]
fn super_runs_the_base_body_for_the_same_array(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Base
        VAR_IN_OUT values : ARRAY[*] OF DINT; END_VAR
        VAR_OUTPUT first : DINT; END_VAR
            first := values[LOWER_BOUND(values, 1)];
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Derived EXTENDS Base
        VAR_OUTPUT last : DINT; END_VAR
            SUPER();
            last := values[UPPER_BOUND(values, 1)];
        END_FUNCTION_BLOCK

        FUNCTION run : DINT
        VAR
            d : Derived;
            a : ARRAY[0..1] OF DINT := [3, 4];
            b : ARRAY[5..7] OF DINT := [5, 6, 7];
        END_VAR
            d(values := a);
            run := d.first * 10 + d.last;
            d(values := b);
            run := run * 100 + d.first * 10 + d.last;
        END_FUNCTION
    "#;
    let result: i32 = execute_wasm(&compile(&mut with_db, source), "run", ());
    assert_eq!(result, 3457);
}

/// A `DIM` outside the array's dimensions raises, as the standard has it.
#[rstest]
fn a_dimension_out_of_range_raises(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION run : DINT
        VAR m : ARRAY[1..3, 1..4] OF INT; d : DINT := 3; END_VAR
            run := UPPER_BOUND(m, 2) + UPPER_BOUND(m, d);
        END_FUNCTION
    "#;
    let wasm = compile(&mut with_db, source);
    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let (instance, memory) = super::instantiate_returning_memory(&mut store, &module);
    let f = instance
        .get_typed_func::<(), i32>(&mut store, "run")
        .unwrap();
    let err = f.call(&mut store, ()).expect_err("dimension 3 of 2");
    let msg = super::fault_message(&mut store, memory, err);
    assert!(msg.contains("array dimension out of range"), "got: {msg}");
}

/// An `ARRAY[*]` of any type takes arrays of every type and number of
/// dimensions, a CONSTANT one included: nothing reads its elements, so it
/// is passed by address. One copy serves the arrays of the same bounds.
#[rstest]
fn an_array_of_any_type_takes_every_array(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Point : STRUCT x : REAL; y : REAL; END_STRUCT END_TYPE

        FUNCTION Count : DINT
        VAR_INPUT a : ARRAY[*]; END_VAR
            Count := UPPER_BOUND(a, 1) - LOWER_BOUND(a, 1) + 1;
        END_FUNCTION

        FUNCTION Cells : DINT
        VAR_IN_OUT a : ARRAY[*]; END_VAR
            Cells := Count(a) * (UPPER_BOUND(a, 2) - LOWER_BOUND(a, 2) + 1);
        END_FUNCTION

        FUNCTION run : DINT
        VAR CONSTANT table : ARRAY[0..9] OF INT := [10(7)]; END_VAR
        VAR
            reals : ARRAY[0..9] OF REAL;
            points : ARRAY[1..3] OF Point;
            grid : ARRAY[1..2, 1..5] OF BOOL;
        END_VAR
            run := Count(table) * 1000 + Count(reals) * 100 + Count(points) * 10;
            run := run + Cells(grid);
        END_FUNCTION
    "#;
    let result: i32 = execute_wasm(&compile(&mut with_db, source), "run", ());
    assert_eq!(result, 10 * 1000 + 10 * 100 + 3 * 10 + 10);
}

/// A row of a two-dimensional array binds to an `ARRAY[*]`, as the standard's
/// `SUM(A2[2])` does: in place for a VAR_IN_OUT, copied for a VAR_INPUT.
#[rstest]
fn a_row_binds_to_an_array_of_one_dimension(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Sum : DINT
        VAR_IN_OUT values : ARRAY[*] OF INT; END_VAR
        VAR i : DINT; END_VAR
            FOR i := LOWER_BOUND(values, 1) TO UPPER_BOUND(values, 1) DO
                Sum := Sum + values[i];
            END_FOR;
            values[LOWER_BOUND(values, 1)] := 0;
        END_FUNCTION

        FUNCTION Peek : DINT
        VAR_INPUT values : ARRAY[*] OF INT; END_VAR
            Peek := LOWER_BOUND(values, 1) * 100 + values[UPPER_BOUND(values, 1)];
            values[UPPER_BOUND(values, 1)] := 0;
        END_FUNCTION

        FUNCTION run : DINT
        VAR m : ARRAY[1..20, -2..2] OF INT := [20(5(1))]; END_VAR
            m[3, 2] := 9;
            run := Sum(m[2]) * 10000 + Peek(m[3]) * 10;
            run := run + m[2, -2] + m[3, 2];
        END_FUNCTION
    "#;
    let result: i32 = execute_wasm(&compile(&mut with_db, source), "run", ());
    assert_eq!(
        result,
        5 * 10000 + (-2 * 100 + 9) * 10 + 9,
        "Sum reset the row in place, Peek changed only its copy"
    );
}

/// A row of a STRING array is an array, not a STRING: it is passed as one,
/// in place, to an `ARRAY[*] OF STRING` and to an `ARRAY[*]`. The call
/// passed a STRING's capacity beside its address, and the module did not
/// validate.
#[rstest]
fn a_row_of_strings_binds_as_an_array(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Shift : DINT
        VAR_IN_OUT names : ARRAY[*] OF STRING[4]; END_VAR
        VAR i : DINT; END_VAR
            FOR i := LOWER_BOUND(names, 1) TO UPPER_BOUND(names, 1) - 1 DO
                names[i] := names[i + 1];
            END_FOR;
            Shift := UPPER_BOUND(names, 1) - LOWER_BOUND(names, 1) + 1;
        END_FUNCTION

        FUNCTION run : DINT
        VAR m : ARRAY[1..2, 0..2] OF STRING[4] := ['a', 'bb', 'ccc', 'dddd', 'e', 'f']; END_VAR
            run := Shift(m[2]) * 100 + UPPER_BOUND(m[1], 1) * 10;
            IF m[2, 0] = 'e' AND m[2, 1] = 'f' AND m[1, 0] = 'a' THEN
                run := run + 1;
            END_IF;
        END_FUNCTION
    "#;
    let result: i32 = execute_wasm(&compile(&mut with_db, source), "run", ());
    assert_eq!(
        result,
        3 * 100 + 2 * 10 + 1,
        "m[2] has 3 elements and moved left, m[1] ends at 2 and is untouched"
    );
}

/// A row of a three-dimensional array has two dimensions, and a row of
/// that row one: each binds to the `ARRAY[*]` of its rank, in place.
#[rstest]
fn the_rows_of_three_dimensions_bind_by_rank(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Shape : DINT
        VAR_IN_OUT m : ARRAY[*, *] OF INT; END_VAR
            Shape := (UPPER_BOUND(m, 1) - LOWER_BOUND(m, 1) + 1) * 10
                + UPPER_BOUND(m, 2) - LOWER_BOUND(m, 2) + 1;
            m[LOWER_BOUND(m, 1), UPPER_BOUND(m, 2)] := 7;
        END_FUNCTION

        FUNCTION Sum : DINT
        VAR_IN_OUT values : ARRAY[*] OF INT; END_VAR
        VAR i : DINT; END_VAR
            FOR i := LOWER_BOUND(values, 1) TO UPPER_BOUND(values, 1) DO
                Sum := Sum + values[i];
            END_FOR;
        END_FUNCTION

        FUNCTION run : DINT
        VAR cube : ARRAY[1..2, 0..2, -1..2] OF INT := [24(1)]; END_VAR
            run := Shape(cube[2]) * 10000;
            run := run + Sum(cube[2, 0]) * 100;
            run := run + Sum(cube[1, 0]);
        END_FUNCTION
    "#;
    let result: i32 = execute_wasm(&compile(&mut with_db, source), "run", ());
    assert_eq!(
        result,
        34 * 10000 + 10 * 100 + 4,
        "cube[2] is 3 rows of 4, Shape set cube[2, 0, 2] to 7, cube[1] is untouched"
    );
}

/// Overloads rank an array's fits: a parameter declaring its bounds, then
/// an `ARRAY[*]` of its element type, then an `ARRAY[*]` of any type.
#[rstest]
fn the_most_specific_array_overload_is_picked(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Kind : DINT
        VAR_INPUT a : ARRAY[0..1] OF INT; END_VAR
            Kind := 1;
        END_FUNCTION

        FUNCTION Kind : DINT
        VAR_INPUT a : ARRAY[*] OF INT; END_VAR
            Kind := 2;
        END_FUNCTION

        FUNCTION Kind : DINT
        VAR_INPUT a : ARRAY[*]; END_VAR
            Kind := 3;
        END_FUNCTION

        FUNCTION run : DINT
        VAR pair : ARRAY[0..1] OF INT; ints : ARRAY[1..5] OF INT; reals : ARRAY[0..1] OF REAL; END_VAR
            run := Kind(pair) * 100 + Kind(ints) * 10 + Kind(reals);
        END_FUNCTION
    "#;
    let result: i32 = execute_wasm(&compile(&mut with_db, source), "run", ());
    assert_eq!(result, 123);
}

/// A method taking an `ARRAY[*]`, called through an interface: the
/// implementer's method, for the array the call binds.
#[rstest]
fn a_method_through_an_interface_takes_an_array(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE IScale
        METHOD Scale
        VAR_IN_OUT values : ARRAY[*] OF DINT; END_VAR
        END_METHOD
        END_INTERFACE

        FUNCTION_BLOCK Doubler IMPLEMENTS IScale
        METHOD PUBLIC Scale
        VAR_IN_OUT values : ARRAY[*] OF DINT; END_VAR
        VAR i : DINT; END_VAR
            FOR i := LOWER_BOUND(values, 1) TO UPPER_BOUND(values, 1) DO
                values[i] := values[i] * 2;
            END_FOR;
        END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION Apply
        VAR_INPUT s : IScale; END_VAR
        VAR_IN_OUT values : ARRAY[*] OF DINT; END_VAR
            s.Scale(values);
        END_FUNCTION

        FUNCTION run : DINT
        VAR d : Doubler; a : ARRAY[1..2] OF DINT := [1, 2]; b : ARRAY[0..2] OF DINT := [3, 4, 5]; END_VAR
            Apply(d, a);
            Apply(d, b);
            run := a[1] * 10000 + a[2] * 1000 + b[0] * 100 + b[1] * 10 + b[2];
        END_FUNCTION
    "#;
    let result: i32 = execute_wasm(&compile(&mut with_db, source), "run", ());
    assert_eq!(result, 2 * 10000 + 4 * 1000 + 6 * 100 + 8 * 10 + 10);
}

/// A copy for an `ARRAY[*]` of any type depends on the bounds alone: arrays
/// of INT and of REAL indexed 0 to 9 share one.
#[rstest]
fn a_copy_for_any_type_is_one_per_bounds(mut with_db: db::RootDatabase) {
    let source = r#"
        USING Std.Arrays;

        FUNCTION Count : DINT
        VAR_INPUT a : ARRAY[*]; END_VAR
            Count := UPPER_BOUND(a, 1) - LOWER_BOUND(a, 1) + 1;
        END_FUNCTION

        FUNCTION run : DINT
        VAR ints : ARRAY[0..9] OF INT; reals : ARRAY[0..9] OF REAL; few : ARRAY[1..3] OF INT; END_VAR
            run := Count(ints) + Count(reals) + Count(few);
        END_FUNCTION
    "#;
    crate::tests::utils::add_library_sources(
        &mut with_db,
        &[include_str!("../../../stdlib/Arrays.st")],
    );
    crate::tests::utils::add_source(&mut with_db, source);
    let module = crate::tests::utils::lower_workspace(&with_db);
    let mut copies: Vec<String> = module
        .functions
        .iter()
        .map(|f| f.name.text(&with_db).to_string())
        .filter(|name| name.starts_with("Count"))
        .collect();
    copies.sort();
    assert_eq!(copies, ["Count$[0..9]", "Count$[1..3]"]);
}

/// With several `ARRAY[*]` parameters, a copy stands for one combination
/// of array types: two products of matrices of the same types share one,
/// a third over other types has its own.
#[rstest]
fn a_copy_is_one_per_combination_of_array_types(mut with_db: db::RootDatabase) {
    let source = r#"
        USING Std.Arrays;

        FUNCTION MATRIX_MUL
        VAR_INPUT
            A : ARRAY[*, *] OF INT;
            B : ARRAY[*, *] OF INT;
        END_VAR
        VAR_OUTPUT C : ARRAY[*, *] OF INT; END_VAR
        VAR i, j, k : DINT; END_VAR
            FOR i := LOWER_BOUND(A, 1) TO UPPER_BOUND(A, 1) DO
                FOR j := LOWER_BOUND(B, 2) TO UPPER_BOUND(B, 2) DO
                    C[i, j] := 0;
                    FOR k := LOWER_BOUND(A, 2) TO UPPER_BOUND(A, 2) DO
                        C[i, j] := C[i, j] + A[i, k] * B[k, j];
                    END_FOR;
                END_FOR;
            END_FOR;
        END_FUNCTION

        FUNCTION run
        VAR
            a, a2 : ARRAY[1..2, 1..3] OF INT;
            b : ARRAY[1..3, 1..2] OF INT;
            c, c2 : ARRAY[1..2, 1..2] OF INT;
            d : ARRAY[1..3, 1..3] OF INT;
        END_VAR
            MATRIX_MUL(A := a, B := b, C => c);
            MATRIX_MUL(A := a2, B := b, C => c2);
            MATRIX_MUL(A := b, B := a, C => d);
        END_FUNCTION
    "#;
    crate::tests::utils::add_library_sources(
        &mut with_db,
        &[include_str!("../../../stdlib/Arrays.st")],
    );
    crate::tests::utils::add_source(&mut with_db, source);
    let module = crate::tests::utils::lower_workspace(&with_db);
    let mut copies: Vec<String> = module
        .functions
        .iter()
        .map(|f| f.name.text(&with_db).to_string())
        .filter(|name| name.starts_with("MATRIX_MUL"))
        .collect();
    copies.sort();
    assert_eq!(
        copies,
        [
            "MATRIX_MUL$[1..2,1..3]$[1..3,1..2]$[1..2,1..2]",
            "MATRIX_MUL$[1..3,1..2]$[1..2,1..3]$[1..3,1..3]",
        ]
    );
}

// The examples of IEC 61131-3, 6.5.3 (Table 15), as the standard prints them
// but for: a `DO` after each `FOR`, which the text leaves out; DINT where the
// bounds land, as `LOWER_BOUND` and `UPPER_BOUND` return one where the
// standard says ANY_INT; and no `;` after a FUNCTION's result type.

/// Example 1: the bounds of each dimension, and an error past them.
#[rstest]
fn the_standards_bounds_example(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION bounds : DINT
        VAR_INPUT upper : BOOL; which : DINT; dim : DINT; END_VAR
        VAR
            A1: ARRAY [1..10] OF INT := [10(1)];
            A2: ARRAY [1..20, -2..2] OF INT := [20(5(1))];
        END_VAR
            IF which = 1 AND upper THEN bounds := UPPER_BOUND(A1, dim);
            ELSIF which = 1 THEN bounds := LOWER_BOUND(A1, dim);
            ELSIF upper THEN bounds := UPPER_BOUND(A2, dim);
            ELSE bounds := LOWER_BOUND(A2, dim);
            END_IF;
        END_FUNCTION
    "#;
    let wasm = compile(&mut with_db, source);
    let bounds = |upper: bool, which: i32, dim: i32| -> Result<i32, String> {
        let engine = crate::tests::codegen::test_engine();
        let module = wasmtime::Module::new(&engine, &wasm).unwrap();
        let mut store = wasmtime::Store::new(&engine, ());
        let (instance, memory) = super::instantiate_returning_memory(&mut store, &module);
        let f = instance
            .get_typed_func::<(i32, i32, i32), i32>(&mut store, "bounds")
            .unwrap();
        f.call(&mut store, (upper as i32, which, dim))
            .map_err(|err| super::fault_message(&mut store, memory, err))
    };
    assert_eq!(bounds(false, 1, 1), Ok(1), "LOWER_BOUND (A1, 1)");
    assert_eq!(bounds(true, 1, 1), Ok(10), "UPPER_BOUND (A1, 1)");
    assert_eq!(bounds(false, 2, 1), Ok(1), "LOWER_BOUND (A2, 1)");
    assert_eq!(bounds(true, 2, 1), Ok(20), "UPPER_BOUND (A2, 1)");
    assert_eq!(bounds(false, 2, 2), Ok(-2), "LOWER_BOUND (A2, 2)");
    assert_eq!(bounds(true, 2, 2), Ok(2), "UPPER_BOUND (A2, 2)");
    for dim in [0, 3] {
        let err = bounds(false, 2, dim).expect_err("LOWER_BOUND (A2, {dim}) is an error");
        assert!(err.contains("array dimension out of range"), "got: {err}");
    }
}

/// Example 2: the sum of an array, and of a row of one.
#[rstest]
fn the_standards_summation_example(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION SUM: DINT
        VAR_IN_OUT A: ARRAY [*] OF INT; END_VAR;
        VAR i, sum2: DINT; END_VAR;
        sum2:= 0;
        FOR i:= LOWER_BOUND(A,1) TO UPPER_BOUND(A,1) DO
          sum2:= sum2 + A[i];
        END_FOR;
        SUM:= sum2;
        END_FUNCTION

        FUNCTION run : DINT
        VAR
            A1: ARRAY [1..10] OF INT := [10(1)];
            A2: ARRAY [1..20, -2..2] OF INT := [20(5(1))];
        END_VAR
            run := SUM (A1) * 100 + SUM (A2[2]);
        END_FUNCTION
    "#;
    let result: i32 = execute_wasm(&compile(&mut with_db, source), "run", ());
    assert_eq!(result, 10 * 100 + 5, "SUM (A1) is 10, SUM (A2[2]) is 5");
}

/// Example 3: matrix multiplication, of a 5 by 3 and a 3 by 4 matrix.
#[rstest]
fn the_standards_matrix_example(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION MATRIX_MUL
        VAR_INPUT
          A: ARRAY [*, *] OF INT;
          B: ARRAY [*, *] OF INT;
        END_VAR;
        VAR_OUTPUT C: ARRAY [*, *] OF INT; END_VAR;
        VAR i, j, k: DINT; s: INT; END_VAR;
        FOR i:= LOWER_BOUND(A,1) TO UPPER_BOUND(A,1) DO
          FOR j:= LOWER_BOUND(B,2) TO UPPER_BOUND(B,2) DO
          s:= 0;
            FOR k:= LOWER_BOUND(A,2) TO UPPER_BOUND(A,2) DO
            s:= s + A[i,k] * B[k,j];
            END_FOR;
          C[i,j]:= s;
          END_FOR;
        END_FOR;
        END_FUNCTION

        FUNCTION run : DINT
        VAR
          A: ARRAY [1..5, 1..3] OF INT := [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];
          B: ARRAY [1..3, 1..4] OF INT := [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
          C: ARRAY [1..5, 1..4] OF INT;
          c11, c23, c54: DINT;
        END_VAR
          MATRIX_MUL (A, B, C);
          c11 := C[1, 1];
          c23 := C[2, 3];
          c54 := C[5, 4];
          run := c11 * 1000000 + c23 * 1000 + c54;
        END_FUNCTION
    "#;
    let result: i32 = execute_wasm(&compile(&mut with_db, source), "run", ());
    assert_eq!(
        result,
        38 * 1_000_000 + 113 * 1000 + 344,
        "C[1,1] = 1+10+27, C[2,3] = 12+35+66, C[5,4] = 52+112+180"
    );
}
