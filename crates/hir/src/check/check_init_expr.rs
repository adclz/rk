use crate::{check::errors::analysis_error::ToIdeDiagnostic, hir_def::expressions::expression::{Expr, InitExpr}, hir_ty::{init_inference::infer_init_expr, ty::Type}};
use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::IdeDiagnostic;


pub fn check_init_expr<'db>(
    db: &'db dyn BaseDatabase,
    ty: Type<'db>,
    expr: InitExpr<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    let infer = infer_init_expr(db, ty, expr);

    eprintln!("errors in check_init_expr: {:?}", infer.errors.len());
    for error in infer.errors.iter() {
        errors.push(error.to_diagnostic(db));
    }

    eprintln!("resolved init expr in check_init_expr: {:?}", infer.body_infer_result.errors.len());
    for error in infer.body_infer_result.errors.iter() {
        errors.push(error.to_diagnostic(db));
    }
}

/* 
fn check_array_dimensions<'db>(
    db: &'db dyn BaseDatabase,
    ranges: &[(Expr<'db>, Expr<'db>)],
    element_type: Ty<'db>,
    values: &[ResolvedInitExpr<'db>],
    errors: &mut Vec<IdeDiagnostic>,
) {
    if let Some((first_range, remaining_ranges)) = ranges.split_first() {
        if let (Some(v1), Some(v2)) = (
            first_range.0.as_range(db),
            first_range.1.as_range(db),
        ) {
            let dimension_capacity = v2 - v1 + 1;
            if let Err(err) = count_elements_at_dimension(db, values, dimension_capacity) {
                errors.push(
                    InitExprError::ArrayTooManyElements {
                        array: element_type,
                        provided_count: err.0,
                        max_capacity: dimension_capacity,
                        init_expr: err.1,
                    }
                    .to_diagnostic(db),
                );
            }

            // Recursively check inner dimensions
            for value in values {
                if let ResolvedInitExprKind::ArrayIndexedElement {
                    values: inner_values,
                    ..
                } = value.kind(db)
                {
                    if remaining_ranges.is_empty() {
                        // Last dimension - check the values against element type
                        for inner_value in inner_values {
                        }
                    } else {
                        // More dimensions - recurse
                        check_array_dimensions(
                            db,
                            remaining_ranges,
                            element_type,
                            &inner_values,
                            errors,
                        );
                    }
                } else if remaining_ranges.is_empty() {
                    // Single value at last dimension
                }
            }
        }
    }
}

fn count_elements_at_dimension<'db>(
    db: &'db dyn BaseDatabase,
    values: &[ResolvedInitExpr<'db>],
    max_capacity: u64,
) -> Result<(), (u64, ResolvedInitExpr<'db>)> {
    let mut count = 0u64;

    for value in values {
        match value.kind(db) {
            ResolvedInitExprKind::ArrayIndexedElement { size, .. } => {
                // The size indicates how many elements at this dimension level
                if let Ok(repeat_count) = size.as_u64(db) {
                    count += repeat_count;
                }
                if count > max_capacity {
                    return Err((count, *value));
                }
            }
            _ => {
                // Single element without repetition count
                count += 1;
                if count > max_capacity {
                    return Err((count, *value));
                }
            }
        }
    }

    Ok(())
}
*/