use auto_lsp::default::db::BaseDatabase;
use indexmap::IndexMap;
use rustc_hash::FxHashMap;

use crate::{
    check::{
        check_semantic_index::Check,
        check_visibility::check_call_visibility,
        coerce::{coerce_bool_with_expr, coerce_ty_with_expr, coerce_ty_with_ty},
        errors::{analysis_error::AnalysisError, stmt::StmtError},
    },
    hir_def::{
        expressions::statement::Stmt,
        interned::identifier::Ident,
        pous::{pou::Pou, variable::VariableDecl},
    },
    hir_ty::{
        expr_resolver::ResolvedExpr,
        func_call_resolver::{ResolvedFuncCall, ResolvedParam, ResolvedParamKind},
        invocation_resolver::{ResolvedInvocationResult, ResolvedMethodKind},
        signatures::LocalVariables,
        stmt_resolver::{ResolvedStmt, ResolvedStmtKind, resolve_stmt},
        ty::{Ty, TyKind},
        ty_var_access_resolver::ResolvedAccess,
        walk::ResolvedPathKind,
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
        match &self.kind(db) {
            ResolvedStmtKind::EmptyPathExpression(var) => {
                if let Err(err) = var.fully_resolved(db) {
                    errors.push(err.into());
                }
                errors.push(StmtError::EmptyPathExpression { var: *var }.into());
            },
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
            ResolvedStmtKind::FuncCall(call) => {
                if let Err(err) = check_func_call(db, call, errors) {
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
                    Ok(is_bool) => {
                        if !is_bool {
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
            ResolvedStmtKind::Invocation(invocation) => {
                if let Err(err) = check_invocation(db, invocation, errors) {
                    errors.push(err);
                }
            }
            ResolvedStmtKind::Case {} => {}
            _ => {}
        }
    }
}

fn check_assignment<'db>(
    db: &'db dyn BaseDatabase,
    access: ResolvedAccess<'db>,
    target: ResolvedExpr<'db>,
) -> Result<Ty<'db>, AnalysisError<'db>> {
    // Variables in VAR_INPUT can not be mutated
    if access.is_var_input(db) {
        return Err(StmtError::AssignmentToInputVar { var: access }.into());
    }
    let resolved = access.fully_resolved(db)?;

    match resolved.kind {
        // Assigning a variable
        ResolvedPathKind::Variable(v) => {
            // A variable type could refer to a Pou (SpecKind::Target).
            // In this case we need to check we're assigning a correct type
            if v.spec(db).to_ty(db).is_pou(db) {
                return Err(StmtError::AssignmentToCallableType { var: access }.into());
            }
        }
        // Assigning to a direct method or pou is not allowed
        // ... unless this pou is a function with return type
        ResolvedPathKind::Pou(pou) => {
            if let Pou::Function(f) = pou.pou(db)
                && let Some(ret) = f.return_type(db)
            {
                // Check the return type
                return match coerce_ty_with_expr(db, ret.to_ty(db), target) {
                    Err(err) => Err(StmtError::AssignmentTypeMismatch { err }.into()),
                    Ok(()) => Ok(ret.to_ty(db)),
                };
            } else {
                return Err(StmtError::AssignmentToDirectType { var: access }.into());
            }
        }
        // Assiging a struct field is valid
        ResolvedPathKind::StructElement(st) => {}
        // Other cases are invalid
        ResolvedPathKind::Spec(_) | ResolvedPathKind::Method(_) => {
            return Err(StmtError::AssignmentToDirectType { var: access }.into());
        }
    }

    let access_type = match access.try_to_ty(db) {
        Ok(ty) => ty,
        Err(err) => return Err(StmtError::UnresolvedAssignmentTarget { var: access, err }.into()),
    };

    // Last, runs the type checker
    if let Err(err) = coerce_ty_with_expr(db, access_type, target) {
        return Err(AnalysisError::from(StmtError::AssignmentTypeMismatch {
            err,
        }));
    }

    Ok(access_type)
}

fn check_func_call<'db>(
    db: &'db dyn BaseDatabase,
    fun_call: &'db ResolvedFuncCall<'db>,
    errors: &mut Vec<AnalysisError<'db>>,
) -> Result<(), AnalysisError<'db>> {
    let resolved = fun_call.target.fully_resolved(db)?;

    let variables = match resolved.kind {
        // A FB or CLASS declared in a variable section
        ResolvedPathKind::Variable(v) => match v.spec(db).to_ty(db).kind(db) {
            TyKind::FunctionBlock(f) => f.local_variables(db),
            TyKind::Class(cl) => cl.local_variables(db),
            _ => {
                return Err(StmtError::CallANonCallableType {
                    call: fun_call.target,
                }
                .into());
            }
        },
        // Direct FUNCTION call
        ResolvedPathKind::Pou(p) => match p.pou(db) {
            Pou::Function(f) => p.local_variables(db),
            _ => {
                return Err(StmtError::CallANonCallableType {
                    call: fun_call.target,
                }
                .into());
            }
        },
        // METHOD call
        ResolvedPathKind::Method(m) => {
            check_call_visibility(db, m.into(), &fun_call.target, errors);
            m.local_variables(db)
        }
        _ => {
            return Err(StmtError::CallANonCallableType {
                call: fun_call.target,
            }
            .into());
        }
    };

    check_parameters(db, fun_call.target, variables, &fun_call.params, errors);
    Ok(())
}

fn check_invocation<'db>(
    db: &'db dyn BaseDatabase,
    invocation: &'db ResolvedInvocationResult<'db>,
    errors: &mut Vec<AnalysisError<'db>>,
) -> Result<(), AnalysisError<'db>> {
    match &invocation.target.kind {
        ResolvedMethodKind::Unresolved(err) => {
            return Err(err.clone().into());
        }
        ResolvedMethodKind::InheritedMethod { target, method }
        | ResolvedMethodKind::DeclaredMethod { target, method } => {
            // Check visibility
            check_call_visibility(db, (*method).into(), &invocation.target.invocation, errors);
            check_parameters(
                db,
                *target,
                method.local_variables(db),
                &invocation.params,
                errors,
            );
        }
        ResolvedMethodKind::FunctionBlockBody { target } => {
            let ty_target = target
                .try_to_ty(db)
                .map_err(|err| StmtError::UnresolvedFuncCall { call: *target })?;
        }
    }

    Ok(())
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

