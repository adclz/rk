//! FUNCTION overloads lower to distinct WASM symbols (via the signature
//! discriminant) and each call routes to the right one.

use crate::tests::codegen::{compile_to_wasm, with_db};
use rstest::*;

// Two `add` overloads (1-arg adds one, 2-arg sums) coexist as distinct wasm
// functions; `test` calls both and gets each result.
#[rstest]
fn overloaded_functions_execute_independently(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION add : INT
        VAR_INPUT a : INT; END_VAR
            add := a + 1;
        END_FUNCTION

        FUNCTION add : INT
        VAR_INPUT a : INT; b : INT; END_VAR
            add := a + b;
        END_FUNCTION

        FUNCTION test : INT
            test := add(10) + add(3, 4);
        END_FUNCTION
    "#;

    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 18, "add/1(10) = 11, add/2(3, 4) = 7");
}

// Overloading works for non-integer types too — two REAL overloads, one per
// arity, each lowered and called independently.
#[rstest]
fn overloaded_real_functions(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION avg : REAL
        VAR_INPUT a : REAL; END_VAR
            avg := a;
        END_FUNCTION

        FUNCTION avg : REAL
        VAR_INPUT a : REAL; b : REAL; END_VAR
            avg := (a + b) / 2.0;
        END_FUNCTION

        FUNCTION test : REAL
            test := avg(3.0) + avg(2.0, 8.0);   // 3.0 + 5.0
        END_FUNCTION
    "#;

    let result: f32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 8.0, "avg/1(3.0) = 3.0, avg/2(2.0, 8.0) = 5.0");
}

// Same arity, different parameter TYPE — `conv(INT)` and `conv(REAL)` lower to
// distinct symbols (conv$Int / conv$Real) and each call routes by argument type.
#[rstest]
fn same_arity_typed_overloads(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION conv : INT
        VAR_INPUT a : INT; END_VAR
            conv := a * 2;
        END_FUNCTION

        FUNCTION conv : INT
        VAR_INPUT a : REAL; END_VAR
            conv := 99;
        END_FUNCTION

        FUNCTION test : INT
        VAR i : INT; END_VAR
            i := 5;
            test := conv(i) + conv(1.0);   // conv(INT)=10 + conv(REAL)=99
        END_FUNCTION
    "#;

    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 109, "conv(INT 5) = 10, conv(REAL 1.0) = 99");
}

/// The overload chosen for an indexed argument must be the one that actually
/// runs — a resolution bug here is silent at the call site and only shows up in
/// the value produced.
#[rstest]
fn indexed_argument_dispatches_to_the_right_overload(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION tag : DINT
        VAR_INPUT v : INT; END_VAR
            tag := 1;
        END_FUNCTION

        FUNCTION tag : DINT
        VAR_INPUT v : DINT; END_VAR
            tag := 2;
        END_FUNCTION

        FUNCTION test : DINT
        VAR
            ints  : ARRAY[0..1] OF INT;
            dints : ARRAY[-1..1] OF DINT;
        END_VAR
            test := tag(ints[0]) * 10 + tag(dints[-1]);
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 12,
        "INT element -> overload 1, DINT element -> overload 2"
    );
}

/// RETURN-directed overloads emit as DISTINCT wasm functions and each call
/// routes to the overload its target picked. The params-only mangle put both
/// on one symbol, and module lowering silently skipped the second body — so
/// both assignments would have run the same function.
#[rstest]
fn return_overloads_route_by_target(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION G : INT
            G := 41;
        END_FUNCTION
        FUNCTION G : DINT
            G := 4200;
        END_FUNCTION
        FUNCTION test : DINT
        VAR i : INT; d : DINT; END_VAR
            i := G();
            d := G();
            test := d + INT_TO_DINT(i);
        END_FUNCTION
        FUNCTION INT_TO_DINT : DINT
        VAR_INPUT IN : INT; END_VAR
            {wasm 'nop' (params IN) (result INT_TO_DINT)}
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "test", ());
    assert_eq!(r, 4241, "each target got its own overload: 4200 + 41");
}

