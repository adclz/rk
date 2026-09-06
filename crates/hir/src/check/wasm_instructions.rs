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

use compact_str::CompactString;
use db::WorkspaceDataBase;

use crate::hir_def::expressions::spec::{ElementarySpec, Enum};
use crate::hir_def::interned::identifier::Ident;
use crate::hir_ty::infer::Infer;
use crate::hir_ty::ty::Type;

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
    "f32.nearest",
    "f64.nearest",
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

/// Grafted Rust builtins, by their dotted export names, with the wasm
/// signature each has in the bundle: what a pragma's operands must push and
/// what it gets back. A STRING operand is its (ptr, len) pair, a STRING
/// result the (out_addr, out_cap) pair appended to the parameters and no
/// result at all. The parity test in `wasm_codegen::builtins` asserts this
/// table equals `BUILTIN_NAMES` and `BUILTIN_SIGS` exactly.
pub const BUILTINS: &[(&str, &[Lane], &[Lane])] = &[
    ("f32.acos", &[Lane::F32], &[Lane::F32]),
    ("f32.asin", &[Lane::F32], &[Lane::F32]),
    ("f32.atan", &[Lane::F32], &[Lane::F32]),
    ("f32.atan2", &[Lane::F32, Lane::F32], &[Lane::F32]),
    ("f32.cos", &[Lane::F32], &[Lane::F32]),
    ("f32.exp", &[Lane::F32], &[Lane::F32]),
    ("f32.ln", &[Lane::F32], &[Lane::F32]),
    ("f32.log", &[Lane::F32], &[Lane::F32]),
    ("f32.pow", &[Lane::F32, Lane::F32], &[Lane::F32]),
    ("f32.sin", &[Lane::F32], &[Lane::F32]),
    ("f32.tan", &[Lane::F32], &[Lane::F32]),
    ("f64.acos", &[Lane::F64], &[Lane::F64]),
    ("f64.asin", &[Lane::F64], &[Lane::F64]),
    ("f64.atan", &[Lane::F64], &[Lane::F64]),
    ("f64.atan2", &[Lane::F64, Lane::F64], &[Lane::F64]),
    ("f64.cos", &[Lane::F64], &[Lane::F64]),
    ("f64.exp", &[Lane::F64], &[Lane::F64]),
    ("f64.ln", &[Lane::F64], &[Lane::F64]),
    ("f64.log", &[Lane::F64], &[Lane::F64]),
    ("f64.pow", &[Lane::F64, Lane::F64], &[Lane::F64]),
    ("f64.sin", &[Lane::F64], &[Lane::F64]),
    ("f64.tan", &[Lane::F64], &[Lane::F64]),
    ("rk.div_i32_checked", &[Lane::I32, Lane::I32], &[Lane::I32]),
    (
        "rk.idx_check",
        &[Lane::I32, Lane::I32, Lane::I32],
        &[Lane::I32],
    ),
    ("rk.null_check", &[Lane::I32], &[Lane::I32]),
    ("rk.raise_str", &[Lane::I32, Lane::I32], &[]),
    (
        "rk.range_check_i32",
        &[Lane::I32, Lane::I32, Lane::I32],
        &[Lane::I32],
    ),
    (
        "rk.range_check_i64",
        &[Lane::I64, Lane::I64, Lane::I64],
        &[Lane::I64],
    ),
    (
        "rk.range_check_u32",
        &[Lane::I32, Lane::I32, Lane::I32],
        &[Lane::I32],
    ),
    (
        "rk.range_check_u64",
        &[Lane::I64, Lane::I64, Lane::I64],
        &[Lane::I64],
    ),
    ("rk.rem_i32_checked", &[Lane::I32, Lane::I32], &[Lane::I32]),
    (
        "rk.str_assign",
        &[Lane::I32, Lane::I32, Lane::I32, Lane::I32],
        &[],
    ),
    ("rk.str_from_bool", &[Lane::I32, Lane::I32, Lane::I32], &[]),
    ("rk.str_from_char", &[Lane::I32, Lane::I32, Lane::I32], &[]),
    ("rk.str_from_f32", &[Lane::F32, Lane::I32, Lane::I32], &[]),
    ("rk.str_from_f64", &[Lane::F64, Lane::I32, Lane::I32], &[]),
    ("rk.str_from_i32", &[Lane::I32, Lane::I32, Lane::I32], &[]),
    ("rk.str_from_i64", &[Lane::I64, Lane::I32, Lane::I32], &[]),
    ("rk.str_from_u32", &[Lane::I32, Lane::I32, Lane::I32], &[]),
    ("rk.str_from_u64", &[Lane::I64, Lane::I32, Lane::I32], &[]),
    (
        "str.byte_cmp",
        &[Lane::I32, Lane::I32, Lane::I32, Lane::I32],
        &[Lane::I32],
    ),
    (
        "str.byte_delete",
        &[
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
        ],
        &[],
    ),
    (
        "str.byte_find",
        &[Lane::I32, Lane::I32, Lane::I32, Lane::I32],
        &[Lane::I32],
    ),
    (
        "str.byte_insert",
        &[
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
        ],
        &[],
    ),
    (
        "str.byte_left",
        &[Lane::I32, Lane::I32, Lane::I32, Lane::I32, Lane::I32],
        &[],
    ),
    ("str.byte_len", &[Lane::I32, Lane::I32], &[Lane::I32]),
    (
        "str.byte_mid",
        &[
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
        ],
        &[],
    ),
    (
        "str.byte_replace",
        &[
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
        ],
        &[],
    ),
    (
        "str.byte_right",
        &[Lane::I32, Lane::I32, Lane::I32, Lane::I32, Lane::I32],
        &[],
    ),
    (
        "str.char_at",
        &[Lane::I32, Lane::I32, Lane::I32],
        &[Lane::I32],
    ),
    ("str.char_count", &[Lane::I32, Lane::I32], &[Lane::I32]),
    (
        "str.char_delete",
        &[
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
        ],
        &[],
    ),
    (
        "str.char_find",
        &[Lane::I32, Lane::I32, Lane::I32, Lane::I32],
        &[Lane::I32],
    ),
    (
        "str.char_insert",
        &[
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
        ],
        &[],
    ),
    (
        "str.char_left",
        &[Lane::I32, Lane::I32, Lane::I32, Lane::I32, Lane::I32],
        &[],
    ),
    (
        "str.char_mid",
        &[
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
        ],
        &[],
    ),
    (
        "str.char_replace",
        &[
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
        ],
        &[],
    ),
    (
        "str.char_right",
        &[Lane::I32, Lane::I32, Lane::I32, Lane::I32, Lane::I32],
        &[],
    ),
    (
        "str.concat",
        &[
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
            Lane::I32,
        ],
        &[],
    ),
    ("str.is_utf8", &[Lane::I32, Lane::I32], &[Lane::I32]),
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
        || BUILTINS.iter().any(|(n, _, _)| *n == name)
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

/// A wasm value type: all the module validator ever sees of an operand.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lane {
    I32,
    I64,
    F32,
    F64,
}

