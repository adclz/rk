// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! An instance is copied by an assignment: the copy takes the whole state
//! and goes its own way afterwards. These
//! tests run the copies and read the values back, for an instance on its
//! own, in an array, in a STRUCT, through a VAR_IN_OUT, a call binding, a
//! derived block, a reference member and a PROGRAM's scan.

use rstest::rstest;

use super::with_db;

/// A counter whose state, inputs and outputs all show in a copy.
const COUNTER: &str = r#"
FUNCTION_BLOCK Counter
VAR_INPUT step : INT := 1; END_VAR
VAR_OUTPUT q : INT; END_VAR
VAR n : INT; END_VAR
    n := n + step;
    q := n * 10;
END_FUNCTION_BLOCK

FUNCTION check
VAR_INPUT ok : BOOL; what : STRING; END_VAR
    IF NOT ok THEN
        __RAISE(what);
    END_IF;
END_FUNCTION
"#;

/// Compiles `source` with [`COUNTER`] and runs its `{test}` functions. Each
/// failed check names what was wrong.
fn copies_hold(db: &mut db::RootDatabase, source: &str) {
    let source = format!("{COUNTER}\n{source}");
    let wasm = super::compile_to_wasm_as_built(db, &source);
    let results = crate::tests::codegen::run_tests(&wasm, None).expect("run tests");
    assert!(!results.is_empty(), "the source declares no {{test}}");
    let failed: Vec<String> = results
        .iter()
        .filter(|r| !r.passed())
        .map(|r| format!("{}: {}", r.name, r.reason.as_deref().unwrap_or("")))
        .collect();
    assert!(failed.is_empty(), "failed checks:\n{}", failed.join("\n"));
}

/// `b := a` copies the state, the inputs and the outputs. Afterwards each
/// runs on its own: a call of one leaves the other as it was.
#[rstest]
fn an_instance_copy_takes_the_state_and_goes_its_own_way(mut with_db: db::RootDatabase) {
    copies_hold(
        &mut with_db,
        r#"
{test}
FUNCTION test_copy
VAR a : Counter; b : Counter; END_VAR
    a(step := 2);
    a();
    b := a;
    check(b.n = 4, 'the copy took the state');
    check(b.q = 40, 'the copy took the output');
    check(b.step = 2, 'the copy took the input');
    a();
    check(a.n = 6, 'the original runs on');
    check(b.n = 4, 'the copy is not the original');
    b(step := 10);
    check(b.n = 14, 'the copy runs on its own state');
    check(a.n = 6, 'the original is not the copy');
END_FUNCTION
"#,
    );
}

/// The copy takes a STRING member and an ARRAY member whole, and a change to
/// the original afterwards is not seen in the copy.
#[rstest]
fn a_copy_takes_string_and_array_members(mut with_db: db::RootDatabase) {
    copies_hold(
        &mut with_db,
        r#"
FUNCTION_BLOCK Buffer
VAR name : STRING; data : ARRAY[0..3] OF INT; END_VAR
END_FUNCTION_BLOCK

{test}
FUNCTION test_members
VAR a : Buffer; b : Buffer; END_VAR
    a.name := 'left';
    a.data[2] := 7;
    a.data[3] := 8;
    b := a;
    check(b.name = 'left', 'the string is copied');
    check(b.data[2] = 7 AND b.data[3] = 8, 'the array is copied');
    a.data[2] := 9;
    a.name := 'right';
    check(b.data[2] = 7, 'the copied array is its own');
    check(b.name = 'left', 'the copied string is its own');
END_FUNCTION
"#,
    );
}

/// An instance held by an instance is copied with it.
#[rstest]
fn a_copy_takes_a_nested_instance(mut with_db: db::RootDatabase) {
    copies_hold(
        &mut with_db,
        r#"
FUNCTION_BLOCK Outer
VAR inner : Counter; k : INT; END_VAR
    inner();
    k := k + 100;
END_FUNCTION_BLOCK

{test}
FUNCTION test_nested
VAR o1 : Outer; o2 : Outer; END_VAR
    o1();
    o1();
    o2 := o1;
    check(o2.inner.n = 2, 'the nested instance is copied');
    check(o2.k = 200, 'the own member is copied');
    o2();
    check(o2.inner.n = 3 AND o1.inner.n = 2, 'the nested copy is its own');
END_FUNCTION
"#,
    );
}

