//! Function Block execution tests.

use crate::tests::{compile_to_wasm, with_db};
use rstest::*;

#[rstest]
fn test_fb_method_execution(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Counter
        VAR
            count : INT;
        END_VAR

        METHOD Increment : INT
            THIS.count := THIS.count + 1;
            Increment := THIS.count;
        END_METHOD

        METHOD GetCount : INT
            GetCount := THIS.count;
        END_METHOD
        END_FUNCTION_BLOCK
    "#;

    let wasm_bytes = compile_to_wasm(&mut with_db, source);

    // Create an FB instance in memory
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm_bytes).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);

    // Get memory and allocate FB instance (just one INT: count = 0)
    let memory = instance
        .get_memory(&mut store, "memory")
        .expect("Failed to get memory");
    let fb_address = 0i32; // FB instance at address 0
    memory
        .write(&mut store, fb_address as usize, &0i32.to_le_bytes())
        .unwrap();

    // Call Increment method (should increment count and return 1)
    let increment = instance
        .get_typed_func::<i32, i32>(&mut store, "Counter#Increment")
        .expect("Failed to get Increment method");

    let result1 = increment.call(&mut store, fb_address).unwrap();
    assert_eq!(result1, 1, "First increment should return 1");

    // Call Increment again (should return 2)
    let result2 = increment.call(&mut store, fb_address).unwrap();
    assert_eq!(result2, 2, "Second increment should return 2");

    // Call GetCount to verify state
    let get_count = instance
        .get_typed_func::<i32, i32>(&mut store, "Counter#GetCount")
        .expect("Failed to get GetCount method");

    let count = get_count.call(&mut store, fb_address).unwrap();
    assert_eq!(count, 2, "Count should be 2 after two increments");
}

/// Calling `a.inc()` / `b.inc()` from ST must dispatch to `Counter#inc` with each
/// instance's own address as `this` — the two counters stay independent. A wrong
/// or shared `this` would change the sum.
#[rstest]
fn test_st_method_call_isolates_instances(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Counter
        VAR c : INT; END_VAR
        METHOD PUBLIC inc : INT
            THIS.c := THIS.c + 1;
            inc := THIS.c;
        END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR a : Counter; b : Counter; END_VAR
            a.inc();                      (* a.c = 1 *)
            a.inc();                      (* a.c = 2 *)
            b.inc();                      (* b.c = 1 *)
            test := a.inc() + b.inc();    (* 3 + 2 = 5 *)
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(
        result, 5,
        "isolated instances: a reaches 3, b reaches 2 (a shared `this` would give 9)"
    );
}

/// A method body accesses its FB's members by BARE name (implicit `THIS`) — no
/// `THIS.` required. It must resolve to the instance's member and codegen as a
/// this-relative access, exactly like the explicit `THIS.c` form: same isolated
/// result (5), proving bare `c` is the instance field, not something stray.
#[rstest]
fn test_st_method_bare_member_access(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Counter
        VAR c : INT; END_VAR
        METHOD PUBLIC inc : INT
            c := c + 1;
            inc := c;
        END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR a : Counter; b : Counter; END_VAR
            a.inc();
            a.inc();
            b.inc();
            test := a.inc() + b.inc();    (* 3 + 2 = 5 *)
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 5, "bare member access resolves to the instance field");
}

/// A method LOCAL that shares a name with an FB member SHADOWS the member — as
/// in IEC and HIR name resolution. Lowering must defer this decision to
/// HIR (not re-match names against the `this_struct` layout), so bare `c` inside
/// the method is the local, while `THIS.c` still reaches the member. Before the
/// HIR-driven fix, MIR matched `c` against the struct first and this returned 99
/// (the member) — a silent miscompile.
#[rstest]
fn test_st_method_local_shadows_member(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Counter
        VAR c : INT; END_VAR
        METHOD PUBLIC shadowed : INT
            VAR c : INT; END_VAR
            c := 5;              (* the method-LOCAL c *)
            THIS.c := 99;        (* the MEMBER c — must not alias the local *)
            shadowed := c;       (* returns the LOCAL: 5, not 99 *)
        END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR a : Counter; END_VAR
            test := a.shadowed();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(
        result, 5,
        "the shadowing method-local wins; the member (99) is a separate slot"
    );
}

/// An inherited method resolves to the *base* it is declared on (`Base#inc`), not
/// the receiver's derived type — and runs, with the derived instance's address as
/// `this` (base fields sit at offset 0). Exercises the HIR-faithful owner
/// resolution: a receiver-type derivation would have built the never-registered
/// `Derived#inc`.
#[rstest]
fn test_st_inherited_method_call(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Base
        VAR c : INT; END_VAR
        METHOD PUBLIC inc : INT
            THIS.c := THIS.c + 1;
            inc := THIS.c;
        END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Derived EXTENDS Base
        VAR d : INT; END_VAR
        END_FUNCTION_BLOCK

        FUNCTION test_inh : INT
        VAR a : Derived; END_VAR
            a.inc();
            a.inc();
            test_inh := a.inc();          (* 3 — inherited Base#inc on a Derived *)
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test_inh", ());
    assert_eq!(result, 3, "inherited method runs on the derived instance");
}
