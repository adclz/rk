// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! REF_TO (pointer/reference type) code generation tests.

use crate::tests::codegen::{compile_to_wasm, validate_wasm, with_db};
use rstest::*;

#[rstest]
fn test_null_ref(mut with_db: db::RootDatabase) {
    // A return type is a type NAME, so a REF_TO return is spelled through a
    // declared one: `FUNCTION f : REF_TO INT` is not a return-type production.
    let source = r#"
        TYPE PInt : REF_TO INT; END_TYPE

        FUNCTION test_null : PInt
            test_null := NULL;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    // Call function
    let result = super::execute_wasm::<(), i32>(&wasm_bytes, "test_null", ());
    assert_eq!(result, 0, "NULL should be represented as 0");
}

#[rstest]
fn test_ref_to_var_in_out(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION get_ptr_value : INT
        VAR_IN_OUT
            ptr : INT;
        END_VAR
        VAR
            ref_ptr : REF_TO INT;
        END_VAR
            ref_ptr := REF(ptr);
            get_ptr_value := ref_ptr^;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());

    let instance = super::instantiate_with_memory(&mut store, &module);

    // Address 0 is NULL, which a dereference now faults on, so the caller's
    // storage sits at a real address — as it does in a compiled program,
    // where IEC allocation starts above the reserved floor.
    const CALLER_SLOT: i32 = 16_384;
    let memory = instance
        .get_memory(&mut store, "memory")
        .expect("Failed to get memory");
    memory
        .write(&mut store, CALLER_SLOT as usize, &42i32.to_le_bytes())
        .unwrap();

    let func = instance
        .get_typed_func::<i32, i32>(&mut store, "get_ptr_value")
        .unwrap();

    let result = func.call(&mut store, CALLER_SLOT).unwrap();
    assert_eq!(result, 42, "Should read value through REF_TO pointer");
}

#[rstest]
fn test_ref_to_assignment_var_in_out(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION set_ptr_value : INT
        VAR_IN_OUT
            target : INT;
        END_VAR
        VAR
            ptr : REF_TO INT;
        END_VAR
            ptr := REF(target);
            ptr^ := 99;
            set_ptr_value := 0;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());

    let instance = super::instantiate_with_memory(&mut store, &module);

    // Address 0 is NULL — see test_ref_to_var_in_out.
    const CALLER_SLOT: i32 = 16_384;
    let memory = instance
        .get_memory(&mut store, "memory")
        .expect("Failed to get memory");
    memory
        .write(&mut store, CALLER_SLOT as usize, &0i32.to_le_bytes())
        .unwrap();

    let func = instance
        .get_typed_func::<i32, i32>(&mut store, "set_ptr_value")
        .unwrap();

    func.call(&mut store, CALLER_SLOT).unwrap();

    // Read back the modified value
    let mut buffer = [0u8; 4];
    memory
        .read(&store, CALLER_SLOT as usize, &mut buffer)
        .unwrap();
    let value = i32::from_le_bytes(buffer);

    assert_eq!(value, 99, "Should write through REF_TO pointer");
}

#[rstest]
fn test_ref_to_local_read(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_ref : INT
        VAR
            x : INT := 42;
            ptr : REF_TO INT;
        END_VAR
            ptr := REF(x);
            test_ref := ptr^;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    let result = super::execute_wasm::<(), i32>(&wasm_bytes, "test_ref", ());
    assert_eq!(result, 42, "Should read value through pointer");
}

#[rstest]
fn test_ref_to_local_write(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_ref_assign : INT
        VAR
            x : INT := 10;
            ptr : REF_TO INT;
        END_VAR
            ptr := REF(x);
            ptr^ := 99;
            test_ref_assign := x;
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    let result = super::execute_wasm::<(), i32>(&wasm_bytes, "test_ref_assign", ());
    assert_eq!(result, 99, "Should modify x through pointer");
}

#[rstest]
fn test_multiple_deref(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test_double_deref : INT
        VAR
            x : INT := 5;
            ptr : REF_TO INT;
            ptrptr : REF_TO REF_TO INT;
        END_VAR
            ptr := REF(x);
            ptrptr := REF(ptr);
            test_double_deref := ptrptr^^;
        END_FUNCTION
        
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");

    let result = super::execute_wasm::<(), i32>(&wasm_bytes, "test_double_deref", ());
    assert_eq!(result, 5, "Should dereference twice to get original value");
}

#[rstest]
fn test_ref_arg_through_local(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION peek : WORD
        VAR_INPUT
            p : REF_TO WORD;
        END_VAR
            peek := p^;
        END_FUNCTION

        FUNCTION test_main : WORD
        VAR
            regs : ARRAY[0..8000] OF WORD;
            q : REF_TO WORD;
        END_VAR
            regs[7000] := WORD#16#BEEF;
            q := REF(regs[7000]);
            test_main := peek(p := q);
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
    let result: i32 = crate::tests::codegen::execute_wasm(&wasm_bytes, "test_main", ());
    assert_eq!(result, 0xBEEF);
}