/// An ARRAY of instances is copied whole, and one element at a time, at a
/// constant and at a computed index.
#[rstest]
fn an_array_of_instances_is_copied_whole_and_by_element(mut with_db: db::RootDatabase) {
    copies_hold(
        &mut with_db,
        r#"
{test}
FUNCTION test_array
VAR arr1 : ARRAY[0..2] OF Counter; arr2 : ARRAY[0..2] OF Counter; i : INT := 2; END_VAR
    arr1[0]();
    arr1[0]();
    arr1[2]();
    arr2 := arr1;
    check(arr2[0].n = 2 AND arr2[1].n = 0 AND arr2[2].n = 1, 'the whole array is copied');
    arr2[1] := arr1[0];
    check(arr2[1].n = 2, 'an element is copied from an element');
    arr2[1]();
    check(arr2[1].n = 3 AND arr1[0].n = 2 AND arr2[0].n = 2, 'the copied element is its own');
    arr1[i] := arr2[1];
    check(arr1[2].n = 3, 'an element at a computed index is copied');
    arr2[0] := arr1[i];
    check(arr2[0].n = 3, 'an element is copied from a computed index');
END_FUNCTION
"#,
    );
}

/// A STRUCT with an instance among its fields is copied whole, the instance
/// with it; the field alone copies too, and so does an element of an ARRAY
/// of such structs.
#[rstest]
fn a_struct_holding_an_instance_is_copied(mut with_db: db::RootDatabase) {
    copies_hold(
        &mut with_db,
        r#"
TYPE Holder : STRUCT c : Counter; k : INT; END_STRUCT; END_TYPE

{test}
FUNCTION test_struct
VAR h1 : Holder; h2 : Holder; hs : ARRAY[0..1] OF Holder; END_VAR
    h1.c();
    h1.k := 4;
    h2 := h1;
    check(h2.c.n = 1 AND h2.k = 4, 'the struct is copied with its instance');
    h2.c();
    check(h2.c.n = 2 AND h1.c.n = 1, 'the copied instance is its own');
    h1.c := h2.c;
    check(h1.c.n = 2, 'the field is copied from a field');
    hs[1] := h2;
    check(hs[1].c.n = 2 AND hs[1].k = 4, 'an element of an array of structs is copied');
    hs[0].c := hs[1].c;
    hs[0].c();
    check(hs[0].c.n = 3 AND hs[1].c.n = 2, 'a field of an element is copied');
END_FUNCTION
"#,
    );
}

/// A CLASS instance is copied like a FUNCTION_BLOCK one: a method called on
/// the copy works on the copy's state.
#[rstest]
fn a_class_instance_is_copied(mut with_db: db::RootDatabase) {
    copies_hold(
        &mut with_db,
        r#"
CLASS Acc
VAR total : INT; END_VAR
METHOD PUBLIC add
VAR_INPUT x : INT; END_VAR
    total := total + x;
END_METHOD
METHOD PUBLIC get : INT
    get := total;
END_METHOD
END_CLASS

{test}
FUNCTION test_class
VAR a : Acc; b : Acc; END_VAR
    a.add(5);
    b := a;
    check(b.get() = 5, 'the class instance is copied');
    b.add(1);
    check(b.get() = 6 AND a.get() = 5, 'the copy has its own state');
END_FUNCTION
"#,
    );
}

/// A derived block is copied with its base part.
#[rstest]
fn a_derived_instance_is_copied_with_its_base_part(mut with_db: db::RootDatabase) {
    copies_hold(
        &mut with_db,
        r#"
FUNCTION_BLOCK Base
VAR n : INT; END_VAR
    n := n + 1;
END_FUNCTION_BLOCK

FUNCTION_BLOCK Derived EXTENDS Base
VAR m : INT; END_VAR
    SUPER();
    m := m + 10;
END_FUNCTION_BLOCK

{test}
FUNCTION test_derived
VAR d1 : Derived; d2 : Derived; END_VAR
    d1();
    d1();
    d2 := d1;
    check(d2.n = 2, 'the base part is copied');
    check(d2.m = 20, 'the derived part is copied');
    d2();
    check(d2.n = 3 AND d2.m = 30 AND d1.n = 2, 'the copy runs on its own');
END_FUNCTION
"#,
    );
}

/// A REF_TO member is copied as it is: the copy's reference points at the
/// original's target, and a write through it reaches that target.
#[rstest]
fn a_copied_reference_member_keeps_its_target(mut with_db: db::RootDatabase) {
    copies_hold(
        &mut with_db,
        r#"
FUNCTION_BLOCK Watcher
VAR p : REF_TO INT; END_VAR
END_FUNCTION_BLOCK

{test}
FUNCTION test_reference
VAR a : Watcher; b : Watcher; x : INT := 1; END_VAR
    a.p := REF(x);
    b := a;
    x := 42;
    check(b.p^ = 42, 'the copied reference still points at the target');
    b.p^ := 7;
    check(x = 7 AND a.p^ = 7, 'a write through the copy reaches the target');
END_FUNCTION
"#,
    );
}

