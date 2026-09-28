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
