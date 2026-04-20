use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::check::errors::e1_duplicates::DuplicateError;
use crate::check::errors::e3_type::TypeError;
use crate::check::errors::e10_control_flow::ControlFlowError;
use crate::hir_def::expressions::expression::{Expr, ParamAssign};
use crate::hir_def::interned::identifier::Ident;
use crate::hir_def::pous::variable::VariableDecl;
use crate::{
    CallSite,
    check::errors::{ToIdeDiagnostic, e2_resolve::ResolveError},
    hir_def::expressions::expression::{FuncCall, ParamAssignKind},
    hir_ty::{
        body::BodyInferenceResult,
        infer::expr::InferExprCtx,
        resolver::Resolver,
        ty::{CallableType, Type},
    },
};

pub fn resolve_func_call<'db>(
    db: &'db dyn WorkspaceDataBase,
    resolver: Resolver<'db>,
    func_call: FuncCall<'db>,
    ctx: &mut BodyInferenceResult<'db>,
) {
    resolver.resolve_begin_path_expr(db, func_call.path(db), None, ctx);

    // If a prior resolution (e.g. during generic inference) already marked this path
    // as CallableType, unwrap it back to the original function/fb/method type.
    // CallableType.normalize() returns the *return type*, which would cause
    // as_callable() to fail on re-entry.
    if let Some(path_expr) = func_call.path(db).expr(db)
        && let Some(Type::CallableType(c)) = ctx.type_of_path_expr.get(&path_expr).copied()
    {
        let original = c.inner_callable();
        ctx.type_of_path_expr.insert(path_expr, original);
    }

    let access_typ = ctx.get_type_of_begin_path_expr(db, func_call.path(db));

    let target_typ = ctx
        .type_of_begin_expr_with_adjustments(db, func_call.path(db))
        .normalize(db);

    if access_typ.is_never() || target_typ.is_never() {
        return;
    }

    // FUNCTION_BLOCKs can only be called if they are variables
    if !access_typ.is_variable() && target_typ.is_fb() {
        ctx.errors.push(
            ControlFlowError::CallNonCallableType {
                typ: target_typ,
                func_call,
            }
            .to_diagnostic(db),
        );
        return;
    }

    let callable = match target_typ.as_callable(db) {
        Some(callable) => callable,
        None => {
            ctx.errors.push(
                ControlFlowError::CallNonCallableType {
                    typ: target_typ,
                    func_call,
                }
                .to_diagnostic(db),
            );
            return;
        }
    };

    // func call requires the type to be a [`CallableType`] otherwise the coercion layer will
    // assume we are calling a non-callable type
    if let Some(expr) = func_call.path(db).expr(db) {
        ctx.type_of_path_expr
            .insert(expr, Type::CallableType(callable));
    }

    let len = func_call.params(db).len();
    let has_variadic = callable
        .def_map(db)
        .local_variables
        .values()
        .any(|v| v.variadic(db));

    if !has_variadic && len > callable.var_len_params(db) {
        ctx.errors.push(
            ResolveError::IncorrectNumberOfParameters {
                expected: callable.var_len_params(db),
                actual: len,
                func_call,
                callable,
            }
            .to_diagnostic(db),
        );
    }

    // Resolve parameter matching (shared with {case} pragma validation)
    let matches = resolve_params(db, func_call.params(db), callable, &mut ctx.errors);

    // Apply coercion and body-level checks on matched parameters
    for m in &matches {
        match m {
            ParamMatch::Matched(param, var) => {
                apply_param_coercion(db, resolver, callable, *param, *var, ctx);
            }
            ParamMatch::Variadic(param, var, pos) => {
                ctx.variadic_position.insert(*param, *pos);
                apply_param_coercion(db, resolver, callable, *param, *var, ctx);
            }
            ParamMatch::Error => {}
        }
    }
}

