use auto_lsp::default::db::BaseDatabase;

use crate::{
    check::errors::{sem_errors::AnalysisError, stmt::StmtError},
    hir_def::expressions::spec::ElementarySpec,
    hir_ty::{
        TyInfo,
        expr_resolver::{ResolvedExpr, ResolvedExprKind},
        ty::{Ty, TyKind},
    },
};

pub fn coerce_ty_with_expr<'db>(
    db: &'db dyn BaseDatabase,
    ty: Ty<'db>,
    target_expr: ResolvedExpr<'db>,
) -> Result<(), AnalysisError<'db>> {
    // If Type is Target, recurse
    if let TyKind::Target(t) = ty.kind(db) {
        return coerce_ty_with_expr(db, t, target_expr);
    }

    match (ty.kind(db), target_expr.kind(db)) {
        // Compare an elementary type with a literal
        (TyKind::Simple(elem), ResolvedExprKind::Literal(prim)) => elem
            .lit_check(db, *prim)
            .map_err(|err| (err, ty, target_expr).into()),
        // Compare an elementary type with a function call
        (TyKind::Simple(elem), ResolvedExprKind::FuncCall { target, .. }) => {
            // Check if the function call has a return type
            match target.ty(db)?.has_return_type(db) {
                Some(ret) => coerce_ty_with_ty(db, target_expr, ty, ret),
                None => Err(StmtError::VoidAssignmentRHS {
                    ty,
                    target: *target,
                }
                .into()),
            }
        }
        // Compare an elementary with a boolean expression (AND ...)
        (TyKind::Simple(elem), ResolvedExprKind::BooleanExpression(lhs, rhs)) => {
            // Check if Ty is bool
            match elem {
                ElementarySpec::Bool => Ok(()),
                _ => Err(StmtError::AssignBoolExpressionToNonBool {
                    ty,
                    expr: target_expr,
                }
                .into()),
            }
        }
        // Compare an elementary with a comparison expression (<> < > <= >= ==)
        (TyKind::Simple(elem), ResolvedExprKind::Compare(lhs, rhs)) => {
            // Check if Ty is bool
            match elem {
                ElementarySpec::Bool => Ok(()),
                _ => Err(StmtError::AssignBoolExpressionToNonBool {
                    ty,
                    expr: target_expr,
                }
                .into()),
            }
        }
        // Check if the PathExpr result type matches the array element type
        (TyKind::Array { ranges, typ }, ResolvedExprKind::PathExpr(result)) => {
            coerce_ty_with_ty(db, target_expr, typ, result.ty(db)?)
        }
        _ => todo!(),
    }
}

pub fn coerce_ty_with_ty<'db>(
    db: &'db dyn BaseDatabase,
    expr: ResolvedExpr<'db>,
    target_ty: Ty<'db>,
    expr_ty: Ty<'db>,
) -> Result<(), AnalysisError<'db>> {
    if let TyKind::Target(t) = target_ty.kind(db) {
        return coerce_ty_with_ty(db, expr, t, expr_ty);
    }

    match (target_ty.kind(db), expr_ty.kind(db)) {
        // Simple equality check between 2 elementary types
        (TyKind::Simple(elem), TyKind::Simple(elem2)) => match elem == elem2 {
            true => Ok(()),
            false => Err(StmtError::TypeMismatch {
                expr,
                ty: target_ty,
                ty2: expr_ty,
            }
            .into()),
        },
        _ => todo!(),
    }
}
