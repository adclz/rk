//! Emit WASM cast instructions from MIR Cast expressions.
//! Pure mechanical mapping — no type inference needed.

use mir::types::MirElementary;
use wasm_encoder::Instruction;

/// Emit cast instructions for `MirExpr::Cast { from, to }`.
/// Returns the instructions to append.
pub(crate) fn emit_cast_instructions(from: MirElementary, to: MirElementary) -> Vec<Instruction<'static>> {
    if from == to {
        return Vec::new();
    }

    let from_vt = super::mir_elementary_to_val_type(from);
    let to_vt = super::mir_elementary_to_val_type(to);

    if from_vt == to_vt {
        // Same WASM type (e.g., SINT -> INT, both i32) — no conversion needed
        return Vec::new();
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

    instrs
}