impl Lane {
    pub fn name(self) -> &'static str {
        match self {
            Lane::I32 => "i32",
            Lane::I64 => "i64",
            Lane::F32 => "f32",
            Lane::F64 => "f64",
        }
    }

    fn parse(s: &str) -> Option<Lane> {
        Some(match s {
            "i32" => Lane::I32,
            "i64" => Lane::I64,
            "f32" => Lane::F32,
            "f64" => Lane::F64,
            _ => return None,
        })
    }
}

/// The lane an IEC scalar lives in. `None` for STRING, which is not a scalar.
/// `mir::types::MirElementary` is derived from the same facts; the parity test
/// in `mir::lower::lower_wasm` holds the two together.
pub fn lane_of(spec: ElementarySpec) -> Option<Lane> {
    use ElementarySpec::*;
    Some(match spec {
        Bool | REDGEBool | FEDGEBool | Byte | Word | DWord | SInt | USInt | UInt | Int | DInt
        | UDInt | Char | Date | Time | Tod => Lane::I32,
        LWord | LInt | ULInt | LDate | DateAndTime | LDateTime | LTime | LTod => Lane::I64,
        Real => Lane::F32,
        LReal => Lane::F64,
        String => return None,
    })
}

/// The IEC width of a scalar, the one shifts and rotates respect (a BYTE is 8
/// bits in a 32-bit lane). A CHAR is a code point: the whole lane.
pub fn rk_bits_of(spec: ElementarySpec) -> u32 {
    use ElementarySpec::*;
    match spec {
        Bool | REDGEBool | FEDGEBool => 1,
        Byte | SInt | USInt => 8,
        Word | Int | UInt => 16,
        DWord | DInt | UDInt | Char | Real | Date | Time | Tod => 32,
        LWord | LInt | ULInt | LReal | LDate | DateAndTime | LDateTime | LTime | LTod => 64,
        String => 0,
    }
}

