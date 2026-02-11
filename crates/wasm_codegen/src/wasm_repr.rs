//! WebAssembly representation of IEC 61131-3 types.
//!
//! This module defines how IEC types map to WASM values and memory layout.

use db::WorkspaceDataBase;
use hir::{
    hir_def::expressions::{
        expression::{Elementary, ExprKind, PrimaryExpr},
        spec::{Array, ElementarySpec, Struct},
    },
    hir_ty::ty::Type,
};
use wasm_encoder::ValType;

/// Represents how an IEC type is stored in WebAssembly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WasmRepr {
    /// Scalar value held in a WASM local (elementary types).
    Scalar(ValType),

    /// Memory-resident value (arrays, structs, large types).
    /// Stores the size in bytes and alignment requirement.
    Memory { size: u32, align: u32 },
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
                    "Array conformant types not yet supported".to_string()
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
    pub fn flatten(self) -> Vec<ValType> {
        match self {
            WasmRepr::Scalar(vt) => vec![vt],
            WasmRepr::Memory { .. } => vec![ValType::I32], // Pointer
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
}

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

/// Calculate struct layout with natural alignment.
/// Returns (total_size, max_alignment).
fn calculate_struct_layout<'db>(
    db: &'db dyn WorkspaceDataBase,
    struct_type: Struct<'db>,
) -> Result<(u32, u32), WasmReprError> {
    use hir::hir_ty::infer::Infer;

    let mut offset = 0u32;
    let mut max_align = 1u32;

    for element in struct_type.elements(db) {
        let field_type = element.spec(db).infer(db);
        let field_repr = WasmRepr::from_type(db, field_type)?;
        let field_align = field_repr.alignment();
        let field_size = field_repr.size_bytes();

        // Track maximum alignment
        max_align = max_align.max(field_align);

        // Align current offset to field's alignment
        offset = align_to(offset, field_align);

        // Add field size
        offset += field_size;
    }

    // Pad to alignment at the end
    offset = align_to(offset, max_align);

    Ok((offset, max_align))
}

/// Calculate array layout.
/// Returns (total_size, element_alignment).
fn calculate_array_layout<'db>(
    db: &'db dyn WorkspaceDataBase,
    array_type: Array<'db>,
) -> Result<(u32, u32), WasmReprError> {
    use hir::hir_ty::infer::Infer;

    // Get element type representation
    let element_type = array_type.of_type(db).infer(db);
    let element_repr = WasmRepr::from_type(db, element_type)?;
    let element_size = element_repr.size_bytes();
    let element_align = element_repr.alignment();

    // Calculate total number of elements
    let total_elements = calculate_array_total_elements(db, array_type)?;

    // Total size = element_size * total_elements
    let total_size = element_size.checked_mul(total_elements)
        .ok_or_else(|| WasmReprError::UnsupportedType(
            format!("Array size overflow: {} * {}", element_size, total_elements)
        ))?;

    Ok((total_size, element_align))
}

/// Calculate the total number of elements in an array (product of all dimension sizes).
fn calculate_array_total_elements<'db>(
    db: &'db dyn WorkspaceDataBase,
    array_type: Array<'db>,
) -> Result<u32, WasmReprError> {
    let mut total = 1u32;

    for (start_expr, end_expr) in array_type.subranges(db) {
        // Extract integer literals from expressions
        let start = extract_integer_literal(db, start_expr)?;
        let end = extract_integer_literal(db, end_expr)?;

        // Calculate dimension size: (end - start + 1)
        let dim_size = (end - start + 1).max(0) as u32;

        total = total.checked_mul(dim_size)
            .ok_or_else(|| WasmReprError::UnsupportedType(
                format!("Array dimension overflow: {} * {}", total, dim_size)
            ))?;
    }

    Ok(total)
}

/// Extract an integer literal from a constant expression.
/// Only supports literal integers (not complex constant expressions).
fn extract_integer_literal<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: hir::hir_def::expressions::expression::Expr<'db>,
) -> Result<i32, WasmReprError> {
    match expr.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(Elementary::InferInteger(int))) => {
            int.as_i32(db).map_err(|e| WasmReprError::UnsupportedType(
                format!("Failed to parse array bound as i32: {}", e)
            ))
        }
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(Elementary::Int(int))) => {
            int.as_i32(db).map_err(|e| WasmReprError::UnsupportedType(
                format!("Failed to parse array bound as i32: {}", e)
            ))
        }
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(Elementary::DInt(int))) => {
            int.as_i32(db).map_err(|e| WasmReprError::UnsupportedType(
                format!("Failed to parse array bound as i32: {}", e)
            ))
        }
        _ => Err(WasmReprError::UnsupportedType(
            "Array bounds must be integer literals".to_string()
        )),
    }
}

/// Helper: align offset to specified alignment.
/// Returns the smallest value >= offset that is a multiple of align.
fn align_to(offset: u32, align: u32) -> u32 {
    (offset + align - 1) & !(align - 1)
}

