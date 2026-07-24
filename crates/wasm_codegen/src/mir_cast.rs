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

/// Normalize an i32-lane value into a sub-width (8/16-bit) target's domain.
///
/// Sub-width types live in i32 locals wider than their IEC domain; the stored
/// representation invariant is: unsigned types zero-extended, signed types
/// sign-extended. Every cast INTO a sub-width type truncates to the type
/// width and re-extends — otherwise the value escapes the target's domain
/// entirely (`INT_TO_SINT(200)` staying 200 instead of the two's-complement
/// -56, `INT_TO_UINT(-1)` reading back as 4294967295 instead of 65535).
/// Idempotent for values already in-domain. BOOL and 32/64-bit targets are
/// untouched.
fn append_subwidth_normalization(to: MirElementary, instrs: &mut Vec<Instruction<'static>>) {
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
/// Division uses WASM signed truncation: results are off by one for negative
/// timestamps (pre-1970) when the remainder is non-zero — accepted because
/// IEC controllers typically operate on post-epoch dates.
fn emit_datetime_cast(from: MirElementary, to: MirElementary) -> Option<Vec<Instruction<'static>>> {
    use MirElementary::*;
    const NS_PER_MS: i64 = 1_000_000;
    const NS_PER_S: i64 = 1_000_000_000;
    const SECS_PER_DAY: i32 = 86_400;
    const NS_PER_DAY: i64 = 86_400_000_000_000;

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
