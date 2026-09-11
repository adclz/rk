//! CLASS execution tests. A CLASS is a bodyless FUNCTION_BLOCK — a pure
//! method/property container: it has state and methods but no cyclic body, so it
//! emits only `Class#method` (no `Class$__body__`) and cannot be invoked like an
//! FB instance.

use crate::tests::codegen::{compile_to_wasm, with_db};
use rstest::*;

/// A CLASS instantiated as a local, with a method call that mutates its state.
/// `Counter#Inc` is emitted the same way as an FB method (there is no
/// `Counter$__body__` — a class has no cyclic body). Three calls advance the
/// instance's `c` to 3.
#[rstest]
fn test_st_class_method_call(mut with_db: db::RootDatabase) {
    let source = r#"
        CLASS Counter
        VAR c : INT; END_VAR
            METHOD Inc : INT
                c := c + 1;
                Inc := c;
            END_METHOD
        END_CLASS
        FUNCTION test : INT
        VAR a : Counter; END_VAR
            a.Inc();
            a.Inc();
            test := a.Inc();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(
        result, 3,
        "class method call mutates the instance across calls"
    );
}

/// Class inheritance: `Derived EXTENDS Base` inherits `Base#inc`, called on a
/// `Derived` instance (base members sit at offset 0), resolving to the base's
/// registered `Base#inc` — exactly like FB inheritance.
#[rstest]
fn test_st_class_inherited_method(mut with_db: db::RootDatabase) {
    let source = r#"
        CLASS Base
        VAR c : INT; END_VAR
            METHOD PUBLIC inc : INT
                c := c + 1;
                inc := c;
            END_METHOD
        END_CLASS
        CLASS Derived EXTENDS Base
        VAR d : INT; END_VAR
        END_CLASS
        FUNCTION test : INT
        VAR a : Derived; END_VAR
            a.inc();
            a.inc();
            test := a.inc();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(
        result, 3,
        "inherited class method runs on the derived instance"
    );
}

/// `ARRAY OF <CLASS>` with a method call on an element. Indexing must compute the
/// element's own address so each instance keeps independent state: `arr[0]`
/// reaches 3 and `arr[1]` reaches 2, summing to 5.
#[rstest]
fn test_st_array_of_class_instances(mut with_db: db::RootDatabase) {
    let source = r#"
        CLASS Counter
        VAR c : INT; END_VAR
            METHOD Inc : INT
                c := c + 1;
                Inc := c;
            END_METHOD
        END_CLASS
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
        "array-of-class elements keep independent state (3 + 2)"
    );
}

/// A CLASS as an interface implementer, monomorphized exactly like an FB:
/// `drive(dev := w)` (w : Worker, a class implementing IWork) specializes `drive`
/// to `drive$Worker` and lowers `dev.Run()` to a direct `Worker#Run`. The shared
/// instance's `n` advances across the two specialized calls, so the second call
/// returns 2.
#[rstest]
fn test_st_class_interface_param(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE IWork
            METHOD Run : INT END_METHOD
        END_INTERFACE
        CLASS Worker IMPLEMENTS IWork
        VAR n : INT; END_VAR
            METHOD Run : INT
                n := n + 1;
                Run := n;
            END_METHOD
        END_CLASS
        FUNCTION drive : INT
            VAR_IN_OUT dev : IWork; END_VAR
            drive := dev.Run();
        END_FUNCTION
        FUNCTION test : INT
        VAR w : Worker; END_VAR
            drive(dev := w);
            test := drive(dev := w);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(
        result, 2,
        "class interface implementer specializes to drive$Worker -> Worker#Run"
    );
}

/// Two distinct CLASS implementers of one interface each get their own
/// specialization: `pick(dev := a)` -> `pick$One` (One#V = 1), `pick(dev := b)` ->
/// `pick$Ten` (Ten#V = 10). Distinct results (1 + 100*10 = 1001) prove genuine
/// per-concrete dispatch with no runtime dispatch and no collapse.
#[rstest]
fn test_st_class_interface_two_impls(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE IWork
            METHOD V : INT END_METHOD
        END_INTERFACE
        CLASS One IMPLEMENTS IWork
            METHOD V : INT  V := 1; END_METHOD
        END_CLASS
        CLASS Ten IMPLEMENTS IWork
            METHOD V : INT  V := 10; END_METHOD
        END_CLASS
        FUNCTION pick : INT
            VAR_IN_OUT dev : IWork; END_VAR
            pick := dev.V();
        END_FUNCTION
        FUNCTION test : INT
        VAR a : One; b : Ten; END_VAR
            test := pick(dev := a) + 100 * pick(dev := b);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(
        result, 1001,
        "pick$One -> 1, pick$Ten -> 10, distinct class specializations"
    );
}

