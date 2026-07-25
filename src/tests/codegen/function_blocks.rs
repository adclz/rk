//! Function Block execution tests.

use crate::tests::codegen::{compile_to_wasm, with_db};
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
    assert_eq!(
        result, 5,
        "bare member access resolves to the instance field"
    );
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

/// `ARRAY OF <FB>` with a method call on an element. Indexing must compute the
/// element's own address so each instance keeps independent state: `arr[0]`
/// reaches 3 and `arr[1]` reaches 2, summing to 5. A shared/wrong element address
/// would change the sum.
#[rstest]
fn test_st_array_of_fb_instances(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Counter
        VAR c : INT; END_VAR
            METHOD Inc : INT
                c := c + 1;
                Inc := c;
            END_METHOD
        END_FUNCTION_BLOCK
        FUNCTION test : INT
        VAR arr : ARRAY[0..2] OF Counter; END_VAR
            arr[0].Inc();
            arr[0].Inc();
            arr[1].Inc();
            test := arr[0].Inc() + arr[1].Inc();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(
        result, 5,
        "array-of-FB elements keep independent state (3 + 2)"
    );
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
    assert_eq!(
        result, 102,
        "SUPER.Tick() dispatches to Base#Tick on the same instance"
    );
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
    assert_eq!(
        result, 30,
        "Worker#Run (10) + Heater#Run (20) via monomorphized interface params"
    );
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
    assert_eq!(
        result, 7,
        "invoke(s := THIS) dispatches to Dog#Speak on the self instance"
    );
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
    assert_eq!(
        result, 2,
        "two dev.Inc() through the interface mutate w.c to 2"
    );
}

/// TRANSITIVE monomorphization: `outer` forwards its own interface `VAR_IN_OUT`
/// param onward to `inner(dev := dev)`. The concrete implementer is only known in
/// the SPECIALIZATION (`outer$Counter`), where `dev` is bound to `Counter`; the
/// worklist must resolve the forwarded `dev` through that binding and specialize
/// `inner` -> `inner$Counter` too, lowering its `dev.Inc()` to `Counter#Inc`.
/// Before transitive collection, `outer(dev := w)` emitted `outer$Counter` whose
/// body still called a bare, unspecialized `inner` — a miscompile. Two `outer`
/// calls mutate `w.c` to 2 through the two-level forward.
#[rstest]
fn test_st_interface_param_transitive(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE ICounter
            METHOD Inc END_METHOD
            METHOD Get : INT END_METHOD
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
        FUNCTION inner : INT
            VAR_IN_OUT dev : ICounter; END_VAR
            dev.Inc();
            inner := 0;
        END_FUNCTION
        FUNCTION outer : INT
            VAR_IN_OUT dev : ICounter; END_VAR
            inner(dev := dev);        (* forward the interface param onward *)
            outer := 0;
        END_FUNCTION
        FUNCTION test : INT
        VAR w : Counter; END_VAR
            test := outer(dev := w) + outer(dev := w) + w.Get();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(
        result, 2,
        "two forwards reach Counter#Inc through outer$C -> inner$C"
    );
}

