use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

use crate::{
    check::{
        check_semantic_index::Check,
        coerce::{coerce_bool_with_expr, coerce_ty_with_expr, coerce_ty_with_ty},
        errors::{analysis_error::AnalysisError, stmt::StmtError},
    },
    hir_def::{expressions::statement::Stmt, pous::pou::Pou},
    hir_ty::{
        expr_resolver::ResolvedExpr,
        func_call_resolver::{ResolvedParam, ResolvedParamKind},
        stmt_resolver::{ResolvedStmt, ResolvedStmtKind, resolve_stmt},
        ty::TyDef,
        ty_path_expr_resolver::ResolvedPathResult,
        ty_var_access_resolver::ResolvedVarResult,
    },
};

impl<'db> Check<'db> for Vec<Stmt<'db>> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>) {
        for stmt in self {
            resolve_stmt(db, *stmt).check(db, errors);
        }
    }
}

impl<'db> Check<'db> for Vec<ResolvedStmt<'db>> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>) {
        for stmt in self {
            stmt.check(db, errors);
        }
    }
}

impl<'db> Check<'db> for ResolvedStmt<'db> {
    fn check(&self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>) {
        match self.kind(db) {
            ResolvedStmtKind::If {
                then,
                else_,
                else_if,
                ..
            } => {
                then.check(db, errors);

                else_.check(db, errors);

                else_if.iter().for_each(|(_, s)| s.check(db, errors));
            }
            ResolvedStmtKind::Assignment { var, target } => {
                if let Err(err) = check_assignment(db, *var, *target) {
                    errors.push(err);
                }
            }
            ResolvedStmtKind::AssignmentAttempt { var, target } => {
                if let Err(err) = check_assignment(db, *var, *target) {
                    errors.push(err);
                }
            }
            ResolvedStmtKind::FuncCall { target, params } => {
                if let Err(err) = check_func_call(db, *self, *target, params, errors) {
                    errors.push(err);
                }
            }
            ResolvedStmtKind::For {
                control_var,
                start,
                step,
                end,
                body,
            } => {
                if let Err(err) = check_for(db, *control_var, *start, *end, *step, errors) {
                    errors.push(err);
                }
                body.check(db, errors);
            }
            ResolvedStmtKind::While { condition, body } => {
                match coerce_bool_with_expr(db, *condition) {
                    Ok(is_valid) => {
                        if !is_valid {
                            errors.push(
                                StmtError::WhileConditionIsNotABool {
                                    condition: *condition,
                                }
                                .into(),
                            );
                        }
                    }
                    Err(err) => errors.push(err),
                }

                body.check(db, errors);
            }
            ResolvedStmtKind::Repeat { condition, body } => {
                match coerce_bool_with_expr(db, *condition) {
                    Ok(is_valid) => {
                        if !is_valid {
                            errors.push(
                                StmtError::RepeatConditionIsNotABool {
                                    condition: *condition,
                                }
                                .into(),
                            );
                        }
                    }
                    Err(err) => errors.push(err),
                }
            }
            ResolvedStmtKind::Case {} => {}
            ResolvedStmtKind::Invocation {} => {
                // todo
            }
            _ => {}
        }
    }
}

fn check_assignment<'db>(
    db: &'db dyn BaseDatabase,
    var: ResolvedVarResult<'db>,
    target: ResolvedExpr<'db>,
) -> Result<(), AnalysisError<'db>> {
    let ty_var = var
        .ty(db)
        .map_err(|err| StmtError::UnresolvedAssignmentTarget { var, err })?;

    // Variables in VAR_INPUT can not be mutated
    if var.is_input(db) {
        return Err(StmtError::AssignmentToInputVar { var, ty: ty_var }.into());
    }

    // Direct type
    if !ty_var.is_variable(db) {
        return Err(StmtError::AssignementToDirectType { var, ty: ty_var }.into());
    }

    // POUs can not be mutated
    // Except for functions if they have a return type
    if ty_var.is_callable(db) {
        if let TyDef::Pou(pou) = ty_var.def(db)
            && let Pou::Function(func) = pou.pou(db)
            && let Some(ret) = ty_var.has_return_type(db)
        {
            return coerce_ty_with_expr(db, ret, target)
                .map_err(|err| StmtError::AssignmentTypeMismatch { err }.into());
        } else {
            return Err(StmtError::AssignementToCallableType { var, ty: ty_var }.into());
        }
    }

    coerce_ty_with_expr(db, ty_var, target)
        .map_err(|err| StmtError::AssignmentTypeMismatch { err }.into())
}