/// An untyped integer literal picks the INT overload by EXACT match; the REAL
/// candidate would also accept it by promotion, so this pins the rule rather
/// than letting either fall out.
#[rstest]
fn untyped_literal_prefers_exact_int(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION conv : INT
        VAR_INPUT a : INT; END_VAR
            conv := a * 2;
        END_FUNCTION
        FUNCTION conv : INT
        VAR_INPUT a : REAL; END_VAR
            conv := 99;
        END_FUNCTION
        FUNCTION test : INT
            test := conv(5);
        END_FUNCTION
    "#;
    let r: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(r, 10, "5 is INT by exact match, not REAL by promotion");
}

/// The declared arity preference at runtime: add(10) runs add/1.
#[rstest]
fn full_arity_beats_defaulted_at_runtime(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION add : INT
        VAR_INPUT a : INT; END_VAR
            add := a + 1;
        END_FUNCTION
        FUNCTION add : INT
        VAR_INPUT a : INT; b : INT := 100; END_VAR
            add := a + b;
        END_FUNCTION
        FUNCTION test : INT
            test := add(10);
        END_FUNCTION
    "#;
    let r: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(r, 11, "add/1 wins over add/2 padded with its default");
}

/// The INITIALIZER path lowers separately from statements — "type-checked to
/// the right overload" does not prove "emitted a call to the right symbol",
/// which is precisely how the params-only mangle bug hid.
#[rstest]
fn return_overload_in_initializer_runs(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION G : INT
            G := 41;
        END_FUNCTION
        FUNCTION G : DINT
            G := 4200;
        END_FUNCTION
        FUNCTION test : DINT
        VAR d : DINT := G(); END_VAR
            test := d;
        END_FUNCTION
    "#;
    let r: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(r, 4200, "the initializer called the DINT overload");
}

/// And the return-slot path: `test := G()` inside a DINT function.
#[rstest]
fn return_overload_in_return_slot_runs(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION G : INT
            G := 41;
        END_FUNCTION
        FUNCTION G : DINT
            G := 4200;
        END_FUNCTION
        FUNCTION test : DINT
            test := G();
        END_FUNCTION
    "#;
    let r: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(r, 4200, "the return slot picked and ran the DINT overload");
}

/// An overloaded call in an INITIALIZER: its resolution lives in init
/// inference, which the callee-symbol lookup never consulted — the call
/// lowered under its BARE name and died in codegen as an unknown function.
#[rstest]
fn overloaded_call_in_initializer_runs(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION add : INT
        VAR_INPUT a : INT; END_VAR
            add := a + 1;
        END_FUNCTION
        FUNCTION add : INT
        VAR_INPUT a : INT; b : INT; END_VAR
            add := a + b;
        END_FUNCTION
        FUNCTION test : INT
        VAR x : INT := add(1, 2); END_VAR
            test := x;
        END_FUNCTION
    "#;
    let r: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(r, 3, "the initializer called the resolved 2-arg overload");
}

/// Overloads on named arrays all lowered to `Which2$T`, and every
/// call ran the first one declared.
#[rstest]
fn overloads_on_named_arrays_reach_their_own_body(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Real5 : ARRAY[0..4] OF REAL; Int5 : ARRAY[0..4] OF INT; Int7 : ARRAY[0..6] OF INT; END_TYPE
        FUNCTION Which2 : INT VAR_INPUT IN : Real5; END_VAR Which2 := 2; END_FUNCTION
        FUNCTION Which2 : INT VAR_INPUT IN : Int5;  END_VAR Which2 := 1; END_FUNCTION
        FUNCTION Which2 : INT VAR_INPUT IN : Int7;  END_VAR Which2 := 7; END_FUNCTION
        FUNCTION test : INT
        VAR i : Int5; j : Int7; r : Real5; END_VAR
            test := Which2(IN := i) * 100 + Which2(IN := j) * 10 + Which2(IN := r);
        END_FUNCTION
    "#;
    let r: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(r, 172, "Int5 ran 1, Int7 ran 7, Real5 ran 2");
}

