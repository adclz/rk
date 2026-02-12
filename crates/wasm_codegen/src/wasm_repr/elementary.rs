use hir::hir_def::expressions::spec::ElementarySpec;
use wasm_encoder::ValType;

use crate::wasm_repr::WasmReprError;


/// Map IEC elementary types to WASM primitive types.
pub fn elementary_to_val_type(spec: ElementarySpec) -> Result<ValType, WasmReprError> {
    Ok(match spec {
        // Boolean and bit strings (8-32 bit) → i32
        ElementarySpec::Bool
        | ElementarySpec::REDGEBool
        | ElementarySpec::FEDGEBool
        | ElementarySpec::Byte
        | ElementarySpec::Word
        | ElementarySpec::DWord => ValType::I32,

        // 64-bit bit string → i64
        ElementarySpec::LWord => ValType::I64,

        // Signed integers
        ElementarySpec::SInt  // 8-bit → i32 (promoted)
        | ElementarySpec::Int   // 16-bit → i32 (promoted)
        | ElementarySpec::DInt  // 32-bit → i32
        => ValType::I32,
        ElementarySpec::LInt => ValType::I64,

        // Unsigned integers
        ElementarySpec::USInt  // 8-bit → i32 (promoted)
        | ElementarySpec::UInt   // 16-bit → i32 (promoted)
        | ElementarySpec::UDInt  // 32-bit → i32
        => ValType::I32,
        ElementarySpec::ULInt => ValType::I64,

        // Floating point
        ElementarySpec::Real => ValType::F32,
        ElementarySpec::LReal => ValType::F64,

        // Time types (stored as 64-bit integers for nanosecond precision)
        ElementarySpec::Time
        | ElementarySpec::LTime
        | ElementarySpec::Date
        | ElementarySpec::LDate
        | ElementarySpec::Tod
        | ElementarySpec::LTod
        | ElementarySpec::DateAndTime
        | ElementarySpec::LDateTime => ValType::I64,

        // String and char types are not supported as direct values
        // They must be stored in memory
        ElementarySpec::String
        | ElementarySpec::WString
        | ElementarySpec::Char
        | ElementarySpec::WChar => {
            return Err(WasmReprError::UnsupportedType(
                format!("String types must be stored in memory: {:?}", spec)
            ));
        }
    })
}

/// Check if an elementary type is signed (affects instruction selection).
pub fn is_signed(spec: ElementarySpec) -> bool {
    matches!(
        spec,
        ElementarySpec::SInt
            | ElementarySpec::Int
            | ElementarySpec::DInt
            | ElementarySpec::LInt
    )
}

/// Check if an elementary type is a floating-point type.
pub fn is_float(spec: ElementarySpec) -> bool {
    matches!(spec, ElementarySpec::Real | ElementarySpec::LReal)
}

/// Check if an elementary type uses 64-bit representation.
pub fn is_64bit(spec: ElementarySpec) -> bool {
    matches!(
        spec,
        ElementarySpec::LWord
            | ElementarySpec::LInt
            | ElementarySpec::ULInt
            | ElementarySpec::LReal
            | ElementarySpec::Time
            | ElementarySpec::LTime
            | ElementarySpec::Date
            | ElementarySpec::LDate
            | ElementarySpec::Tod
            | ElementarySpec::LTod
            | ElementarySpec::DateAndTime
            | ElementarySpec::LDateTime
    )
}