#[derive(Clone, Copy)]
enum FormalCall {
    Unset,
    Formal,
    NonFormal,
}

impl FormalCall {
    fn check_consistency(&mut self, kind: &ResolvedParamKind) -> bool {
        match (*self, kind) {
            (
                FormalCall::Unset,
                ResolvedParamKind::FormalInput { .. } | ResolvedParamKind::FormalOutput { .. },
            ) => {
                *self = FormalCall::Formal;
                true
            }
            (FormalCall::Unset, ResolvedParamKind::NonFormal { .. }) => {
                *self = FormalCall::NonFormal;
                true
            }
            (
                FormalCall::Formal,
                ResolvedParamKind::FormalInput { .. } | ResolvedParamKind::FormalOutput { .. },
            ) => true,
            (FormalCall::NonFormal, ResolvedParamKind::NonFormal { .. }) => true,
            _ => false, // Mixed formal/non-formal
        }
    }
}

fn check_func_call<'db>(
    db: &'db dyn BaseDatabase,
    stmt: ResolvedStmt<'db>,
    target: ResolvedPathResult<'db>,
    params: &Vec<ResolvedParam<'db>>,
    errors: &mut Vec<AnalysisError<'db>>,
) -> Result<(), AnalysisError<'db>> {
    let ty_target = target
        .ty(db)
        .map_err(|err| StmtError::UnresolvedFuncCall { stmt })?;

    if !ty_target.is_callable(db) {
        return Err(StmtError::CallANonCallableType {
            ty: ty_target,
            var: target,
        }
        .into());
    }

    if let Some(ret) = ty_target.has_return_type(db) {
        errors.push(
            StmtError::UnusedReturnType {
                ty: ty_target,
                var: target,
                ret,
            }
            .into(),
        )
    }

    // SAFETY: unwrap is safe because is_callable was checked before
    // // todo: check if Default can not be implemented
    let signature = ty_target.to_signature(db).unwrap();
    let mut format = FormalCall::Unset;

    let too_many_params = params.len() > signature.length();
    if too_many_params {
        errors.push(
            StmtError::TooManyParameters {
                stmt,
                target: ty_target,
                expected: signature.length(),
                found: params.len(),
            }
            .into(),
        );
    }

    let mut seen = FxHashMap::default();

    for p in params.iter() {
        if !format.check_consistency(&p.kind(db)) {
            errors.push(
                StmtError::MixedFormalNonFormalParams {
                    ty: ty_target,
                    var: target,
                }
                .into(),
            );
            break;
        }

        match p.kind(db) {
            ResolvedParamKind::NonFormal {
                resolved_param,
                value,
            } => match resolved_param {
                Some(other_param) => match other_param.ty(db) {
                    Ok(p_ty) => {
                        let _ = coerce_ty_with_expr(db, p_ty, value).map_err(|err| {
                            errors.push(
                                StmtError::ParameterExprMismatch {
                                    expr: value,
                                    var: other_param,
                                    err,
                                }
                                .into(),
                            )
                        });
                    }
                    Err(err) => errors.push(
                        StmtError::UnresolvedNonFormalParam {
                            var: other_param,
                            err,
                        }
                        .into(),
                    ),
                },
                None => {
                    if !too_many_params {
                        errors.push(
                            StmtError::UnknownNonFormalParam {
                                ty: ty_target,
                                var: target,
                            }
                            .into(),
                        );
                    }
                }
            },
            ResolvedParamKind::FormalInput {
                param,
                resolved_param,
                value,
            } => {
                match seen.get(&param.ident) {
                    None => {
                        seen.insert(param.ident, param);
                    }
                    Some(prev) => errors.push(
                        StmtError::DuplicateParameter {
                            param1: param,
                            param2: *prev,
                        }
                        .into(),
                    ),
                }
                match resolved_param {
                    Some(other_param) => match other_param.ty(db) {
                        Ok(p_ty) => {
                            let _ = coerce_ty_with_expr(db, p_ty, value).map_err(|err| {
                                errors.push(
                                    StmtError::ParameterExprMismatch {
                                        expr: value,
                                        var: other_param,
                                        err,
                                    }
                                    .into(),
                                )
                            });
                        }
                        Err(err) => errors.push(
                            StmtError::UnresolvedInputParam {
                                var: other_param,
                                err,
                            }
                            .into(),
                        ),
                    },
                    None => errors.push(
                        StmtError::UnknownFormalInputParam {
                            ty: ty_target,
                            var: target,
                            param,
                        }
                        .into(),
                    ),
                }
            }
            ResolvedParamKind::FormalOutput {
                not,
                param,
                resolved_param,
                variable,
            } => {
                match seen.get(&param.ident) {
                    None => {
                        seen.insert(param.ident, param);
                    }
                    Some(prev) => errors.push(
                        StmtError::DuplicateParameter {
                            param1: param,
                            param2: *prev,
                        }
                        .into(),
                    ),
                }
                match resolved_param {
                    Some(other_param) => match other_param.ty(db) {
                        Ok(p_ty) => match variable.ty(db) {
                            Ok(var_ty) => {
                                if variable.is_input(db) {
                                    errors.push(
                                        StmtError::AssignmentToInputVar {
                                            ty: var_ty,
                                            var: variable,
                                        }
                                        .into(),
                                    );
                                } else if !var_ty.is_variable(db) {
                                    errors.push(
                                        StmtError::AssignementToDirectType {
                                            ty: var_ty,
                                            var: variable,
                                        }
                                        .into(),
                                    );
                                } else {
                                    let _ = coerce_ty_with_ty(db, p_ty, var_ty).map_err(|err| {
                                        errors.push(
                                            StmtError::ParameterTypeMismatch {
                                                param,
                                                var: variable,
                                                err,
                                            }
                                            .into(),
                                        )
                                    });
                                }
                            }
                            Err(err) => errors.push(
                                StmtError::UnresolvedOutputParamTarget {
                                    var: other_param,
                                    err,
                                }
                                .into(),
                            ),
                        },
                        Err(err) => errors.push(
                            StmtError::UnresolvedOutputParam {
                                var: other_param,
                                err,
                            }
                            .into(),
                        ),
                    },
                    None => errors.push(
                        StmtError::UnknownFormalOutputParam {
                            ty: ty_target,
                            var: target,
                            param,
                        }
                        .into(),
                    ),
                }
            }
        }
    }

    Ok(())
}