/// TRANSITIVE, TWO implementers: `mid` forwards `s` to `leaf(s := s)`, and `test`
/// calls `mid` with two different concretes (Inc1 +1, Inc10 +10). Correct
/// transitive monomorphization must produce DISTINCT chains — `mid$Inc1 ->
/// leaf$Inc1 -> Inc1#Step` and `mid$Inc10 -> leaf$Inc10 -> Inc10#Step`. A
/// collapsed/shared specialization (both forwards routed to one `leaf`) could not
/// yield 1 + 10 = 11; it would give 2 or 20.
#[rstest]
fn test_st_interface_param_transitive_two_impls(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE IStep
            METHOD Step END_METHOD
            METHOD Get : INT END_METHOD
        END_INTERFACE
        FUNCTION_BLOCK Inc1 IMPLEMENTS IStep
            VAR v : INT; END_VAR
            METHOD Step
                v := v + 1;
            END_METHOD
            METHOD Get : INT
                Get := v;
            END_METHOD
        END_FUNCTION_BLOCK
        FUNCTION_BLOCK Inc10 IMPLEMENTS IStep
            VAR v : INT; END_VAR
            METHOD Step
                v := v + 10;
            END_METHOD
            METHOD Get : INT
                Get := v;
            END_METHOD
        END_FUNCTION_BLOCK
        FUNCTION leaf : INT
            VAR_IN_OUT s : IStep; END_VAR
            s.Step();
            leaf := 0;
        END_FUNCTION
        FUNCTION mid : INT
            VAR_IN_OUT s : IStep; END_VAR
            leaf(s := s);             (* forward onward *)
            mid := 0;
        END_FUNCTION
        FUNCTION test2 : INT
        VAR a : Inc1; b : Inc10; END_VAR
            mid(s := a);              (* mid$Inc1 -> leaf$Inc1 -> Inc1#Step: a.v = 1 *)
            mid(s := b);              (* mid$Inc10 -> leaf$Inc10 -> Inc10#Step: b.v = 10 *)
            test2 := a.Get() + b.Get();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test2", ());
    assert_eq!(
        result, 11,
        "distinct transitive chains: Inc1 (+1) and Inc10 (+10)"
    );
}

/// TRANSITIVE, THREE levels: `l1` -> `l2` -> `l3` each forward the interface param
/// onward, and only `l3` actually calls `dev.Inc()`. The worklist must propagate
/// the concrete binding through the whole chain (`l1$C` -> `l2$C` -> `l3$C`), at
/// arbitrary depth. Three `l1(dev := w)` calls drive `w.c` to 3.
#[rstest]
fn test_st_interface_param_transitive_three_levels(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE ICounter
            METHOD Inc END_METHOD
            METHOD Get : INT END_METHOD
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
        FUNCTION l3 : INT
            VAR_IN_OUT dev : ICounter; END_VAR
            dev.Inc();
            l3 := 0;
        END_FUNCTION
        FUNCTION l2 : INT
            VAR_IN_OUT dev : ICounter; END_VAR
            l3(dev := dev);
            l2 := 0;
        END_FUNCTION
        FUNCTION l1 : INT
            VAR_IN_OUT dev : ICounter; END_VAR
            l2(dev := dev);
            l1 := 0;
        END_FUNCTION
        FUNCTION test3 : INT
        VAR w : Counter; END_VAR
            l1(dev := w);
            l1(dev := w);
            l1(dev := w);
            test3 := w.Get();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test3", ());
    assert_eq!(
        result, 3,
        "concrete binding propagates through a 3-level forward chain"
    );
}

/// Regression for the seed change: interface-param functions are no longer walked
/// in the seed pass (only as specializations, by the worklist). So a call INSIDE a
/// specialization body — whether it FORWARDS the interface param (`helper(dev :=
/// dev)`) or passes a CONCRETE local (`helper(dev := local)`) — must be collected
/// by the per-instance walk. Both routes here target `helper$Counter`; the forward
/// mutates `w`, the concrete-local mutates an internal `local` (unobservable), so
/// `w.c` ends at 1.
#[rstest]
fn test_st_interface_param_specialization_body_calls(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE ICounter
            METHOD Inc END_METHOD
            METHOD Get : INT END_METHOD
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
        FUNCTION helper : INT
            VAR_IN_OUT dev : ICounter; END_VAR
            dev.Inc();
            helper := dev.Get();
        END_FUNCTION
        FUNCTION outer : INT
            VAR_IN_OUT dev : ICounter; END_VAR
            VAR local : Counter; END_VAR
            helper(dev := dev);            (* forward: mutates the caller's w *)
            outer := helper(dev := local); (* concrete local inside the specialization *)
        END_FUNCTION
        FUNCTION test_body : INT
        VAR w : Counter; END_VAR
            outer(dev := w);
            test_body := w.Get();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test_body", ());
    assert_eq!(
        result, 1,
        "both the forwarded and concrete-local calls resolve to helper$Counter"
    );
}

/// Keying torture (from the adversarial finder): `mid`'s own params are named
/// `p, q` — IDENTICAL to `leaf`'s — and it forwards them SWAPPED: `leaf(p := q,
/// q := p)`. `resolve_concrete` must key the active substitution by the ARG's name
/// (`q` -> Ten, `p` -> One) while the new instance binds by the CALLEE's param name
/// -> `leaf` gets `{p: Ten, q: One}`. Any confusion of which name is the key would
/// silently drop the swap and yield 1001 instead of 10 + 100*1 = 110.
#[rstest]
fn test_st_interface_param_same_name_swapped_forward(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE I
            METHOD V : INT END_METHOD
        END_INTERFACE
        FUNCTION_BLOCK One IMPLEMENTS I
            METHOD V : INT  V := 1; END_METHOD
        END_FUNCTION_BLOCK
        FUNCTION_BLOCK Ten IMPLEMENTS I
            METHOD V : INT  V := 10; END_METHOD
        END_FUNCTION_BLOCK
        FUNCTION leaf : INT
            VAR_IN_OUT p : I; q : I; END_VAR
            leaf := p.V() + 100 * q.V();
        END_FUNCTION
        FUNCTION mid : INT
            VAR_IN_OUT p : I; q : I; END_VAR
            mid := leaf(p := q, q := p);   (* swap, with names identical to leaf's *)
        END_FUNCTION
        FUNCTION entry : INT
        VAR x : One; y : Ten; END_VAR
            entry := mid(p := x, q := y);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "entry", ());
    assert_eq!(
        r, 110,
        "arg-name keying binds leaf p->Ten, q->One (swap preserved)"
    );
}

/// Per-instance rewrites invariant (from the adversarial finder): `mid` is
/// instantiated TWICE — `mid$One_Ten` and `mid$Ten_One` — and both share the SAME
/// inner `leaf(p := a, q := b)` `FuncCall` node, which must rewrite to DIFFERENT
/// `leaf` specializations per `mid` instance. A single shared/global rewrite
/// (last-writer-wins) would collapse both to one `leaf` spec. Correct:
/// `mid$One_Ten` -> leaf(One,Ten)=1001; `mid$Ten_One` -> leaf(Ten,One)=110;
/// 1001 + 10000*110 = 1101001.
#[rstest]
fn test_st_interface_param_per_instance_rewrites(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE I
            METHOD V : INT END_METHOD
        END_INTERFACE
        FUNCTION_BLOCK One IMPLEMENTS I
            METHOD V : INT  V := 1; END_METHOD
        END_FUNCTION_BLOCK
        FUNCTION_BLOCK Ten IMPLEMENTS I
            METHOD V : INT  V := 10; END_METHOD
        END_FUNCTION_BLOCK
        FUNCTION leaf : INT
            VAR_IN_OUT p : I; q : I; END_VAR
            leaf := p.V() + 100 * q.V();
        END_FUNCTION
        FUNCTION mid : INT
            VAR_IN_OUT a : I; b : I; END_VAR
            mid := leaf(p := a, q := b);
        END_FUNCTION
        FUNCTION entry : DINT
        VAR x : One; y : Ten; a1 : DINT; a2 : DINT; END_VAR
            (* DINT accumulators: the 1101001 checksum exceeds INT's 16-bit
               domain, and sub-width arithmetic wraps at the type width. *)
            a1 := mid(a := x, b := y);
            a2 := mid(a := y, b := x);
            entry := a1 + 10000 * a2;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "entry", ());
    assert_eq!(
        r, 1101001,
        "the one inner FuncCall routes to two different leaf specs"
    );
}

/// A self-recursive interface-param function forwards to ITSELF (`f(dev := dev)`)
/// with a base case. The worklist re-encounters `f`'s own forwarded self-call
/// while processing `f$Counter`; `by_canonical` must dedup it back to `f$Counter`
/// (else collection never reaches fixpoint) AND record the self-call in
/// `f$Counter`'s own rewrites pointing at `f$Counter` — leaving it un-rewritten
/// would target the bare, never-emitted `f`. Runs `dev.Inc()` until `Get()` = 3.
#[rstest]
fn test_st_interface_param_self_recursive_forward(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE ICounter
            METHOD Inc END_METHOD
            METHOD Get : INT END_METHOD
        END_INTERFACE
        FUNCTION_BLOCK Counter IMPLEMENTS ICounter
            VAR c : INT; END_VAR
            METHOD Inc  c := c + 1; END_METHOD
            METHOD Get : INT  Get := c; END_METHOD
        END_FUNCTION_BLOCK
        FUNCTION f : INT
            VAR_IN_OUT dev : ICounter; END_VAR
            IF dev.Get() < 3 THEN
                dev.Inc();
                f(dev := dev);          (* self-recursive forward *)
            END_IF
            f := 0;
        END_FUNCTION
        FUNCTION test : INT
        VAR w : Counter; END_VAR
            f(dev := w);
            test := w.Get();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(
        r, 3,
        "self-recursive forward terminates and routes to f$Counter"
    );
}

/// THIS forwarded transitively: a METHOD passes `THIS` into an interface-param
/// function (`outer(s := THIS)`), which then FORWARDS it onward (`inner(s := s)`).
/// The `THIS` concrete is bound in the SEED (via `self_pou` = the method's owner,
/// `Dog`), seeding `outer$Dog`; the worklist must then expand that instance's
/// forwarded `inner(s := s)` to `inner$Dog`. So the seed's `self_pou` binding has
/// to compose with the transitive worklist — `s.Speak()` reaches `Dog#Speak` = 7.
#[rstest]
fn test_st_interface_param_this_forwarded_transitively(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE ISpeaker
            METHOD Speak : INT END_METHOD
        END_INTERFACE
        FUNCTION_BLOCK Dog IMPLEMENTS ISpeaker
            METHOD Speak : INT  Speak := 7; END_METHOD
            METHOD PUBLIC Run : INT
                Run := outer(s := THIS);      (* pass THIS into an iface-param fn *)
            END_METHOD
        END_FUNCTION_BLOCK
        FUNCTION inner : INT
            VAR_IN_OUT s : ISpeaker; END_VAR
            inner := s.Speak();
        END_FUNCTION
        FUNCTION outer : INT
            VAR_IN_OUT s : ISpeaker; END_VAR
            outer := inner(s := s);           (* forward THIS onward *)
        END_FUNCTION
        FUNCTION test : INT
        VAR d : Dog; END_VAR
            test := d.Run();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(
        r, 7,
        "THIS bound in the seed (self_pou=Dog) drives outer$Dog -> inner$Dog"
    );
}

/// `VAR_INPUT` interface param. An interface value is a REFERENCE, so `VAR_INPUT`
/// passes the address (a copy of the reference), not a by-value struct copy — the
/// callee gets a pointer to the same instance and its `dev.Inc()` mutates the
/// caller's `w`. Before widening the monomorphization to `Input`, this hard-errored
/// at MIR (`UnsupportedType("Interface(...)")`). Two calls take `w.c` 0->1->2; the
/// second returns 2, plus `w.Get()` = 2 -> 4.
#[rstest]
fn test_st_interface_var_input_param(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE I
            METHOD Inc END_METHOD
            METHOD Get : INT END_METHOD
        END_INTERFACE
        FUNCTION_BLOCK C IMPLEMENTS I
            VAR c : INT; END_VAR
            METHOD Inc  c := c + 1; END_METHOD
            METHOD Get : INT  Get := c; END_METHOD
        END_FUNCTION_BLOCK
        FUNCTION use_input : INT
            VAR_INPUT dev : I; END_VAR
            dev.Inc();
            use_input := dev.Get();
        END_FUNCTION
        FUNCTION test : INT
        VAR w : C; END_VAR
            use_input(dev := w);
            test := use_input(dev := w) + w.Get();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(
        r, 4,
        "VAR_INPUT interface is a reference: mutation persists to w"
    );
}

/// `VAR_INPUT` interface param monomorphizes per concrete implementer, exactly like
/// `VAR_IN_OUT`. `pick(dev := a)` specializes to `pick$One` (-> One#V = 1),
/// `pick(dev := b)` to `pick$Ten` (-> Ten#V = 10). Distinct results prove genuine
/// per-concrete dispatch with zero runtime dispatch: 1 + 100*10 = 1001.
#[rstest]
fn test_st_interface_var_input_two_impls(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE I
            METHOD V : INT END_METHOD
        END_INTERFACE
        FUNCTION_BLOCK One IMPLEMENTS I
            METHOD V : INT  V := 1; END_METHOD
        END_FUNCTION_BLOCK
        FUNCTION_BLOCK Ten IMPLEMENTS I
            METHOD V : INT  V := 10; END_METHOD
        END_FUNCTION_BLOCK
        FUNCTION pick : INT
            VAR_INPUT dev : I; END_VAR
            pick := dev.V();
        END_FUNCTION
        FUNCTION test : INT
        VAR a : One; b : Ten; END_VAR
            test := pick(dev := a) + 100 * pick(dev := b);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(
        r, 1001,
        "pick$One -> 1, pick$Ten -> 10, distinct specializations"
    );
}

/// Mixed-kind forwarding: a `VAR_INPUT` interface param is forwarded onward to a
/// `VAR_IN_OUT` interface param (`leaf(dev := dev)`). Both kinds are references, so
/// the transitive worklist specializes both (`mid$C` -> `leaf$C`) and the shared
/// instance's mutation persists through the whole chain: two calls drive `w.c` to 2.
#[rstest]
fn test_st_interface_var_input_forwarded_to_inout(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE I
            METHOD Inc END_METHOD
            METHOD Get : INT END_METHOD
        END_INTERFACE
        FUNCTION_BLOCK C IMPLEMENTS I
            VAR c : INT; END_VAR
            METHOD Inc  c := c + 1; END_METHOD
            METHOD Get : INT  Get := c; END_METHOD
        END_FUNCTION_BLOCK
        FUNCTION leaf : INT
            VAR_IN_OUT dev : I; END_VAR
            dev.Inc();
            leaf := 0;
        END_FUNCTION
        FUNCTION mid : INT
            VAR_INPUT dev : I; END_VAR
            leaf(dev := dev);          (* VAR_INPUT forwarded to VAR_IN_OUT *)
            mid := 0;
        END_FUNCTION
        FUNCTION test : INT
        VAR w : C; END_VAR
            mid(dev := w);
            mid(dev := w);
            test := w.Get();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(
        r, 2,
        "VAR_INPUT forwarded to VAR_IN_OUT: mutation persists through mid$C -> leaf$C"
    );
}

/// `SUPER()` (IEC 10c) — a derived FB's body calls the immediate base FB's cyclic
/// body on the *same* instance. `Derived`'s body is just `SUPER()`, which lowers
/// to `Base$__body__(this)`; `Base`'s body does `c := c + 5`. Invoking
/// `Derived$__body__` twice on one instance drives the shared `c` (Base's field,
/// at offset 0) to 10. Uses direct body invocation + memory read to observe.
#[rstest]
fn test_st_super_body_call(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Base
        VAR c : INT; END_VAR
            c := c + 5;
        END_FUNCTION_BLOCK
        FUNCTION_BLOCK Derived EXTENDS Base
            SUPER();
        END_FUNCTION_BLOCK
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);
    let memory = instance.get_memory(&mut store, "memory").unwrap();
    memory.write(&mut store, 0, &0i32.to_le_bytes()).unwrap();
    let body = instance
        .get_typed_func::<i32, ()>(&mut store, "Derived$__body__")
        .unwrap();
    body.call(&mut store, 0).unwrap();
    body.call(&mut store, 0).unwrap();
    let mut buf = [0u8; 4];
    memory.read(&store, 0, &mut buf).unwrap();
    assert_eq!(
        i32::from_le_bytes(buf),
        10,
        "SUPER() runs Base$__body__ on the shared instance (c += 5 each)"
    );
}

/// `SUPER()` does NOT auto-chain — each level opts in. `C EXTENDS B EXTENDS A`:
/// `C`'s body calls `SUPER()` (-> `B$__body__`), and `B`'s body ALSO calls
/// `SUPER()` (-> `A$__body__`), so only through explicit chaining does `A`'s body
/// (`n := n + 1`) run. One `C$__body__` call increments `n` once; two calls -> 2.
#[rstest]
fn test_st_super_body_multi_level(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK A
        VAR n : INT; END_VAR
            n := n + 1;
        END_FUNCTION_BLOCK
        FUNCTION_BLOCK B EXTENDS A
            SUPER();
        END_FUNCTION_BLOCK
        FUNCTION_BLOCK C EXTENDS B
            SUPER();
        END_FUNCTION_BLOCK
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, &wasm).unwrap();
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = super::instantiate_with_memory(&mut store, &module);
    let memory = instance.get_memory(&mut store, "memory").unwrap();
    memory.write(&mut store, 0, &0i32.to_le_bytes()).unwrap();
    let body = instance
        .get_typed_func::<i32, ()>(&mut store, "C$__body__")
        .unwrap();
    body.call(&mut store, 0).unwrap();
    body.call(&mut store, 0).unwrap();
    let mut buf = [0u8; 4];
    memory.read(&store, 0, &mut buf).unwrap();
    assert_eq!(
        i32::from_le_bytes(buf),
        2,
        "C -> SUPER() -> B -> SUPER() -> A body chains only via explicit SUPER()"
    );
}

