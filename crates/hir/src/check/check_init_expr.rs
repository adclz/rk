use auto_lsp::default::db::BaseDatabase;

use crate::{
    check::{coerce::coerce_ty_with_expr, errors::{init_expr::InitExprError, sem_errors::AnalysisError}},
    hir_ty::{
        expr_resolver::ResolvedExprKind,
        init_expr_resolver::{ResolvedInitExpr, ResolvedInitExprKind},
        ty::{Ty, TyKind},
    },
};

pub fn check_init_expr<'db>(
    db: &'db dyn BaseDatabase,
    ty: Ty<'db>,
    expr: ResolvedInitExpr<'db>,
    errors: &mut Vec<AnalysisError<'db>>,
) {
    if let TyKind::Target(target) = ty.kind(db) {
        check_init_expr(db, target, expr, errors);
    }

    match (ty.kind(db), expr.kind(db)) {
        // Check that all fields in struct init exist in struct definition
        (TyKind::Struct { spec, elements }, ResolvedInitExprKind::StructInit { values }) => {
            for field in values {
                if let ResolvedInitExprKind::StructElement { name, value } = field.kind(db) {
                    match elements.get(name) {
                        None => {
                            errors.push(
                                InitExprError::UnknownStructField {
                                    ztruct: ty,
                                    field_name: *name,
                                    unknown_field: **value,
                                }
                                .into(),
                            );
                        }
                        Some(elem) => check_init_expr(db, *elem, **value, errors),
                    }
                }
            }
        }
        (_, ResolvedInitExprKind::ConstantExpr(expr)) => {
            if let Err(err) = coerce_ty_with_expr(db, ty, *expr) {
                errors.push(err)
            }
        },
        _ => {}
    }
}