/// The instruction a type-basis pragma (`{wasm IN 'op' ...}`) names once the
/// basis variable's lane is known: the lane prefixes the op (`sqrt` on a
/// REAL is `f32.sqrt`), and the integer shifts and rotates become the
/// width-aware `rk.*` pseudo-ops, since a raw lane op leaks shifted-out
/// bits past a sub-width type and rotates at bit 31 instead of the type's
/// MSB. Full-width rotates keep the native op. The check and the MIR lowering
/// both resolve here, so what is checked is what is emitted.
pub fn resolve_type_basis(op: &str, lane: Lane, rk_bits: u32) -> CompactString {
    if matches!(op, "shl" | "shr_u" | "rotl" | "rotr") && !matches!(lane, Lane::F32 | Lane::F64) {
        return match (op, rk_bits) {
            ("shl", b) => format!("rk.shl{b}"),
            ("shr_u", b) => format!("rk.shr{b}"),
            ("rotl", b) if b <= 16 => format!("rk.rotl{b}"),
            ("rotr", b) if b <= 16 => format!("rk.rotr{b}"),
            ("rotl", 64) => "i64.rotl".to_string(),
            ("rotl", _) => "i32.rotl".to_string(),
            ("rotr", 64) => "i64.rotr".to_string(),
            ("rotr", _) => "i32.rotr".to_string(),
            _ => unreachable!("the match above lists every op"),
        }
        .into();
    }
    format!("{}.{op}", lane.name()).into()
}

/// What an instruction consumes and produces.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Signature {
    pub params: Vec<Lane>,
    pub result: Option<Lane>,
}

impl Signature {
    fn new(params: &[Lane], result: Option<Lane>) -> Self {
        Signature {
            params: params.to_vec(),
            result,
        }
    }

    pub fn text(&self) -> String {
        let params = self
            .params
            .iter()
            .map(|l| l.name())
            .collect::<Vec<_>>()
            .join(", ");
        match self.result {
            Some(r) => format!("({params}) -> {}", r.name()),
            None => format!("({params}) -> ()"),
        }
    }
}

/// The signature of a known instruction, `None` for a name the emitter does
/// not have (E0248's business) and for the `nop`/`cast` placeholders, whose
/// shape the types decide.
pub fn signature(name: &str) -> Option<Signature> {
    if let Some((_, params, results)) = BUILTINS.iter().find(|(n, _, _)| *n == name) {
        return Some(Signature::new(params, results.first().copied()));
    }
    if let Some(rest) = name.strip_prefix("rk.") {
        // rk.shl8 .. rk.shl64, rk.shr*, rk.rotl8/16, rk.rotr8/16
        let bits: u32 = rest
            .trim_start_matches(|c: char| c.is_ascii_alphabetic())
            .parse()
            .ok()?;
        let lane = if bits == 64 { Lane::I64 } else { Lane::I32 };
        return RK_PSEUDO
            .contains(&name)
            .then(|| Signature::new(&[lane, Lane::I32], Some(lane)));
    }
    let (prefix, op) = name.split_once('.')?;
    let lane = Lane::parse(prefix)?;
    let is_native = NATIVE.contains(&name);
    if !is_native {
        return None;
    }
    // Conversions name their source lane in the op: `trunc_sat_f32_s`,
    // `convert_i64_u`, `wrap_i64`, `extend_i32_s`, `reinterpret_f64`.
    let source = ["i32", "i64", "f32", "f64"]
        .iter()
        .find(|l| op.contains(*l))
        .and_then(|l| Lane::parse(l));
    if let Some(from) = source {
        return Some(Signature::new(&[from], Some(lane)));
    }
    Some(match op {
        // The emitter widens the count of a 64-bit shift or rotate itself
        // (`i64.extend_i32_u` before the op), so these take an i32 count: an
        // INT `N` on a LWORD, the way the library writes them.
        "shl" | "shr_s" | "shr_u" | "rotl" | "rotr" if lane == Lane::I64 => {
            Signature::new(&[Lane::I64, Lane::I32], Some(Lane::I64))
        }
        "eqz" => Signature::new(&[lane], Some(Lane::I32)),
        "abs" | "neg" | "sqrt" | "ceil" | "floor" | "trunc" | "nearest" | "clz" | "ctz"
        | "popcnt" | "extend8_s" | "extend16_s" | "extend32_s" => {
            Signature::new(&[lane], Some(lane))
        }
        "eq" | "ne" | "lt" | "lt_s" | "lt_u" | "gt" | "gt_s" | "gt_u" | "le" | "le_s" | "le_u"
        | "ge" | "ge_s" | "ge_u" => Signature::new(&[lane, lane], Some(Lane::I32)),
        _ => Signature::new(&[lane, lane], Some(lane)),
    })
}