/// FB aggregate VAR_INPUT: an ARRAY argument is bulk-copied into the instance
/// field before the body runs. (Regression: previously non-elementary fields
/// were silently skipped — the instance array stayed zeroed and sum was 10.)
#[rstest]
fn fb_array_input(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK SumFb
        VAR_INPUT arr_in : ARRAY[0..1] OF INT; scal_in : INT; END_VAR
        VAR_OUTPUT sum : INT; END_VAR
            sum := arr_in[0] + arr_in[1] + scal_in;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR inst : SumFb; a : ARRAY[0..1] OF INT; END_VAR
            a[0] := 3;
            a[1] := 4;
            inst(arr_in := a, scal_in := 10);
            test := inst.sum;
        END_FUNCTION
    "#;
    let wasm = super::compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 17, "array input copied into the instance: 3+4+10");
}

/// FB aggregate VAR_INPUT: a STRUCT argument is bulk-copied by value — the
/// callee sees the fields, and mutating the caller's struct afterwards does
/// not retroactively change what the FB consumed.
#[rstest]
fn fb_struct_input(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Vec2 : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION_BLOCK AddFb
        VAR_INPUT v : Vec2; END_VAR
        VAR_OUTPUT total : INT; END_VAR
            total := v.x + v.y;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR inst : AddFb; p : Vec2; END_VAR
            p.x := 3;
            p.y := 4;
            inst(v := p);
            test := inst.total;
        END_FUNCTION
    "#;
    let wasm = super::compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 7, "struct input copied into the instance: 3+4");
}

