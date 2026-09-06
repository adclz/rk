//! Emit WASM cast instructions from MIR Cast expressions.
//! Pure mechanical mapping — no type inference needed.

use mir::expr::{MirCall, MirExpr};
use mir::stmt::MirStmt;
use mir::types::MirElementary;
use wasm_encoder::Instruction;

thread_local! {
    /// i64 scratch for the calendar floor-division sequences, which need the
    /// dividend twice; set per function when `body_needs_datetime_floor_tmp`
    /// says so.
    pub(crate) static DATETIME_FLOOR_TMP: std::cell::Cell<Option<u32>> =
        const { std::cell::Cell::new(None) };
}

/// The cast pairs that need the i64 floor scratch; the allocation scan and
/// the arms below must agree.
fn needs_floor_tmp(from: MirElementary, to: MirElementary) -> bool {
    use MirElementary::*;
    matches!(
        (from, to),
        (DateAndTime, Date | LDate) | (LDateTime, Date | LDate | DateAndTime)
    )
}

pub(crate) fn body_needs_datetime_floor_tmp(stmts: &[MirStmt]) -> bool {
    stmts.iter().any(floor_tmp_in_stmt)
}

fn floor_tmp_in_stmt(stmt: &MirStmt) -> bool {
    match stmt {
        MirStmt::Assign { value, .. } => floor_tmp_in_expr(value),
        MirStmt::If {
            condition,
            then_body,
            else_ifs,
            else_body,
        } => {
            floor_tmp_in_expr(condition)
                || body_needs_datetime_floor_tmp(then_body)
                || else_ifs
                    .iter()
                    .any(|(c, b)| floor_tmp_in_expr(c) || body_needs_datetime_floor_tmp(b))
                || else_body
                    .as_ref()
                    .is_some_and(|b| body_needs_datetime_floor_tmp(b))
        }
        MirStmt::Case {
            selector,
            arms,
            else_body,
        } => {
            floor_tmp_in_expr(selector)
                || arms.iter().any(|a| body_needs_datetime_floor_tmp(&a.body))
                || else_body
                    .as_ref()
                    .is_some_and(|b| body_needs_datetime_floor_tmp(b))
        }
        MirStmt::For {
            start,
            end,
            step,
            body,
            ..
        } => {
            floor_tmp_in_expr(start)
                || floor_tmp_in_expr(end)
                || floor_tmp_in_expr(step)
                || body_needs_datetime_floor_tmp(body)
        }
        MirStmt::While { condition, body } | MirStmt::Repeat { condition, body } => {
            floor_tmp_in_expr(condition) || body_needs_datetime_floor_tmp(body)
        }
        MirStmt::Call(call) => floor_tmp_in_call(call),
        MirStmt::FbCall { input_writes, .. } => {
            input_writes.iter().any(|(_, v, _)| floor_tmp_in_expr(v))
        }
        MirStmt::Raise { message } => floor_tmp_in_expr(message),
        MirStmt::Return
        | MirStmt::MemStore { .. }
        | MirStmt::WasmIntrinsic { .. }
        | MirStmt::Exit
        | MirStmt::Continue
        | MirStmt::DebugTrap { .. } => false,
    }
}

fn floor_tmp_in_expr(expr: &MirExpr) -> bool {
    match expr {
        MirExpr::Cast { expr, from, to } => {
            needs_floor_tmp(*from, *to) || floor_tmp_in_expr(expr)
        }
        MirExpr::Call(call) => floor_tmp_in_call(call),
        MirExpr::BinOp { lhs, rhs, .. } => floor_tmp_in_expr(lhs) || floor_tmp_in_expr(rhs),
        MirExpr::UnaryOp { expr, .. } => floor_tmp_in_expr(expr),
        _ => false,
    }
}

fn floor_tmp_in_call(call: &MirCall) -> bool {
    call.args.iter().any(|a| floor_tmp_in_expr(&a.value))
}

/// Floor division of the i64 on the stack by `n`: `q - (r < 0)`, total
/// over the whole i64 range.
fn floordiv_i64(n: i64) -> Vec<Instruction<'static>> {
    let tmp = DATETIME_FLOOR_TMP.with(|c| c.get()).expect(
        "calendar floor division needs the i64 scratch local:          body_needs_datetime_floor_tmp and needs_floor_tmp disagree",
    );
    vec![
        Instruction::LocalSet(tmp),
        Instruction::LocalGet(tmp),
        Instruction::I64Const(n),
        Instruction::I64DivS,
        Instruction::LocalGet(tmp),
        Instruction::I64Const(n),
        Instruction::I64RemS,
        Instruction::I64Const(0),
        Instruction::I64LtS,
        Instruction::I64ExtendI32U,
        Instruction::I64Sub,
    ]
}

