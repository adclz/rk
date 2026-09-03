//! The lifetime of a POU's own locals: a FUNCTION or METHOD starts every call
//! with fresh storage, an instance keeps its state between invocations.

use crate::tests::codegen::{compile_to_wasm, with_db};
use rstest::*;

/// A FUNCTION's aggregate `VAR` is fresh on every call.
///
/// Scalars always were: they are wasm locals, zeroed by the engine. An array,
/// struct or string lives at a fixed linear-memory address reused across
/// calls, and the entry reset that clears it was gated on `VAR_TEMP` alone -
/// so a plain `VAR` array carried the previous call's contents, and this
/// checksum read 123 instead of 111 from a program `check` called clean.
#[rstest]
#[case::array("VAR v : ARRAY[0..2] OF INT; END_VAR v[0] := v[0] + 1; bump := v[0];")]
#[case::structure("VAR p : Pt; END_VAR p.x := p.x + 1; bump := p.x;")]
#[case::string(
    "VAR s : STRING; END_VAR IF s = '' THEN bump := 1; ELSE bump := 0; END_IF; s := 'dirty';"
)]
fn a_functions_aggregate_local_is_fresh_on_every_call(
    mut with_db: db::RootDatabase,
    #[case] body: &str,
) {
    let source = format!(
        r#"
        TYPE Pt : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

        FUNCTION bump : INT
            {body}
        END_FUNCTION

        FUNCTION run : INT
        VAR a : INT; b : INT; c : INT; END_VAR
            a := bump(); b := bump(); c := bump();
            run := a * 100 + b * 10 + c;
        END_FUNCTION
    "#
    );
    let wasm = compile_to_wasm(&mut with_db, &source);
    let r: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(r, 111, "each call must start from zero; 123 means the local was static");
}

/// The same for a METHOD's own local: its lifetime is the call, not the
/// instance it runs on.
#[rstest]
fn a_methods_aggregate_local_is_fresh_on_every_call(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Holder
        METHOD PUBLIC Bump : INT
        VAR v : ARRAY[0..2] OF INT; END_VAR
            v[0] := v[0] + 1;
            Bump := v[0];
        END_METHOD
        END_FUNCTION_BLOCK

        FUNCTION run : INT
        VAR h : Holder; a : INT; b : INT; c : INT; END_VAR
            a := h.Bump(); b := h.Bump(); c := h.Bump();
            run := a * 100 + b * 10 + c;
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(r, 111, "a method local is per call, not per instance");
}

/// The control: an FB's OWN member is instance state and must keep
/// accumulating across invocations. The reset applies to the call's own
/// storage, never to what lives behind `this`.
#[rstest]
fn an_instance_member_still_persists_between_invocations(mut with_db: db::RootDatabase) {
    let source = r#"
        FUNCTION_BLOCK Acc
        VAR a : ARRAY[0..0] OF INT; END_VAR
            a[0] := a[0] + 1;
        END_FUNCTION_BLOCK

        FUNCTION run : INT
        VAR acc : Acc; END_VAR
            acc(); acc(); acc();
            run := acc.a[0];
        END_FUNCTION
    "#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = super::execute_wasm(&wasm, "run", ());
    assert_eq!(r, 3, "instance state accumulates within the call that owns the instance");
}
