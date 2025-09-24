use auto_lsp::default::db::BaseDatabase;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    check::{
        check_semantic_index::Check,
        coerce::{coerce_ty_with_expr, coerce_ty_with_ty},
        errors::{coerce::CoerceError, sem_errors::AnalysisError, stmt::StmtError},
    },
    hir_def::expressions::statement::Stmt,
    hir_ty::{
        TyInfo,
        expr_resolver::ResolvedExpr,
        stmt_resolver::{
            ResolvedParam, ResolvedParamKind, ResolvedStmt, ResolvedStmtKind, resolve_stmt,
        },
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
            ResolvedStmtKind::For { body, .. } => {
                body.check(db, errors);
            }
            ResolvedStmtKind::While { body, .. } => {
                body.check(db, errors);
            }
            ResolvedStmtKind::Repeat { body, .. } => {
                body.check(db, errors);
            }
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
            ResolvedStmtKind::FuncCall { target, params } => {
                if let Err(err) = check_func_call(db, *self, *target, params, errors) {
                    errors.push(err);
                }
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
    let ty_var = var.ty(db)?;

    // Variables in VAR_INPUT can not be mutated
    if var.is_input(db) {
        return Err(StmtError::AssignmentToInputVar { var, ty: ty_var }.into());
    }

    // Direct type
    if !ty_var.is_variable(db) {
        return Err(StmtError::AssignementToDirectType { var, ty: ty_var }.into());
    }

    // POUs can not be mutated
    if ty_var.is_callable(db) {
        return Err(StmtError::AssignementToCallableType { var, ty: ty_var }.into());
    }

    coerce_ty_with_expr(db, ty_var, target)
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
    let ty_target = target.ty(db)?;
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
                        let _ =
                            coerce_ty_with_expr(db, p_ty, value).map_err(|err| errors.push(err));
                    }
                    Err(err) => errors.push(err),
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
                            let _ = coerce_ty_with_expr(db, p_ty, value)
                                .map_err(|err| errors.push(err));
                        }
                        Err(err) => errors.push(err),
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
                                            CoerceError::new_param_type_mismatch(param, err).into(),
                                        )
                                    });
                                }
                            }
                            Err(err) => errors.push(err),
                        },
                        Err(err) => errors.push(err),
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
