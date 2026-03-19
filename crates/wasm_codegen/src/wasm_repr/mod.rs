use db::WorkspaceDataBase;
use hir::hir_ty::ty::Type;
use wasm_encoder::ValType;

use crate::wasm_repr::{
    array::calculate_array_layout,
    elementary::elementary_to_val_type,
    instance::{calculate_class_layout, calculate_fb_layout},
    strukt::calculate_struct_layout,
};

pub mod array;
pub mod elementary;
pub mod instance;
pub mod strukt;

#[derive(Debug, thiserror::Error)]
pub enum WasmReprError {
    #[error("Unsupported type: {0}")]
    UnsupportedType(String),

    #[error("Type could not be resolved (Type::Never encountered)")]
    UnresolvedType,
}

/// Represents how an IEC type is stored in WebAssembly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmRepr {
    /// Scalar value held in a WASM local (elementary types).
    Scalar(ValType),

    /// Memory-resident value (arrays, structs, large types).
    /// Stores the size in bytes and alignment requirement.
    Memory { size: u32, align: u32 },

    /// String value: (ptr: i32, len: i32) pair in linear memory.
    /// In the component model, this maps to the native `string` type.
    StringPtr,
}

impl WasmRepr {
    /// Convert an IEC 61131-3 type to its WASM representation.
    pub fn from_type<'db>(
        db: &'db dyn WorkspaceDataBase,
        ty: Type<'db>,
    ) -> Result<Self, WasmReprError> {
        // Normalize the type first (resolve Variables, DataTypes, etc.)
        let ty = ty.normalize(db);

        match ty {
            Type::Elementary(spec) if elementary::is_string_type(spec) => Ok(WasmRepr::StringPtr),

            Type::Elementary(spec) => Ok(WasmRepr::Scalar(elementary_to_val_type(spec)?)),

            Type::RefTo(_spec) => {
                // References are represented as i32 pointers
                Ok(WasmRepr::Scalar(ValType::I32))
            }

            Type::Struct(s) => {
                // Calculate struct size and alignment with natural alignment
                let (size, align) = calculate_struct_layout(db, s)?;
                Ok(WasmRepr::Memory { size, align })
            }

            Type::Array(arr) => {
                // Calculate array size: element_size * total_elements
                let (size, align) = calculate_array_layout(db, arr)?;
                Ok(WasmRepr::Memory { size, align })
            }

            Type::ArrayConformand(_) => {
                // Array conformants (open arrays) not supported yet
                Err(WasmReprError::UnsupportedType(
                    "Array conformant types not yet supported".to_string(),
                ))
            }

            Type::FunctionBlock(fb) => {
                // Function block instances are memory-resident objects
                // Calculate size based on FB variables (similar to structs)
                let (size, align) = calculate_fb_layout(db, fb)?;
                Ok(WasmRepr::Memory { size, align })
            }

            Type::Class(class) => {
                // Class instances are memory-resident objects (like FBs)
                // Calculate size based on class variables
                let (size, align) = calculate_class_layout(db, class)?;
                Ok(WasmRepr::Memory { size, align })
            }

            Type::Never => Err(WasmReprError::UnresolvedType),

            _ => Err(WasmReprError::UnsupportedType(format!("{:?}", ty))),
        }
    }

    /// Flatten this representation into a list of ValTypes.
    /// Used for function signatures (params and results).
    /// - Scalar(vt) → [vt]
    /// - Memory{..} → [i32] (pointer to memory)
    /// - StringPtr → [i32, i32] (pointer, length)
    pub fn flatten(self) -> Vec<ValType> {
        match self {
            WasmRepr::Scalar(vt) => vec![vt],
            WasmRepr::Memory { .. } => vec![ValType::I32], // Pointer
            WasmRepr::StringPtr => vec![ValType::I32, ValType::I32], // (ptr, len)
        }
    }

    /// Calculate size in bytes.
    pub fn size_bytes(self) -> u32 {
        match self {
            WasmRepr::Scalar(vt) => match vt {
                ValType::I32 | ValType::F32 => 4,
                ValType::I64 | ValType::F64 => 8,
                _ => 0,
            },
            WasmRepr::Memory { size, .. } => size,
            WasmRepr::StringPtr => 8, // ptr(4) + len(4)
        }
    }

    /// Calculate alignment requirement.
    pub fn alignment(self) -> u32 {
        match self {
            WasmRepr::Scalar(vt) => match vt {
                ValType::I32 | ValType::F32 => 4,
                ValType::I64 | ValType::F64 => 8,
                _ => 1,
            },
            WasmRepr::Memory { align, .. } => align,
            WasmRepr::StringPtr => 4,
        }
    }

    /// Get the WASM ValType if this is a scalar representation.
    pub fn as_val_type(&self) -> Option<ValType> {
        match self {
            WasmRepr::Scalar(vt) => Some(*vt),
            _ => None,
        }
    }

    /// Check if this is a scalar value.
    pub fn is_scalar(&self) -> bool {
        matches!(self, WasmRepr::Scalar(_))
    }

    /// Check if this is memory-resident.
    pub fn is_memory(&self) -> bool {
        matches!(self, WasmRepr::Memory { .. })
    }

    /// Check if this is a string type.
    pub fn is_string(&self) -> bool {
        matches!(self, WasmRepr::StringPtr)
    }
}

/// Helper: align offset to specified alignment.
/// Returns the smallest value >= offset that is a multiple of align.
pub fn align_to(offset: u32, align: u32) -> u32 {
    (offset + align - 1) & !(align - 1)
}