fn apply_param_coercion<'db>(
    db: &'db dyn WorkspaceDataBase,
    resolver: Resolver<'db>,
    callable: CallableType<'db>,
    param: ParamAssign<'db>,
    var: VariableDecl<'db>,
    ctx: &mut BodyInferenceResult<'db>,
) {
    match param.kind(db) {
        ParamAssignKind::NonFormal { value } => {
            coerce_with_var_target(db, resolver, value, var, ctx);

            if (var.is_in_out(db) || var.is_output(db)) && ctx.is_constant_type(db, value) {
                ctx.errors.push(
                    ControlFlowError::AssignToConstant {
                        access: CallSite::from_scoped(db, &value),
                    }
                    .to_diagnostic(db),
                );
            }

            if var.is_output(db) {
                ctx.errors.push(
                    ResolveError::OutputParameterUsedAsInput {
                        func: callable,
                        var,
                        expr: value,
                        param: 0,
                    }
                    .to_diagnostic(db),
                );
            }
            ctx.variable_of_param.insert(param, var);
        }
        ParamAssignKind::FormalInput { value, .. } => {
            coerce_with_var_target(db, resolver, value, var, ctx);

            if (var.is_in_out(db) || var.is_output(db)) && ctx.is_constant_type(db, value) {
                ctx.errors.push(
                    ControlFlowError::AssignToConstant {
                        access: CallSite::from_scoped(db, &value),
                    }
                    .to_diagnostic(db),
                );
            }
            ctx.variable_of_param.insert(param, var);
        }
        ParamAssignKind::FormalOutput { variable, .. } => {
            let lhs_typ = Type::new_var(db, var);

            resolver.resolve_variable_access(db, variable, ctx);
            let call_site = CallSite::from_scoped(db, &variable);
            let rhs_typ = ctx.type_of_variable_access_with_adjustments(db, variable);

            if (var.is_in_out(db) || var.is_output(db)) && ctx.is_constant_access(db, variable) {
                ctx.errors.push(
                    ControlFlowError::AssignToConstant { access: call_site }.to_diagnostic(db),
                );
            }

            rhs_typ.check_assignable(db, call_site, ctx);

            lhs_typ
                .coerce_with_type(db, rhs_typ, None, resolver)
                .map_err(|err| {
                    ctx.errors
                        .push(err.into_non_assignable(db, rhs_typ, call_site))
                })
                .ok();

            ctx.variable_of_param.insert(param, var);
        }
    }
}

fn coerce_with_var_target<'db>(
    db: &'db dyn WorkspaceDataBase,
    resolver: Resolver<'db>,
    expr: Expr<'db>,
    var: VariableDecl<'db>,
    ctx: &mut BodyInferenceResult<'db>,
) {
    let mut caller_infer_ctx = InferExprCtx::new(resolver);
    // Only resolve if not already resolved (e.g. by infer_generic_types_from_args).
    // Re-resolving causes stale adjustments to corrupt the walk.
    if !ctx.type_of_expr.contains_key(&expr) {
        caller_infer_ctx.resolve_expr(db, expr, ctx);
    }
    caller_infer_ctx.check_expr(db, expr, ctx);

    if let Err(e) = caller_infer_ctx.coerce_var_decl_with_expr(db, var, expr, ctx) {
        let base_target = Type::new_var(db, var);
        ctx.errors.push(
            TypeError::NotAssignable {
                base_target,
                lhs: e.expected,
                rhs: e.actual,
                adjustment: e.adjustment,
                expr: CallSite::from_scoped(db, &expr),
            }
            .to_diagnostic(db),
        );
    }
}

/// Result of resolving a single parameter against a callable's signature.
pub enum ParamMatch<'db> {
    /// Positional or named param matched a variable declaration.
    Matched(ParamAssign<'db>, VariableDecl<'db>),
    /// Matched a variadic parameter, with the variadic position (1-indexed).
    Variadic(ParamAssign<'db>, VariableDecl<'db>, usize),
    /// No match - error already emitted.
    Error,
}