/// FB aggregate VAR_OUTPUT bound with `=>`: the STRUCT field is bulk-copied
/// back into the caller's variable after the body.
#[rstest]
fn fb_struct_output_binding(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Vec2 : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION_BLOCK MakeFb
        VAR_INPUT seed : INT; END_VAR
        VAR_OUTPUT v : Vec2; END_VAR
            v.x := seed;
            v.y := seed * 2;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR inst : MakeFb; got : Vec2; END_VAR
            inst(seed := 5, v => got);
            test := got.x + got.y;
        END_FUNCTION
    "#;
    let wasm = super::compile_to_wasm_checked(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 15, "struct output copied back: 5 + 10");
}

/// FB STRING VAR_INPUT and VAR_OUTPUT: the input arg is capacity-bounded
/// copied into the instance field before the body, and the `=>`-bound output
/// field is copied back into the caller's variable after it.
#[rstest]
fn fb_string_input_output(mut with_db: db::RootDatabase) {
    use runtime::{Config, Plc};

    let source = r#"
        FUNCTION_BLOCK EchoFb
        VAR_INPUT s_in : STRING; END_VAR
        VAR_OUTPUT s_out : STRING; END_VAR
            s_out := s_in;
        END_FUNCTION_BLOCK

        PROGRAM P
        VAR RETAIN r : STRING; END_VAR
        VAR fb : EchoFb; src : STRING; END_VAR
            src := 'agg-str';
            fb(s_in := src, s_out => r);
        END_PROGRAM

        CONFIGURATION Cfg
            RESOURCE Res ON CPU
                TASK T(INTERVAL := T#10ms, PRIORITY := 1);
                PROGRAM P1 WITH T : P;
            END_RESOURCE
        END_CONFIGURATION
    "#;
    let (_mir, wasm) = crate::tests::codegen::compile_to_mir_and_wasm(&mut with_db, source);

    let mut plc = Plc::load(&wasm, Config::default()).expect("load");
    plc.run(1).expect("scan");
    let r = plc.read_retain();
    let len = i32::from_le_bytes(r[0..4].try_into().unwrap()) as usize;
    assert_eq!(String::from_utf8_lossy(&r[4..4 + len]), "agg-str");
}

/// IEC 61131-3: `VAR_TEMP` is scratch, fresh at every invocation — never
/// instance state.
///
/// Two bugs conspired here. `lower_fb_type` put *every* variable in the
/// instance struct, including temps, so `root_place` resolved the name to a
/// `ThisField` in persistent instance memory (the separately allocated body
/// local sat unused); and aggregate temps, living at a fixed address, were
/// never reset on entry. A read-before-write therefore saw the *previous*
/// call's bytes.
#[rstest]
fn var_temp_is_fresh_on_every_invocation(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Scratch
            VAR_OUTPUT seen : INT; END_VAR
            VAR_TEMP buf : ARRAY[0..3] OF INT; END_VAR
            seen := buf[0];
            buf[0] := 42;
        END_FUNCTION_BLOCK

        FUNCTION entry : INT
        VAR fb : Scratch; END_VAR
            fb();
            IF fb.seen <> 0 THEN
                entry := -1;
                RETURN;
            END_IF;
            // Second call must NOT observe the 42 written by the first.
            fb();
            entry := fb.seen;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "entry", ());
    assert_eq!(r, 0, "aggregate VAR_TEMP must be zeroed at each invocation");
}

