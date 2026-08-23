use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::check::errors::e1_duplicates::DuplicateError;
use crate::check::errors::e3_type::TypeError;
use crate::check::errors::e10_control_flow::ControlFlowError;
use crate::hir_def::expressions::expression::{Expr, ExprKind, ParamAssign, PrimaryExpr};
use crate::hir_def::interned::identifier::Ident;
use crate::hir_def::pous::variable::VariableDecl;
use crate::hir_ty::resolver::name::{OverloadPick, select_overload};
use crate::{
    CallSite, HirNodeInfo,
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
    // The type the call's value lands in when the consuming site knows it —
    // what a RETURN-directed overload set is picked by (`select_overload`).
    expected: Option<Type<'db>>,
) {
    // Re-entry happens (see the CallableType unwrap below), so record once.
    if !ctx.calls.contains(&func_call) {
        ctx.calls.push(func_call);
    }

    resolver.resolve_begin_path_expr(db, func_call.path(db), None, ctx);

    // If a prior resolution already marked this path
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
            .to_diagnostic(db, ctx.scope.file(db)),
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
                .to_diagnostic(db, ctx.scope.file(db)),
            );
            return;
        }
    };

    // Overload selection: name resolution binds a bare function name to the
    // first same-name FUNCTION in scope; if it's an overload set, re-select the
    // one whose signature matches this call's argument types. The picking lives
    // in the resolver (`select_overload`); this call stays overload-unaware —
    // it just supplies the arg types and handles an ambiguous result.
    let arg_types = call_input_arg_types(db, resolver, func_call, ctx);
    let callable = match select_overload(db, callable, &arg_types, expected) {
        OverloadPick::One(c) => c,
        OverloadPick::Ambiguous(candidates) => {
            let name = match candidates.first() {
                Some(f) => f.name(db),
                None => return,
            };
            ctx.errors.push(
                ResolveError::AmbiguousOverload {
                    func_call,
                    name,
                    candidates,
                }
                .to_diagnostic(db, ctx.scope.file(db)),
            );
            // If no overload could be selected, we register Type::Never
            if let Some(expr) = func_call.path(db).expr(db) {
                ctx.type_of_path_expr.insert(expr, Type::Never);
            }
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
            .to_diagnostic(db, ctx.scope.file(db)),
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

    // Flag any expected param that is required-at-call-site but not supplied.
    let matched_idents: FxHashSet<Ident> = matches
        .iter()
        .filter_map(|m| match m {
            ParamMatch::Matched(_, var) | ParamMatch::Variadic(_, var, _) => Some(var.name(db)),
            ParamMatch::Error => None,
        })
        .collect();

    // What each call-site assign bound, grouped by declared parameter and
    // kept in call order (a variadic collects several).
    let mut bound: FxHashMap<VariableDecl<'db>, crate::hir_ty::body::ParamBinding<'db>> =
        FxHashMap::default();
    for m in &matches {
        let (pa, var) = match m {
            ParamMatch::Matched(pa, var) | ParamMatch::Variadic(pa, var, _) => (pa, var),
            ParamMatch::Error => continue,
        };
        match pa.kind(db) {
            ParamAssignKind::NonFormal { value } | ParamAssignKind::FormalInput { value, .. } => {
                if let crate::hir_ty::body::ParamBinding::Values(vs) = bound
                    .entry(*var)
                    .or_insert_with(|| crate::hir_ty::body::ParamBinding::Values(Vec::new()))
                {
                    vs.push(value);
                }
            }
            ParamAssignKind::FormalOutput { variable, .. } => {
                bound.insert(*var, crate::hir_ty::body::ParamBinding::Output(variable));
            }
        }
    }

    let mut missing: Vec<VariableDecl<'db>> = Vec::new();
    let mut params: Vec<(VariableDecl<'db>, crate::hir_ty::body::ParamBinding<'db>)> = Vec::new();
    for (var_name, var) in &callable.def_map(db).local_variables {
        if let Some(binding) = bound.remove(var) {
            params.push((*var, binding));
            continue;
        }
        if matched_idents.contains(var_name) {
            // Matched but not bound above: an erroneous duplicate — already
            // reported; nothing coherent to record.
            params.push((*var, crate::hir_ty::body::ParamBinding::Omitted));
            continue;
        }
        if !var.variadic(db) && is_param_required(db, callable, *var) {
            // Variadic params accept zero or more values — empty is valid.
            missing.push(*var);
            params.push((*var, crate::hir_ty::body::ParamBinding::Omitted));
        } else if var.is_input(db)
            && !matches!(callable, CallableType::FunctionBlock(_))
            && let Some(expr) = input_default(db, *var)
        {
            // The omission is legal BECAUSE of this default, so what the
            // callee receives is recorded here, where that is decided. An
            // omitted FB input keeps its instance storage instead.
            params.push((*var, crate::hir_ty::body::ParamBinding::Default(expr)));
        } else {
            params.push((*var, crate::hir_ty::body::ParamBinding::Omitted));
        }
    }
    ctx.resolved_calls.insert(
        func_call,
        crate::hir_ty::body::ResolvedCall { callable, params },
    );

    if !missing.is_empty() {
        ctx.errors.push(
            ResolveError::MissingRequiredParameter {
                func: callable,
                vars: missing,
                func_call,
            }
            .to_diagnostic(db, ctx.scope.file(db)),
        );
    }
}

/// The types of a call's positional value arguments (VAR_INPUT / VAR_IN_OUT),
/// in order — the arguments that drive overload selection. Pure `=>` output
/// bindings are skipped (they don't participate). Each argument is inferred here
/// and cached in `ctx.type_of_expr`, so the later coercion pass reuses it rather
/// than re-resolving (see the guard in `coerce_with_var_target`).
fn call_input_arg_types<'db>(
    db: &'db dyn WorkspaceDataBase,
    resolver: Resolver<'db>,
    func_call: FuncCall<'db>,
    ctx: &mut BodyInferenceResult<'db>,
) -> Vec<Type<'db>> {
    let mut types = Vec::new();
    for p in func_call.params(db) {
        let value = match p.kind(db) {
            ParamAssignKind::NonFormal { value } | ParamAssignKind::FormalInput { value, .. } => {
                value
            }
            ParamAssignKind::FormalOutput { .. } => continue,
        };
        let mut ictx = InferExprCtx::new(resolver);
        if !ctx.type_of_expr.contains_key(&value) {
            ictx.resolve_expr(db, value, ctx);
        }
        // Adjusted, not raw: indexing and dereference are recorded as
        // ADJUSTMENTS over the base type, so the raw type of `arr[0]` is the
        // ARRAY, not its element. Overload resolution classifying that raw type
        // matched no elementary parameter and silently picked an unrelated
        // overload (`ASSERT_EQ(arr[0], 5)` selected the CHAR one).
        types.push(ctx.type_of_expr_with_adjustments(db, value));
    }
    types
}