/// Named enums and structs lost their names the same way.
#[rstest]
fn overloads_on_named_enums_and_structs_reach_their_own_body(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE
            Color : (Red, Green);
            Shape : (Circle, Square);
            Point : STRUCT x : INT; END_STRUCT;
            Size : STRUCT w : INT; END_STRUCT;
        END_TYPE
        FUNCTION ByEnum : INT VAR_INPUT c : Color; END_VAR ByEnum := 1; END_FUNCTION
        FUNCTION ByEnum : INT VAR_INPUT s : Shape; END_VAR ByEnum := 2; END_FUNCTION
        FUNCTION ByStruct : INT VAR_INPUT p : Point; END_VAR ByStruct := 1; END_FUNCTION
        FUNCTION ByStruct : INT VAR_INPUT s : Size; END_VAR ByStruct := 2; END_FUNCTION
        FUNCTION test : INT
        VAR c : Color; s : Shape; p : Point; z : Size; END_VAR
            test := ByEnum(c) * 1000 + ByEnum(s) * 100 + ByStruct(p) * 10 + ByStruct(z);
        END_FUNCTION
    "#;
    let r: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(r, 1212, "each argument type ran its own overload");
}

/// `Motion.Axis` and a top-level `Motion_Axis` mangled to one fragment.
#[rstest]
fn namespaced_parameter_types_reach_their_own_body(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Motion_Axis VAR x : INT; END_VAR END_FUNCTION_BLOCK
        NAMESPACE Motion
            FUNCTION_BLOCK Axis VAR x : INT; END_VAR END_FUNCTION_BLOCK
        END_NAMESPACE
        FUNCTION Which : INT VAR_IN_OUT v : Motion_Axis; END_VAR Which := 1; END_FUNCTION
        FUNCTION Which : INT VAR_IN_OUT v : Motion.Axis; END_VAR Which := 2; END_FUNCTION
        FUNCTION test : INT
        VAR a : Motion_Axis; b : Motion.Axis; END_VAR
            test := Which(a) * 10 + Which(b);
        END_FUNCTION
    "#;
    let r: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(r, 12, "Motion_Axis ran 1, Motion.Axis ran 2");
}

