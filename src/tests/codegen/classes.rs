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
