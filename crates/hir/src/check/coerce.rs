use auto_lsp::default::db::BaseDatabase;

use crate::{
    check::errors::{
        analysis_error::AnalysisError,
        coerce::{ExprMismatch, TypeMismatch},
    },
    hir_def::expressions::{
        expression::{Expr, ExprKind, PrimaryExpr, RefValue},
        spec::ElementarySpec,
    },
    hir_ty::{
        array_resolver::resolve_range,
        ty::{Ty, TyKind},
        ty_var_access_resolver::{LookUp},
    },
};

pub fn coerce_ty_with_ty<'db>(
    db: &'db dyn BaseDatabase,
    ty1: Ty<'db>,
    ty2: Ty<'db>,
) -> Result<(), TypeMismatch<'db>> {
    match (ty1.kind(db), ty2.kind(db)) {
        // Simple equality check between 2 elementary types
        (TyKind::Simple(elem), TyKind::Simple(elem2)) => match elem == elem2 {
            true => Ok(()),
            false => Err(TypeMismatch { ty1, ty2 }),
        },
        // For all other types, we check for exact equality of the HIR def node
        _ => {
            if ty1.kind(db) == ty2.kind(db) {
                Ok(())
            } else {
                Err(TypeMismatch { ty1, ty2 })
            }
        }
    }
}