/// A derived FB must be layout-compatible with its base: inherited fields come
/// FIRST, at the offsets they have in the base, so an inherited method —
/// compiled once against the base's layout and then called with a derived
/// instance pointer — reads the right slots.
///
/// MIR used to emit only the POU's OWN variables, so a derived field landed at
/// offset 0 and silently aliased the first inherited field: writing `d`
/// clobbered `b`. The pre-existing inheritance test missed it by only ever
/// reading the base field.
#[rstest]
fn derived_fb_fields_do_not_alias_base_fields(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Base
        VAR b : INT; END_VAR
            METHOD PUBLIC SetB : INT
                b := 11;
                SetB := 0;
            END_METHOD
            METHOD PUBLIC GetB : INT
                GetB := b;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Derived EXTENDS Base
        VAR d : INT; END_VAR
            METHOD PUBLIC SetD : INT
                d := 22;
                SetD := 0;
            END_METHOD
            METHOD PUBLIC GetD : INT
                GetD := d;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR x : Derived; END_VAR
            x.SetB();
            x.SetD();          (* must not overwrite b *)
            test := x.GetB() * 100 + x.GetD();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(
        result, 1122,
        "base b stays 11 and derived d is 22 (aliasing would give 2222)"
    );
}

/// The same guarantee for CLASS inheritance, whose layout builder carried an
/// explicit `TODO: Handle inheritance` and emitted no base fields at all.
#[rstest]
fn derived_class_fields_do_not_alias_base_fields(mut with_db: db::RootDatabase) {
    let source = r#"
        CLASS CBase
        VAR cb : INT; END_VAR
            METHOD PUBLIC SetCB : INT
                cb := 11;
                SetCB := 0;
            END_METHOD
            METHOD PUBLIC GetCB : INT
                GetCB := cb;
            END_METHOD
        END_CLASS

        CLASS CDerived EXTENDS CBase
        VAR cd : INT; END_VAR
            METHOD PUBLIC SetCD : INT
                cd := 22;
                SetCD := 0;
            END_METHOD
            METHOD PUBLIC GetCD : INT
                GetCD := cd;
            END_METHOD
        END_CLASS

        FUNCTION test : INT
        VAR x : CDerived; END_VAR
            x.SetCB();
            x.SetCD();
            test := x.GetCB() * 100 + x.GetCD();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 1122, "class base field survives a derived write");
}

/// A method declared two or more levels up the `EXTENDS` chain must be
/// inherited. `inherited_methods` used to collect only from the DIRECT bases'
/// own declarations, so a grandparent's method was invisible and the call
/// failed to resolve (E0202 "no such field") — the field query recursed the
/// whole chain while the method query did not.
#[rstest]
fn method_inherited_from_a_grandparent(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK L1
        VAR a : INT; END_VAR
            METHOD PUBLIC FromL1 : INT
                a := a + 5;
                FromL1 := a;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK L2 EXTENDS L1
        VAR b : INT; END_VAR
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK L3 EXTENDS L2
        VAR c : INT; END_VAR
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR x : L3; END_VAR
            x.FromL1();
            test := x.FromL1();   (* state persists: 5 then 10 *)
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 10, "grandparent method runs against the derived instance");
}

/// A method calls a sibling with no receiver — `Helper()`, not
/// `THIS.Helper()`. HIR resolves the bare name against the enclosing POU
/// (walking `EXTENDS`), and lowering gives it the implicit THIS receiver;
/// this used to be the one call form MIR refused to lower. State proves the
/// receiver: both calls advance the SAME instance.
#[rstest]
fn a_bare_sibling_method_call_receives_this(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK FB
        VAR n : INT; END_VAR
            METHOD PUBLIC Helper : INT
                n := n + 3;
                Helper := n;
            END_METHOD
            METHOD PUBLIC Caller : INT
                Caller := Helper() + Helper();
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Base EXTENDS FB
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Derived EXTENDS Base
            METHOD PUBLIC UsesInherited : INT
                UsesInherited := Helper();
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR f : FB; d : Derived; END_VAR
            (* 3 + 6: both bare calls advance the same instance. *)
            test := f.Caller();
            (* the bare name resolves two EXTENDS hops up: 3 *)
            test := test + d.UsesInherited();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 12, "9 from the sibling pair, 3 from the inherited bare call");
}