/// The same guarantee for a scalar temp, which lives in a wasm local rather
/// than linear memory — covering the other storage path.
#[rstest]
fn scalar_var_temp_is_fresh_on_every_invocation(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK ScalarScratch
            VAR_OUTPUT seen : INT; END_VAR
            VAR_TEMP n : INT; END_VAR
            seen := n;
            n := 42;
        END_FUNCTION_BLOCK

        FUNCTION entry : INT
        VAR fb : ScalarScratch; END_VAR
            fb();
            fb();
            entry := fb.seen;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "entry", ());
    assert_eq!(r, 0, "scalar VAR_TEMP must not carry over between calls");
}

/// Guard the other half: real FB state (`VAR`) MUST persist across calls, so
/// the temp fix cannot have over-reached into instance fields.
#[rstest]
fn fb_var_state_still_persists_across_calls(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Counter
            VAR_OUTPUT cv : INT; END_VAR
            VAR acc : INT; END_VAR
            acc := acc + 1;
            cv := acc;
        END_FUNCTION_BLOCK

        FUNCTION entry : INT
        VAR fb : Counter; END_VAR
            fb();
            fb();
            fb();
            entry := fb.cv;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "entry", ());
    assert_eq!(r, 3, "FB VAR is instance state and must survive invocations");
}
