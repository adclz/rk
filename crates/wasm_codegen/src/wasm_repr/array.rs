use db::WorkspaceDataBase;
use hir::hir_def::expressions::{
    expression::{Elementary, ExprKind, PrimaryExpr},
    spec::Array,
};

use crate::wasm_repr::{WasmRepr, WasmReprError};

/// Calculate array layout.
/// Returns (total_size, element_alignment).
pub fn calculate_array_layout<'db>(
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
    let total_size = element_size.checked_mul(total_elements).ok_or_else(|| {
        WasmReprError::UnsupportedType(format!(
            "Array size overflow: {} * {}",
            element_size, total_elements
        ))
    })?;

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

        total = total.checked_mul(dim_size).ok_or_else(|| {
            WasmReprError::UnsupportedType(format!(
                "Array dimension overflow: {} * {}",
                total, dim_size
            ))
        })?;
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
            int.as_i32(db).map_err(|e| {
                WasmReprError::UnsupportedType(format!("Failed to parse array bound as i32: {}", e))
            })
        }
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(Elementary::Int(int))) => {
            int.as_i32(db).map_err(|e| {
                WasmReprError::UnsupportedType(format!("Failed to parse array bound as i32: {}", e))
            })
        }
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(Elementary::DInt(int))) => {
            int.as_i32(db).map_err(|e| {
                WasmReprError::UnsupportedType(format!("Failed to parse array bound as i32: {}", e))
            })
        }
        _ => Err(WasmReprError::UnsupportedType(
            "Array bounds must be integer literals".to_string(),
        )),
    }
}
