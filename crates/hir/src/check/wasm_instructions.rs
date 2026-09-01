//! The instruction names a `{wasm}` pragma may carry.
//!
//! The emitter accepts four disjoint name families, and nothing else: an
//! unknown name used to fall through to `unreachable` (or, on the
//! single-input conversion shape, to a silent identity) — a valid module
//! carrying code the program never asked for, from a compile that exited 0.
//! This module is the check-time answer to "will the emitter do something
//! DEFINED with this name". Parity tests in `wasm_codegen` hold the two
//! sides together (`builtins::tests`), so a name added to one without the
//! other fails the suite instead of drifting.

/// Native instructions the emitter has an arm for, plus the full numeric
/// conversion families the single-input conversion shape accepts by TYPE
/// (where the name documents intent rather than selecting the op).
pub const NATIVE: &[&str] = &[
    // shifts / rotates / bitwise
    "i32.shl",
    "i32.shr_u",
    "i32.shr_s",
    "i32.rotl",
    "i32.rotr",
    "i32.and",
    "i32.or",
    "i32.xor",
    "i64.shl",
    "i64.shr_u",
    "i64.shr_s",
    "i64.rotl",
    "i64.rotr",
    // reinterpret (bit pattern)
    "i32.reinterpret_f32",
    "i64.reinterpret_f64",
    "f32.reinterpret_i32",
    "f64.reinterpret_i64",
    // int -> float
    "f32.convert_i32_s",
    "f32.convert_i32_u",
    "f32.convert_i64_s",
    "f32.convert_i64_u",
    "f64.convert_i32_s",
    "f64.convert_i32_u",
    "f64.convert_i64_s",
    "f64.convert_i64_u",
    // float -> int, trapping and saturating
    "i32.trunc_f32_s",
    "i32.trunc_f32_u",
    "i32.trunc_f64_s",
    "i32.trunc_f64_u",
    "i64.trunc_f32_s",
    "i64.trunc_f32_u",
    "i64.trunc_f64_s",
    "i64.trunc_f64_u",
    "i32.trunc_sat_f32_s",
    "i32.trunc_sat_f32_u",
    "i32.trunc_sat_f64_s",
    "i32.trunc_sat_f64_u",
    "i64.trunc_sat_f32_s",
    "i64.trunc_sat_f32_u",
    "i64.trunc_sat_f64_s",
    "i64.trunc_sat_f64_u",
    // width moves
    "i32.wrap_i64",
    "i64.extend_i32_s",
    "i64.extend_i32_u",
    "f32.demote_f64",
    "f64.promote_f32",
    // float math with a native op
    "f32.sqrt",
    "f64.sqrt",
    "f32.abs",
    "f64.abs",
    // synthesized (no single native op, emitted as a short sequence)
    "i32.abs",
    "i64.abs",
];

/// Sub-width shift/rotate pseudo-ops the emitter synthesizes.
const RK_PSEUDO: &[&str] = &[
    "rk.shl8",
    "rk.shl16",
    "rk.shl32",
    "rk.shl64",
    "rk.shr8",
    "rk.shr16",
    "rk.shr32",
    "rk.shr64",
    "rk.rotl8",
    "rk.rotl16",
    "rk.rotr8",
    "rk.rotr16",
];

/// Grafted Rust builtins, by their dotted export names. The parity test in
/// `wasm_codegen::builtins` asserts this list equals `BUILTIN_NAMES` exactly.
pub const BUILTINS: &[&str] = &[
    "f32.sin",
    "f32.cos",
    "f32.tan",
    "f32.asin",
    "f32.acos",
    "f32.atan",
    "f32.atan2",
    "f32.pow",
    "f32.exp",
    "f32.ln",
    "f32.log",
    "f64.sin",
    "f64.cos",
    "f64.tan",
    "f64.asin",
    "f64.acos",
    "f64.atan",
    "f64.atan2",
    "f64.pow",
    "f64.exp",
    "f64.ln",
    "f64.log",
    "rk.div_i32_checked",
    "rk.rem_i32_checked",
    "rk.range_check_i32",
    "rk.range_check_u32",
    "rk.range_check_i64",
    "rk.range_check_u64",
    "rk.idx_check",
    "rk.null_check",
    "rk.raise_str",
    "rk.str_assign",
    "rk.str_from_char",
    "rk.str_from_bool",
    "rk.str_from_i32",
    "rk.str_from_u32",
    "rk.str_from_i64",
    "rk.str_from_u64",
    "rk.str_from_f32",
    "rk.str_from_f64",
    "str.byte_cmp",
    "str.byte_delete",
    "str.byte_find",
    "str.byte_insert",
    "str.byte_left",
    "str.byte_len",
    "str.byte_mid",
    "str.byte_replace",
    "str.byte_right",
    "str.concat",
    "str.is_utf8",
];

/// The two sanctioned pseudo-names of the single-input conversion shape,
/// where the operation is chosen by the parameter and return TYPES and the
/// name only says which kind of move the author meant.
const PSEUDO: &[&str] = &["nop", "cast"];

/// Whether the emitter does something DEFINED with `name` as written.
pub fn known(name: &str) -> bool {
    PSEUDO.contains(&name)
        || NATIVE.contains(&name)
        || RK_PSEUDO.contains(&name)
        || BUILTINS.contains(&name)
}

/// As [`known`], for a pragma WITH a type basis (`{wasm IN 'shl' ...}`):
/// lowering prefixes the name with the basis type's lane (`sin` on a `LREAL`
/// becomes `f64.sin`), and `shl`/`shr_u`/`rotl`/`rotr` on integers become
/// the width-aware `rk.*` pseudo-ops. Accepting a name if ANY lane form is
/// known needs no type inference here; the lane itself is checked by the
/// emitter's typed lookup.
pub fn known_with_type_basis(name: &str) -> bool {
    if known(name) {
        return true;
    }
    if matches!(name, "shl" | "shr_u" | "shr_s" | "rotl" | "rotr") {
        return true;
    }
    ["i32", "i64", "f32", "f64"]
        .iter()
        .any(|p| known(&format!("{p}.{name}")))
}