/// Calculate field offsets for a struct (used by body.rs for field access).
pub fn calculate_field_offsets<'db>(
    db: &'db dyn WorkspaceDataBase,
    struct_type: Struct<'db>,
) -> Result<Vec<(hir::hir_def::interned::identifier::Ident, u32)>, WasmReprError> {
    use hir::hir_ty::infer::Infer;

    let mut offset = 0u32;
    let mut field_offsets = Vec::new();

    for element in struct_type.elements(db) {
        let field_type = element.spec(db).infer(db);
        let field_repr = WasmRepr::from_type(db, field_type)?;
        let field_align = field_repr.alignment();
        let field_size = field_repr.size_bytes();

        // Align offset to field's alignment
        offset = align_to(offset, field_align);

        // Store field name and offset
        field_offsets.push((element.name(db), offset));

        // Advance offset
        offset += field_size;
    }

    Ok(field_offsets)
}

/// Calculate function block instance layout with natural alignment.
/// Returns (total_size, max_alignment).
fn calculate_fb_layout<'db>(
    db: &'db dyn WorkspaceDataBase,
    fb: hir::hir_def::pous::function_block::FunctionBlock<'db>,
) -> Result<(u32, u32), WasmReprError> {
    use hir::hir_ty::infer::Infer;

    let mut offset = 0u32;
    let mut max_align = 1u32;

    // Layout all instance variables (VAR, VAR_INPUT, VAR_OUTPUT, etc.)
    for var in fb.variables(db) {
        let var_type = var.spec(db).infer(db);
        let var_repr = WasmRepr::from_type(db, var_type)?;
        let var_align = var_repr.alignment();
        let var_size = var_repr.size_bytes();

        // Track maximum alignment
        max_align = max_align.max(var_align);

        // Align current offset to variable's alignment
        offset = align_to(offset, var_align);

        // Add variable size
        offset += var_size;
    }

    // Pad to alignment at the end
    offset = align_to(offset, max_align);

    Ok((offset, max_align))
}

/// Calculate class instance layout with natural alignment.
/// Returns (total_size, max_alignment).
fn calculate_class_layout<'db>(
    db: &'db dyn WorkspaceDataBase,
    class: hir::hir_def::pous::class::Class<'db>,
) -> Result<(u32, u32), WasmReprError> {
    use hir::hir_ty::infer::Infer;

    let mut offset = 0u32;
    let mut max_align = 1u32;

    // TODO: Handle inheritance - should include parent class fields first
    // For now, just layout this class's variables
    for var in class.variables(db) {
        let var_type = var.spec(db).infer(db);
        let var_repr = WasmRepr::from_type(db, var_type)?;
        let var_align = var_repr.alignment();
        let var_size = var_repr.size_bytes();

        // Track maximum alignment
        max_align = max_align.max(var_align);

        // Align current offset to variable's alignment
        offset = align_to(offset, var_align);

        // Add variable size
        offset += var_size;
    }

    // Align final size to maximum alignment
    offset = align_to(offset, max_align);

    Ok((offset, max_align))
}

/// Calculate field offsets for a function block (used for instance variable access).
pub fn calculate_fb_field_offsets<'db>(
    db: &'db dyn WorkspaceDataBase,
    fb: hir::hir_def::pous::function_block::FunctionBlock<'db>,
) -> Result<Vec<(hir::hir_def::interned::identifier::Ident, u32)>, WasmReprError> {
    use hir::hir_ty::infer::Infer;

    let mut offset = 0u32;
    let mut field_offsets = Vec::new();

    for var in fb.variables(db) {
        let var_type = var.spec(db).infer(db);
        let var_repr = WasmRepr::from_type(db, var_type)?;
        let var_align = var_repr.alignment();
        let var_size = var_repr.size_bytes();

        // Align offset to variable's alignment
        offset = align_to(offset, var_align);

        // Store variable name and offset
        field_offsets.push((var.name(db), offset));

        // Advance offset
        offset += var_size;
    }

    Ok(field_offsets)
}

/// Calculate byte offsets for each field in an instance (FunctionBlock or Class).
///
/// Returns a vector of (field_name, byte_offset) tuples.
pub fn calculate_instance_field_offsets<'db>(
    db: &'db dyn WorkspaceDataBase,
    instance: crate::func_codegen::InstanceType<'db>,
) -> Result<Vec<(hir::hir_def::interned::identifier::Ident, u32)>, WasmReprError> {
    use hir::hir_ty::infer::Infer;

    let mut offset = 0u32;
    let mut field_offsets = Vec::new();

    for var in instance.variables(db) {
        let var_type = var.spec(db).infer(db);
        let var_repr = WasmRepr::from_type(db, var_type)?;
        let var_align = var_repr.alignment();
        let var_size = var_repr.size_bytes();

        // Align offset to variable's alignment
        offset = align_to(offset, var_align);

        // Store variable name and offset
        field_offsets.push((var.name(db), offset));

        // Advance offset
        offset += var_size;
    }

    Ok(field_offsets)
}

#[derive(Debug, thiserror::Error)]
pub enum WasmReprError {
    #[error("Unsupported type: {0}")]
    UnsupportedType(String),

    #[error("Type could not be resolved (Type::Never encountered)")]
    UnresolvedType,
}