/// The expression a call site passes for `var` when it omits it: the input's
/// compile-time-constant default. `None` makes the input required
/// ([`is_param_required`]); `Some` is recorded per omitting call in
/// [`BodyInferenceResult::omitted_param_defaults`].
///
/// [`BodyInferenceResult::omitted_param_defaults`]: crate::hir_ty::body::BodyInferenceResult::omitted_param_defaults
fn input_default<'db>(
    db: &'db dyn WorkspaceDataBase,
    var: VariableDecl<'db>,
) -> Option<crate::hir_def::expressions::expression::Expr<'db>> {
    match var.init(db)?.kind(db) {
        crate::hir_def::expressions::expression::InitExprKind::ConstantExpr(expr) => Some(expr),
        _ => None,
    }
}

/// Returns `true` if `var` must be supplied as an argument at every call site
/// of `callable`.
fn is_param_required<'db>(
    db: &'db dyn WorkspaceDataBase,
    callable: CallableType<'db>,
    var: VariableDecl<'db>,
) -> bool {
    if var.is_in_out(db) {
        return true;
    }
    if !var.is_input(db) {
        // Non-parameter locals (Var, Temp, Output, External, ...) aren't required at call sites.
        return false;
    }
    match callable {
        // An omitted FB input keeps its instance storage.
        CallableType::FunctionBlock(_) => false,
        CallableType::Function(_) | CallableType::MethodDecl(_) => {
            input_default(db, var).is_none()
        }
    }
}