fn check_parameters<'db>(
    db: &'db dyn BaseDatabase,
    target: ResolvedAccess<'db>,
    signature: &IndexMap<Ident, VariableDecl<'db>>,
    params: &Vec<ResolvedParam<'db>>,
    errors: &mut Vec<AnalysisError<'db>>,
) {
    let mut format = FormalCall::Unset;

    let too_many_params = params.len() > signature.len();
    if too_many_params {
        errors.push(
            StmtError::TooManyParameters {
                call: target,
                expected: signature.len(),
                found: params.len(),
            }
            .into(),
        );
    }

    let mut seen = FxHashMap::default();

    for p in params.iter() {
        if !format.check_consistency(&p.kind(db)) {
            errors.push(StmtError::MixedFormalNonFormalParams { call: target }.into());
            break;
        }

        match p.kind(db) {
            ResolvedParamKind::NonFormal {
                resolved_param,
                value,
            } => match resolved_param {
                Some(other_param) => match other_param.try_to_ty(db) {
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
                        errors.push(StmtError::UnknownNonFormalParam { call: target }.into());
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
                    Some(other_param) => match other_param.try_to_ty(db) {
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
                            call: target,
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
                    Some(other_param) => match other_param.try_to_ty(db) {
                        Ok(p_ty) => match variable.try_to_ty(db) {
                            Ok(var_ty) => {
                                if variable.is_var_input(db) {
                                    errors.push(
                                        StmtError::AssignmentToInputVar { var: variable }.into(),
                                    );
                                } else if variable.as_var(db).is_none() {
                                    errors.push(
                                        StmtError::AssignmentToDirectType { var: variable }.into(),
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
                            call: target,
                            param,
                        }
                        .into(),
                    ),
                }
            }
        }
    }
}

fn check_for<'db>(
    db: &'db dyn BaseDatabase,
    control_var: ResolvedAccess<'db>,
    start: ResolvedExpr<'db>,
    end: ResolvedExpr<'db>,
    step: Option<ResolvedExpr<'db>>,
    errors: &mut Vec<AnalysisError<'db>>,
) -> Result<(), AnalysisError<'db>> {
    let control_var_ty = check_assignment(db, control_var, start)?;

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
