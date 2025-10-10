
use auto_lsp::default::db::BaseDatabase;

use crate::{
    check::errors::{
        analysis_error::AnalysisError,
        coerce::{ExprMismatch, TypeMismatch},
    },
    hir_def::expressions::spec::ElementarySpec,
    hir_ty::{
        expr_resolver::{ResolvedExpr, ResolvedExprKind},
        ty::{Ty, TyKind},
    },
};

pub fn coerce_ty_with_ty<'db>(
    db: &'db dyn BaseDatabase,
    ty1: Ty<'db>,
    ty2: Ty<'db>,
) -> Result<(), TypeMismatch<'db>> {
    if let TyKind::Target(target) = ty1.kind(db) {
        return coerce_ty_with_ty(db, *target, ty2);
    }

    match (ty1.kind(db), ty2.kind(db)) {
        // Simple equality check between 2 elementary types
        (TyKind::Simple(elem), TyKind::Simple(elem2)) => match elem == elem2 {
            true => Ok(()),
            false => Err(TypeMismatch { ty1, ty2 }),
        },
        // Type mismatch
        _ => Err(TypeMismatch { ty1, ty2 }),
    }
}

pub fn coerce_ty_with_expr<'db>(
    db: &'db dyn BaseDatabase,
    ty: Ty<'db>,
    target_expr: ResolvedExpr<'db>,
) -> Result<(), ExprMismatch<'db>> {
    // If Type is Target, recurse
    if let TyKind::Target(t) = ty.kind(db) {
        return coerce_ty_with_expr(db, *t, target_expr);
    }

    match (ty.kind(db), target_expr.kind(db)) {
        // Compare an elementary type with a literal
        (TyKind::Simple(elem), ResolvedExprKind::Literal(prim)) => elem
            .check_literal(db, *prim)
            .map_err(|err| ExprMismatch::literal(target_expr, ty, err)),
        // Compare an elementary type with a function call
        (TyKind::Simple(elem), ResolvedExprKind::FuncCall(call)) => {
            // Check if the function call has a return type
            match call
                .target
                .ty(db)
                .map_err(|err| ExprMismatch::unresolved_var(target_expr, err))?
                .has_return_type(db)
            {
                Some(ret) => coerce_ty_with_ty(db, ty, ret)
                    .map_err(|err| ExprMismatch::type_mismatch(target_expr, err)),
                None => Err(ExprMismatch::expr_void(target_expr, ty)),
            }
        }
        // Compare an elementary type with a variable access
        (TyKind::Simple(elem), ResolvedExprKind::VarAccess(var)) => {
            // Check if the variable type can be coerced to the target type
            coerce_ty_with_ty(
                db,
                ty,
                var.ty(db)
                    .map_err(|err| ExprMismatch::unresolved_var(target_expr, err))?,
            )
            .map_err(|err| ExprMismatch::type_mismatch(target_expr, err))
        }
        // Compare an elementary with a boolean expression (AND ...)
        (TyKind::Simple(elem), ResolvedExprKind::BooleanExpression(lhs, rhs)) => {
            // Check if Ty is bool
            match elem {
                ElementarySpec::Bool => Ok(()),
                _ => Err(ExprMismatch::lhs_is_not_abool(target_expr, ty)),
            }
        }
        // Compare an elementary with a comparison expression (<> < > <= >= ==)
        (TyKind::Simple(elem), ResolvedExprKind::Compare(lhs, rhs)) => {
            // Check if Ty is bool
            match elem {
                ElementarySpec::Bool => Ok(()),
                _ => Err(ExprMismatch::lhs_is_not_abool(target_expr, ty)),
            }
        }
        // Check if the PathExpr result type matches the array element type
        (TyKind::Array { ranges, typ }, ResolvedExprKind::PathExpr(result)) => coerce_ty_with_ty(
            db,
            typ.spec_to_ty(db, ty.decl(db)),
            result
                .ty(db)
                .map_err(|err| ExprMismatch::unresolved_path(target_expr, err))?,
        )
        .map_err(|err| ExprMismatch::type_mismatch(target_expr, err)),
        (TyKind::Struct { spec, elements }, ResolvedExprKind::PathExpr(result)) => {
            // Check if the PathExpr type matches the struct type
            match result
                .ty(db)
                .map_err(|err| ExprMismatch::unresolved_path(target_expr, err))?
                .kind(db)
            {
                TyKind::Struct { spec: s, elements: e } if s == spec && e == elements => Ok(()),
                _ => Err(ExprMismatch::type_mismatch(
                    target_expr,
                    TypeMismatch {
                        ty1: ty,
                        ty2: result
                            .ty(db)
                            .map_err(|err| ExprMismatch::unresolved_path(target_expr, err))?,
                    },
                )),
            }
        }
        _ => Err(ExprMismatch::expr_mismatch(target_expr, ty)),
    }
}

pub fn coerce_bool_with_ty<'db>(
    db: &'db dyn BaseDatabase,
    ty: Ty<'db>,
) -> Result<bool, AnalysisError<'db>> {
    match ty.kind(db) {
        TyKind::Target(target) => coerce_bool_with_ty(db, *target),
        TyKind::Simple(simple) => match simple {
            ElementarySpec::Bool => Ok(true),
            _ => Ok(false),
        },
        _ => Ok(false),
    }
}

pub fn coerce_bool_with_expr<'db>(
    db: &'db dyn BaseDatabase,
    target_expr: ResolvedExpr<'db>,
) -> Result<bool, AnalysisError<'db>> {
    match target_expr.kind(db) {
        ResolvedExprKind::BooleanExpression(_, _) => Ok(true),
        ResolvedExprKind::Compare(_, _) => Ok(true),
        _ => Ok(false),
    }
}
