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
        expressions::{
            expression::{Expr, FuncCall, ParamAssign, ParamAssignKind, VarAccess, VariableAccess},
            invocation::Invocation,
            statement::{Stmt, StmtKind},
        },
        interned::identifier::Ident,
        pous::{pou::Pou, variable::VariableDecl},
        semantic_index::{get_scope, semantic_index},
    },
    hir_ty::{
        func_call_resolver::ResolvedFuncCall,
        param_resolver::{ResolvedParamKind, resolve_parameters},
        signatures::LocalVariables,
        ty::{Ty, TyKind},
        ty_var_access_resolver::{LookUp, ResolvedAccess},
        walk::ResolvedPathKind,
    },
};

impl<'db> Check<'db> for Vec<Stmt<'db>> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>) {
        for stmt in self {
            stmt.check(db, errors);
        }
    }
}

impl<'db> Check<'db> for Stmt<'db> {
    fn check(&self, db: &'db dyn BaseDatabase, errors: &mut Vec<AnalysisError<'db>>) {
        match &self.stmt(db) {
            StmtKind::EmptyPathExpression(var) => {
                let resolved = var.lookup(db);
                if let Err(err) = resolved.fully_resolved(db) {
                    errors.push(err.into());
                }
                errors.push(StmtError::EmptyPathExpression { stmt: *self }.into());
            }
            StmtKind::If {
                then,
                else_,
                else_if,
                ..
            } => {
                then.as_ref().map(|then| then.check(db, errors));

                else_.as_ref().map(|else_| else_.check(db, errors));

                else_if.iter().for_each(|(_, s)| s.check(db, errors));
            }
            StmtKind::Assignment { var, target } => {
                if let Err(err) = check_assignment(db, *var, *target) {
                    errors.push(err);
                }
            }
            StmtKind::AssignmentAttempt { var, target } => {
                if let Err(err) = check_assignment(db, *var, *target) {
                    errors.push(err);
                }
            }
            StmtKind::FuncCall(call) => {
                if let Err(err) = check_func_call(db, call, errors) {
                    errors.push(err);
                }
            }
            StmtKind::For {
                control_variable,
                start,
                step,
                end,
                body,
            } => {
                if let Err(err) = check_for(db, *control_variable, *start, *end, *step, errors) {
                    errors.push(err);
                }
                body.check(db, errors);
            }
            StmtKind::While { condition, body } => {
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
            StmtKind::Repeat { condition, body } => match coerce_bool_with_expr(db, *condition) {
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
            },
            StmtKind::Case { .. } => {}
            _ => {}
        }
    }
}

fn check_assign_target<'db>(
    db: &'db dyn BaseDatabase,
    access: VariableAccess<'db>,
) -> Result<ResolvedAccess<'db>, AnalysisError<'db>> {
    let access = access.lookup(db);
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
        // ... unless this pou is a function with return type and is the actual pou being assigned
        ResolvedPathKind::Pou(pou)  => {
            if let Pou::Function(func) = pou.pou(db) {
                if func.return_type(db).is_some() {
                    return Ok(access);
                }
            }
            return Err(StmtError::AssignmentToDirectType { var: access }.into());
        }
        // Assigning a struct field is valid
        ResolvedPathKind::StructElement(_) => {}
        // Other cases are invalid
        ResolvedPathKind::Spec(_) | ResolvedPathKind::Method(_) => {
            return Err(StmtError::AssignmentToDirectType { var: access }.into());
        }
        _ => {
            return Err(StmtError::AssignmentToDirectType { var: access }.into());
        }
    }

    Ok(access)
}

fn check_assignment<'db>(
    db: &'db dyn BaseDatabase,
    access: VariableAccess<'db>,
    target: Expr<'db>,
) -> Result<Ty<'db>, AnalysisError<'db>> {
    let access = check_assign_target(db, access)?;

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
    fun_call: &'db FuncCall<'db>,
    errors: &mut Vec<AnalysisError<'db>>,
) -> Result<(), AnalysisError<'db>> {
    let fun_call = fun_call.resolve_func_call(db);
    let resolved = fun_call.target.fully_resolved(db)?;

    let variables = match resolved.kind {
        // A FB or CLASS declared in a variable section
        ResolvedPathKind::Variable(v) => match v.spec(db).to_ty(db).kind(db) {
            TyKind::FunctionBlock(f) => {
                check_parameters(db, fun_call.target, f, &fun_call.params, errors)
            }
            TyKind::Class(cl) => {
                check_parameters(db, fun_call.target, cl, &fun_call.params, errors)
            }
            _ => {
                return Err(StmtError::CallANonCallableType {
                    call: fun_call.target,
                }
                .into());
            }
        },
        // Direct FUNCTION call
        ResolvedPathKind::Pou(p) => match p.pou(db) {
            Pou::Function(f) => {
                check_parameters(db, fun_call.target, &p, &fun_call.params, errors);
            }
            _ => {
                return Err(StmtError::CallANonCallableType {
                    call: fun_call.target,
                }
                .into());
            }
        },
        // METHOD call
        ResolvedPathKind::Method(m) => {
            check_call_visibility(db, m.into(), fun_call.target.clone(), errors);
            check_parameters(db, fun_call.target.clone(), &m, &fun_call.params, errors);
        }
        _ => {
            return Err(StmtError::CallANonCallableType {
                call: fun_call.target,
            }
            .into());
        }
    };
    Ok(())
}