#[rstest]
fn test_inline_ref_arg_past_32k(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION peek : WORD
        VAR_INPUT
            p : REF_TO WORD;
        END_VAR
            peek := p^;
        END_FUNCTION

        FUNCTION test_main : WORD
        VAR
            regs : ARRAY[0..8000] OF WORD;
        END_VAR
            regs[7000] := WORD#16#BEEF;
            test_main := peek(p := REF(regs[7000]));
        END_FUNCTION
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);
    validate_wasm(&wasm_bytes).expect("WASM validation failed");
    let result: i32 = crate::tests::codegen::execute_wasm(&wasm_bytes, "test_main", ());
    assert_eq!(result, 0xBEEF);
}

// --- Aggregates reached through a dereference ---
//
// `lower_type` answers `Pointer(Void)` for a REF_TO, because resolving the
// pointee there would not terminate on a type holding a reference to itself.
// The layout sites read that Void and reported "unsupported type", so every
// aggregate access through a `^` was an internal compiler error from code
// `rk check` called clean. These pin the VALUES, not just that it compiles.

#[rstest]
fn test_deref_struct_field_round_trips(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE S : STRUCT a : INT; b : INT; END_STRUCT; END_TYPE

        FUNCTION test : INT
        VAR
            s : S;
            q : REF_TO S := REF(s);
        END_VAR
            q^.a := 11;
            q^.b := 31;
            test := q^.a + s.b;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 42, "writes through q^ must land in s's own fields");
}

#[rstest]
fn test_deref_struct_field_addresses_the_right_slot(mut with_db: db::RootDatabase) {
    // A wrong field offset still compiles and still returns a number, so the
    // second field is read back through the struct to catch an off-by-one slot.
    let source = r#"
        TYPE S : STRUCT a : INT; b : INT; c : INT; END_STRUCT; END_TYPE

        FUNCTION test : INT
        VAR
            s : S;
            q : REF_TO S := REF(s);
        END_VAR
            s.a := 1;
            s.b := 2;
            s.c := 3;
            q^.b := 7;
            test := s.a * 100 + s.b * 10 + s.c;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 173, "only b changes: a=1, b=7, c=3");
}

#[rstest]
fn test_deref_array_element_round_trips(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR
            a : ARRAY[0..3] OF INT;
            r : REF_TO ARRAY[0..3] OF INT := REF(a);
        END_VAR
            r^[1] := 40;
            r^[2] := 2;
            test := a[1] + r^[2];
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 42,
        "element writes through r^ must land at the right stride"
    );
}

#[rstest]
fn test_deref_fb_output_reads_through_the_reference(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK counter
        VAR_INPUT step : INT; END_VAR
        VAR_OUTPUT total : INT; END_VAR
            total := total + step;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR
            c : counter;
            f : REF_TO counter := REF(c);
        END_VAR
            c(step := 20);
            c(step := 22);
            test := f^.total;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 42, "f^ addresses c's own instance state");
}

#[rstest]
fn test_deref_self_referential_struct(mut with_db: db::RootDatabase) {
    // The pointee is resolved one level at the use site, so a type that holds
    // a reference to itself lowers without recursing forever.
    let source = r#"
        TYPE Node : STRUCT value : INT; next : REF_TO Node; END_STRUCT; END_TYPE

        FUNCTION test : INT
        VAR
            head : Node;
            p : REF_TO Node := REF(head);
        END_VAR
            p^.value := 42;
            test := head.value;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 42);
}

// A reference that ARRIVES AS A PARAMETER takes a different address path: the
// callee reads a wasm local holding a passed pointer, rather than computing an
// address in its own frame. And every test above reads back through a local in
// the frame that owns the storage, so a `q^` addressing a COPY would still
// pass. These observe the write from the OTHER side of a call.

#[rstest]
fn test_deref_struct_field_through_parameter_reference(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE S : STRUCT a : INT; b : INT; END_STRUCT; END_TYPE

        FUNCTION bump : INT
        VAR_INPUT
            p : REF_TO S;
        END_VAR
            p^.a := p^.a + 1;
            bump := p^.b;
        END_FUNCTION

        FUNCTION test : INT
        VAR
            s : S;
            got : INT;
        END_VAR
            s.a := 10;
            s.b := 5;
            got := bump(p := REF(s));
            test := got * 100 + s.a;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 511,
        "callee read b=5 through the parameter, and its write to a reached the CALLER's s (11)"
    );
}

#[rstest]
fn test_deref_array_element_through_parameter_reference(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE A4 : ARRAY[0..3] OF INT; END_TYPE

        FUNCTION fill : INT
        VAR_INPUT
            p : REF_TO A4;
        END_VAR
            p^[2] := 40;
            fill := p^[0];
        END_FUNCTION

        FUNCTION test : INT
        VAR
            a : A4;
            got : INT;
        END_VAR
            a[0] := 2;
            got := fill(p := REF(a));
            test := a[2] + got;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 42,
        "the element write reached the CALLER's a[2] (40), and a[0] read back as 2"
    );
}

