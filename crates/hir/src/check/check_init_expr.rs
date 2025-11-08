use crate::{check::errors::analysis_error::ToIdeDiagnostic, hir_def::expressions::expression::Expr};
use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::IdeDiagnostic;

use crate::{
    check::{
        coerce::coerce_ty_with_expr,
        errors::init_expr::InitExprError,
    },
    hir_ty::{
        array_resolver::resolve_range,
        init_expr_resolver::{ResolvedInitExpr, ResolvedInitExprKind},
        ty::{Ty, TyKind},
    },
};

pub fn check_init_expr<'db>(
    db: &'db dyn BaseDatabase,
    ty: Ty<'db>,
    expr: ResolvedInitExpr<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    match expr.kind(db) {
        // Process resolver errors first
        ResolvedInitExprKind::Error(err) => {
            errors.push(err.to_diagnostic(db));
        }
        // Handle constant expressions - only case that needs original type for coercion
        ResolvedInitExprKind::ConstantExpr(expr_val) => {
            if let Err(err) = coerce_ty_with_expr(db, ty, expr_val) {
                errors.push(
                    InitExprError::InitExprTypeExprMismatch {
                        err,
                        init_expr: expr_val,
                    }
                    .to_diagnostic(db),
                )
            }
        }
        // Recursively check nested structures - resolver has already validated types
        ResolvedInitExprKind::StructInit { values } => {
            for field_expr in values {
                check_init_expr(db, ty, field_expr, errors);
            }
        }
        ResolvedInitExprKind::StructElement { field, value } => if let Ok(field_ty) = field.try_to_ty(db) {
            check_init_expr(db, field_ty, *value, errors);
        },
        ResolvedInitExprKind::ArrayInit { values } => {
            // For arrays, validate bounds if we have array type info
            if let TyKind::Array(array) = ty.kind(db) {
                check_array_dimensions(
                    db,
                    &array.subranges(db),
                    array.of_type(db).to_ty(db),
                    &values,
                    errors,
                );
            } else {
                // Just recursively check the values - resolver has already validated types
                for value in values {
                    check_init_expr(db, ty, value, errors);
                }
            }
        }
        ResolvedInitExprKind::ArrayIndexedElement { values, .. } => {
            for value in values {
                check_init_expr(db, ty, value, errors);
            }
        }
    }
}

fn check_array_dimensions<'db>(
    db: &'db dyn BaseDatabase,
    ranges: &[(Expr<'db>, Expr<'db>)],
    element_type: Ty<'db>,
    values: &[ResolvedInitExpr<'db>],
    errors: &mut Vec<IdeDiagnostic>,
) {
    if let Some((first_range, remaining_ranges)) = ranges.split_first() {
        if let (Some(v1), Some(v2)) = (
            resolve_range(db, first_range.0),
            resolve_range(db, first_range.1),
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
                            check_init_expr(db, element_type, inner_value, errors);
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
                    check_init_expr(db, element_type, *value, errors);
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
