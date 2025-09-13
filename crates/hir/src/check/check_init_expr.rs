use auto_lsp::default::db::BaseDatabase;

use crate::{
    check::errors::{init_expr::InitExprError, sem_errors::AnalysisError},
    hir_ty::{
        expr_resolver::{ResolvedExprKind},
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

    match ty.kind(db) {
        TyKind::Array { typ, ranges } => {}
        TyKind::Struct { spec, elements } => {
            if let ResolvedInitExprKind::StructInit { values } = expr.kind(db) {
                for field in values {
                    if let ResolvedInitExprKind::StructElement { name, value } = field.kind(db) {
                        if !elements.contains_key(name) {
                            errors.push(
                                InitExprError::UnknownStructField {
                                    ztruct: ty,
                                    field_name: name.clone(),
                                    unknown_field: **value,
                                }
                                .into(),
                            );
                        }
                    }
                }
            }
        }
        TyKind::Enum { typ, list } => {
            if let ResolvedInitExprKind::ConstantExpr(expr) = expr.kind(db) {
                if let ResolvedExprKind::VarAccess(var) = expr.kind(db) {}
            }
        }
        _ => {}
    }
}