/// Why a pragma is refused by the signature check.
pub enum Refusal {
    /// The operands do not fit the instruction, in the words of the message:
    /// what it takes, and what the pragma gives it.
    Mismatch {
        instruction: CompactString,
        expected: String,
        actual: String,
    },
    /// A type basis resolved to a form the emitter does not have (`shl` on a
    /// BOOL is `rk.shl1`): an unknown instruction, once named.
    Unknown(CompactString),
}

/// The lanes an operand of `ty` pushes: one for a scalar, the (ptr, len)
/// pair for a STRING; `None` for a type with no wasm operand shape.
fn operand_lanes<'db>(db: &'db dyn WorkspaceDataBase, ty: Type<'db>) -> Option<Vec<Lane>> {
    match ty.normalize(db) {
        Type::Elementary(ElementarySpec::String) => Some(vec![Lane::I32, Lane::I32]),
        Type::Elementary(spec) => lane_of(spec).map(|l| vec![l]),
        Type::Enum(e) => Some(vec![enum_lane(db, e)]),
        _ => None,
    }
}

fn enum_lane<'db>(db: &'db dyn WorkspaceDataBase, e: Enum<'db>) -> Lane {
    e.typ(db)
        .and_then(|spec| match spec.infer(db).normalize(db) {
            Type::Elementary(spec) => lane_of(spec),
            _ => None,
        })
        .unwrap_or(Lane::I32)
}

