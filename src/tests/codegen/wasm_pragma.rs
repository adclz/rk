// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

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

/// A byte swap reverses the bytes of its type's width, with no instruction
/// of its own in wasm: 16 bits zero extended, 32, and 64 through the result
/// local, which may be the operand itself.
#[rstest]
fn a_byte_swap_reverses_the_bytes_of_its_width(mut with_db: db::RootDatabase) {
    let source = r#"
FUNCTION swap16 : WORD
VAR_INPUT IN : WORD; END_VAR
    {wasm IN 'bswap' (params IN) (result swap16)}
END_FUNCTION

FUNCTION swap16_signed : WORD
VAR_INPUT IN : INT; END_VAR
    {wasm IN 'bswap' (params IN) (result swap16_signed)}
END_FUNCTION

FUNCTION swap32 : DWORD
VAR_INPUT IN : DWORD; END_VAR
    {wasm IN 'bswap' (params IN) (result swap32)}
END_FUNCTION

FUNCTION swap64 : LWORD
VAR_INPUT IN : LWORD; END_VAR
VAR v : LWORD; END_VAR
    v := IN;
    {wasm v 'bswap' (params v) (result v)}
    swap64 := v;
END_FUNCTION

FUNCTION run16 : WORD
    run16 := swap16(WORD#16#1234);
END_FUNCTION

FUNCTION run16_signed : WORD
    run16_signed := swap16_signed(INT#-2);
END_FUNCTION

FUNCTION run32 : DWORD
    run32 := swap32(DWORD#16#12345678);
END_FUNCTION

FUNCTION run64 : LWORD
    run64 := swap64(LWORD#16#0102030405060708);
END_FUNCTION

FUNCTION high16 : WORD
    high16 := swap16(WORD#16#80FF);
END_FUNCTION

FUNCTION high16_signed : WORD
    high16_signed := swap16_signed(INT#-129);
END_FUNCTION

FUNCTION high32 : DWORD
    high32 := swap32(DWORD#16#8091A2B3);
END_FUNCTION

FUNCTION high64 : LWORD
    high64 := swap64(LWORD#16#8899AABBCCDDEEFF);
END_FUNCTION
"#;
    let wasm = compile_to_wasm(&mut with_db, source);
    let r16: i32 = execute_wasm(&wasm, "run16", ());
    assert_eq!(r16, 0x3412);
    let signed: i32 = execute_wasm(&wasm, "run16_signed", ());
    assert_eq!(signed, 0xFEFF, "the bits past 16 are not read, nor set");
    let r32: i32 = execute_wasm(&wasm, "run32", ());
    assert_eq!(r32 as u32, 0x7856_3412);
    let r64: i64 = execute_wasm(&wasm, "run64", ());
    assert_eq!(r64 as u64, 0x0807_0605_0403_0201);
    // Each byte with its high bit set, which a shift that keeps the sign
    // would spread.
    let h16: i32 = execute_wasm(&wasm, "high16", ());
    assert_eq!(h16, 0xFF80);
    let h16_signed: i32 = execute_wasm(&wasm, "high16_signed", ());
    assert_eq!(
        h16_signed, 0x7FFF,
        "INT#-129 is 16#FF7F, its bits past 16 set"
    );
    let h32: i32 = execute_wasm(&wasm, "high32", ());
    assert_eq!(h32 as u32, 0xB3A2_9180);
    let h64: i64 = execute_wasm(&wasm, "high64", ());
    assert_eq!(h64 as u64, 0xFFEE_DDCC_BBAA_9988);
}

/// The 32-bit swap in the one form Cranelift turns into the processor's
/// own byte swap: `bitops.isle` in cranelift-codegen matches this tree, in
/// this order, and no other. `IN` is the swapped value's local.
const SWAP_32: &str = "
local.get IN
i32.const 24
i32.shl
local.get IN
i32.const 0xFF00
i32.and
i32.const 8
i32.shl
i32.or
local.get IN
i32.const 8
i32.shr_u
i32.const 0xFF00
i32.and
local.get IN
i32.const 24
i32.shr_u
i32.or
i32.or";

/// The 64-bit swap in that form.
const SWAP_64: &str = "
local.get IN
i64.const 56
i64.shl
local.get IN
i64.const 0xFF00
i64.and
i64.const 40
i64.shl
i64.or
local.get IN
i64.const 0xFF0000
i64.and
i64.const 24
i64.shl
local.get IN
i64.const 0xFF000000
i64.and
i64.const 8
i64.shl
i64.or
i64.or
local.get IN
i64.const 8
i64.shr_u
i64.const 0xFF000000
i64.and
local.get IN
i64.const 24
i64.shr_u
i64.const 0xFF0000
i64.and
i64.or
local.get IN
i64.const 40
i64.shr_u
i64.const 0xFF00
i64.and
local.get IN
i64.const 56
i64.shr_u
i64.or
i64.or
i64.or";

/// The 16-bit swap: the 32-bit one, shifted down by 16.
fn swap_16() -> String {
    format!("{SWAP_32}\ni32.const 16\ni32.shr_u")
}

/// The instructions of the function `export` of `wasm`, one a line, as
/// their text: a constant of 256 or more in hexadecimal.
fn instructions(wasm: &[u8], export: &str) -> Vec<String> {
    let mut imported = 0;
    let mut index = None;
    let mut bodies = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(wasm) {
        match payload.expect("a valid module") {
            wasmparser::Payload::ImportSection(reader) => {
                imported += reader
                    .into_imports()
                    .filter(|import| {
                        matches!(
                            import,
                            Ok(wasmparser::Import {
                                ty: wasmparser::TypeRef::Func(_),
                                ..
                            })
                        )
                    })
                    .count();
            }
            wasmparser::Payload::ExportSection(reader) => {
                index = reader
                    .into_iter()
                    .flatten()
                    .find(|e| e.kind == wasmparser::ExternalKind::Func && e.name == export)
                    .map(|e| e.index as usize);
            }
            wasmparser::Payload::CodeSectionEntry(body) => bodies.push(body),
            _ => {}
        }
    }
    let body = &bodies[index.expect("the function is exported") - imported];
    let constant = |value: i64| match value {
        0..=255 => value.to_string(),
        _ => format!("{value:#X}").replace("0X", "0x"),
    };
    body.get_operators_reader()
        .expect("a body")
        .into_iter()
        .map(|op| match op.expect("an operator") {
            wasmparser::Operator::LocalGet { local_index } => format!("local.get {local_index}"),
            wasmparser::Operator::LocalSet { local_index } => format!("local.set {local_index}"),
            wasmparser::Operator::I32Const { value } => {
                format!("i32.const {}", constant(value.into()))
            }
            wasmparser::Operator::I64Const { value } => format!("i64.const {}", constant(value)),
            wasmparser::Operator::I32Shl => "i32.shl".to_string(),
            wasmparser::Operator::I32ShrU => "i32.shr_u".to_string(),
            wasmparser::Operator::I32And => "i32.and".to_string(),
            wasmparser::Operator::I32Or => "i32.or".to_string(),
            wasmparser::Operator::I64Shl => "i64.shl".to_string(),
            wasmparser::Operator::I64ShrU => "i64.shr_u".to_string(),
            wasmparser::Operator::I64And => "i64.and".to_string(),
            wasmparser::Operator::I64Or => "i64.or".to_string(),
            other => format!("{other:?}"),
        })
        .collect()
}

/// Whether `form` runs in `body` from one of its instructions on, `IN` one
/// local throughout.
fn contains(body: &[String], form: &str) -> bool {
    let form: Vec<&str> = form.trim().lines().collect();
    body.windows(form.len()).any(|window| {
        let mut local = None;
        window
            .iter()
            .zip(&form)
            .all(|(written, wanted)| match wanted {
                &"local.get IN" => {
                    let Some(index) = written.strip_prefix("local.get ") else {
                        return false;
                    };
                    *local.get_or_insert(index) == index
                }
                _ => written == wanted,
            })
    })
}

/// `TO_BIG_ENDIAN` and every other `{wasm 'bswap'}` is written in the form
/// Cranelift turns into one byte swap, instruction for instruction: its
/// rule matches that order alone.
#[rstest]
fn a_byte_swap_is_written_as_cranelift_matches_it(mut with_db: db::RootDatabase) {
    let source = r#"
FUNCTION swap16 : WORD
VAR_INPUT IN : WORD; END_VAR
    {wasm IN 'bswap' (params IN) (result swap16)}
END_FUNCTION

FUNCTION swap32 : DWORD
VAR_INPUT IN : DWORD; END_VAR
    {wasm IN 'bswap' (params IN) (result swap32)}
END_FUNCTION

FUNCTION swap64 : LWORD
VAR_INPUT IN : LWORD; END_VAR
    {wasm IN 'bswap' (params IN) (result swap64)}
END_FUNCTION
"#;
    let wasm = compile_to_wasm(&mut with_db, source);
    for (export, form) in [
        ("swap16", swap_16()),
        ("swap32", SWAP_32.to_string()),
        ("swap64", SWAP_64.to_string()),
    ] {
        let body = instructions(&wasm, export);
        assert!(contains(&body, &form), "{export}:\n{}", body.join("\n"));
    }
}

/// How many byte-swap instructions of the processor the machine code
/// Cranelift writes for `wat` holds: `bswap` on x86-64, `rev` on aarch64.
fn processor_swaps(wat: &str) -> usize {
    let wasm = wat::parse_str(wat).expect("valid text");
    let code = wasmtime::Engine::default()
        .precompile_module(&wasm)
        .expect("it compiles");
    if cfg!(target_arch = "x86_64") {
        // `0F C8+r`, after a REX prefix for a 64-bit or a high register.
        code.windows(2)
            .filter(|w| w[0] == 0x0F && (0xC8..=0xCF).contains(&w[1]))
            .count()
    } else {
        // `REV Wd, Wn` and `REV Xd, Xn`, one aligned word each.
        code.chunks_exact(4)
            .map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]) & 0xFFFF_FC00)
            .filter(|&w| w == 0x5AC0_0800 || w == 0xDAC0_0C00)
            .count()
    }
}

/// A module of a swap of each width, `IN` its parameter.
fn swaps_module(s16: &str, s32: &str, s64: &str) -> String {
    let body = |form: &str| form.replace("local.get IN", "local.get $IN");
    format!(
        "(module
           (func (export \"s16\") (param $IN i32) (result i32) {})
           (func (export \"s32\") (param $IN i32) (result i32) {})
           (func (export \"s64\") (param $IN i64) (result i64) {}))",
        body(s16),
        body(s32),
        body(s64)
    )
}

/// The wasmtime rk runs on compiles each swap of that form to the
/// processor's own instruction, and each of the form rk wrote before to
/// none: a wasmtime that drops the rule fails here, its swaps correct but
/// several instructions each.
#[test]
#[cfg_attr(not(any(target_arch = "x86_64", target_arch = "aarch64")), ignore)]
fn cranelift_compiles_the_form_to_one_byte_swap() {
    let matched = swaps_module(&swap_16(), SWAP_32, SWAP_64);
    assert_eq!(processor_swaps(&matched), 3);
    let before = swaps_module(
        "local.get IN i32.const 0xFF i32.and i32.const 8 i32.shl
         local.get IN i32.const 8 i32.shr_u i32.const 0xFF i32.and i32.or",
        "local.get IN i32.const 8 i32.shr_u i32.const 0x00FF00FF i32.and
         local.get IN i32.const 0x00FF00FF i32.and i32.const 8 i32.shl i32.or
         i32.const 16 i32.rotl",
        "local.get IN i64.const 8 i64.shr_u i64.const 0x00FF00FF00FF00FF i64.and
         local.get IN i64.const 0x00FF00FF00FF00FF i64.and i64.const 8 i64.shl i64.or
         local.set $IN
         local.get IN i64.const 16 i64.shr_u i64.const 0x0000FFFF0000FFFF i64.and
         local.get IN i64.const 0x0000FFFF0000FFFF i64.and i64.const 16 i64.shl i64.or
         i64.const 32 i64.rotl",
    );
    assert_eq!(processor_swaps(&before), 0);
}
