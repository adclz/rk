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

    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());

    let instance = super::instantiate_with_memory(&mut store, &module);

    // Call function
    let func = instance
        .get_typed_func::<(), i32>(&mut store, "test_null")
        .unwrap();

    let result = func.call(&mut store, ()).unwrap();
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
    memory.read(&store, CALLER_SLOT as usize, &mut buffer).unwrap();
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

    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());

    let instance = super::instantiate_with_memory(&mut store, &module);

    let func = instance
        .get_typed_func::<(), i32>(&mut store, "test_ref")
        .unwrap();

    let result = func.call(&mut store, ()).unwrap();
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

    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());

    let instance = super::instantiate_with_memory(&mut store, &module);

    let func = instance
        .get_typed_func::<(), i32>(&mut store, "test_ref_assign")
        .unwrap();

    let result = func.call(&mut store, ()).unwrap();
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

    let engine = crate::tests::codegen::test_engine();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());

    let instance = super::instantiate_with_memory(&mut store, &module);

    let func = instance
        .get_typed_func::<(), i32>(&mut store, "test_double_deref")
        .unwrap();

    let result = func.call(&mut store, ()).unwrap();
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
    let wasm = super::compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
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
    let wasm = super::compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
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
    let wasm = super::compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 42, "element writes through r^ must land at the right stride");
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
    let wasm = super::compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
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
    let wasm = super::compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
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
    let wasm = super::compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
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
    let wasm = super::compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
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
    let wasm = super::compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 72, "s.a became 7 in the caller's own storage; s.b untouched");
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
    let wasm = super::compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
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
    let wasm = super::compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
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
    let wasm = super::compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
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