#[rstest]
fn test_deref_struct_write_is_visible_to_the_caller(mut with_db: db::RootDatabase) {
    // The aggregate counterpart of test_ref_to_assignment_var_in_out: take a
    // reference to a VAR_IN_OUT struct and write a field through it, then read
    // the caller's own instance. A write into a copy leaves s.a at 1.
    let source = r#"
        TYPE S : STRUCT a : INT; b : INT; END_STRUCT; END_TYPE

        FUNCTION mutate : INT
        VAR_IN_OUT
            target : S;
        END_VAR
        VAR
            q : REF_TO S;
        END_VAR
            q := REF(target);
            q^.a := 7;
            mutate := 0;
        END_FUNCTION

        FUNCTION test : INT
        VAR
            s : S;
            ignored : INT;
        END_VAR
            s.a := 1;
            s.b := 2;
            ignored := mutate(target := s);
            test := s.a * 10 + s.b;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 72,
        "s.a became 7 in the caller's own storage; s.b untouched"
    );
}

// A `REF()` in a DECLARATION initializer marks its target address-taken just
// as the statement form does. Scanning only statements left a FUNCTION's
// scalar in a wasm local, which has no address: `emit_addr_of` pushed nothing
// and the module was invalid, from a compile that exited 0. Only FUNCTION
// scalars could reach it — every other local is already memory-resident.

#[rstest]
fn test_ref_initializer_to_scalar_local(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : INT
        VAR
            x : INT := 7;
            q : REF_TO INT := REF(x);
        END_VAR
            q^ := q^ + 35;
            test := x;
        END_FUNCTION
    "#;
    let wasm = super::compile_to_wasm(&mut with_db, source);
    super::validate_wasm(&wasm).expect("WASM validation failed");
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 42, "the write through q must land in x itself");
}

#[rstest]
fn test_method_local_initializers_run(mut with_db: db::RootDatabase) {
    // A method's locals are per-call, and their declared values are stores at
    // method entry. The two METHOD paths had no initializer step at all, so
    // `x : INT := 42` silently started at 0.
    let source = r#"
        FUNCTION_BLOCK holder
            METHOD get : INT
            VAR x : INT := 42; END_VAR
                get := x;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR h : holder; END_VAR
            test := h.get();
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 42);
}

#[rstest]
fn test_class_method_local_initializers_run(mut with_db: db::RootDatabase) {
    let source = r#"
        CLASS holder
            METHOD PUBLIC get : INT
            VAR x : INT := 42; END_VAR
                get := x;
            END_METHOD
        END_CLASS

        FUNCTION test : INT
        VAR h : holder; END_VAR
            test := h.get();
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 42);
}

#[rstest]
fn test_method_local_initializers_run_every_call(mut with_db: db::RootDatabase) {
    // Per-call, not once: a method local that is modified must start again
    // from its declared value on the next call, unlike instance state.
    let source = r#"
        FUNCTION_BLOCK holder
            METHOD bump : INT
            VAR x : INT := 10; END_VAR
                x := x + 1;
                bump := x;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR h : holder; first : INT; second : INT; END_VAR
            first := h.bump();
            second := h.bump();
            test := first * 100 + second;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 1111, "both calls start from 10, so both return 11");
}

#[rstest]
fn test_ref_initializer_in_a_method(mut with_db: db::RootDatabase) {
    // The reference form, now that a method's initializers run at all: the
    // REF() marks x address-taken and q holds x's address.
    let source = r#"
        FUNCTION_BLOCK holder
            METHOD get : INT
            VAR
                x : INT := 42;
                q : REF_TO INT := REF(x);
            END_VAR
                get := q^;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR h : holder; END_VAR
            test := h.get();
        END_FUNCTION
    "#;
    let wasm = super::compile_to_wasm(&mut with_db, source);
    super::validate_wasm(&wasm).expect("WASM validation failed");
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 42);
}

#[rstest]
fn test_ref_initializer_nested_in_a_struct_initializer(mut with_db: db::RootDatabase) {
    // The scan descends into aggregate initializers, so a REF() nested inside
    // one marks its target too.
    let source = r#"
        TYPE Holder : STRUCT p : REF_TO INT; END_STRUCT; END_TYPE

        FUNCTION test : INT
        VAR
            x : INT := 42;
            h : Holder := (p := REF(x));
        END_VAR
            test := h.p^;
        END_FUNCTION
    "#;
    let wasm = super::compile_to_wasm(&mut with_db, source);
    super::validate_wasm(&wasm).expect("WASM validation failed");
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 42);
}

// --- References as a return type ---
//
// A function or method NAME on the left of an assignment is its return slot,
// which `normalize` leaves as `Type::Function`/`Type::MethodDecl`. The
// reference-coercion branch demanded a literal `RefTo` there and refused
// everything else, so `f := REF(x)` could not be written for ANY
// reference-returning POU — and the message named the slot where it wanted a
// type ("expected 'mk'").

#[rstest]
fn test_function_returns_a_reference(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE PInt : REF_TO INT; END_TYPE

        FUNCTION borrow : PInt
        VAR_IN_OUT t : INT; END_VAR
            borrow := REF(t);
        END_FUNCTION

        FUNCTION test : INT
        VAR
            x : INT := 7;
            p : PInt;
        END_VAR
            p := borrow(t := x);
            p^ := p^ + 35;
            test := x;
        END_FUNCTION
    "#;
    let wasm = super::compile_to_wasm(&mut with_db, source);
    super::validate_wasm(&wasm).expect("WASM validation failed");
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(
        result, 42,
        "the returned reference addresses the caller's x"
    );
}

