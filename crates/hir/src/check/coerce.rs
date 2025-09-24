use auto_lsp::default::db::BaseDatabase;

use crate::{
    check::errors::{coerce::{CoerceError, TypeMismatch}, literals::LiteralError, sem_errors::AnalysisError, stmt::StmtError},
    hir_def::expressions::spec::ElementarySpec,
    hir_ty::{
        expr_resolver::{ResolvedExpr, ResolvedExprKind}, ty::{Ty, TyKind}, TyInfo
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
            .check_literal(db, *prim)
            .map_err(|err| LiteralError::new(ty, target_expr, err).into()),
        // Compare an elementary type with a function call
        (TyKind::Simple(elem), ResolvedExprKind::FuncCall { target, .. }) => {
            // Check if the function call has a return type
            match target.ty(db)?.has_return_type(db) {
                Some(ret) => coerce_ty_with_ty(db, ty, ret)
                   .map_err(|err| CoerceError::new_expr_type_mismatch(target_expr, err).into()),
                None => Err(StmtError::VoidAssignmentRHS {
                    ty,
                    target: *target,
                }
                .into()),
            }
        }
        // Compare an elementary type with a variable access
        (TyKind::Simple(elem), ResolvedExprKind::VarAccess(var)) => {
            // Check if the variable type can be coerced to the target type
            coerce_ty_with_ty(db, ty, var.ty(db)?)
                .map_err(|err| CoerceError::new_expr_type_mismatch(target_expr, err).into())
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
            coerce_ty_with_ty(db, typ, result.ty(db)?)
                .map_err(|err| CoerceError::new_expr_type_mismatch(target_expr, err).into())
        },
        _ => {
            Err(CoerceError::UnknownTypeExpr {
                expr: target_expr,
                ty,
            }
            .into())
        }
    }
}

pub fn coerce_ty_with_ty<'db>(
    db: &'db dyn BaseDatabase,
    ty1: Ty<'db>,
    ty2: Ty<'db>,
) -> Result<(), TypeMismatch<'db>> {
    if let TyKind::Target(t) = ty1.kind(db) {
        return coerce_ty_with_ty(db, t, ty2);
    }

    match (ty1.kind(db), ty2.kind(db)) {
        // Simple equality check between 2 elementary types
        (TyKind::Simple(elem), TyKind::Simple(elem2)) => match elem == elem2 {
            true => Ok(()),
            false => Err(TypeMismatch {
                ty1,
                ty2,
            }
            .into()),
        },
        // Type mismatch
        _ => Err(TypeMismatch { ty1, ty2 }.into()),
    }
}
