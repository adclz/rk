// Monomorphization of `{#if X is T}` arms.
//
// At MIR time, each `(func, T)` pair pulls the matching arm body and
// classifies it: wasm pragma > intrinsic path, extern pragma > import
// path, else > local body. Arms that don't match `T` are dropped.
//
// If no arm matches, the monomorphization still emits a function (with
// empty body). It's the implementer's responsibility to cover the
// variants they care about - covered by the `generic-extern` lint.

use db::RootDatabase;
use insta::assert_snapshot;
use rstest::rstest;

use super::utils::mir_exports;
use crate::tests::utils::with_db;

#[rstest]
fn wasm_pragma_inside_if_arm(mut with_db: RootDatabase) {
    // Arm BYTE has the wasm pragma. Call site uses BYTE > SHL.BYTE
    // gets the wasm intrinsic body; no other variants are emitted.
    let source = r#"
FUNCTION SHL : ANY_BIT
VAR_INPUT IN : INTO(SHL); N : INT; END_VAR
{#if IN is BYTE}
    {wasm IN 'shl' (params IN N) (result SHL)}
{#endif}
END_FUNCTION

FUNCTION test
VAR x : BYTE; END_VAR
    x := SHL(IN := BYTE#1, N := 4);
END_FUNCTION
"#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export SHL.BYTE(Byte, Int) -> Byte
    export test()
    ");
}

#[rstest]
fn wasm_pragma_per_branch_chain(mut with_db: RootDatabase) {
    // Each arm has its own wasm pragma. Multiple call sites pull each
    // arm independently.
    let source = r#"
FUNCTION SHL : ANY_BIT
VAR_INPUT IN : INTO(SHL); N : INT; END_VAR
{#if IN is BYTE}
    {wasm IN 'shl' (params IN N) (result SHL)}
{#elif IN is WORD}
    {wasm IN 'shl' (params IN N) (result SHL)}
{#elif IN is DWORD}
    {wasm IN 'shl' (params IN N) (result SHL)}
{#endif}
END_FUNCTION

FUNCTION test
VAR b : BYTE; w : WORD; d : DWORD; END_VAR
    b := SHL(IN := BYTE#1, N := 1);
    w := SHL(IN := WORD#1, N := 1);
    d := SHL(IN := DWORD#1, N := 1);
END_FUNCTION
"#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export SHL.BYTE(Byte, Int) -> Byte
    export SHL.DWORD(DWord, Int) -> DWord
    export SHL.WORD(Word, Int) -> Word
    export test()
    ");
}

#[rstest]
fn no_matching_arm_emits_empty_function(mut with_db: RootDatabase) {
    // Arms cover BYTE only; the call uses LWORD. SHL.LWORD is still
    // emitted (the implementer's responsibility to cover variants).
    let source = r#"
FUNCTION SHL : ANY_BIT
VAR_INPUT IN : INTO(SHL); N : INT; END_VAR
{#if IN is BYTE}
    {wasm IN 'shl' (params IN N) (result SHL)}
{#endif}
END_FUNCTION

FUNCTION test
VAR l : LWORD; END_VAR
    l := SHL(IN := LWORD#1, N := 1);
END_FUNCTION
"#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export SHL.LWORD(LWord, Int) -> LWord
    export test()
    ");
}

#[rstest]
fn extern_pragma_inside_if_arm(mut with_db: RootDatabase) {
    // Arm REAL routes to a host import; arm LREAL too. Each pulls a
    // separate `MirExternFunction`.
    let source = r#"
FUNCTION abs : ANY_NUM
VAR_INPUT IN : INTO(abs); END_VAR
{#if IN is REAL}
    {extern 'math' 'abs_f32' (params IN) (result abs)}
{#elif IN is LREAL}
    {extern 'math' 'abs_f64' (params IN) (result abs)}
{#endif}
END_FUNCTION

FUNCTION test
VAR r : REAL; lr : LREAL; END_VAR
    r := abs(IN := REAL#1.5);
    lr := abs(IN := LREAL#2.5);
END_FUNCTION
"#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export test()
    import math.abs_f32.REAL(Real) -> Real [from abs]
    import math.abs_f64.LREAL(LReal) -> LReal [from abs]
    ");
}

#[rstest]
fn mixed_wasm_and_extern_per_branch(mut with_db: RootDatabase) {
    // BYTE arm uses wasm; WORD arm uses extern. Per-T classification
    // routes each through the right path.
    let source = r#"
FUNCTION foo : ANY_BIT
VAR_INPUT IN : INTO(foo); END_VAR
{#if IN is BYTE}
    {wasm IN 'popcnt' (params IN) (result foo)}
{#elif IN is WORD}
    {extern 'host' 'word_popcnt' (params IN) (result foo)}
{#endif}
END_FUNCTION

FUNCTION test
VAR b : BYTE; w : WORD; END_VAR
    b := foo(IN := BYTE#1);
    w := foo(IN := WORD#1);
END_FUNCTION
"#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export foo.BYTE(Byte) -> Byte
    export test()
    import host.word_popcnt.WORD(Word) -> Word [from foo]
    ");
}

#[rstest]
fn local_body_inside_if_arm(mut with_db: RootDatabase) {
    // Arm body is regular ST code (not a pragma). Lowered as a local
    // function body for the matching T.
    let source = r#"
FUNCTION foo : ANY_INT
VAR_INPUT x : INTO(foo); END_VAR
{#if x is INT}
    foo := x;
{#elif x is DINT}
    foo := x;
{#endif}
END_FUNCTION

FUNCTION test
VAR i : INT; d : DINT; END_VAR
    i := foo(x := INT#1);
    d := foo(x := DINT#2);
END_FUNCTION
"#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export foo.DINT(DInt) -> DInt
    export foo.INT(Int) -> Int
    export test()
    ");
}

// -- FB bodies with `{#if}` arms --------------------------------------
//
// FBs with ANY_* fields use the same arm-selection. The FB's
// `__body__` lowers exactly the matching arm against the concrete type
// resolved from `Counter<T>` at the call site.

#[rstest]
fn fb_body_with_if_arm_local(mut with_db: RootDatabase) {
    // FB has ANY_INT field, body uses `{#if}` to dispatch local logic
    // per concrete T. Call site pins T > only the matching arm body
    // ends up in the FB's __body__ function.
    let source = r#"
FUNCTION_BLOCK Counter
VAR_INPUT  PV : ANY_INT; END_VAR
VAR_OUTPUT CV : INTO(PV); END_VAR
{#if PV is INT}
    CV := PV;
{#elif PV is DINT}
    CV := PV;
{#endif}
END_FUNCTION_BLOCK

FUNCTION test
VAR c : Counter<INT>; END_VAR
    c(PV := 10);
END_FUNCTION
"#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export Counter$INT$__body__(*struct(Counter$INT))
    export test()
    ");
}

#[rstest]
fn fb_body_with_if_arm_no_match(mut with_db: RootDatabase) {
    // Call site uses LINT, no arm matches. FB still emitted; the
    // body for LINT ends up empty (the implementer's responsibility).
    let source = r#"
FUNCTION_BLOCK Counter
VAR_INPUT  PV : ANY_INT; END_VAR
VAR_OUTPUT CV : INTO(PV); END_VAR
{#if PV is INT}
    CV := PV;
{#endif}
END_FUNCTION_BLOCK

FUNCTION test
VAR c : Counter<LINT>; END_VAR
    c(PV := LINT#10);
END_FUNCTION
"#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export Counter$LINT$__body__(*struct(Counter$LINT))
    export test()
    ");
}

#[rstest]
fn fb_two_call_sites_different_types(mut with_db: RootDatabase) {
    // Two distinct call sites pin different T → two distinct
    // monomorphized FB structs (`Counter$INT`, `Counter$DINT`) and
    // bodies (`Counter$INT$__body__`, `Counter$DINT$__body__`). Each
    // call site's FbCall routes to the correct per-T body via the
    // per-variable `local_fb_mangling` lookup.
    let source = r#"
FUNCTION_BLOCK Counter
VAR_INPUT  PV : ANY_INT; END_VAR
VAR_OUTPUT CV : INTO(PV); END_VAR
    CV := PV;
END_FUNCTION_BLOCK

FUNCTION test
VAR
    c_int  : Counter<INT>;
    c_dint : Counter<DINT>;
END_VAR
    c_int(PV := 10);
    c_dint(PV := DINT#100);
END_FUNCTION
"#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @r"
    export Counter$DINT$__body__(*struct(Counter$DINT))
    export Counter$INT$__body__(*struct(Counter$INT))
    export test()
    ");
}

#[rstest]
fn fb_method_with_if_arm(mut with_db: RootDatabase) {
    // Method body of an ANY_* FB also runs through arm expansion at
    // its call sites (per concrete T).
    let source = r#"
FUNCTION_BLOCK Counter
VAR_INPUT  PV : ANY_INT; END_VAR
VAR_OUTPUT CV : INTO(PV); END_VAR
END_FUNCTION_BLOCK

FUNCTION test
VAR c : Counter<INT>; END_VAR
    c(PV := 5);
END_FUNCTION
"#;
    assert_snapshot!(mir_exports(&mut with_db, &[source]), @"export test()");
}