/// E0234: a VAR_IN_OUT argument must be an l-value (a variable, field, or
/// array-element access) — it binds the callee to the caller's storage by
/// reference, so a literal, arithmetic expression, or call result has no
/// address to bind. Constants are caught separately (AssignToConstant).
///
/// A partial access (`b.%X1`, `d.3`) is a VariableAccess SYNTACTICALLY but
/// names a slice of a variable, and a slice has no address either. The
/// syntax check alone waved it through, and the argument then reached the
/// callee as a bit VALUE standing where a pointer belongs — writes through
/// it corrupted memory at address 0 or 1, from code `rk check` called clean.
fn check_in_out_lvalue<'db>(
    db: &'db dyn WorkspaceDataBase,
    callable: CallableType<'db>,
    var: VariableDecl<'db>,
    value: Expr<'db>,
    ctx: &mut BodyInferenceResult<'db>,
) {
    // The RAW recorded type, not the adjusted view: adjustments apply the
    // slice and would answer BOOL for `b.%X1`, hiding exactly the marker
    // this check needs. (And never `Expr::infer` here — this runs INSIDE
    // `infer_body`, and the query would cycle into itself.)
    let is_partial_access = matches!(
        ctx.get_type_of_expr(value),
        Type::Variable((_, Some(_))) | Type::DirectVariable((_, Some(_)))
    );
    if var.is_in_out(db)
        && (is_partial_access
            || !matches!(
                value.expr(db),
                ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(_))
            ))
    {
        ctx.errors.push(
            ResolveError::InOutParameterRequiresLValue {
                func: callable,
                var,
                expr: value,
            }
            .to_diagnostic(db, ctx.scope.file(db)),
        );
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
                    .to_diagnostic(db, ctx.scope.file(db)),
                );
            }

            check_in_out_lvalue(db, callable, var, value, ctx);

            if var.is_output(db) {
                ctx.errors.push(
                    ResolveError::OutputParameterUsedAsInput {
                        func: callable,
                        var,
                        expr: value,
                        param: 0,
                    }
                    .to_diagnostic(db, ctx.scope.file(db)),
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
                    .to_diagnostic(db, ctx.scope.file(db)),
                );
            }

            check_in_out_lvalue(db, callable, var, value, ctx);

            ctx.variable_of_param.insert(param, var);
        }
        ParamAssignKind::FormalOutput {
            variable,
            param: param_ident,
            ..
        } => {
            // E0236: `v => x` on a VAR_IN_OUT would leave the reference
            // unbound — inouts are bound by reference at call entry with `:=`.
            if var.is_in_out(db) {
                ctx.errors.push(
                    ResolveError::InOutParameterBoundWithArrow {
                        func: callable,
                        var,
                        param: param_ident,
                    }
                    .to_diagnostic(db, ctx.scope.file(db)),
                );
            }

            let lhs_typ = Type::new_var(db, var);

            resolver.resolve_variable_access(db, variable, ctx);
            let call_site = CallSite::from_scoped(db, &variable);
            let rhs_typ = ctx.type_of_variable_access_with_adjustments(db, variable);

            if (var.is_in_out(db) || var.is_output(db)) && ctx.is_constant_access(db, variable) {
                ctx.errors.push(
                    ControlFlowError::AssignToConstant { access: call_site }
                        .to_diagnostic(db, ctx.scope.file(db)),
                );
            }

            // `o => d` writes the output INTO d, so d is the target. Checked
            // the other way, every widening binding was refused and every
            // narrowing one accepted. Reported around the OUTPUT though: the
            // caret is on `d`, so the type named is the one `d` had to hold.
            if rhs_typ.check_assignable(db, call_site, ctx)
                && rhs_typ
                    .coerce_with_type(db, lhs_typ, None, resolver)
                    .is_err()
            {
                ctx.errors.push(
                    TypeError::NotAssignable {
                        base_target: lhs_typ,
                        lhs: lhs_typ,
                        rhs: rhs_typ,
                        adjustment: None,
                        expr: call_site,
                        suggest_cast: false,
                    }
                    .to_diagnostic(db, ctx.scope.file(db)),
                );
            }

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
                suggest_cast: true,
                base_target,
                lhs: e.expected,
                rhs: e.actual,
                adjustment: e.adjustment,
                expr: CallSite::from_scoped(db, &expr),
            }
            .to_diagnostic(db, ctx.scope.file(db)),
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
                        .to_diagnostic(db, callable.get_scope_id(db).file(db)),
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
                        .to_diagnostic(db, callable.get_scope_id(db).file(db)),
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
                        .to_diagnostic(db, callable.get_scope_id(db).file(db)),
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
                        .to_diagnostic(db, callable.get_scope_id(db).file(db)),
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
                        .to_diagnostic(db, callable.get_scope_id(db).file(db)),
                    );
                    results.push(ParamMatch::Error);
                }
            }
        }
    }

    results
}
