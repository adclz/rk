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

/// `THIS.m()` — an explicit self method call from inside another method — lowers
/// to a direct `Counter#Inc` on the current `this` pointer (offset 0), exactly
/// like the implicit-receiver forms. `IncTwice` calls `THIS.Inc()` twice, so the
/// shared instance's `c` advances 0→1→2 and the wrapper sees 2. A wrong `this`
/// (fresh/zero) would not accumulate.
#[rstest]
fn test_st_this_method_call(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Counter
        VAR c : INT; END_VAR
        METHOD Inc : INT
            THIS.c := THIS.c + 1;
            Inc := THIS.c;
        END_METHOD
        METHOD IncTwice : INT
            THIS.Inc();
            IncTwice := THIS.Inc();
        END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION test_this : INT
        VAR a : Counter; END_VAR
            test_this := a.IncTwice();     (* Inc->1, Inc->2 on the same instance *)
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test_this", ());
    assert_eq!(result, 2, "THIS.Inc() twice on the same instance yields 2");
}

/// `SUPER.m()` — static (non-virtual) dispatch to the *base* method on the same
/// `this` (IEC tables 9b/10b). `Derived.Tick` overrides `Base.Tick` and calls
/// `SUPER.Tick()`, which must resolve to `Base#Tick` (increment `c`), NOT back to
/// `Derived#Tick` (that would recurse forever). Each call: base `c`+1, then +100.
/// So two calls give 101 then 102 — the 102 proves both the base dispatch and the
/// shared instance state.
#[rstest]
fn test_st_super_method_call(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Base
        VAR c : INT; END_VAR
        METHOD Tick : INT
            THIS.c := THIS.c + 1;
            Tick := THIS.c;
        END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Derived EXTENDS Base
        METHOD OVERRIDE Tick : INT
            Tick := SUPER.Tick() + 100;    (* base Tick on the same this, then +100 *)
        END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION test_super : INT
        VAR a : Derived; END_VAR
            a.Tick();                      (* c:0->1 -> 101 (discarded) *)
            test_super := a.Tick();        (* c:1->2 -> 102 *)
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test_super", ());
    assert_eq!(result, 102, "SUPER.Tick() dispatches to Base#Tick on the same instance");
}

/// Phase B: an interface `VAR_IN_OUT` parameter is monomorphized per concrete
/// implementer. `drive(dev := w)` specializes `drive` to `drive$Worker` and
/// lowers `dev.Run()` to a direct `Worker#Run`; `drive(dev := h)` specializes to
/// `drive$Heater` → `Heater#Run`. Distinct results (10 vs 20) prove genuine
/// per-concrete dispatch — a shared/wrong `this` or a single collapsed
/// specialization could not yield both.
#[rstest]
fn test_st_interface_param_monomorphized(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE ITF1
            METHOD Run : INT END_METHOD
        END_INTERFACE

        FUNCTION_BLOCK Worker IMPLEMENTS ITF1
            METHOD Run : INT
                Run := 10;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Heater IMPLEMENTS ITF1
            METHOD Run : INT
                Run := 20;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION drive : INT
        VAR_IN_OUT dev : ITF1; END_VAR
            drive := dev.Run();
        END_FUNCTION

        FUNCTION test : INT
        VAR w : Worker; h : Heater; END_VAR
            test := drive(dev := w) + drive(dev := h);   (* 10 + 20 = 30 *)
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 30, "Worker#Run (10) + Heater#Run (20) via monomorphized interface params");
}

/// Phase B: a bare statement-context call `bump(dev := w);` (no assignment) must
/// also be collected and routed to the specialization — the collection walks the
/// statement tree, not just expression-context calls. Two calls mutate the same
/// instance through the interface method, proving state persists.
#[rstest]
fn test_st_interface_param_statement_context(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE ICounter
            METHOD Inc END_METHOD
        END_INTERFACE
        FUNCTION_BLOCK Counter IMPLEMENTS ICounter
            VAR c : INT; END_VAR
            METHOD Inc
                c := c + 1;
            END_METHOD
            METHOD Get : INT
                Get := c;
            END_METHOD
        END_FUNCTION_BLOCK
        FUNCTION bump : INT
            VAR_IN_OUT dev : ICounter; END_VAR
            dev.Inc();
            bump := 0;
        END_FUNCTION
        FUNCTION test : INT
        VAR w : Counter; END_VAR
            bump(dev := w);
            bump(dev := w);
            test := w.Get();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 2, "two bare-statement bump() calls mutate w.c to 2");
}

/// Phase B: the interface argument may be `THIS` (self) — inside `Dog.CallVia`,
/// `invoke(s := THIS)` passes the current instance. Collection resolves THIS's
/// type to the enclosing FB (`Dog`), specializes `invoke$Dog`, and the `this`
/// pointer passed to `invoke` is the method's own `this` (= &d).
#[rstest]
fn test_st_interface_arg_this(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE ISpeaker
            METHOD Speak : INT END_METHOD
        END_INTERFACE
        FUNCTION_BLOCK Dog IMPLEMENTS ISpeaker
            METHOD Speak : INT
                Speak := 7;
            END_METHOD
            METHOD CallVia : INT
                CallVia := invoke(s := THIS);
            END_METHOD
        END_FUNCTION_BLOCK
        FUNCTION invoke : INT
        VAR_IN_OUT s : ISpeaker; END_VAR
            invoke := s.Speak();
        END_FUNCTION
        FUNCTION test : INT
        VAR d : Dog; END_VAR
            test := d.CallVia();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 7, "invoke(s := THIS) dispatches to Dog#Speak on the self instance");
}

/// Phase B: an interface method with its own parameter — `dev.Add(x := 41)` must
/// pass the argument (41) alongside the `this` pointer.
#[rstest]
fn test_st_interface_method_with_arg(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE IAdder
            METHOD Add : INT
                VAR_INPUT x : INT; END_VAR
            END_METHOD
        END_INTERFACE
        FUNCTION_BLOCK Plus IMPLEMENTS IAdder
            METHOD Add : INT
                VAR_INPUT x : INT; END_VAR
                Add := x + 1;
            END_METHOD
        END_FUNCTION_BLOCK
        FUNCTION apply : INT
        VAR_IN_OUT dev : IAdder; END_VAR
            apply := dev.Add(x := 41);
        END_FUNCTION
        FUNCTION test : INT
        VAR p : Plus; END_VAR
            test := apply(dev := p);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 42, "dev.Add(x := 41) → Plus#Add(this, 41) → 42");
}

/// Phase B: an interface method that MUTATES instance state — `dev.Inc()` writes
/// through the `this` pointer to the real Counter, and the mutation persists
/// across two calls (0 + 0 + 2). Proves `this` is the live instance, not a copy.
#[rstest]
fn test_st_interface_method_mutates_instance(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE ICounter
            METHOD Inc END_METHOD
        END_INTERFACE
        FUNCTION_BLOCK Counter IMPLEMENTS ICounter
            VAR c : INT; END_VAR
            METHOD Inc
                c := c + 1;
            END_METHOD
            METHOD Get : INT
                Get := c;
            END_METHOD
        END_FUNCTION_BLOCK
        FUNCTION bump : INT
            VAR_IN_OUT dev : ICounter; END_VAR
            dev.Inc();
            bump := 0;
        END_FUNCTION
        FUNCTION test : INT
        VAR w : Counter; END_VAR
            test := bump(dev := w) + bump(dev := w) + w.Get();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 2, "two dev.Inc() through the interface mutate w.c to 2");
}