#[rstest]
fn test_method_returns_a_reference_to_instance_state(mut with_db: db::RootDatabase) {
    // The reference outlives the call, so writing through it must reach the
    // instance's own storage, not a copy that died with the frame.
    let source = r#"
        TYPE PInt : REF_TO INT; END_TYPE

        FUNCTION_BLOCK counter
        VAR total : INT := 10; END_VAR
            METHOD PUBLIC slot : PInt
                slot := REF(total);
            END_METHOD
            METHOD PUBLIC read : INT
                read := total;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR
            c : counter;
            p : PInt;
        END_VAR
            p := c.slot();
            p^ := p^ + 32;
            test := c.read();
        END_FUNCTION
    "#;
    let wasm = super::compile_to_wasm(&mut with_db, source);
    super::validate_wasm(&wasm).expect("WASM validation failed");
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(
        result, 42,
        "the write through the returned reference reached c.total"
    );
}

#[rstest]
fn test_function_returns_a_reference_to_an_aggregate(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE
            S : STRUCT a : INT; b : INT; END_STRUCT;
            PS : REF_TO S;
        END_TYPE

        FUNCTION borrow : PS
        VAR_IN_OUT t : S; END_VAR
            borrow := REF(t);
        END_FUNCTION

        FUNCTION test : INT
        VAR
            s : S;
            p : PS;
        END_VAR
            s.a := 40;
            s.b := 2;
            p := borrow(t := s);
            test := p^.a + p^.b;
        END_FUNCTION
    "#;
    let wasm = super::compile_to_wasm(&mut with_db, source);
    super::validate_wasm(&wasm).expect("WASM validation failed");
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 42);
}

#[rstest]
fn test_var_temp_reference(mut with_db: db::RootDatabase) {
    // `p : REF_TO INT` in a VAR_TEMP section: the shared `ref_spec` rule
    // carried no colon of its own, so the parser wanted `p REF_TO INT` and
    // rejected the real spelling. Behind that, the spec builder returned the
    // TARGET rather than a reference to it, which typed p as a plain INT.
    let source = r#"
        FUNCTION_BLOCK holder
        VAR
            value : INT := 7;
            seen : INT;
        END_VAR
        VAR_TEMP
            scratch : REF_TO INT;
        END_VAR
            scratch := REF(value);
            scratch^ := scratch^ + 35;
            seen := value;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR h : holder; END_VAR
            h();
            test := h.seen;
        END_FUNCTION
    "#;
    let wasm = super::compile_to_wasm(&mut with_db, source);
    super::validate_wasm(&wasm).expect("WASM validation failed");
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(
        result, 42,
        "the temp reference addresses the instance member"
    );
}

/// A value as it comes back from wasm, by lane.
#[derive(Debug, PartialEq)]
enum Lane {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
}

/// A dereference reads and writes the pointee's type, not the reference's.
/// For every elementary type: `r^ := value` stores through a reference, a
/// direct read gets it back, and `r^` reads it through a second reference;
/// the function returns what that last read saw. A narrow value is written
/// as its own bytes, so the direct copy and the read through `r^` have to
/// agree on its width: -1 in a SINT comes back as -1.
#[rstest]
#[case::bool("BOOL", "TRUE", Lane::I32(1))]
#[case::sint("SINT", "-1", Lane::I32(-1))]
#[case::int("INT", "-300", Lane::I32(-300))]
#[case::dint("DINT", "-70000", Lane::I32(-70_000))]
#[case::lint("LINT", "-5000000000", Lane::I64(-5_000_000_000))]
#[case::usint("USINT", "200", Lane::I32(200))]
#[case::uint("UINT", "60000", Lane::I32(60_000))]
#[case::udint("UDINT", "4000000000", Lane::I32(4_000_000_000u32 as i32))]
#[case::ulint("ULINT", "10000000000", Lane::I64(10_000_000_000))]
#[case::byte("BYTE", "16#AB", Lane::I32(0xAB))]
#[case::word("WORD", "16#ABCD", Lane::I32(0xABCD))]
#[case::dword("DWORD", "16#DEADBEEF", Lane::I32(0xDEAD_BEEFu32 as i32))]
#[case::lword("LWORD", "16#0123456789ABCDEF", Lane::I64(0x0123_4567_89AB_CDEF))]
#[case::real("REAL", "2.5", Lane::F32(2.5))]
#[case::lreal("LREAL", "2.5", Lane::F64(2.5))]
#[case::char("CHAR", "CHAR#'A'", Lane::I32(65))]
#[case::time("TIME", "T#1s500ms", Lane::I32(1500))]
#[case::ltime("LTIME", "LT#1s", Lane::I64(1_000_000_000))]
#[case::date("DATE", "D#1970-01-02", Lane::I32(1))]
#[case::ldate("LDATE", "LD#1973-01-01", Lane::I64(1096))]
#[case::tod("TOD", "TOD#00:00:01.5", Lane::I32(1500))]
#[case::ltod("LTOD", "LTOD#00:00:00.000000042", Lane::I64(42))]
#[case::dt("DT", "DT#1970-01-01-00:01:00", Lane::I64(60))]
#[case::ldt("LDT", "LDT#1970-01-01-00:00:01", Lane::I64(1_000_000_000))]
fn test_deref_reads_and_writes_the_pointee_type(
    mut with_db: db::RootDatabase,
    #[case] ty: &str,
    #[case] value: &str,
    #[case] expected: Lane,
) {
    let source = format!(
        r#"
        FUNCTION through : {ty}
        VAR x : {ty}; y : {ty}; r : REF_TO {ty}; END_VAR
            r := REF(x);
            r^ := {value};
            y := x;
            r := REF(y);
            through := r^;
        END_FUNCTION
    "#
    );
    let wasm = super::compile_to_wasm(&mut with_db, &source);
    validate_wasm(&wasm).expect("WASM validation failed");

    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);
    let func = instance.get_func(&mut store, "through").expect("through");
    let mut results = [wasmtime::Val::I32(0)];
    func.call(&mut store, &[], &mut results).expect("call");
    let got = match results[0] {
        wasmtime::Val::I32(v) => Lane::I32(v),
        wasmtime::Val::I64(v) => Lane::I64(v),
        wasmtime::Val::F32(bits) => Lane::F32(f32::from_bits(bits)),
        wasmtime::Val::F64(bits) => Lane::F64(f64::from_bits(bits)),
        ref other => panic!("unexpected result {other:?}"),
    };
    assert_eq!(got, expected);
}