pub fn coerce_ty_with_expr<'db>(
    db: &'db dyn BaseDatabase,
    ty: Ty<'db>,
    target_expr: Expr<'db>,
) -> Result<(), ExprMismatch<'db>> {
    if let TyKind::Err(err) = ty.kind(db) {
        return Err(ExprMismatch::unresolved_path(target_expr, err.clone()));
    }
    match (ty.kind(db), target_expr.expr(db)) {
        // Compare an elementary type with a function call
        (TyKind::Simple(elem), ExprKind::PrimaryExpr(PrimaryExpr::FuncCall(call))) => {
            let call = call.resolve_func_call(db);
            // Check if the function call has a return type
            match call.target.with_return_type(db) {
                Some(ret) => coerce_ty_with_ty(db, ty, ret.to_ty(db))
                    .map_err(|err| ExprMismatch::type_mismatch(target_expr, err)),
                None => Err(ExprMismatch::expr_void(target_expr, ty)),
            }
        }
        // Compare an elementary type with a variable access
        (
            TyKind::Simple(elem),
            ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(variable)),
        ) => {
            // Check if the variable type can be coerced to the target type
            coerce_ty_with_ty(
                db,
                ty,
                variable.lookup(db).try_to_ty(db)
                    .map_err(|err| ExprMismatch::unresolved_path(target_expr, err))?,
            )
            .map_err(|err| ExprMismatch::type_mismatch(target_expr, err))
        }
        // Compare an elementary with a boolean expression (AND ...)
        (TyKind::Simple(elem), ExprKind::BooleanOperator { left, right, .. }) => {
            // Check if Ty is bool
            match elem {
                ElementarySpec::Bool => Ok(()),
                _ => Err(ExprMismatch::lhs_is_not_abool(target_expr, ty)),
            }
        }
        // Compare an elementary with a comparison expression (<> < > <= >= ==)
        (TyKind::Simple(elem), ExprKind::ComparisonOperator { left, right, .. }) => {
            // Check if Ty is bool
            match elem {
                ElementarySpec::Bool => Ok(()),
                _ => Err(ExprMismatch::lhs_is_not_abool(target_expr, ty)),
            }
        }
        // Compare an Array with PathExpr (PathExpr should be an indexed access)
        (
            TyKind::Array(array),
            ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(variable)),
        ) => {
            coerce_ty_with_ty(
                db,
                array.of_type(db).to_ty(db),
                variable.lookup(db).try_to_ty(db)
                    .map_err(|err| ExprMismatch::unresolved_path(target_expr, err))?,
            )
        }
        .map_err(|err| ExprMismatch::type_mismatch(target_expr, err)),
        // Compare a Struct with PathExpr (PathExpr should be a field access)
        (TyKind::Struct(ztruct), ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(variable))) => {
            let result = variable.lookup(db);
            match result
                .try_to_ty(db)
                .map_err(|err| ExprMismatch::unresolved_path(target_expr, err))?
                .kind(db)
            {
                TyKind::Struct(ztruct_2) if ztruct == ztruct_2 => Ok(()),
                _ => Err(ExprMismatch::type_mismatch(
                    target_expr,
                    TypeMismatch {
                        ty1: ty,
                        ty2: result
                            .try_to_ty(db)
                            .map_err(|err| ExprMismatch::unresolved_path(target_expr, err))?,
                    },
                )),
            }
        }
        // Compare an Enum with EnumValue
        (
            TyKind::Enum(enum_),
            ExprKind::PrimaryExpr(PrimaryExpr::EnumValue { name, variant }))  => {
            let name = name.lookup(db);
            // Check if the EnumValue type matches the enum type
            match name
                .try_to_ty(db)
                .map_err(|err| ExprMismatch::unresolved_path(target_expr, err))?
                .kind(db)
            {
                TyKind::Enum(enum_2) => match enum_2.variants(db).iter().find(|v| v.name.ident == variant.ident) {
                    Some(variant) => Ok(()),
                    None => Err(ExprMismatch::invalid_enum_variant(target_expr, ty, *variant)),
                },
                _ => unreachable!("resolver should ensure enum value matches enum type"),
            }
        }
        // Compare a Subrange with any expression
        // todo: check that the expression is within the subrange
        // for that, we could
        (TyKind::SubRange(subrange), _) => {
            // We do not return an error in case of failure to resolve the range bounds
            // as the error will be reported when checking the subrange type itself
            let min = match resolve_range(db, subrange.lower(db)) {
                Some(v) => v,
                None => return Ok(()),
            };

            let max = match resolve_range(db, subrange.upper(db)) {
                Some(v) => v,
                None => return Ok(()),
            };

            match coerce_ty_with_expr(db, subrange._type(db).to_ty(db), target_expr) {
                Ok(()) => match resolve_range(db, target_expr) {
                    Some(integer) => {
                        if integer >= min && integer <= max {
                            Ok(())
                        } else {
                            Err(ExprMismatch::subrange_value_out_of_bounds(
                                target_expr,
                                ty,
                                min,
                                max,
                                integer,
                            ))
                        }
                    }
                    None => Ok(()),
                },
                Err(err) => Err(err),
            }
        }
        (TyKind::RefTo(ref_), ExprKind::PrimaryExpr(PrimaryExpr::RefValue { value })) => {
            let ref_to = ref_.to_ty(db);

            match value {
                // A NULL reference can be assigned to any reference type
                RefValue::Null => Ok(()),
                RefValue::Address(v) => {
                    let v = v.lookup(db);
                    // Retrives the element that the reference points to
                    let var_ty = v
                        .try_to_ty(db)
                        .map_err(|err| ExprMismatch::unresolved_path(target_expr, err))?;

                    // Check that the type of the variable is the same as the type the reference points to
                    coerce_ty_with_ty(db, ref_to, var_ty)
                        .map_err(|err| ExprMismatch::type_mismatch(target_expr, err))?;

                    Ok(())
                }
            }
        }
        (TyKind::RefTo(ref_), ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(variable))) => {
            // Get the variable type being accessed
            let var_ty = variable.lookup(db)
                .try_to_ty(db)
                .map_err(|err| ExprMismatch::unresolved_path(target_expr, err))?;

            // Check that the variable is a reference
            let deref = match var_ty.kind(db) {
                TyKind::RefTo(deref) => deref.to_ty(db),
                _ => {
                    return Err(ExprMismatch::expr_mismatch(target_expr, ty));
                }
            };

            // Check that the type of the variable is the same as the type the reference points to
            coerce_ty_with_ty(db, ref_.to_ty(db), deref)
                .map_err(|err| ExprMismatch::type_mismatch(target_expr, err))
        }
        _ => Err(ExprMismatch::expr_mismatch(target_expr, ty)),
    }
}

pub fn coerce_bool_with_ty<'db>(
    db: &'db dyn BaseDatabase,
    ty: Ty<'db>,
) -> Result<bool, AnalysisError<'db>> {
    match ty.kind(db) {
        TyKind::Simple(simple) => match simple {
            ElementarySpec::Bool => Ok(true),
            _ => Ok(false),
        },
        _ => Ok(false),
    }
}

pub fn coerce_bool_with_expr<'db>(
    db: &'db dyn BaseDatabase,
    target_expr: Expr<'db>,
) -> Result<bool, AnalysisError<'db>> {
    match target_expr.expr(db) {
        ExprKind::BooleanOperator { .. } => Ok(true),
        ExprKind::ComparisonOperator { .. } => Ok(true),
        _ => Ok(false),
    }
}