fn lanes_text(lanes: &[Lane]) -> String {
    lanes
        .iter()
        .map(|l| l.name())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Check a pragma's operands against its instruction: `basis` is the type
/// of the `{wasm IN 'op'}` basis variable when there is one, `params` the
/// typed operands in order, `result` the typed destination. `None` when
/// they fit, or when the written name is unknown, which E0248 reports on
/// its own.
pub fn check_signature<'db>(
    db: &'db dyn WorkspaceDataBase,
    instruction: &str,
    basis: Option<(Ident, Type<'db>)>,
    params: &[(Ident, Type<'db>)],
    result: Option<(Ident, Type<'db>)>,
) -> Option<Refusal> {
    let describe = |ident: &Ident, ty: &Type<'db>, lanes: &[Lane]| {
        format!(
            "{}: {} ({})",
            ident.text(db),
            ty.type_name(db),
            lanes_text(lanes)
        )
    };
    let mismatch = |instruction: &str, expected: String, actual: String| {
        Some(Refusal::Mismatch {
            instruction: instruction.into(),
            expected,
            actual,
        })
    };

    // The concrete name: a basis prefixes the op with its lane.
    let name: CompactString = match &basis {
        Some((ident, ty)) => {
            let (lane, bits) = match ty.normalize(db) {
                Type::Elementary(spec) => match lane_of(spec) {
                    Some(lane) => (lane, rk_bits_of(spec)),
                    None => {
                        return mismatch(
                            instruction,
                            "a scalar type basis".to_string(),
                            describe(ident, ty, &[Lane::I32, Lane::I32]),
                        );
                    }
                },
                Type::Enum(e) => (enum_lane(db, e), 32),
                _ => {
                    return mismatch(
                        instruction,
                        "a scalar type basis".to_string(),
                        format!("{}: {}", ident.text(db), ty.type_name(db)),
                    );
                }
            };
            resolve_type_basis(instruction, lane, bits)
        }
        None => instruction.into(),
    };

    // What the pragma gives.
    let mut actual_params: Vec<Lane> = Vec::new();
    let mut actual_text: Vec<String> = Vec::new();
    for (ident, ty) in params {
        match operand_lanes(db, *ty) {
            Some(lanes) => {
                actual_text.push(describe(ident, ty, &lanes));
                actual_params.extend(lanes);
            }
            None => {
                return mismatch(
                    &name,
                    "scalar or STRING operands".to_string(),
                    format!(
                        "{}: {}, which is not a wasm operand",
                        ident.text(db),
                        ty.type_name(db)
                    ),
                );
            }
        }
    }
    let (actual_result, result_text): (Option<Lane>, String) = match &result {
        None => (None, "nothing".to_string()),
        Some((ident, ty)) => match operand_lanes(db, *ty) {
            Some(lanes) if lanes.len() == 1 => (Some(lanes[0]), describe(ident, ty, &lanes)),
            // A STRING result is produced into (out_addr, out_cap), appended
            // to the parameters; the instruction returns nothing.
            Some(lanes) => {
                actual_params.extend(lanes.iter().copied());
                (None, describe(ident, ty, &lanes))
            }
            None => {
                return mismatch(
                    &name,
                    "a scalar or STRING result".to_string(),
                    format!(
                        "{}: {}, which is not a wasm operand",
                        ident.text(db),
                        ty.type_name(db)
                    ),
                );
            }
        },
    };
    let actual = format!("({}) -> {result_text}", actual_text.join(", "));

    // What the instruction takes.
    if PSEUDO.contains(&name.as_str()) {
        // One scalar in, one scalar out, the types picking the conversion;
        // or nothing at all, a body that emits nothing.
        let conversion = params.len() == 1
            && actual_params.len() == 1
            && result.is_some()
            && actual_result.is_some();
        let empty = params.is_empty() && result.is_none();
        return if conversion || empty {
            None
        } else {
            mismatch(&name, "(scalar) -> scalar".to_string(), actual)
        };
    }
    let Some(sig) = signature(&name) else {
        // A basis resolved to a form the emitter has no instruction for
        // (`shl` on a BOOL); without a basis the written name is E0248's
        // already.
        return basis.is_some().then(|| Refusal::Unknown(name.clone()));
    };
    if sig.params == actual_params && sig.result == actual_result {
        return None;
    }
    mismatch(&name, sig.text(), actual)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A name the check knows without a signature would be accepted and
    /// never checked; every native and pseudo-op has one.
    #[test]
    fn every_native_and_pseudo_op_has_a_signature() {
        for name in NATIVE.iter().chain(RK_PSEUDO) {
            assert!(signature(name).is_some(), "`{name}` has no signature");
        }
    }

    #[test]
    fn signatures_read_as_expected() {
        let sig = |n: &str| signature(n).unwrap().text();
        assert_eq!(sig("i32.trunc_sat_f32_s"), "(f32) -> i32");
        assert_eq!(sig("f64.promote_f32"), "(f32) -> f64");
        assert_eq!(sig("i64.extend_i32_u"), "(i32) -> i64");
        assert_eq!(sig("i32.wrap_i64"), "(i64) -> i32");
        assert_eq!(sig("f32.nearest"), "(f32) -> f32");
        assert_eq!(sig("i32.reinterpret_f32"), "(f32) -> i32");
        assert_eq!(sig("i64.shl"), "(i64, i32) -> i64");
        assert_eq!(sig("i32.shl"), "(i32, i32) -> i32");
        assert_eq!(sig("rk.shl16"), "(i32, i32) -> i32");
        assert_eq!(sig("rk.shl64"), "(i64, i32) -> i64");
        assert_eq!(sig("str.concat"), "(i32, i32, i32, i32, i32, i32) -> ()");
        assert_eq!(sig("rk.str_from_f32"), "(f32, i32, i32) -> ()");
    }

    #[test]
    fn a_type_basis_names_the_lane_and_the_width() {
        assert_eq!(resolve_type_basis("sqrt", Lane::F64, 64), "f64.sqrt");
        assert_eq!(resolve_type_basis("shl", Lane::I32, 8), "rk.shl8");
        assert_eq!(resolve_type_basis("rotl", Lane::I32, 32), "i32.rotl");
        assert_eq!(resolve_type_basis("rotr", Lane::I64, 64), "i64.rotr");
        assert_eq!(resolve_type_basis("rotl", Lane::I32, 16), "rk.rotl16");
    }
}