/// `f(INT) : INT`, tied with `f(INT) : REAL`, was spelled like
/// `f(INT, INT)`; the two-argument call ran the one-parameter body.
#[rstest]
fn a_return_directed_pair_beside_a_longer_sibling(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION f : INT VAR_INPUT a : INT; END_VAR f := 1; END_FUNCTION
        FUNCTION f : REAL VAR_INPUT a : INT; END_VAR f := 2.0; END_FUNCTION
        FUNCTION f : INT VAR_INPUT a : INT; b : INT; END_VAR f := 3; END_FUNCTION
        FUNCTION test : INT
        VAR one : INT; two : INT; END_VAR
            one := f(INT#0);
            two := f(INT#0, INT#0);
            test := one * 10 + two;
        END_FUNCTION
    "#;
    let r: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(r, 13, "f(INT) : INT ran 1, f(INT, INT) ran 3");
}

/// Two overloads with an interface parameter, specialized at one
/// implementer, shared one specialization.
#[rstest]
fn interface_overloads_specialize_separately(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE IDev METHOD Id : INT END_METHOD END_INTERFACE
        FUNCTION_BLOCK Pump IMPLEMENTS IDev
            METHOD PUBLIC Id : INT Id := 7; END_METHOD
        END_FUNCTION_BLOCK
        FUNCTION drive : INT VAR_IN_OUT d : IDev; END_VAR VAR_INPUT n : INT; END_VAR drive := 100 + d.Id(); END_FUNCTION
        FUNCTION drive : INT VAR_IN_OUT d : IDev; END_VAR VAR_INPUT n : DINT; END_VAR drive := 200 + d.Id(); END_FUNCTION
        // Through an interface parameter: an overloaded callee does not take
        // an implementer yet (E0810).
        FUNCTION viaInt : INT VAR_IN_OUT d : IDev; END_VAR viaInt := drive(d, INT#1); END_FUNCTION
        FUNCTION viaDint : INT VAR_IN_OUT d : IDev; END_VAR viaDint := drive(d, DINT#1); END_FUNCTION
        FUNCTION runInt : INT VAR p : Pump; END_VAR runInt := viaInt(p); END_FUNCTION
        FUNCTION runDint : INT VAR p : Pump; END_VAR runDint := viaDint(p); END_FUNCTION
    "#;
    let wasm = super::compile_to_wasm(&mut with_db, source);
    let int: i32 = super::execute_wasm(&wasm, "runInt", ());
    let dint: i32 = super::execute_wasm(&wasm, "runDint", ());
    assert_eq!(
        (int, dint),
        (107, 207),
        "each overload ran its own specialization"
    );
}

/// The specialization of `drive(d : IDev)` at Pump was `drive$Pump`, the
/// symbol of the overload `drive(p : Pump)`.
#[rstest]
fn an_fb_overload_beside_an_interface_overload(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE IDev METHOD Id : INT END_METHOD END_INTERFACE
        FUNCTION_BLOCK Pump IMPLEMENTS IDev
            METHOD PUBLIC Id : INT Id := 7; END_METHOD
        END_FUNCTION_BLOCK
        FUNCTION drive : INT VAR_IN_OUT d : IDev; END_VAR drive := 100 + d.Id(); END_FUNCTION
        FUNCTION drive : INT VAR_IN_OUT p : Pump; END_VAR drive := 1; END_FUNCTION
        FUNCTION via : INT VAR_IN_OUT d : IDev; END_VAR via := drive(d); END_FUNCTION
        FUNCTION viaIface : INT VAR p : Pump; END_VAR viaIface := via(p); END_FUNCTION
        FUNCTION viaFb : INT VAR p : Pump; END_VAR viaFb := drive(p); END_FUNCTION
    "#;
    let wasm = super::compile_to_wasm(&mut with_db, source);
    let iface: i32 = super::execute_wasm(&wasm, "viaIface", ());
    let fb: i32 = super::execute_wasm(&wasm, "viaFb", ());
    assert_eq!(
        (iface, fb),
        (107, 1),
        "drive(IDev) and drive(Pump) are two bodies"
    );
}

// ---------------------------------------------------------------------------
// Selection binds the call as a plain call binds it: by name, with defaults
// and variadics, each argument against the parameter it lands on.
// ---------------------------------------------------------------------------

/// Named arguments bind by name: written out of order they still pick the
/// overload whose parameters they name; one may skip a defaulted input, and
/// a name only one overload declares chooses it.
#[rstest]
fn named_arguments_select_by_name(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION f : INT VAR_INPUT a : DINT; b : LINT; END_VAR f := 1; END_FUNCTION
        FUNCTION f : INT VAR_INPUT a : LINT; b : DINT; END_VAR f := 2; END_FUNCTION

        FUNCTION g : INT VAR_INPUT a : INT; b : INT := 5; c : REAL; END_VAR g := b; END_FUNCTION
        FUNCTION g : INT VAR_INPUT a : DATE; END_VAR g := 2; END_FUNCTION

        FUNCTION h : INT VAR_INPUT a : INT; END_VAR h := 1; END_FUNCTION
        FUNCTION h : INT VAR_INPUT b : INT; c : INT := 0; END_VAR h := 2; END_FUNCTION

        FUNCTION test : INT
        VAR i : INT := 1; d : DINT := 2; END_VAR
            test := f(b := i, a := d) * 100 + g(a := 1, c := 2.5) * 10 + h(b := 1);
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 152,
        "f(a: DINT) is exact for `a := d`, g takes b's default 5, h is the one with a `b`"
    );
}

/// An output binding goes with the overload its inputs pick, in any order.
#[rstest]
fn an_output_binding_goes_with_the_overload(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Get : INT VAR_INPUT i : INT; END_VAR VAR_OUTPUT o : INT; END_VAR
            o := i * 2; Get := 1;
        END_FUNCTION
        FUNCTION Get : INT VAR_INPUT i : REAL; END_VAR VAR_OUTPUT o : REAL; END_VAR
            o := i * 2.0; Get := 2;
        END_FUNCTION

        FUNCTION test : BOOL
        VAR x : INT; y : REAL; a : INT; b : INT; END_VAR
            a := Get(i := 3, o => x);
            b := Get(o => y, i := REAL#1.5);
            test := a = 1 AND x = 6 AND b = 2 AND y = 3.0;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 1, "INT then REAL, each writing its own output");
}

/// An untyped literal matches its default type exactly only when its value
/// fits it: 70000 is no INT, so the DINT overload takes it.
#[rstest]
fn a_literal_picks_an_overload_it_fits(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Which : DINT VAR_INPUT v : INT; END_VAR Which := 1; END_FUNCTION
        FUNCTION Which : DINT VAR_INPUT v : DINT; END_VAR Which := v; END_FUNCTION

        FUNCTION test : DINT
            test := Which(70000) + Which(5);
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 70001, "70000 through DINT, 5 through INT");
}

/// An inline `REF_TO` or `ARRAY` parameter is its structure, as the call's
/// coercion compares it: an exact argument used to match no overload.
#[rstest]
fn inline_composite_parameters_select_by_structure(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Pointee : INT VAR_INPUT r : REF_TO INT; END_VAR Pointee := 1; END_FUNCTION
        FUNCTION Pointee : INT VAR_INPUT r : REF_TO REAL; END_VAR Pointee := 2; END_FUNCTION
        FUNCTION Arr : INT VAR_INPUT a : ARRAY[0..2] OF INT; END_VAR Arr := 10; END_FUNCTION
        FUNCTION Arr : INT VAR_INPUT a : ARRAY[0..2] OF REAL; END_VAR Arr := 20; END_FUNCTION

        FUNCTION test : INT
        VAR
            x : INT; y : REAL;
            ri : REF_TO INT; rr : REF_TO REAL;
            ai : ARRAY[0..2] OF INT; ar : ARRAY[0..2] OF REAL;
        END_VAR
            ri := REF(x);
            rr := REF(y);
            test := Pointee(ri) + Pointee(rr) * 100 + Arr(ai) + Arr(ar) * 100;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 2211, "each argument reaches its own overload");
}

/// An enum literal, an implementer and NULL select the overload a single
/// function would take them into.
#[rstest]
fn an_enum_an_implementer_and_null_select(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE Color : (Red, Green); RInt : REF_TO INT; END_TYPE
        INTERFACE IDev METHOD Id : INT END_METHOD END_INTERFACE
        FUNCTION_BLOCK Pump IMPLEMENTS IDev METHOD PUBLIC Id : INT Id := 7; END_METHOD END_FUNCTION_BLOCK

        FUNCTION TwoC : INT VAR_INPUT c : Color; END_VAR TwoC := 1; END_FUNCTION
        FUNCTION TwoC : INT VAR_INPUT s : STRING; END_VAR TwoC := -1; END_FUNCTION
        FUNCTION TwoD : INT VAR_IN_OUT d : IDev; END_VAR TwoD := d.Id(); END_FUNCTION
        FUNCTION TwoD : INT VAR_INPUT s : STRING; END_VAR TwoD := -1; END_FUNCTION
        FUNCTION TwoR : INT VAR_INPUT r : RInt; END_VAR TwoR := 3; END_FUNCTION
        FUNCTION TwoR : INT VAR_INPUT s : STRING; END_VAR TwoR := -1; END_FUNCTION

        FUNCTION test : INT
        VAR p : Pump; END_VAR
            test := TwoC(Color#Green) * 100 + TwoD(p) * 10 + TwoR(NULL);
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 173, "Color, IDev and RInt, not STRING");
}

/// Variadic overloads bind their packs: a call with more arguments than
/// declared parameters matched none and fell back to the first declared.
#[rstest]
fn variadic_overloads_select_by_element_type(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION total : DINT VAR_INPUT args : DINT...; END_VAR total := ...args+; END_FUNCTION
        FUNCTION total : LREAL VAR_INPUT args : LREAL...; END_VAR total := ...args+; END_FUNCTION

        FUNCTION test : BOOL
            test := total(DINT#1, DINT#2) = 3 AND total(LREAL#1.5, LREAL#2.5, LREAL#1.0) = 5.0;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 1, "the DINT pack and the LREAL pack");
}

/// A VAR_IN_OUT candidate takes a variable of its own type only: a wider
/// variable or a literal leaves the input overload, which made both calls
/// ambiguous or refused.
#[rstest]
fn an_in_out_candidate_takes_its_own_type_only(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION Which : INT VAR_IN_OUT x : DINT; END_VAR Which := 1; END_FUNCTION
        FUNCTION Which : INT VAR_INPUT x : REAL; END_VAR Which := 2; END_FUNCTION

        FUNCTION test : INT
        VAR i : INT; d : DINT; END_VAR
            test := Which(i) * 100 + Which(5) * 10 + Which(d);
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(
        result, 221,
        "INT and a literal go to REAL, a DINT binds the in-out"
    );
}

/// `REF(x)` is a reference to `x`, not `x`.
#[rstest]
fn a_reference_argument_selects_the_reference_overload(mut with_db: db::RootDatabase) {
    let source = r#"
        TYPE RInt : REF_TO INT; END_TYPE
        FUNCTION Which : INT VAR_INPUT r : RInt; END_VAR Which := 1; END_FUNCTION
        FUNCTION Which : INT VAR_INPUT i : INT; END_VAR Which := 2; END_FUNCTION

        FUNCTION test : INT
        VAR x : INT := 4; END_VAR
            test := Which(REF(x)) * 10 + Which(x);
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 12, "REF(x) to RInt, x to INT");
}

/// A function that is not overloaded knows its parameter's type, which picks
/// a RETURN-directed overload passed to it, as an assignment's target does.
#[rstest]
fn a_parameter_picks_a_return_overload(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION G : INT G := 1; END_FUNCTION
        FUNCTION G : REAL G := 2.5; END_FUNCTION
        FUNCTION twice : INT VAR_INPUT x : INT; END_VAR twice := x * 2; END_FUNCTION

        FUNCTION test : INT
            test := twice(G());
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 2, "G : INT, doubled");
}

