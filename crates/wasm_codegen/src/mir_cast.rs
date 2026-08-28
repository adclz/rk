//! Emit WASM cast instructions from MIR Cast expressions.
//! Pure mechanical mapping — no type inference needed.

use mir::types::MirElementary;
use wasm_encoder::Instruction;

/// Emit cast instructions for `MirExpr::Cast { from, to }`.
/// Returns the instructions to append.
pub(crate) fn emit_cast_instructions(
    from: MirElementary,
    to: MirElementary,
) -> Vec<Instruction<'static>> {
    if from == to {
        return Vec::new();
    }

    if let Some(instrs) = emit_datetime_cast(from, to) {
        return instrs;
    }

    let from_vt = super::mir_elementary_to_val_type(from);
    let to_vt = super::mir_elementary_to_val_type(to);

    if from_vt == to_vt {
        // Same WASM lane (e.g., SINT -> INT, both i32) — no lane conversion,
        // but the target may still be sub-width (normalization below).
        let mut instrs = Vec::new();
        append_subwidth_normalization(to, &mut instrs);
        return instrs;
    }

    let mut instrs = Vec::new();

    match (from.is_float(), to.is_float()) {
        // Integer → Integer
        (false, false) => {
            if from.is_64bit() && !to.is_64bit() {
                // i64 → i32
                instrs.push(Instruction::I32WrapI64);
            } else if !from.is_64bit() && to.is_64bit() {
                // i32 → i64
                if from.is_signed() {
                    instrs.push(Instruction::I64ExtendI32S);
                } else {
                    instrs.push(Instruction::I64ExtendI32U);
                }
            }
        }

        // Integer → Float
        (false, true) => {
            if !from.is_64bit() && !to.is_64bit() {
                // i32 → f32
                if from.is_signed() {
                    instrs.push(Instruction::F32ConvertI32S);
                } else {
                    instrs.push(Instruction::F32ConvertI32U);
                }
            } else if !from.is_64bit() && to.is_64bit() {
                // i32 → f64
                if from.is_signed() {
                    instrs.push(Instruction::F64ConvertI32S);
                } else {
                    instrs.push(Instruction::F64ConvertI32U);
                }
            } else if from.is_64bit() && !to.is_64bit() {
                // i64 → f32
                if from.is_signed() {
                    instrs.push(Instruction::F32ConvertI64S);
                } else {
                    instrs.push(Instruction::F32ConvertI64U);
                }
            } else {
                // i64 → f64
                if from.is_signed() {
                    instrs.push(Instruction::F64ConvertI64S);
                } else {
                    instrs.push(Instruction::F64ConvertI64U);
                }
            }
        }

        // Float → Integer
        (true, false) => {
            if !from.is_64bit() && !to.is_64bit() {
                // f32 → i32
                if to.is_signed() {
                    instrs.push(Instruction::I32TruncF32S);
                } else {
                    instrs.push(Instruction::I32TruncF32U);
                }
            } else if !from.is_64bit() && to.is_64bit() {
                // f32 → i64
                if to.is_signed() {
                    instrs.push(Instruction::I64TruncF32S);
                } else {
                    instrs.push(Instruction::I64TruncF32U);
                }
            } else if from.is_64bit() && !to.is_64bit() {
                // f64 → i32
                if to.is_signed() {
                    instrs.push(Instruction::I32TruncF64S);
                } else {
                    instrs.push(Instruction::I32TruncF64U);
                }
            } else {
                // f64 → i64
                if to.is_signed() {
                    instrs.push(Instruction::I64TruncF64S);
                } else {
                    instrs.push(Instruction::I64TruncF64U);
                }
            }
        }

        // Float → Float
        (true, true) => {
            if from.is_64bit() && !to.is_64bit() {
                instrs.push(Instruction::F32DemoteF64);
            } else if !from.is_64bit() && to.is_64bit() {
                instrs.push(Instruction::F64PromoteF32);
            }
        }
    }

    append_subwidth_normalization(to, &mut instrs);

    instrs
}

/// Normalize an i32-lane value into a sub-width (8/16-bit) target's
/// domain: unsigned zero-extended, signed sign-extended, so
/// `INT_TO_SINT(200)` is -56. Idempotent in-domain; BOOL and 32/64-bit
/// targets untouched. Also wraps overflowing arithmetic
/// (`USINT 255 + 1` = 0).
pub(crate) fn append_subwidth_normalization(
    to: MirElementary,
    instrs: &mut Vec<Instruction<'static>>,
) {
    match to.rk_bits() {
        8 => {
            if to.is_signed() {
                instrs.push(Instruction::I32Extend8S);
            } else {
                instrs.push(Instruction::I32Const(0xFF));
                instrs.push(Instruction::I32And);
            }
        }
        16 => {
            if to.is_signed() {
                instrs.push(Instruction::I32Extend16S);
            } else {
                instrs.push(Instruction::I32Const(0xFFFF));
                instrs.push(Instruction::I32And);
            }
        }
        _ => {}
    }
}