/// A copy made inside a FUNCTION through two VAR_IN_OUTs lands in the
/// caller's instances, not in the function's frame.
#[rstest]
fn a_copy_through_var_in_out_lands_in_the_callers_instance(mut with_db: db::RootDatabase) {
    copies_hold(
        &mut with_db,
        r#"
FUNCTION clone_into
VAR_IN_OUT dst : Counter; src : Counter; END_VAR
    dst := src;
END_FUNCTION

{test}
FUNCTION test_in_out
VAR a : Counter; b : Counter; END_VAR
    a();
    a();
    clone_into(dst := b, src := a);
    check(b.n = 2, 'the copy reached the caller');
    b();
    check(b.n = 3 AND a.n = 2, 'the caller holds two instances');
END_FUNCTION
"#,
    );
}

/// A call binding copies an instance in, `seed := a`, and out, `made => c`.
#[rstest]
fn a_call_binding_copies_an_instance_in_and_out(mut with_db: db::RootDatabase) {
    copies_hold(
        &mut with_db,
        r#"
FUNCTION_BLOCK Taker
VAR_INPUT seed : Counter; END_VAR
VAR_OUTPUT made : Counter; END_VAR
VAR n : INT; END_VAR
    n := seed.n;
    made();
END_FUNCTION_BLOCK

{test}
FUNCTION test_bindings
VAR a : Counter; t : Taker; c : Counter; END_VAR
    a();
    a();
    t(seed := a);
    check(t.n = 2, 'the input binding copied the instance in');
    t.seed();
    check(t.seed.n = 3 AND a.n = 2, 'the input is a copy');
    t(made => c);
    check(c.n = 2, 'the output binding copied the instance out');
    c();
    check(c.n = 3 AND t.made.n = 2, 'the output is a copy');
END_FUNCTION
"#,
    );
}

/// A copy between two members of a block, in its own body, from a `THIS`
/// and from a method.
#[rstest]
fn a_block_copies_between_its_own_members(mut with_db: db::RootDatabase) {
    copies_hold(
        &mut with_db,
        r#"
FUNCTION_BLOCK Pair
VAR a : Counter; b : Counter; END_VAR
METHOD PUBLIC snapshot
    THIS.b := THIS.a;
END_METHOD
    a();
END_FUNCTION_BLOCK

{test}
FUNCTION test_members
VAR p : Pair; END_VAR
    p();
    p();
    p.snapshot();
    check(p.b.n = 2, 'the method copied one member over the other');
    p();
    check(p.a.n = 3 AND p.b.n = 2, 'the members are two instances');
END_FUNCTION
"#,
    );
}

/// A PROGRAM copies an instance at each scan, and the copy survives the
/// scan: the state is in the instance, not in a frame.
#[rstest]
fn a_program_copies_an_instance_across_scans(mut with_db: db::RootDatabase) {
    use debug_format::{DebugInfo, VarValue};
    let source = format!(
        "{COUNTER}{}",
        r#"
PROGRAM Main
VAR a : Counter; b : Counter; saved : Counter; scans : INT; END_VAR
    a();
    scans := scans + 1;
    IF scans = 2 THEN
        saved := a;
    END_IF;
    b := a;
END_PROGRAM

CONFIGURATION Cfg
    RESOURCE Res ON CPU
        TASK T(INTERVAL := T#10ms, PRIORITY := 1);
        PROGRAM P1 WITH T : Main;
    END_RESOURCE
END_CONFIGURATION
"#
    );
    let (_mir, wasm) = super::compile_to_mir_and_wasm(&mut with_db, &source);
    let info = DebugInfo::from_wasm(&wasm);
    let mut plc = super::TestPlc::load(&wasm).expect("load");
    plc.run(4).expect("scans");
    let read = |plc: &super::TestPlc, path: &str| plc.read_var(&info, path).expect(path);
    assert_eq!(read(&plc, "P1.a.n"), VarValue::I16(4), "four scans");
    assert_eq!(
        read(&plc, "P1.b.n"),
        VarValue::I16(4),
        "copied after the last scan"
    );
    assert_eq!(
        read(&plc, "P1.b.q"),
        VarValue::I16(40),
        "the output came with it"
    );
    assert_eq!(
        read(&plc, "P1.saved.n"),
        VarValue::I16(2),
        "the copy of the second scan is untouched since"
    );
}