/// Redeclaring a method further down the chain is an OVERRIDE, not a conflict:
/// the nearest declaration wins, no duplicate diagnostic is produced — and
/// the override runs against the DERIVED layout while still reaching the
/// inherited field at its base offset. A constant return proved selection
/// only; the state proves the override reads `v` where B1 put it.
#[rstest]
fn nearest_override_wins_along_the_chain(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK B1
        VAR v : INT; END_VAR
            METHOD PUBLIC Pick : INT
                v := v + 1;
                Pick := v;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK B2 EXTENDS B1
        VAR w : INT; END_VAR
            METHOD PUBLIC OVERRIDE Pick : INT
                v := v + 10;   (* inherited field, base offset *)
                w := w + 2;    (* own field, derived offset *)
                Pick := v * 100 + w;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK B3 EXTENDS B2
        VAR z : INT; END_VAR
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR x : B3; END_VAR
            x.Pick();
            test := x.Pick();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 2004, "the override advances v (10,20) and w (2,4): 20*100+4");
}

/// An override calling the method it overrides: `SUPER.Pick()` must SKIP the
/// nearest declaration and take the next one up — a third resolution path
/// beside plain inheritance and override selection, and explicitly static.
#[rstest]
fn super_reaches_the_overridden_method(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK S1
        VAR v : INT; END_VAR
            METHOD PUBLIC Pick : INT
                v := v + 1;
                Pick := v;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK S2 EXTENDS S1
            METHOD PUBLIC OVERRIDE Pick : INT
                Pick := SUPER.Pick() * 100 + 5;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR x : S2; END_VAR
            test := x.Pick();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 105, "SUPER runs S1's body (v -> 1), the override adds 5");
}

/// A FUNCTION_BLOCK may satisfy an interface with a method it INHERITS rather
/// than one it declares itself — the ordinary
/// `FB EXTENDS Base IMPLEMENTS Iface` shape.
///
/// `Run` arrived from two independent bases (the base's concrete method and the
/// interface's prototype) and collided: it was reported as a duplicate AND, since
/// the prototype won the map slot, as an unimplemented interface method. A
/// prototype and a concrete method for one name are not a conflict — the
/// concrete one implements the prototype.
#[rstest]
fn interface_satisfied_by_an_inherited_method(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE IRun
            METHOD Run : INT END_METHOD
        END_INTERFACE

        FUNCTION_BLOCK BaseImpl
        VAR n : INT; END_VAR
            METHOD PUBLIC Run : INT
                n := n + 3;
                Run := n;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK DerivedImpl EXTENDS BaseImpl IMPLEMENTS IRun
        VAR extra : INT; END_VAR
        END_FUNCTION_BLOCK

        FUNCTION drive : INT
        VAR_IN_OUT dev : IRun; END_VAR
            drive := dev.Run();
        END_FUNCTION

        FUNCTION test : INT
        VAR d : DerivedImpl; END_VAR
            drive(dev := d);
            test := drive(dev := d);   (* state persists: 3 then 6 *)
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 6, "the inherited method implements the interface");
}

/// A CLASS method with an interface param monomorphizes exactly like an FB
/// method: `c.Get(dev := a)` -> `Owner#Get$One` -> direct `One#V`. Two
/// implementers stay distinct: 1 + 100*10 = 1001.
#[rstest]
fn test_st_class_method_with_interface_param(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE I
            METHOD V : INT END_METHOD
        END_INTERFACE
        CLASS One IMPLEMENTS I
            METHOD V : INT  V := 1; END_METHOD
        END_CLASS
        CLASS Ten IMPLEMENTS I
            METHOD V : INT  V := 10; END_METHOD
        END_CLASS
        CLASS Owner
            METHOD PUBLIC Get : INT
                VAR_INPUT dev : I; END_VAR
                Get := dev.V();
            END_METHOD
        END_CLASS
        FUNCTION test : INT
        VAR c : Owner; a : One; b : Ten; END_VAR
            test := c.Get(dev := a) + 100 * c.Get(dev := b);
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 1001, "Owner#Get$One -> 1, Owner#Get$Ten -> 10");
}