/// A type that comes back to itself through a reference, by every route a name
/// gives it. Lowering one followed the reference until the stack ran out, and a value
/// of `P : REF_TO P` could not be given to a variable of `P`: "expected 'P', got 'P'".
/// Each row runs its steps in a function with these variables, and then holds.
#[rstest]
// Through the cycle.
#[case::an_array_read_back("a1[0] := REF(a2); a1[0]^[1] := REF(a1); ra := REF(a1);", "a2[1] = ra")]
#[case::an_array_round_and_round(
    "a1[0] := REF(a1); a1[0]^[0]^[0]^[1] := REF(a2); ra := REF(a2);",
    "a1[1] = ra"
)]
#[case::null_to_start_with("", "a1[0] = NULL AND p1 = NULL")]
#[case::a_reference_held_by_another("p1 := REF(p2); p2 := REF(p1); rp := REF(p1);", "p1^ = rp")]
#[case::a_reference_twice_through("p1 := REF(p2); p2 := REF(p1); rp := REF(p2);", "p1^^ = rp")]
#[case::a_reference_to_itself("p1 := REF(p1);", "p1^ = p1 AND p1^^^^ = p1")]
#[case::two_names_holding_each_other("pa := REF(qa); qa := REF(pa);", "pa^ = qa AND pa^^ = pa")]
#[case::through_a_second_name(
    "b1[0] := REF(b2); b1[0]^[1] := REF(b1); rb := REF(b1);",
    "b2[1] = rb"
)]
#[case::through_an_alias("c1[0] := REF(c2); c1[0]^[1] := REF(c1); rc := REF(c1);", "c2[1] = rc")]
#[case::an_array_of_arrays(
    "e1[0][0] := REF(e2); e1[0][0]^[1][1] := REF(e1); re := REF(e1);",
    "e2[1][1] = re"
)]
#[case::a_struct_with_a_name("g1[0].again := REF(g2); g1[0].again^[1].n := 3;", "g2[1].n = 3")]
#[case::two_dimensions(
    "t1[1, 0] := REF(t2); t1[1, 0]^[0, 1] := REF(t1); rt := REF(t1);",
    "t2[0, 1] = rt"
)]
#[case::a_reference_to_the_array("ra := REF(a1); ra^[0] := ra;", "a1[0] = ra AND a1[0]^[0] = ra")]
#[case::written_through_a_reference(
    "p2 := REF(p3); rp := REF(p1); rp^ := p2;",
    "p1 = p2 AND p1 <> NULL"
)]
// One type, two values.
#[case::a_reference_assigned("p1 := REF(p2); p3 := p1; rp := REF(p2);", "p3 = rp")]
#[case::a_reference_compared("p1 := REF(p2);", "p1 = p1 AND p1 <> p2")]
#[case::an_array_copied("a1[0] := REF(a2); a2 := a1; ra := REF(a2);", "a2[0] = ra")]
#[case::an_element_compared("a1[0] := REF(a2);", "a1[0] = a1[0]")]
#[case::an_alias_copied("c1[0] := REF(c2); c2 := c1; rc := REF(c2);", "c2[0] = rc")]
#[case::an_array_of_them(
    "aa[1][0] := REF(aa[0]); aa[1][0]^[1] := REF(aa[1]); ra := REF(aa[1]);",
    "aa[0][1] = ra"
)]
#[case::an_element_copied("aa[1] := a1;", "aa[1][0] = NULL")]
#[case::an_alias_and_what_it_names("d1[0] := REF(c1); c1 := d1; rc := REF(c1);", "c1[0] = rc")]
// Through calls.
#[case::by_value("p1 := REF(p2);", "id_p(p1) = p1")]
#[case::an_output("p1 := REF(p2); out_p(p1, o => p3);", "p3 = p1")]
#[case::an_in_out("p1 := REF(p2); io_p(p3, p1);", "p3 = p1")]
#[case::an_array_by_value("a1[0] := REF(a2); ra := REF(a2);", "first(a1) = ra")]
#[case::an_array_in_out("set_first(a1, REF(a2)); ra := REF(a2);", "a1[0] = ra")]
#[case::an_array_returned("a1 := ret_a();", "a1[0] = NULL")]
#[case::overloads("", "over(p1) = 1 AND over(pa) = 2 AND over(a1) = 3 AND over(b1) = 4")]
// Held elsewhere.
#[case::a_block_member("fb();", "fb.q")]
#[case::a_class_member("", "cl.Me()")]
#[case::a_global("gp := REF(gp);", "gp^ = gp")]
#[case::a_global_array("ga[0] := REF(ga); ra := ga[0];", "ra^[0] = ra")]
fn a_type_that_comes_back_through_a_reference(
    mut with_db: db::RootDatabase,
    #[case] steps: &str,
    #[case] holds: &str,
) {
    let source = format!(
        r#"
        TYPE
            A : ARRAY[0..1] OF REF_TO A;
            P : REF_TO P;
            Pa : REF_TO Qa;
            Qa : REF_TO Pa;
            B : ARRAY[0..1] OF Qb;
            Qb : REF_TO B;
            C : D;
            D : ARRAY[0..1] OF REF_TO C;
            E : ARRAY[0..1] OF F;
            F : ARRAY[0..1] OF REF_TO E;
            G : ARRAY[0..1] OF H;
            H : STRUCT again : REF_TO G; n : INT; END_STRUCT;
            Two : ARRAY[0..1, 0..1] OF REF_TO Two;
            Ra : REF_TO A;
        END_TYPE

        CONFIGURATION Cfg
            VAR_GLOBAL gp : P; ga : A; END_VAR
        END_CONFIGURATION

        FUNCTION id_p : P
        VAR_INPUT x : P; END_VAR
            id_p := x;
        END_FUNCTION

        FUNCTION first : Ra
        VAR_INPUT x : A; END_VAR
            first := x[0];
        END_FUNCTION

        FUNCTION set_first
        VAR_IN_OUT x : A; END_VAR
        VAR_INPUT t : REF_TO A; END_VAR
            x[0] := t;
        END_FUNCTION

        FUNCTION out_p
        VAR_INPUT x : P; END_VAR
        VAR_OUTPUT o : P; END_VAR
            o := x;
        END_FUNCTION

        FUNCTION io_p
        VAR_IN_OUT x : P; END_VAR
        VAR_INPUT t : P; END_VAR
            x := t;
        END_FUNCTION

        FUNCTION ret_a : A
        VAR x : A; END_VAR
            x[1] := REF(x);
            ret_a := x;
        END_FUNCTION

        FUNCTION_BLOCK Fb
        VAR a : A; p : P; END_VAR
        VAR_OUTPUT q : BOOL; END_VAR
            a[0] := REF(a);
            p := REF(p);
            q := a[0]^[0] = a[0] AND p^ = p;
        END_FUNCTION_BLOCK

        CLASS Cl
        VAR PUBLIC a : A; END_VAR
            METHOD PUBLIC Me : BOOL
            VAR r : REF_TO A; END_VAR
                THIS.a[0] := REF(THIS.a);
                r := REF(a);
                Me := a[0]^[0] = r;
            END_METHOD
        END_CLASS

        FUNCTION over : INT
        VAR_INPUT x : P; END_VAR
            over := 1;
        END_FUNCTION

        FUNCTION over : INT
        VAR_INPUT x : Pa; END_VAR
            over := 2;
        END_FUNCTION

        FUNCTION over : INT
        VAR_INPUT x : A; END_VAR
            over := 3;
        END_FUNCTION

        FUNCTION over : INT
        VAR_INPUT x : B; END_VAR
            over := 4;
        END_FUNCTION

        FUNCTION test : BOOL
        VAR_EXTERNAL gp : P; ga : A; END_VAR
        VAR
            a1, a2 : A;
            p1, p2, p3 : P;
            pa : Pa;
            qa : Qa;
            b1, b2 : B;
            c1, c2 : C;
            d1 : D;
            e1, e2 : E;
            g1, g2 : G;
            t1, t2 : Two;
            aa : ARRAY[0..1] OF A;
            fb : Fb;
            cl : Cl;
            ra : REF_TO A;
            rb : REF_TO B;
            rc : REF_TO C;
            re : REF_TO E;
            rt : REF_TO Two;
            rp : REF_TO P;
        END_VAR
            {steps}
            test := {holds};
        END_FUNCTION
    "#
    );
    let result: i32 = crate::tests::codegen::run(&mut with_db, &source, "test", ());
    assert_eq!(result, 1, "{steps} {holds}");
}

