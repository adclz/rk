use auto_lsp::default::db::BaseDatabase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    check::{
        check_semantic_index::Check,
        check_visibility::check_call_visibility,
        coerce::{coerce_bool_with_expr, coerce_ty_with_expr, coerce_ty_with_ty},
        errors::{
            analysis_error::{AnalysisError, ToIdeDiagnostic},
            stmt::StmtError,
        },
    },
    hir_def::{
        expressions::{
            expression::{Expr, FuncCall, ParamAssign, ParamAssignKind, VariableAccess},
            statement::{Stmt, StmtKind},
        },
        pous::pou::Pou,
        scope::ScopeId,
    },
    hir_ty::{
        param_resolver::{ResolvedParamKind, resolve_parameters},
        ty::Ty,
        ty_var_access_resolver::{LookUp, ResolvedAccess},
        walk::ResolvedPathKind,
    },
};

/*impl<'db> Check<'db> for Vec<Stmt<'db>> {
    fn check(&'db self, db: &'db dyn BaseDatabase, errors: &mut Vec<IdeDiagnostic>) {
        for stmt in self {
            stmt.check(db, errors);
        }
    }
}

impl<'db> Check<'db> for Stmt<'db> {
    fn check(&self, db: &'db dyn BaseDatabase, errors: &mut Vec<IdeDiagnostic>) {
        match &self.stmt(db) {
            StmtKind::EmptyPathExpression(var) => {
                let resolved = var.lookup(db);
                if let Err(err) = resolved.fully_resolved(db) {
                    errors.push(err.to_diagnostic(db));
                }
                errors.push(StmtError::EmptyPathExpression { stmt: *self }.to_diagnostic(db));
            }
            StmtKind::If {
                then,
                else_,
                else_if,
                ..
            } => {
                if let Some(then) = then.as_ref() {
                    then.check(db, errors)
                }

                if let Some(else_) = else_.as_ref() {
                    else_.check(db, errors)
                }

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
                                .to_diagnostic(db),
                            );
                        }
                    }
                    Err(err) => errors.push(err.to_diagnostic(db)),
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
                            .to_diagnostic(db),
                        );
                    }
                }
                Err(err) => errors.push(err.to_diagnostic(db)),
            },
            StmtKind::Case { .. } => {}
            _ => {}
        }
    }
}

fn check_assign_target<'db>(
    db: &'db dyn BaseDatabase,
    access: VariableAccess<'db>,
) -> Result<ResolvedAccess<'db>, IdeDiagnostic> {
    let access = access.lookup(db);
    // Variables in VAR_INPUT can not be mutated
    if access.is_var_input(db) {
        return Err(StmtError::AssignmentToInputVar { var: access }.to_diagnostic(db));
    }
    let resolved = access
        .fully_resolved(db)
        .map_err(|err| err.to_diagnostic(db))?;

    match resolved.kind {
        // Assigning a variable
        ResolvedPathKind::Variable(v) => {
            // A variable type could refer to a Pou (SpecKind::Target).
            // In this case we need to check we're assigning a correct type
            if v.spec(db).to_ty(db).is_pou(db) {
                return Err(StmtError::AssignmentToCallableType { var: access }.to_diagnostic(db));
            }
        }
        // Assigning to a direct method or pou is not allowed
        // ... unless this pou is a function with return type and is the actual pou being assigned
        ResolvedPathKind::Pou(pou) => {
            if let Pou::Function(func) = pou.pou(db) {
                if func.return_type(db).is_some() {
                    return Ok(access);
                }
            }
            return Err(StmtError::AssignmentToDirectType { var: access }.to_diagnostic(db));
        }
        // Assigning a method is ok as long as it has a return type
        ResolvedPathKind::Method(m) => {
            if m.return_type(db).is_some() {
                return Ok(access);
            }
            return Err(StmtError::AssignmentToDirectType { var: access }.to_diagnostic(db));
        }
        // Assigning a struct field is valid
        ResolvedPathKind::StructElement(_) => {}
        // Other cases are invalid
        ResolvedPathKind::Spec(_) => {
            return Err(StmtError::AssignmentToDirectType { var: access }.to_diagnostic(db));
        }
        _ => {
            return Err(StmtError::AssignmentToDirectType { var: access }.to_diagnostic(db));
        }
    }

    Ok(access)
}

fn check_assignment<'db>(
    db: &'db dyn BaseDatabase,
    access: VariableAccess<'db>,
    target: Expr<'db>,
) -> Result<Ty<'db>, IdeDiagnostic> {
    let access = check_assign_target(db, access)?;

    let access_type = match access.try_to_ty(db) {
        Ok(ty) => ty,
        Err(err) => {
            return Err(StmtError::UnresolvedAssignmentTarget { var: access, err }.to_diagnostic(db));
        }
    };

    // Last, runs the type checker
    if let Err(err) = coerce_ty_with_expr(db, access_type, target) {
        return Err(
            AnalysisError::from(StmtError::AssignmentTypeMismatch { err }).to_diagnostic(db),
        );
    }

    Ok(access_type)
}

fn check_func_call<'db>(
    db: &'db dyn BaseDatabase,
    fun_call: &'db FuncCall<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) -> Result<(), IdeDiagnostic> {
    let fun_call = fun_call.resolve_func_call(db);
    let resolved = fun_call
        .target
        .fully_resolved(db)
        .map_err(|err| err.to_diagnostic(db))?;
    if !resolved.is_callable(db) {
        return Err(StmtError::CallANonCallableType {
            call: fun_call.target,
        }
        .to_diagnostic(db));
    }
    check_call_visibility(db, &fun_call.target, &resolved, errors);
    check_parameters(
        db,
        fun_call.target.clone(),
        resolved.target_scope_id(db),
        &fun_call.params,
        errors,
    );
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
    signature: ScopeId<'db>,
    params: &Vec<ParamAssign<'db>>,
    errors: &mut Vec<IdeDiagnostic>,
) {
    let mut format = FormalCall::Unset;
    let len: usize = signature.def_map(db).local_variables.len();
    let too_many_params = params.len() > len;
    if too_many_params {
        errors.push(
            StmtError::TooManyParameters {
                call: target.clone(),
                expected: len,
                found: params.len(),
            }
            .to_diagnostic(db),
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
                .to_diagnostic(db),
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
                                    var,
                                    err,
                                }
                                .to_diagnostic(db),
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
                            .to_diagnostic(db),
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
                        .to_diagnostic(db),
                    ),
                }
                match resolved_param {
                    Some(var) => {
                        if let Err(err) = coerce_ty_with_expr(db, var.spec(db).to_ty(db), value) {
                            errors.push(
                                StmtError::ParameterExprMismatch {
                                    expr: value,
                                    var,
                                    err,
                                }
                                .to_diagnostic(db),
                            );
                        }
                    }
                    None => errors.push(
                        StmtError::UnknownFormalInputParam {
                            call: target.clone(),
                            param,
                        }
                        .to_diagnostic(db),
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
                        .to_diagnostic(db),
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
                                    param,
                                    var: variable,
                                    err,
                                }
                                .to_diagnostic(db),
                            );
                        }
                    }
                    None => {
                        errors.push(
                            StmtError::UnknownFormalOutputParam {
                                call: target.clone(),
                                param,
                            }
                            .to_diagnostic(db),
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
    errors: &mut Vec<IdeDiagnostic>,
) -> Result<(), IdeDiagnostic> {
    let control_var_ty = check_assignment(db, control_var, start)?;
    let control_var = control_var.lookup(db);

    // Check start value
    if let Err(err) = coerce_ty_with_expr(db, control_var_ty, start) {
        errors.push(StmtError::ForLoopStartTypeMismatch { start, err }.to_diagnostic(db));
    }

    // Check end value
    if let Err(err) = coerce_ty_with_expr(db, control_var_ty, end) {
        errors.push(StmtError::ForLoopEndTypeMismatch { end, err }.to_diagnostic(db));
    }

    // Check step value
    if let Some(step) = step {
        if let Err(err) = coerce_ty_with_expr(db, control_var_ty, step) {
            errors.push(StmtError::ForLoopStepTypeMismatch { step, err }.to_diagnostic(db));
        }
    }

    Ok(())
}
*/