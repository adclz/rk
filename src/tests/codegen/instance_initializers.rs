//! Initializers on the *members* of an FB or CLASS, seen through an instance.
//!
//! `VAR f : Flags;` carries no initializer of its own — the `:= 3` sits on
//! `Flags`'s own member declarations. Nothing in the enclosing POU walks those,
//! so an instance declared inside a FUNCTION/FB/CLASS used to start life all
//! zeroes no matter what its type declared. PROGRAM instances took a different
//! route (`collect_const_inits`, applied at load) and were never affected,
//! which is why `tests::codegen::initializers` missed this entirely.

use crate::tests::codegen::{compile_to_wasm, execute_wasm, with_db};
use rstest::*;

/// The bug in its simplest form: read a member back without ever writing it.
#[rstest]
fn fb_member_initializer_applies_to_a_function_local_instance(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Flags
        VAR
            bits : BYTE := 2#0000_1010;
        END_VAR
        END_FUNCTION_BLOCK

        FUNCTION run : BYTE
        VAR
            f : Flags;
        END_VAR
            run := f.bits;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 0b0000_1010);
}

/// Several members of different widths, so a single mis-sized store would show.
#[rstest]
fn fb_initializes_every_member(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Config
        VAR
            a : SINT := 7;
            b : INT := -300;
            c : DINT := 100000;
            d : LINT := 5000000000;
            e : BOOL := TRUE;
        END_VAR
        END_FUNCTION_BLOCK

        FUNCTION run : LINT
        VAR
            cfg : Config;
        END_VAR
            run := cfg.d;
            IF cfg.a = 7 AND cfg.b = -300 AND cfg.c = 100000 AND cfg.e THEN
                run := run + 1;
            END_IF;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i64 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 5000000001, "every member holds its initializer");
}

/// The FB body must see the initialized state, not zeroes — the initializers
/// run before it.
#[rstest]
fn fb_body_observes_its_own_initializers(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Counter
        VAR
            n : INT := 10;
        END_VAR
        VAR_OUTPUT
            out : INT;
        END_VAR
            n := n + 1;
            out := n;
        END_FUNCTION_BLOCK

        FUNCTION run : INT
        VAR
            c : Counter;
        END_VAR
            c();
            run := c.out;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 11, "body ran against 10, not 0");
}

/// Inherited members are initialized too, and by their declaring base's value.
/// `instance_members` supplies the whole EXTENDS chain, so this needs no
/// separate walk.
#[rstest]
fn inherited_members_are_initialized(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Base
        VAR
            base_val : INT := 4;
        END_VAR
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Derived EXTENDS Base
        VAR
            derived_val : INT := 9;
        END_VAR
        END_FUNCTION_BLOCK

        FUNCTION run : INT
        VAR
            d : Derived;
        END_VAR
            run := d.base_val * 100 + d.derived_val;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(
        result, 409,
        "base 4, derived 9 — neither aliased nor zeroed"
    );
}

/// An FB member that is itself an FB instance carries its own type's
/// initializers, one level further down.
#[rstest]
fn nested_instance_members_are_initialized(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Inner
        VAR
            v : INT := 42;
        END_VAR
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Outer
        VAR
            inner : Inner;
            own : INT := 8;
        END_VAR
        END_FUNCTION_BLOCK

        FUNCTION run : INT
        VAR
            o : Outer;
        END_VAR
            run := o.inner.v * 10 + o.own;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 428, "inner 42 and outer 8");
}

/// CLASS instances take the same path as FBs.
#[rstest]
fn class_member_initializers_apply(mut with_db: db::RootDatabase) {
    let source = r#"
        CLASS C
        VAR
            x : INT := 3;
            y : INT := 5;
        END_VAR
            METHOD PUBLIC sum : INT
                sum := x + y;
            END_METHOD
        END_CLASS

        FUNCTION run : INT
        VAR
            c : C;
        END_VAR
            run := c.sum();
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 8);
}

/// A member without an initializer stays zero rather than picking up a
/// neighbour's value — a mis-computed offset would show up here.
#[rstest]
fn uninitialized_members_stay_zero(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Mixed
        VAR
            a : INT;
            b : INT := 77;
            c : INT;
        END_VAR
        END_FUNCTION_BLOCK

        FUNCTION run : INT
        VAR
            m : Mixed;
        END_VAR
            run := m.a * 10000 + m.b * 100 + m.c;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 7700, "only b is initialized");
}

/// An FB instance declared inside another FB — the enclosing instance's own
/// prologue is not where this runs, so it exercises the FB-local path.
#[rstest]
fn fb_local_instance_inside_a_function_block(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Leaf
        VAR
            v : INT := 21;
        END_VAR
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Holder
        VAR
            leaf : Leaf;
        END_VAR
        VAR_OUTPUT
            out : INT;
        END_VAR
            out := leaf.v;
        END_FUNCTION_BLOCK

        FUNCTION run : INT
        VAR
            h : Holder;
        END_VAR
            h();
            run := h.out;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 21);
}