/// An array of references to its own type: the reference reads what it points at,
/// and the debug tables describe all of it but where the type comes back.
#[rstest]
fn an_array_that_refers_to_itself(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE A : ARRAY[0..1] OF REF_TO A; END_TYPE

        PROGRAM Main
        VAR
            first, second : A;
            same : BOOL;
        END_VAR
            first[0] := REF(second);
            second[1] := REF(first);
            same := first[0]^[1] = REF(first);
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : Main;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_, wasm) = crate::tests::codegen::compile_to_mir_and_wasm(&mut with_db, source);
    let mut plc = crate::tests::codegen::TestPlc::load(&wasm).expect("load");
    plc.run(1).expect("a scan");
    let info = debug_format::DebugInfo::from_wasm(&wasm);
    assert!(info.problems().is_empty(), "{:?}", info.problems());
    assert_eq!(
        plc.read_var(&info, "P1.same").unwrap(),
        debug_format::VarValue::Bool(true)
    );
}

/// A reference compares by the address it holds, with `REF()` on either side or on
/// both, whatever it points at. Compared at the type it points at, a 64-bit target
/// or a REAL read the address as one, an array or a STRUCT did not build, and a
/// STRING compared its bytes.
#[rstest]
#[case::bool("BOOL")]
#[case::sint("SINT")]
#[case::int("INT")]
#[case::lint("LINT")]
#[case::ulint("ULINT")]
#[case::real("REAL")]
#[case::lreal("LREAL")]
#[case::word("WORD")]
#[case::lword("LWORD")]
#[case::time("TIME")]
#[case::ltime("LTIME")]
#[case::char("CHAR")]
#[case::string("STRING")]
#[case::sized_string("STRING[4]")]
#[case::array("ARRAY[0..1] OF INT")]
#[case::named_array("Row")]
#[case::named_struct("Pt")]
#[case::instance("Fb")]
#[case::named_values("Color")]
fn references_compare_by_address(mut with_db: db::RootDatabase, #[case] ty: &str) {
    let source = format!(
        r#"
        TYPE
            Pt : STRUCT a : INT; END_STRUCT;
            Row : ARRAY[0..1] OF INT;
            Color : (Red, Green);
        END_TYPE

        FUNCTION_BLOCK Fb
        VAR v : INT; END_VAR
        END_FUNCTION_BLOCK

        FUNCTION test : DINT
        VAR
            x, y : {ty};
            r : REF_TO {ty};
        END_VAR
            r := REF(x);
            IF r = REF(x) THEN test := test + 1; END_IF;
            IF r <> REF(y) THEN test := test + 2; END_IF;
            IF REF(x) = r THEN test := test + 4; END_IF;
            IF REF(y) <> r THEN test := test + 8; END_IF;
            IF REF(x) = REF(x) THEN test := test + 16; END_IF;
            IF REF(x) <> REF(y) THEN test := test + 32; END_IF;
            IF r <> NULL THEN test := test + 64; END_IF;
            IF NULL <> REF(x) THEN test := test + 128; END_IF;
            IF (r) = (REF(x)) THEN test := test + 256; END_IF;
            IF r = REF(y) THEN test := test + 512; END_IF;
            IF REF(x) = NULL THEN test := test + 1024; END_IF;
        END_FUNCTION
    "#
    );
    let result: i32 = crate::tests::codegen::run(&mut with_db, &source, "test", ());
    assert_eq!(result, 511);
}