/// A pack takes each argument by the same rule: the DINT one refuses an
/// LREAL, so the LREAL one, which widens the DINT, is the one that fits.
#[rstest]
fn a_mixed_pack_takes_the_overload_every_argument_fits(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION total : DINT VAR_INPUT args : DINT...; END_VAR total := ...args+; END_FUNCTION
        FUNCTION total : LREAL VAR_INPUT args : LREAL...; END_VAR total := ...args+; END_FUNCTION

        FUNCTION test : BOOL
            test := total(DINT#1, LREAL#2.5) = 3.5;
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 1, "the LREAL pack");
}

/// An instance of two interfaces fits both overloads; naming the parameter,
/// when they name it differently, picks one.
#[rstest]
fn naming_the_parameter_picks_among_interfaces(mut with_db: db::RootDatabase) {
    let source = r#"
        INTERFACE IA METHOD A : INT END_METHOD END_INTERFACE
        INTERFACE IB METHOD B : INT END_METHOD END_INTERFACE
        FUNCTION_BLOCK Both IMPLEMENTS IA, IB
            METHOD PUBLIC A : INT A := 1; END_METHOD
            METHOD PUBLIC B : INT B := 2; END_METHOD
        END_FUNCTION_BLOCK
        FUNCTION Dev : INT VAR_IN_OUT da : IA; END_VAR Dev := 10 + da.A(); END_FUNCTION
        FUNCTION Dev : INT VAR_IN_OUT db : IB; END_VAR Dev := 20 + db.B(); END_FUNCTION

        FUNCTION test : INT
        VAR b : Both; END_VAR
            test := Dev(da := b) * 100 + Dev(db := b);
        END_FUNCTION
    "#;
    let result: i32 = super::run(&mut with_db, source, "test", ());
    assert_eq!(result, 1122, "Dev(IA) then Dev(IB)");
}
