use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::check::errors::e01_duplicates::DuplicateError;
use crate::check::errors::e03_type::TypeError;
use crate::check::errors::e04_init::InitError;
use crate::check::errors::e08_call::CallError;
use crate::hir_def::expressions::expression::{Expr, ExprKind, ParamAssign, PrimaryExpr};
use crate::hir_def::interned::identifier::CaselessIdent;
use crate::hir_def::pous::variable::VariableDecl;
use crate::hir_ty::def_map::FxIndexMap;
use crate::hir_ty::head::inheritance::instance_members;
use crate::hir_ty::resolver::name::{OverloadPick, select_overload};
use crate::{
    CallSite, HirNodeInfo,
    check::errors::ToIdeDiagnostic,
    hir_def::expressions::expression::{FuncCall, ParamAssignKind},
    hir_def::pous::pou::Pou,
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
            CallError::CallNonCallableType {
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
                CallError::CallNonCallableType {
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
                CallError::AmbiguousOverload {
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
        OverloadPick::None(candidates) => {
            let name = match candidates.first() {
                Some(f) => f.name(db),
                None => return,
            };
            ctx.errors.push(
                CallError::NoMatchingOverload {
                    func_call,
                    name,
                    arg_types: arg_types.clone(),
                    candidates,
                }
                .to_diagnostic(db, ctx.scope.file(db)),
            );
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

    if let CallableType::Function(f) = callable {
        crate::hir_ty::resolver::visibility::check_function_visibility(
            db,
            &CallSite::from_scoped(db, &func_call.path(db)),
            f,
            &mut ctx.errors,
        );
    }

    let len = func_call.params(db).len();
    let formals = call_site_params(db, callable);
    let has_variadic = formals.values().any(|v| v.variadic(db));

    if !has_variadic && len > formals.len() {
        ctx.errors.push(
            CallError::IncorrectNumberOfParameters {
                overloads: match callable {
                    CallableType::Function(f) => {
                        use crate::hir_ty::index_graphs::{
                            namespace_pou_candidates, pou_candidates,
                        };
                        let name = f.name(db);
                        let candidates =
                            match crate::hir_ty::resolver::name::enclosing_namespace_path(
                                db,
                                f.scope_id(db),
                            ) {
                                Some(path) => namespace_pou_candidates(db, path, name),
                                None => pou_candidates(db, name),
                            };
                        candidates
                            .iter()
                            .filter(|p| matches!(p, crate::hir_def::pous::pou::Pou::Function(_)))
                            .count()
                    }
                    _ => 1,
                },
                expected: formals.len(),
                actual: len,
                func_call,
                callable,
            }
            .to_diagnostic(db, ctx.scope.file(db)),
        );
    }

    // Resolve parameter matching
    let matches = resolve_params(
        db,
        func_call.params(db),
        callable,
        &formals,
        &mut ctx.errors,
    );

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
    let matched_idents: FxHashSet<CaselessIdent> = matches
        .iter()
        .filter_map(|m| match m {
            ParamMatch::Matched(_, var) | ParamMatch::Variadic(_, var, _) => {
                Some(var.name(db).caseless(db))
            }
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
    for (var_name, var) in &formals {
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
        if var.variadic(db) {
            // Nothing bound to the pack: every value it could have collected
            // would have appeared in `bound` above. A fold over an empty pack
            // has no value, so this is refused here rather than reaching MIR
            // with an arity it cannot lower.
            ctx.errors.push(
                CallError::EmptyVariadicCall {
                    func: callable,
                    var: *var,
                    func_call,
                }
                .to_diagnostic(db, ctx.scope.file(db)),
            );
            params.push((*var, crate::hir_ty::body::ParamBinding::Omitted));
            continue;
        }
        if is_param_required(db, callable, *var) {
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
            CallError::MissingRequiredParameter {
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
        CallableType::Function(_) | CallableType::MethodDecl(_) => input_default(db, var).is_none(),
    }
}

/// E0806: a VAR_IN_OUT argument must be an l-value (a variable, field, or
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
            CallError::InOutParameterRequiresLValue {
                func: callable,
                var,
                expr: value,
            }
            .to_diagnostic(db, ctx.scope.file(db)),
        );
    }
}

/// A VAR_IN_OUT aliases the caller's storage, so its two ends must be the
/// SAME type: the value table's widening (an INT into a REAL) would have the
/// callee read and write four bytes over the caller's two. Only reported
/// where the value coercion passed, so a plain mismatch keeps its one E0301.
fn check_by_ref_invariance<'db>(
    db: &'db dyn WorkspaceDataBase,
    resolver: Resolver<'db>,
    var: VariableDecl<'db>,
    arg_ty: Type<'db>,
    value: Expr<'db>,
    ctx: &mut BodyInferenceResult<'db>,
) {
    let param_ty = Type::new_var(db, var);
    // An interface-typed parameter takes any implementer: that is dispatch,
    // not a reinterpretation of the caller's slot, and IMPLEMENTS is checked
    // by the coercion itself.
    if matches!(param_ty.normalize(db), Type::Interface(_))
        || crate::hir_ty::infer::coerce::same_type(db, param_ty.normalize(db), arg_ty.normalize(db))
        || param_ty
            .coerce_with_type(db, arg_ty, None, resolver)
            .is_err()
    {
        return;
    }
    ctx.errors.push(
        TypeError::NotAssignable {
            suggest_cast: false,
            base_target: param_ty,
            lhs: param_ty,
            rhs: arg_ty,
            adjustment: None,
            expr: CallSite::from_scoped(db, &value),
        }
        .to_diagnostic(db, ctx.scope.file(db)),
    );
}

/// E0704: the two ends of a by-reference binding must agree about the
/// subrange. A VAR_IN_OUT aliases the caller's storage for reads AND writes,
/// so any disagreement lets one side escape the other's bounds: an INT param
/// scribbling 99 into the caller's `INT (0..10)` goes around the range check
/// entirely. An `=>` output flows callee to caller only, so only a subrange
/// DESTINATION constrains; a checked subrange output landing in a plain
/// variable is already in range.
///
/// Bases that differ are the coercion machinery's complaint, not this one's.
fn check_by_ref_subrange<'db>(
    db: &'db dyn WorkspaceDataBase,
    var: VariableDecl<'db>,
    value_ty: Type<'db>,
    span: auto_lsp::tree_sitter::Range,
    output_binding: bool,
    ctx: &mut BodyInferenceResult<'db>,
) {
    let param_ty = Type::new_var(db, var);
    if param_ty.normalize(db) != value_ty.normalize(db) {
        return;
    }
    let bounds = |t: Type<'db>| {
        t.as_subrange(db)
            .map(|s| crate::hir_ty::infer::const_eval::subrange_bounds(db, s))
    };
    let agree = match (bounds(param_ty), bounds(value_ty)) {
        (None, None) => true,
        (Some(p), Some(a)) => p == a,
        (Some(_), None) => output_binding,
        (None, Some(_)) => false,
    };
    if !agree {
        ctx.errors.push(
            crate::check::errors::e07_subrange::SubRangeError::ByRefSubrangeMismatch {
                span,
                param: param_ty,
                arg: value_ty,
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
                    InitError::AssignToConstant {
                        access: CallSite::from_scoped(db, &value),
                    }
                    .to_diagnostic(db, ctx.scope.file(db)),
                );
            }

            check_in_out_lvalue(db, callable, var, value, ctx);
            if let Some(err) = ctx.ref_subrange_mismatch(db, Type::new_var(db, var), value) {
                ctx.errors.push(err.to_diagnostic(db, ctx.scope.file(db)));
            }

            if var.is_in_out(db)
                && let ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(va)) = value.expr(db)
            {
                let arg_ty = ctx.type_of_variable_access_with_adjustments(db, *va);
                check_by_ref_invariance(db, resolver, var, arg_ty, value, ctx);
                check_by_ref_subrange(db, var, arg_ty, va.get_span(db), false, ctx);
            }

            if var.is_output(db) {
                ctx.errors.push(
                    CallError::OutputParameterUsedAsInput {
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
                    InitError::AssignToConstant {
                        access: CallSite::from_scoped(db, &value),
                    }
                    .to_diagnostic(db, ctx.scope.file(db)),
                );
            }

            check_in_out_lvalue(db, callable, var, value, ctx);
            if let Some(err) = ctx.ref_subrange_mismatch(db, Type::new_var(db, var), value) {
                ctx.errors.push(err.to_diagnostic(db, ctx.scope.file(db)));
            }

            if var.is_in_out(db)
                && let ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(va)) = value.expr(db)
            {
                let arg_ty = ctx.type_of_variable_access_with_adjustments(db, *va);
                check_by_ref_invariance(db, resolver, var, arg_ty, value, ctx);
                check_by_ref_subrange(db, var, arg_ty, va.get_span(db), false, ctx);
            }

            ctx.variable_of_param.insert(param, var);
        }
        ParamAssignKind::FormalOutput {
            variable,
            param: param_ident,
            ..
        } => {
            // E0807: `v => x` on a VAR_IN_OUT would leave the reference
            // unbound — inouts are bound by reference at call entry with `:=`.
            if var.is_in_out(db) {
                ctx.errors.push(
                    CallError::InOutParameterBoundWithArrow {
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

            if !var.is_in_out(db) {
                check_by_ref_subrange(db, var, rhs_typ, variable.get_span(db), true, ctx);
            }

            if (var.is_in_out(db) || var.is_output(db)) && ctx.is_constant_access(db, variable) {
                ctx.errors.push(
                    InitError::AssignToConstant { access: call_site }
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

/// Every parameter bindable at a call site of `callable`, by caseless name,
/// in binding order.
///
/// For an FB this is the flattened `EXTENDS` view — [`instance_members`]
/// filtered to Input/Output/InOut, base parameters first — because a call
/// site binds inherited parameters too. Reading only the scope's own
/// `def_map` left them unknown here: naming one was E0803, and omitting an
/// inherited VAR_IN_OUT went unreported. Transient on purpose: the chain
/// walk is already memoized in `instance_members`, so this is a re-keying,
/// not a query.
pub fn call_site_params<'db>(
    db: &'db dyn WorkspaceDataBase,
    callable: CallableType<'db>,
) -> FxIndexMap<CaselessIdent, VariableDecl<'db>> {
    match callable {
        CallableType::FunctionBlock(fb) => instance_members(db, Pou::FunctionBlock(fb))
            .iter()
            .filter(|m| m.var.is_input(db) || m.var.is_output(db) || m.var.is_in_out(db))
            .map(|m| (m.var.name(db).caseless(db), m.var))
            .collect(),
        _ => callable.def_map(db).local_variables.clone(),
    }
}

/// Resolve a list of parameters against a callable's signature.
///
/// Handles positional/named param resolution, variadic parameters, duplicate
/// detection, and emits errors for unknown params.
pub fn resolve_params<'db>(
    db: &'db dyn WorkspaceDataBase,
    params: &[ParamAssign<'db>],
    callable: CallableType<'db>,
    formals: &FxIndexMap<CaselessIdent, VariableDecl<'db>>,
    errors: &mut Vec<IdeDiagnostic>,
) -> Vec<ParamMatch<'db>> {
    let mut results = vec![];
    let mut seen: FxHashMap<CaselessIdent, ParamAssign<'db>> = FxHashMap::default();
    let mut formal_idx = 0;
    let mut variadic_count = 0;

    let has_variadic = formals.values().any(|v| v.variadic(db));

    // Pre-collect named parameter idents so positional args skip them
    let named_params: FxHashSet<_> = params
        .iter()
        .filter_map(|p| match p.kind(db) {
            ParamAssignKind::FormalInput { param, .. } => Some(param.ident.caseless(db)),
            ParamAssignKind::FormalOutput { param, .. } => Some(param.ident.caseless(db)),
            _ => None,
        })
        .collect();

    for parameter in params {
        match parameter.kind(db) {
            ParamAssignKind::NonFormal { value } => {
                // Skip parameters already filled by named arguments
                while formal_idx < formals.len() {
                    if let Some((name, _)) = formals.get_index(formal_idx)
                        && named_params.contains(name)
                    {
                        formal_idx += 1;
                        continue;
                    }
                    break;
                }

                let var = formals.values().nth(formal_idx);

                // If past the last param, check if a variadic param exists
                let var = var.or_else(|| {
                    if has_variadic {
                        formals.values().rev().find(|v| v.variadic(db))
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
                    // Past the last parameter: the arg count already exceeds
                    // the callable's, which the caller reported as E0801 —
                    // a per-argument error here would restate it.
                    results.push(ParamMatch::Error);
                    formal_idx += 1;
                }
            }
            ParamAssignKind::FormalInput { param, .. } => {
                if let Some(prev) = seen.insert(param.ident.caseless(db), *parameter) {
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

                if let Some(var) = formals.get(&param.ident.caseless(db)) {
                    results.push(ParamMatch::Matched(*parameter, *var));
                } else {
                    errors.push(
                        CallError::UnknownInputParameter {
                            func: callable,
                            param,
                        }
                        .to_diagnostic(db, callable.get_scope_id(db).file(db)),
                    );
                    results.push(ParamMatch::Error);
                }
            }
            ParamAssignKind::FormalOutput { param, .. } => {
                if let Some(prev) = seen.insert(param.ident.caseless(db), *parameter) {
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

                if let Some(var) = formals.get(&param.ident.caseless(db)) {
                    results.push(ParamMatch::Matched(*parameter, *var));
                } else {
                    errors.push(
                        CallError::UnknownOutputParameter {
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