/// Floor modulo of the i64 on the stack by `n`: `((x rem n) + n) rem n`,
/// pure stack, result in `[0, n)` for any x.
fn floormod_i64(n: i64) -> Vec<Instruction<'static>> {
    vec![
        Instruction::I64Const(n),
        Instruction::I64RemS,
        Instruction::I64Const(n),
        Instruction::I64Add,
        Instruction::I64Const(n),
        Instruction::I64RemS,
    ]
}

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

        // Float → integer is saturating, as the library's conversions are.
        (true, false) => {
            if !from.is_64bit() && !to.is_64bit() {
                // f32 → i32
                if to.is_signed() {
                    instrs.push(Instruction::I32TruncSatF32S);
                } else {
                    instrs.push(Instruction::I32TruncSatF32U);
                }
            } else if !from.is_64bit() && to.is_64bit() {
                // f32 → i64
                if to.is_signed() {
                    instrs.push(Instruction::I64TruncSatF32S);
                } else {
                    instrs.push(Instruction::I64TruncSatF32U);
                }
            } else if from.is_64bit() && !to.is_64bit() {
                // f64 → i32
                if to.is_signed() {
                    instrs.push(Instruction::I32TruncSatF64S);
                } else {
                    instrs.push(Instruction::I32TruncSatF64U);
                }
            } else {
                // f64 → i64
                if to.is_signed() {
                    instrs.push(Instruction::I64TruncSatF64S);
                } else {
                    instrs.push(Instruction::I64TruncSatF64U);
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
/// `stdlib/Convert.st`: TIME i32 ms, LTIME i64 ns, DATE i32 days, LDATE
/// i64 days, TOD i32 ms-of-day, LTOD i64 ns-of-day, DT i64 secs, LDT i64
/// ns. `Some` only when both `from` and `to` are date/time variants.
/// Calendar decompositions floor (`floordiv_i64`/`floormod_i64`), so a
/// pre-epoch timestamp yields the right DATE and an in-domain TOD; the one
/// truncation is LTIME -> TIME, a duration narrowing toward zero.
fn emit_datetime_cast(from: MirElementary, to: MirElementary) -> Option<Vec<Instruction<'static>>> {
    use MirElementary::*;
    // The unit scales come from hir, the same constants its containment
    // assertions use.
    use hir::hir_ty::infer::literals::{NS_PER_MS, NS_PER_S};
    const SECS_PER_DAY: i64 = 86_400;
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
        // DT (i64 secs) <-> LDT (i64 ns): a pure scale; hir bounds DT to LDT's
        // span.
        (DateAndTime, LDateTime) => vec![
            Instruction::I64Const(NS_PER_S),
            Instruction::I64Mul,
        ],
        (LDateTime, DateAndTime) => floordiv_i64(NS_PER_S),

        // DT (i64 secs since epoch) decompositions
        (DateAndTime, Date) => {
            let mut v = floordiv_i64(SECS_PER_DAY);
            v.push(Instruction::I32WrapI64);
            v
        }
        (DateAndTime, LDate) => floordiv_i64(SECS_PER_DAY),
        (DateAndTime, Tod) => {
            let mut v = floormod_i64(SECS_PER_DAY);
            v.push(Instruction::I64Const(1_000));
            v.push(Instruction::I64Mul);
            v.push(Instruction::I32WrapI64);
            v
        }
        (DateAndTime, LTod) => {
            let mut v = floormod_i64(SECS_PER_DAY);
            v.push(Instruction::I64Const(NS_PER_S));
            v.push(Instruction::I64Mul);
            v
        }

        // LDT (i64 ns since epoch) decompositions
        (LDateTime, Date) => {
            let mut v = floordiv_i64(NS_PER_DAY);
            v.push(Instruction::I32WrapI64);
            v
        }
        (LDateTime, LDate) => floordiv_i64(NS_PER_DAY),
        (LDateTime, Tod) => {
            // floor-mod to ns-of-day (non-negative), then plain division to
            // ms is exact-enough truncation on a non-negative value.
            let mut v = floormod_i64(NS_PER_DAY);
            v.push(Instruction::I64Const(NS_PER_MS));
            v.push(Instruction::I64DivS);
            v.push(Instruction::I32WrapI64);
            v
        }
        (LDateTime, LTod) => floormod_i64(NS_PER_DAY),

        _ => return None,
    };
    Some(instrs)
}