fn check_for<'db>(
    db: &'db dyn BaseDatabase,
    control_var: ResolvedVarResult<'db>,
    start: ResolvedExpr<'db>,
    end: ResolvedExpr<'db>,
    step: Option<ResolvedExpr<'db>>,
    errors: &mut Vec<AnalysisError<'db>>,
) -> Result<(), AnalysisError<'db>> {
    let control_var_ty = control_var
        .ty(db)
        .map_err(|err| StmtError::UnresolvedControlVar {
            control: control_var,
            err,
        })?;

    // Variables in VAR_INPUT can not be mutated
    if control_var.is_input(db) {
        return Err(StmtError::AssignmentToInputVar {
            var: control_var,
            ty: control_var_ty,
        }
        .into());
    }

    // Direct type
    if !control_var_ty.is_variable(db) {
        return Err(StmtError::AssignementToDirectType {
            var: control_var,
            ty: control_var_ty,
        }
        .into());
    }

    // POUs can not be mutated
    if control_var_ty.is_callable(db) {
        return Err(StmtError::AssignementToCallableType {
            var: control_var,
            ty: control_var_ty,
        }
        .into());
    }

    // Check start value
    if let Err(err) = coerce_ty_with_expr(db, control_var_ty, start) {
        errors.push(StmtError::ForLoopStartTypeMismatch { start, err }.into());
    }

    // Check end value
    if let Err(err) = coerce_ty_with_expr(db, control_var_ty, end) {
        errors.push(StmtError::ForLoopEndTypeMismatch { end, err }.into());
    }

    // Check step value
    if let Some(step) = step {
        if let Err(err) = coerce_ty_with_expr(db, control_var_ty, step) {
            errors.push(StmtError::ForLoopStepTypeMismatch { step, err }.into());
        }
    }

    Ok(())
}
