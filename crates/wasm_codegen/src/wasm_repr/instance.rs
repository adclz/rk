use db::WorkspaceDataBase;

use crate::wasm_repr::{WasmRepr, WasmReprError, align_to};

/// Calculate function block instance layout with natural alignment.
/// Returns (total_size, max_alignment).
pub fn calculate_fb_layout<'db>(
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

/// Calculate class instance layout with natural alignment.
/// Returns (total_size, max_alignment).
pub fn calculate_class_layout<'db>(
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