/// A `REF()` in parentheses writes the same address, in a body and in an initializer.
#[rstest]
fn a_ref_in_parentheses_points_at_its_variable(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION test : DINT
        VAR
            x, y : DINT;
            a : ARRAY[0..1] OF DINT;
            r : REF_TO DINT := (REF(x));
            e : REF_TO DINT;
        END_VAR
            r^ := 5;
            e := ((REF(a[1])));
            e^ := 7;
            r := (REF(y));
            r^ := 9;
            test := x * 100 + a[1] * 10 + y;
        END_FUNCTION
    "#;
    let result: i32 = crate::tests::codegen::run(&mut with_db, source, "test", ());
    assert_eq!(result, 579);
}

/// A `REF()` compares with a reference held anywhere, and goes wherever a reference
/// does. What a reference reads stays a value. Each row runs its steps in a function
/// with these variables, and then holds.
#[rstest]
#[case::an_element(
    "refs[1] := REF(i);",
    "refs[1] = REF(i) AND REF(i) = refs[1] AND refs[0] <> REF(i)"
)]
#[case::a_field("h1.r := REF(i);", "h1.r = REF(i) AND REF(j) <> h1.r")]
#[case::a_blocks_output("fb(p := REF(i));", "fb.q = REF(i) AND REF(j) <> fb.q")]
#[case::through_a_reference(
    "q := REF(i); pp := REF(q);",
    "pp^ = REF(i) AND pp = REF(q) AND pp^^ = i"
)]
#[case::a_result(
    "",
    "make(REF(i)) = REF(i) AND REF(i) = make(REF(i)) AND make(REF(i)) <> REF(j)"
)]
#[case::a_global("rg := REF(g);", "rg = REF(g) AND rg <> REF(h) AND REF(g) <> REF(h)")]
#[case::of_an_element(
    "r := REF(a[1]);",
    "r = REF(a[1]) AND REF(a[1]) <> REF(a[2]) AND REF(a[1]) = REF(a[1])"
)]
#[case::null(
    "h1.r := NULL;",
    "h1.r = NULL AND NULL = h1.r AND REF(i) <> NULL AND NULL <> REF(i)"
)]
#[case::in_a_calculation("b := REF(i) = REF(i) AND REF(j) = REF(j);", "b")]
#[case::inverted("", "NOT (REF(i) = REF(j))")]
#[case::in_parentheses(
    "r := REF(i); q := REF(i);",
    "(r) = (q) AND (REF(i)) = r AND r = (REF(i))"
)]
#[case::a_condition("r := REF(i); IF r = REF(i) THEN j := 5; END_IF;", "j = 5")]
#[case::a_loop("r := REF(i); WHILE r = REF(i) DO r := NULL; END_WHILE;", "r = NULL")]
#[case::named_values("rk := REF(k);", "rk = REF(k) AND REF(k) = rk AND REF(k) = REF(k)")]
#[case::a_subrange(
    "rsu := REF(su);",
    "rsu = REF(su) AND REF(su) = rsu AND REF(su) <> REF(i)"
)]
#[case::a_capacity(
    "rs4 := REF(s4);",
    "rs4 = REF(s4) AND REF(s4) = rs4 AND REF(s4) = REF(s4)"
)]
#[case::an_instance(
    "rfb := REF(fb);",
    "rfb = REF(fb) AND REF(fb) = rfb AND REF(fb) <> REF(fb2)"
)]
#[case::as_an_argument("i := 7;", "takes_ref(REF(i)) = 7")]
#[case::as_a_named_argument("i := 7;", "takes_ref(p := REF(i)) = 7")]
#[case::as_an_argument_in_parentheses("i := 7;", "takes_ref((REF(i))) = 7")]
#[case::bound_in_parentheses("r := ((REF(i))); r^ := 5;", "i = 5")]
#[case::a_read_in_a_calculation("r := REF(i); i := 3; j := r^ + 1;", "j = 4")]
#[case::a_read_as_a_condition("rb := REF(b); b := TRUE; IF rb^ THEN j := 5; END_IF;", "j = 5")]
#[case::a_read_as_a_subscript("r := REF(i); i := 2; a[r^] := 9;", "a[2] = 9")]
#[case::a_read_as_a_selector("r := REF(i); i := 2; CASE r^ OF 2: j := 8; END_CASE;", "j = 8")]
fn a_ref_goes_where_a_reference_goes(
    mut with_db: db::RootDatabase,
    #[case] steps: &str,
    #[case] holds: &str,
) {
    let source = format!(
        r#"
        TYPE
            Sub : INT (0..9);
            Light : INT (Stop := 1, Go := 2);
            PInt : REF_TO INT;
            PP : REF_TO PInt;
            Holder : STRUCT r : REF_TO INT; n : INT; END_STRUCT;
        END_TYPE

        CONFIGURATION Cfg
            VAR_GLOBAL g, h : INT; rg : PInt; END_VAR
        END_CONFIGURATION

        FUNCTION_BLOCK Fb
        VAR_INPUT p : REF_TO INT; END_VAR
        VAR_OUTPUT q : REF_TO INT; END_VAR
            q := p;
        END_FUNCTION_BLOCK

        FUNCTION make : PInt
        VAR_INPUT place : PInt; END_VAR
            make := place;
        END_FUNCTION

        FUNCTION takes_ref : INT
        VAR_INPUT p : REF_TO INT; END_VAR
            takes_ref := p^;
        END_FUNCTION

        FUNCTION test : BOOL
        VAR_EXTERNAL g : INT; h : INT; rg : PInt; END_VAR
        VAR
            i, j : INT := 1;
            k : Light;
            su : Sub;
            b : BOOL;
            s4 : STRING[4];
            a : ARRAY[0..2] OF INT;
            h1 : Holder;
            fb, fb2 : Fb;
            r, q : PInt;
            pp : PP;
            refs : ARRAY[0..1] OF REF_TO INT;
            rb : REF_TO BOOL;
            rk : REF_TO Light;
            rsu : REF_TO Sub;
            rs4 : REF_TO STRING[4];
            rfb : REF_TO Fb;
        END_VAR
            {steps}
            test := {holds};
        END_FUNCTION
    "#
    );
    let result: i32 = crate::tests::codegen::run(&mut with_db, &source, "test", ());
    assert_eq!(result, 1, "{steps} {holds}");
}

/// A `REF()` in an initial value: in parentheses, in a struct or an array, and
/// compared.
#[rstest]
fn a_ref_in_an_initial_value(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Holder : STRUCT r : REF_TO INT; END_STRUCT; END_TYPE

        FUNCTION test : BOOL
        VAR
            i, j : INT;
            p : Holder := (r := (REF(i)));
            a : ARRAY[0..1] OF REF_TO INT := [(REF(i)), NULL];
            same : BOOL := REF(i) = REF(i);
            other : BOOL := REF(i) = REF(j);
        END_VAR
            test := p.r = REF(i) AND a[0] = REF(i) AND a[1] = NULL AND same AND NOT other;
        END_FUNCTION
    "#;
    let result: i32 = crate::tests::codegen::run(&mut with_db, source, "test", ());
    assert_eq!(result, 1);
}
