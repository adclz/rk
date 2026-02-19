use db::WorkspaceDataBase;
use hir::hir_def::expressions::spec::Struct;

use crate::wasm_repr::{WasmRepr, WasmReprError, align_to};

/// Calculate struct layout with natural alignment.
/// Returns (total_size, max_alignment).
pub fn calculate_struct_layout<'db>(
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