/// Date / time conversions, matching the integer encodings documented in
/// `stdlib/Convert.st`:
///
/// ```text
/// TIME = i32 ms,           LTIME = i64 ns
/// DATE = i32 days-1970,    LDATE = i64 days-1970
/// TOD  = i32 ms-of-day,    LTOD  = i64 ns-of-day
/// DT   = i32 secs-1970,    LDT   = i64 ns-1970
/// ```
///
/// Returns `Some` only when both `from` and `to` are date/time variants.
///
/// Division uses WASM signed truncation, which is exact post-epoch and wrong
/// pre-epoch in two DIFFERENT ways that must not be conflated:
///
/// * The DATE half is off by one day for negative timestamps with a nonzero
///   remainder — a bounded, statable inaccuracy, accepted because IEC
///   controllers typically operate on post-epoch dates.
/// * The TOD half `rem`s to a NEGATIVE ms-of-day — a value outside TOD's
///   declared domain (`TOD_MIN_MS..=TOD_MAX_MS` in hir's literals.rs), which
///   then participates in comparisons as though it were in-domain (it sorts
///   below `TOD#00:00:00` — coherently out of domain, what a reader would
///   predict of a negative value; before date/time comparisons were made
///   signed it sorted ABOVE `TOD#23:59:59` and silently won any max — the
///   signed fix downgraded this escape from catastrophic to predictable,
///   not to correct). The original acceptance does not cover
///   this; it is a domain escape, not an off-by-one, and it is pinned by
///   `dt_pre_epoch_tod_escapes_its_domain` in codegen's time_literals tests.
///
/// The standing ruling options, so a revision is deliberate — and they are
/// not equally priced: (1) floor division, both effects gone (the planned
/// i64-seconds pass touches exactly these arms; note the LDT arms need a
/// scratch local — the add-a-bias trick does not cover the outer decades of
/// the i64 ns range); (2) keep truncated dates but clamp the TOD into its
/// domain — which trades a detectable wrong answer for an undetectable one:
/// a clamped `TOD#00:00:00` from a pre-epoch DT is indistinguishable from a
/// legitimate midnight, the opposite of this compiler's refuse-or-fault
/// direction (E0804 is the closest analogue: a value crossing a boundary its
/// check cannot police is refused, not approximated); (3) keep both — which
/// makes negative TODs part of the type's REAL
/// behavior, owed coherent handling by every consumer forever: the widening
/// and comparison arms are pinned for it today
/// (`escaped_tod_widens_sign_extended`), the debug plane happens to be safe
/// because it shows the raw integer, and every future formatter
/// (TOD_TO_STRING, pretty watch rendering) inherits the obligation on
/// arrival. Options 1 and 2 retire the obligation and the two escape pins
/// with it. Whoever picks, update this comment and the pinning tests in the
/// same change.
fn emit_datetime_cast(from: MirElementary, to: MirElementary) -> Option<Vec<Instruction<'static>>> {
    use MirElementary::*;
    // The unit scales come from hir, the same constants its containment
    // assertions use.
    use hir::hir_ty::infer::literals::{NS_PER_MS, NS_PER_S};
    const SECS_PER_DAY: i32 = 86_400;
    const NS_PER_DAY: i64 = 86_400 * NS_PER_S;

    let instrs: Vec<Instruction<'static>> = match (from, to) {
        // Same-pair precision conversions
        (Time, LTime) | (Tod, LTod) => vec![
            Instruction::I64ExtendI32S,
            Instruction::I64Const(NS_PER_MS),
            Instruction::I64Mul,
        ],
        (LTime, Time) | (LTod, Tod) => vec![
            Instruction::I64Const(NS_PER_MS),
            Instruction::I64DivS,
            Instruction::I32WrapI64,
        ],
        (Date, LDate) => vec![Instruction::I64ExtendI32S],
        (LDate, Date) => vec![Instruction::I32WrapI64],
        (DateAndTime, LDateTime) => vec![
            Instruction::I64ExtendI32S,
            Instruction::I64Const(NS_PER_S),
            Instruction::I64Mul,
        ],
        (LDateTime, DateAndTime) => vec![
            Instruction::I64Const(NS_PER_S),
            Instruction::I64DivS,
            Instruction::I32WrapI64,
        ],

        // DT (i32 secs since epoch) decompositions
        (DateAndTime, Date) => vec![Instruction::I32Const(SECS_PER_DAY), Instruction::I32DivS],
        (DateAndTime, LDate) => vec![
            Instruction::I32Const(SECS_PER_DAY),
            Instruction::I32DivS,
            Instruction::I64ExtendI32S,
        ],
        (DateAndTime, Tod) => vec![
            Instruction::I32Const(SECS_PER_DAY),
            Instruction::I32RemS,
            Instruction::I32Const(1_000),
            Instruction::I32Mul,
        ],
        (DateAndTime, LTod) => vec![
            Instruction::I32Const(SECS_PER_DAY),
            Instruction::I32RemS,
            Instruction::I64ExtendI32S,
            Instruction::I64Const(NS_PER_S),
            Instruction::I64Mul,
        ],

        // LDT (i64 ns since epoch) decompositions
        (LDateTime, Date) => vec![
            Instruction::I64Const(NS_PER_DAY),
            Instruction::I64DivS,
            Instruction::I32WrapI64,
        ],
        (LDateTime, LDate) => vec![Instruction::I64Const(NS_PER_DAY), Instruction::I64DivS],
        (LDateTime, Tod) => vec![
            Instruction::I64Const(NS_PER_DAY),
            Instruction::I64RemS,
            Instruction::I64Const(NS_PER_MS),
            Instruction::I64DivS,
            Instruction::I32WrapI64,
        ],
        (LDateTime, LTod) => vec![Instruction::I64Const(NS_PER_DAY), Instruction::I64RemS],

        _ => return None,
    };
    Some(instrs)
}