/// Resolve a list of parameters against a callable's signature.
///
/// Shared matching logic used by both function calls and {case} pragmas.
/// Handles positional/named param resolution, variadic parameters, duplicate
/// detection, and emits errors for unknown params.
pub fn resolve_params<'db>(
    db: &'db dyn WorkspaceDataBase,
    params: &[ParamAssign<'db>],
    callable: CallableType<'db>,
    errors: &mut Vec<IdeDiagnostic>,
) -> Vec<ParamMatch<'db>> {
    let mut results = vec![];
    let mut seen: FxHashMap<Ident, ParamAssign<'db>> = FxHashMap::default();
    let mut formal_idx = 0;
    let mut variadic_count = 0;

    let has_variadic = callable
        .def_map(db)
        .local_variables
        .values()
        .any(|v| v.variadic(db));

    // Pre-collect named parameter idents so positional args skip them
    let named_params: FxHashSet<_> = params
        .iter()
        .filter_map(|p| match p.kind(db) {
            ParamAssignKind::FormalInput { param, .. } => Some(param.ident),
            ParamAssignKind::FormalOutput { param, .. } => Some(param.ident),
            _ => None,
        })
        .collect();

    for parameter in params {
        match parameter.kind(db) {
            ParamAssignKind::NonFormal { value } => {
                // Skip parameters already filled by named arguments
                let def_map = callable.def_map(db);
                while formal_idx < def_map.local_variables.len() {
                    if let Some((name, _)) = def_map.local_variables.get_index(formal_idx)
                        && named_params.contains(name)
                    {
                        formal_idx += 1;
                        continue;
                    }
                    break;
                }

                let var = callable
                    .def_map(db)
                    .local_variables
                    .values()
                    .nth(formal_idx);

                // If past the last param, check if a variadic param exists
                let var = var.or_else(|| {
                    if has_variadic {
                        callable
                            .def_map(db)
                            .local_variables
                            .values()
                            .rev()
                            .find(|v| v.variadic(db))
                    } else {
                        None
                    }
                });

                if let Some(var) = var {
                    if var.variadic(db) {
                        variadic_count += 1;
                        results.push(ParamMatch::Variadic(*parameter, *var, variadic_count));
                    } else {
                        results.push(ParamMatch::Matched(*parameter, *var));
                        formal_idx += 1;
                    }
                } else {
                    errors.push(
                        ResolveError::UnknownNonFormalParameter {
                            func: callable,
                            expr: value,
                            param: formal_idx,
                        }
                        .to_diagnostic(db),
                    );
                    results.push(ParamMatch::Error);
                    formal_idx += 1;
                }
            }
            ParamAssignKind::FormalInput { param, .. } => {
                if let Some(prev) = seen.insert(param.ident, *parameter) {
                    errors.push(
                        DuplicateError::Parameter {
                            param_1: prev,
                            param_2: *parameter,
                            name: param.ident,
                        }
                        .to_diagnostic(db),
                    );
                    results.push(ParamMatch::Error);
                    continue;
                }

                if let Some(var) = callable.def_map(db).local_variables.get(&param.ident) {
                    results.push(ParamMatch::Matched(*parameter, *var));
                } else {
                    errors.push(
                        ResolveError::UnknownInputParameter {
                            func: callable,
                            param,
                        }
                        .to_diagnostic(db),
                    );
                    results.push(ParamMatch::Error);
                }
            }
            ParamAssignKind::FormalOutput { param, .. } => {
                if let Some(prev) = seen.insert(param.ident, *parameter) {
                    errors.push(
                        DuplicateError::Parameter {
                            param_1: prev,
                            param_2: *parameter,
                            name: param.ident,
                        }
                        .to_diagnostic(db),
                    );
                    results.push(ParamMatch::Error);
                    continue;
                }

                if let Some(var) = callable.def_map(db).local_variables.get(&param.ident) {
                    results.push(ParamMatch::Matched(*parameter, *var));
                } else {
                    errors.push(
                        ResolveError::UnknownOutputParameter {
                            func: callable,
                            param,
                        }
                        .to_diagnostic(db),
                    );
                    results.push(ParamMatch::Error);
                }
            }
        }
    }

    results
}