#[derive(Clone, Copy)]
enum FormalCall {
    Unset,
    Formal,
    NonFormal,
}

impl FormalCall {
    fn check_consistency(&mut self, kind: &ParamAssignKind) -> bool {
        match (*self, kind) {
            (
                FormalCall::Unset,
                ParamAssignKind::FormalInput { .. } | ParamAssignKind::FormalOutput { .. },
            ) => {
                *self = FormalCall::Formal;
                true
            }
            (FormalCall::Unset, ParamAssignKind::NonFormal { .. }) => {
                *self = FormalCall::NonFormal;
                true
            }
            (
                FormalCall::Formal,
                ParamAssignKind::FormalInput { .. } | ParamAssignKind::FormalOutput { .. },
            ) => true,
            (FormalCall::NonFormal, ParamAssignKind::NonFormal { .. }) => true,
            _ => false, // Mixed formal/non-formal
        }
    }
}

fn check_parameters<'db>(
    db: &'db dyn BaseDatabase,
    target: ResolvedAccess<'db>,
    signature: &impl LocalVariables<'db>,
    params: &Vec<ParamAssign<'db>>,
    errors: &mut Vec<AnalysisError<'db>>,
) {
    let mut format = FormalCall::Unset;
    let len: usize = signature.local_variables(db).len();
    let too_many_params = params.len() > len;
    if too_many_params {
        errors.push(
            StmtError::TooManyParameters {
                call: target.clone(),
                expected: len,
                found: params.len(),
            }
            .into(),
        );
    }

    let mut seen = FxHashMap::default();

    let parameters = resolve_parameters(db, signature, params);
    for parameter in parameters {
        if !format.check_consistency(&parameter.param_assign.kind(db)) {
            errors.push(
                StmtError::MixedFormalNonFormalParams {
                    call: target.clone(),
                }
                .into(),
            );
            break;
        }
        match parameter.kind {
            ResolvedParamKind::NonFormal {
                resolved_param,
                value,
            } => {
                match resolved_param {
                    Some(var) => {
                        if let Err(err) = coerce_ty_with_expr(db, var.spec(db).to_ty(db), value) {
                            errors.push(
                                StmtError::ParameterExprMismatch {
                                    expr: value,
                                    var: var,
                                    err,
                                }
                                .into(),
                            );
                        }
                    }
                    None => {
                        if too_many_params {
                            break;
                        }
                        // No more formal parameters
                        errors.push(
                            StmtError::UnknownNonFormalParam {
                                call: target.clone(),
                            }
                            .into(),
                        );
                    }
                }
            }
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
                    Some(var) => {
                        if let Err(err) = coerce_ty_with_expr(db, var.spec(db).to_ty(db), value) {
                            errors.push(
                                StmtError::ParameterExprMismatch {
                                    expr: value,
                                    var: var,
                                    err,
                                }
                                .into(),
                            );
                        }
                    }
                    None => errors.push(
                        StmtError::UnknownFormalInputParam {
                            call: target.clone(),
                            param: param,
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
                    Some(var) => {
                        let variable = match check_assign_target(db, variable) {
                            Ok(v) => v,
                            Err(err) => {
                                errors.push(err);
                                continue;
                            }
                        };
                        if let Err(err) = coerce_ty_with_ty(
                            db,
                            variable.try_to_ty(db).unwrap(),
                            var.spec(db).to_ty(db),
                        ) {
                            errors.push(
                                StmtError::ParameterTypeMismatch {
                                    param: param,
                                    var: variable,
                                    err,
                                }
                                .into(),
                            );
                        }
                    }
                    None => {
                        errors.push(
                            StmtError::UnknownFormalOutputParam {
                                call: target.clone(),
                                param: param,
                            }
                            .into(),
                        );
                        continue;
                    }
                }
            }
        }
    }
}

fn check_for<'db>(
    db: &'db dyn BaseDatabase,
    control_var: VariableAccess<'db>,
    start: Expr<'db>,
    end: Expr<'db>,
    step: Option<Expr<'db>>,
    errors: &mut Vec<AnalysisError<'db>>,
) -> Result<(), AnalysisError<'db>> {
    let control_var_ty = check_assignment(db, control_var, start)?;
    let control_var = control_var.lookup(db);

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
