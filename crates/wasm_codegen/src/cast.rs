//! Implicit type conversion (casting) support for WASM code generation.
//!
//! This module provides functions to emit WASM conversion instructions
//! for implicit casts between IEC 61131-3 elementary types according to
//! the standard (see IEC 61131-3 section 6.6.1.6).

use hir::hir_def::expressions::spec::ElementarySpec;
use wasm_encoder::Instruction;

use crate::wasm_repr::elementary::{elementary_to_val_type, is_signed};

/// Emit WASM conversion instructions for implicit type cast.
///
/// This function determines the appropriate WASM conversion instruction(s)
/// needed to convert a value from `from_spec` type to `to_spec` type.
///
/// # Arguments
/// * `from_spec` - Source type specification
/// * `to_spec` - Target type specification
///
/// # Returns
/// A vector of WASM instructions needed for the conversion. Empty vector if no conversion needed.
///
/// # Examples
/// - SINT (i32) → INT (i32): no conversion needed (same WASM type)
/// - INT (i32) → LINT (i64): i64.extend_i32_s
/// - INT (i32) → REAL (f32): f32.convert_i32_s
/// - REAL (f32) → LREAL (f64): f64.promote_f32
pub fn emit_cast(from_spec: ElementarySpec, to_spec: ElementarySpec) -> Vec<Instruction<'static>> {
    // If types are the same, no conversion needed
    if from_spec == to_spec {
        return vec![];
    }

    let from_val_type = elementary_to_val_type(from_spec).expect("Should be valid elementary type");
    let to_val_type = elementary_to_val_type(to_spec).expect("Should be valid elementary type");

    // If both map to the same ValType, no WASM-level conversion needed
    // (e.g., SINT → INT, both are i32)
    if from_val_type == to_val_type {
        return vec![];
    }

    // Determine signedness for integer conversions
    let from_signed = is_signed(from_spec);
    let to_signed = is_signed(to_spec);

    use wasm_encoder::{Instruction::*, ValType};

    match (from_val_type, to_val_type) {
        // i32 → i64
        (ValType::I32, ValType::I64) => {
            vec![if from_signed { I64ExtendI32S } else { I64ExtendI32U }]
        }

        // i32 → f32
        (ValType::I32, ValType::F32) => {
            vec![if from_signed { F32ConvertI32S } else { F32ConvertI32U }]
        }

        // i32 → f64
        (ValType::I32, ValType::F64) => {
            vec![if from_signed { F64ConvertI32S } else { F64ConvertI32U }]
        }

        // i64 → i32 (truncate) - not typically an implicit cast, but included for completeness
        (ValType::I64, ValType::I32) => {
            vec![I32WrapI64]
        }

        // i64 → f32
        (ValType::I64, ValType::F32) => {
            vec![if from_signed { F32ConvertI64S } else { F32ConvertI64U }]
        }

        // i64 → f64
        (ValType::I64, ValType::F64) => {
            vec![if from_signed { F64ConvertI64S } else { F64ConvertI64U }]
        }

        // f32 → f64 (promote)
        (ValType::F32, ValType::F64) => {
            vec![F64PromoteF32]
        }

        // f64 → f32 (demote) - not typically an implicit cast
        (ValType::F64, ValType::F32) => {
            vec![F32DemoteF64]
        }

        // f32 → i32
        (ValType::F32, ValType::I32) => {
            vec![if to_signed { I32TruncF32S } else { I32TruncF32U }]
        }

        // f32 → i64
        (ValType::F32, ValType::I64) => {
            vec![if to_signed { I64TruncF32S } else { I64TruncF32U }]
        }

        // f64 → i32
        (ValType::F64, ValType::I32) => {
            vec![if to_signed { I32TruncF64S } else { I32TruncF64U }]
        }

        // f64 → i64
        (ValType::F64, ValType::I64) => {
            vec![if to_signed { I64TruncF64S } else { I64TruncF64U }]
        }

        // Same types - should have been caught earlier, but handle it
        (from, to) if from == to => vec![],

        // Unsupported conversion
        _ => panic!("Unsupported type conversion: {:?} → {:?}", from_val_type, to_val_type),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_cast_same_type() {
        let instructions = emit_cast(ElementarySpec::Int, ElementarySpec::Int);
        assert!(instructions.is_empty());
    }

    #[test]
    fn test_no_cast_same_valtype() {
        // SINT and INT both map to i32, so no WASM conversion needed
        let instructions = emit_cast(ElementarySpec::SInt, ElementarySpec::Int);
        assert!(instructions.is_empty());
    }

    #[test]
    fn test_int_to_lint() {
        let instructions = emit_cast(ElementarySpec::Int, ElementarySpec::LInt);
        assert_eq!(instructions.len(), 1);
        assert!(matches!(instructions[0], Instruction::I64ExtendI32S));
    }

    #[test]
    fn test_uint_to_ulint() {
        let instructions = emit_cast(ElementarySpec::UInt, ElementarySpec::ULInt);
        assert_eq!(instructions.len(), 1);
        assert!(matches!(instructions[0], Instruction::I64ExtendI32U));
    }

    #[test]
    fn test_int_to_real() {
        let instructions = emit_cast(ElementarySpec::Int, ElementarySpec::Real);
        assert_eq!(instructions.len(), 1);
        assert!(matches!(instructions[0], Instruction::F32ConvertI32S));
    }

    #[test]
    fn test_int_to_lreal() {
        let instructions = emit_cast(ElementarySpec::Int, ElementarySpec::LReal);
        assert_eq!(instructions.len(), 1);
        assert!(matches!(instructions[0], Instruction::F64ConvertI32S));
    }

    #[test]
    fn test_real_to_lreal() {
        let instructions = emit_cast(ElementarySpec::Real, ElementarySpec::LReal);
        assert_eq!(instructions.len(), 1);
        assert!(matches!(instructions[0], Instruction::F64PromoteF32));
    }
}
