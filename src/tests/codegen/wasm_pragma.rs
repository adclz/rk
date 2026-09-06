//! A `{wasm}` pragma emits the instruction it names. Every single-input
//! pragma without a type basis used to lower as a CAST between its parameter
//! and result types, the instruction string ignored: with REAL in and REAL
//! out that is a plain load, so `f32.nearest` and a user's `f32.sqrt`
//! returned their input unchanged.

use crate::tests::codegen::{compile_to_wasm, execute_wasm, with_db};
use rstest::*;

#[rstest]
#[case::nearest("f32.nearest", 3.7, 4.0)]
#[case::nearest_ties_to_even("f32.nearest", 2.5, 2.0)]
#[case::sqrt("f32.sqrt", 16.0, 4.0)]
#[case::abs("f32.abs", -1.5, 1.5)]
fn a_same_type_pragma_emits_its_instruction(
    mut with_db: db::RootDatabase,
    #[case] instruction: &str,
    #[case] input: f32,
    #[case] expected: f32,
) {
    let source = format!(
        r#"
FUNCTION op : REAL
VAR_INPUT IN : REAL; END_VAR
    {{wasm '{instruction}' (params IN) (result op)}}
END_FUNCTION

FUNCTION run : REAL
    run := op({input:?});
END_FUNCTION
"#
    );
    let wasm = compile_to_wasm(&mut with_db, &source);
    let r: f32 = execute_wasm(&wasm, "run", ());
    assert_eq!(r, expected, "{instruction} on {input} must not be identity");
}

/// A conversion pragma still takes the cast road, which normalizes a
/// sub-width target: the instruction named and the narrowing both happen.
#[rstest]
fn a_conversion_pragma_still_narrows(mut with_db: db::RootDatabase) {
    let source = r#"
FUNCTION to_int : INT
VAR_INPUT IN : REAL; END_VAR
    {wasm 'i32.trunc_sat_f32_s' (params IN) (result to_int)}
END_FUNCTION

FUNCTION run : INT
    run := to_int(70000.7);
END_FUNCTION
"#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(
        r,
        70000 - 65536,
        "truncated toward zero, then wrapped to 16 bits"
    );
}

/// Two pragmas in one body run in order, each on the operands it names:
/// the first writes a local, the second reads it. The result is spelled in
/// another case than the FUNCTION and still lands in its return slot. A
/// FUNCTION used to be lowered from its first pragma alone, on its own
/// parameter list, so the second was dropped and the float landed in the
/// i32 return: a module that checked clean and failed to load.
#[rstest]
#[case::rounds_up("3.7", 4)]
#[case::ties_to_even("2.5", 2)]
#[case::saturates("1.0E10", i32::MAX)]
fn pragmas_run_in_order_on_the_operands_they_name(
    mut with_db: db::RootDatabase,
    #[case] input: &str,
    #[case] expected: i32,
) {
    let source = format!(
        r#"
FUNCTION round_sat : DINT
VAR_INPUT IN : REAL; END_VAR
VAR r : REAL; END_VAR
    {{wasm 'f32.nearest' (params IN) (result r)}}
    {{wasm 'i32.trunc_sat_f32_s' (params r) (result ROUND_SAT)}}
END_FUNCTION

FUNCTION run : DINT
    run := round_sat({input});
END_FUNCTION
"#
    );
    let wasm = compile_to_wasm(&mut with_db, &source);
    let r: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(r, expected, "round_sat({input})");
}

/// A pragma is a statement, so it sits inside control flow like any other:
/// the ELSE branch of a clamp converts, and the branches above never reach
/// it.
#[rstest]
#[case::in_range("3.7", 4)]
#[case::clamped_high("40000.0", 32767)]
#[case::clamped_low("-40000.0", -32768)]
fn a_pragma_inside_a_branch_runs_only_there(
    mut with_db: db::RootDatabase,
    #[case] input: &str,
    #[case] expected: i32,
) {
    let source = format!(
        r#"
FUNCTION to_int : INT
VAR_INPUT IN : REAL; END_VAR
VAR r : REAL; END_VAR
    {{wasm 'f32.nearest' (params IN) (result r)}}
    IF r >= 32767.0 THEN
        to_int := INT#32767;
    ELSIF r <= -32768.0 THEN
        to_int := INT#-32768;
    ELSE
        {{wasm 'i32.trunc_sat_f32_s' (params r) (result to_int)}}
    END_IF;
END_FUNCTION

FUNCTION run : INT
    run := to_int({input});
END_FUNCTION
"#
    );
    let wasm = compile_to_wasm(&mut with_db, &source);
    let r: i32 = execute_wasm(&wasm, "run", ());
    assert_eq!(r, expected, "to_int({input})");
}