/// A SELF method call with an OUTPUT BINDING: `Tick(cnt => v)` inside a
/// sibling method passes &v, so the method-local v must live in memory — a
/// wasm local has no address, and the AddrOf silently emitted nothing: an
/// invalid module from a compile that exited 0. Called from OUTSIDE it
/// always worked; the two must agree.
#[rstest]
fn a_self_method_call_output_binding_lands(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK OutBind
            VAR
                state : INT;
            END_VAR

            METHOD PUBLIC Tick
                VAR_OUTPUT
                    cnt : INT;
                END_VAR
                state := state + 1;
                cnt := state;
            END_METHOD

            METHOD PUBLIC Caller : INT
                VAR
                    v : INT;
                END_VAR
                Tick(cnt => v);
                Caller := v;
            END_METHOD

            METHOD PUBLIC ThisCaller : INT
                VAR
                    v : INT;
                END_VAR
                THIS.Tick(cnt => v);
                ThisCaller := v;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION run : DINT
            VAR
                f : OutBind;
            END_VAR
            run := f.Caller() * 100 + f.ThisCaller();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(result, 102, "bare and THIS. forms both land the output: 1*100 + 2");
}

/// THREE declarations of one name in the resolution set: the interface
/// PROTOTYPE, the base's concrete method, and a derived OVERRIDE. The
/// recorded collision bug had two of these fighting; this is the neighbouring
/// shape. Compiled CHECKED, so any duplicate or unimplemented-method
/// diagnostic fails the test; the override must win the interface dispatch.
#[rstest]
fn interface_prototype_base_method_and_override_coexist(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE IRun
            METHOD Run : INT END_METHOD
        END_INTERFACE

        FUNCTION_BLOCK BaseImpl
        VAR n : INT; END_VAR
            METHOD PUBLIC Run : INT
                n := n + 3;
                Run := n;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK DerivedImpl EXTENDS BaseImpl IMPLEMENTS IRun
            METHOD PUBLIC OVERRIDE Run : INT
                n := n + 7;
                Run := n;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION drive : INT
        VAR_IN_OUT dev : IRun; END_VAR
            drive := dev.Run();
        END_FUNCTION

        FUNCTION test : INT
        VAR d : DerivedImpl; END_VAR
            drive(dev := d);
            test := drive(dev := d);   (* override state: 7 then 14 *)
        END_FUNCTION
    "#;
    let wasm = crate::tests::codegen::compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 14, "the override implements the interface, not the base");
}

/// A BARE call to an INHERITED method with an OUTPUT BINDING: the name walks
/// EXTENDS to the base, and &v must survive the inherited-receiver path — the
/// two fixes (bare-sibling dispatch, address-taken method locals) composed.
#[rstest]
fn a_bare_inherited_call_output_binding_lands(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK OBBase
        VAR state : INT; END_VAR
            METHOD PUBLIC Tick
                VAR_OUTPUT cnt : INT; END_VAR
                state := state + 1;
                cnt := state;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK OBDerived EXTENDS OBBase
            METHOD PUBLIC Grab : INT
                VAR v : INT; END_VAR
                Tick(cnt => v);
                Grab := v;
            END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION run : DINT
        VAR d : OBDerived; END_VAR
            run := d.Grab() * 100 + d.Grab();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(result, 102, "the inherited bare call lands 1 then 2 through &v");
}

/// A derived FB call binds inherited parameters — the base's VAR_IN_OUT and
/// VAR_INPUT — alongside its own, and writes through the inherited VAR_IN_OUT
/// reach the caller's variable.
#[rstest]
fn test_inherited_parameters_bind_and_flow(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK BaseIO
        VAR_IN_OUT io : INT; END_VAR
        VAR_INPUT inp : INT; END_VAR
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK DerivedIO EXTENDS BaseIO
        VAR_INPUT own : INT; END_VAR
            io := io + inp + own;
        END_FUNCTION_BLOCK

        FUNCTION test : INT
        VAR d : DerivedIO; x : INT; END_VAR
            x := 5;
            d(io := x, inp := 10, own := 2);
            test := x;
        END_FUNCTION
    "#;
    let wasm = super::compile_to_wasm(&mut with_db, source);
    let result: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(result, 17, "5 + 10 + 2 through the inherited in_out");
}