/// An array-typed member's initializer list still lands element by element
/// through the instance's base offset.
#[rstest]
fn array_member_initializer_applies(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Table
        VAR
            lead : INT := 1;
            vals : ARRAY[0..2] OF INT := [10, 20, 30];
        END_VAR
        END_FUNCTION_BLOCK

        FUNCTION run : INT
        VAR
            t : Table;
        END_VAR
            run := t.lead * 1000 + t.vals[0] + t.vals[1] + t.vals[2];
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 1060, "lead 1, elements 10+20+30");
}

/// A PROGRAM-hosted FB instance takes a different route to initialization
/// (`collect_const_inits`, applied at load), so it needs its own coverage: the
/// scan must observe the FB's declared member values, not zeroes.
#[rstest]
fn program_hosted_fb_instance_members_are_initialized(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Counter
        VAR
            n : INT := 10;
        END_VAR
        VAR_OUTPUT
            out : INT;
        END_VAR
            n := n + 1;
            out := n;
        END_FUNCTION_BLOCK

        PROGRAM P
        VAR RETAIN observed : INT; END_VAR
        VAR c : Counter; END_VAR
            c();
            observed := c.out;
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
    plc.run(1).expect("scan");
    let observed = i32::from_le_bytes(plc.read_retain()[..4].try_into().unwrap());
    assert_eq!(observed, 11, "the scan ran against n = 10, not 0");
}

/// Two members of the same instance type each get their own initializers: the
/// composition walk tracks the current *path*, not a global seen-set, so a type
/// reached twice through different members is not skipped the second time.
#[rstest]
fn two_members_of_the_same_instance_type_are_both_initialized(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Inner
        VAR
            v : INT := 6;
        END_VAR
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Outer
        VAR
            a : Inner;
            b : Inner;
        END_VAR
        END_FUNCTION_BLOCK

        FUNCTION run : INT
        VAR
            o : Outer;
        END_VAR
            run := o.a.v * 10 + o.b.v;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 66, "both Inner members initialized, not just the first");
}

/// A three-deep chain, to show the path prefix keeps accumulating rather than
/// flattening to the leaf's own name.
#[rstest]
fn three_level_nesting_is_initialized(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK L3
        VAR
            v : INT := 3;
        END_VAR
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK L2
        VAR
            pad : INT := 2;
            l3 : L3;
        END_VAR
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK L1
        VAR
            pad : INT := 1;
            l2 : L2;
        END_VAR
        END_FUNCTION_BLOCK

        FUNCTION run : INT
        VAR
            l1 : L1;
        END_VAR
            run := l1.pad * 100 + l1.l2.pad * 10 + l1.l2.l3.v;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 123, "each level lands at its own offset");
}

/// An array of instances gets its element type's member initializers, once per
/// element. `instance_initializers` has no `Cell` to report against the array
/// member itself, so the walk descends through the array with an `AllElements`
/// step and the layout expands it.
#[rstest]
fn array_of_instances_initializes_every_element(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Cell
        VAR
            v : INT := 5;
        END_VAR
        END_FUNCTION_BLOCK

        FUNCTION run : DINT
        VAR
            cells : ARRAY[0..2] OF Cell;
        END_VAR
            run := cells[0].v * 100 + cells[1].v * 10 + cells[2].v;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 555, "all three elements start at 5");
}

/// The same array held as a MEMBER of another FB — this is the path that goes
/// through HIR's `AllElements` step rather than the declaration-site descent.
#[rstest]
fn array_member_of_an_instance_initializes_every_element(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Cell
        VAR
            v : INT := 5;
        END_VAR
        END_FUNCTION_BLOCK

        FUNCTION_BLOCK Bank
        VAR
            lead : INT := 9;
            cells : ARRAY[0..2] OF Cell;
        END_VAR
        END_FUNCTION_BLOCK

        FUNCTION run : DINT
        VAR
            b : Bank;
        END_VAR
            run := b.lead * 1000 + b.cells[0].v * 100 + b.cells[1].v * 10 + b.cells[2].v;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 9555, "lead 9, and every cell 5");
}

/// A non-zero lower bound must not shift the element addresses.
#[rstest]
fn array_of_instances_with_non_zero_lower_bound(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Cell
        VAR
            v : INT := 7;
        END_VAR
        END_FUNCTION_BLOCK

        FUNCTION run : DINT
        VAR
            cells : ARRAY[1..3] OF Cell;
        END_VAR
            run := cells[1].v * 100 + cells[2].v * 10 + cells[3].v;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 777);
}

/// Initialized elements and a call on one of them compose: the call must find
/// the element already initialized, and tick only that one.
#[rstest]
fn initialized_array_elements_then_a_call(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Cell
        VAR
            v : INT := 5;
        END_VAR
            v := v + 1;
        END_FUNCTION_BLOCK

        FUNCTION run : DINT
        VAR
            cells : ARRAY[0..2] OF Cell;
        END_VAR
            cells[1]();
            run := cells[0].v * 100 + cells[1].v * 10 + cells[2].v;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let result: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(result, 565, "element 1 went 5 -> 6; the others stayed 5");
}
