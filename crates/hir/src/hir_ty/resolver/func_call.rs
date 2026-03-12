use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

use crate::check::errors::e1_duplicates::DuplicateError;
use crate::check::errors::e3_type::TypeError;
use crate::check::errors::e10_control_flow::ControlFlowError;
use crate::hir_def::expressions::expression::Expr;
use crate::hir_def::pous::variable::VariableDecl;
use crate::{
    CallSite, HasName, HirNodeInfo,
    check::errors::{ToIdeDiagnostic, e2_resolve::ResolveError},
    hir_def::expressions::expression::{FuncCall, ParamAssignKind},
    hir_ty::{
        body::BodyInferenceResult,
        infer::{Infer, expr::InferExprCtx},
        resolver::Resolver,
        ty::Type,
    },
};

pub fn resolve_func_call<'db>(
    db: &'db dyn WorkspaceDataBase,
    resolver: Resolver<'db>,
    func_call: FuncCall<'db>,
    ctx: &mut BodyInferenceResult<'db>,
) {
    resolver.resolve_begin_path_expr(db, func_call.path(db), None, ctx);

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

    // Validate generic type arguments (returns false if validation failed)
    if !validate_generic_type_args(db, &callable, func_call, ctx, resolver) {
        // Set path expr type to Never to prevent cascading errors
        // (the caller reads this to determine the func call's return type)
        if let Some(expr) = func_call.path(db).expr(db) {
            ctx.type_of_path_expr.insert(expr, Type::Never);
        }
        return;
    }

    let mut seen = FxHashMap::default();
    let mut formal_idx = 0;
    let len = func_call.params(db).len();

    // Check if the callable has a variadic parameter
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

    for parameter in func_call.params(db) {
        match parameter.kind(db) {
            ParamAssignKind::NonFormal { value } => {
                // Try to get the param by index; if the current param is variadic,
                // stay on it for all remaining arguments
                let var = callable
                    .def_map(db)
                    .local_variables
                    .values()
                    .nth(formal_idx);

                // If we've gone past the last param, check if the last one is variadic
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
                    coerce_with_var_target(db, resolver, value, *var, ctx);

                    if var.is_output(db) {
                        ctx.errors.push(
                            ResolveError::OutputParameterUsedAsInput {
                                func: callable,
                                var: *var,
                                expr: value,
                                param: formal_idx,
                            }
                            .to_diagnostic(db),
                        );
                    }
                    ctx.variable_of_param.insert(*parameter, *var);

                    // Don't advance past a variadic parameter
                    if !var.variadic(db) {
                        formal_idx += 1;
                    }
                } else {
                    ctx.errors.push(
                        ResolveError::UnknownNonFormalParameter {
                            func: callable,
                            expr: value,
                            param: formal_idx,
                        }
                        .to_diagnostic(db),
                    );
                    formal_idx += 1;
                }
            }
            ParamAssignKind::FormalInput { param, value } => {
                if let Some(seen) = seen.insert(param.ident, parameter) {
                    ctx.errors.push(
                        DuplicateError::Parameter {
                            param_1: *seen,
                            param_2: *parameter,
                            name: param.ident,
                        }
                        .to_diagnostic(db),
                    );
                    continue;
                }
                let var = callable.def_map(db).local_variables.get(&param.ident);

                if let Some(var) = var {
                    coerce_with_var_target(db, resolver, value, *var, ctx);
                    ctx.variable_of_param.insert(*parameter, *var);
                } else {
                    ctx.errors.push(
                        ResolveError::UnknownInputParameter {
                            func: callable,
                            param,
                        }
                        .to_diagnostic(db),
                    );
                }
            }
            ParamAssignKind::FormalOutput {
                not,
                param,
                variable,
            } => {
                // check duplicates
                if let Some(seen) = seen.insert(param.ident, parameter) {
                    ctx.errors.push(
                        DuplicateError::Parameter {
                            param_1: *seen,
                            param_2: *parameter,
                            name: param.ident,
                        }
                        .to_diagnostic(db),
                    );
                    continue;
                }

                if let Some(lhs_var) = callable.def_map(db).local_variables.get(&param.ident) {
                    let lhs_typ = Type::new_var(db, *lhs_var);

                    resolver.resolve_variable_access(db, variable, ctx);
                    let call_site = CallSite::from_scoped(db, &variable);

                    let rhs_typ = ctx.type_of_variable_access_with_adjustments(db, variable);

                    // is the variable assignable?
                    rhs_typ.check_assignable(db, call_site, ctx);

                    // type coercion
                    lhs_typ
                        .coerce_with_type(db, rhs_typ, None, resolver)
                        .map_err(|err| {
                            ctx.errors
                                .push(err.into_non_assignable(db, rhs_typ, call_site))
                        })
                        .ok();

                    ctx.variable_of_param.insert(*parameter, *lhs_var);
                } else {
                    ctx.errors.push(
                        ResolveError::UnknownOutputParameter {
                            func: callable,
                            param,
                        }
                        .to_diagnostic(db),
                    );
                }
            }
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
    caller_infer_ctx.resolve_expr(db, expr, ctx);
    caller_infer_ctx.check_expr(db, expr, ctx);

    // Check if this variable's type involves generic substitutions
    let var_type = var.spec(db).infer(db);
    let has_generic_subst =
        matches!(var_type, Type::Generic(_)) && !ctx.generic_substitutions.is_empty();

    if has_generic_subst {
        use crate::hir_ty::infer::table::InferenceTable;

        // Apply generic substitutions to get the concrete expected type
        let expected_type = var_type.apply_generic_substitution(db, &ctx.generic_substitutions);

        // Use InferenceTable to resolve Type::Infer variants
        let rhs_type = ctx.type_of_expr[&expr];
        let mut table = InferenceTable::new();
        table.set_target_type(db, Some(expected_type.into()), expected_type);
        table.add_type(db, expr, rhs_type, resolver);
        table.resolve_completly(db, resolver, ctx);

        // Perform coercion: expected (variable type) coerces TO actual (expression type)
        let actual_type = ctx.type_of_expr_with_adjustments(db, expr);
        let call_site = CallSite::from_scoped(db, &expr);

        if let Err(e) = expected_type.coerce_with_type(
            db,
            actual_type,
            ctx.adjustments_of_expr(db, expr),
            resolver,
        ) {
            ctx.errors.push(
                TypeError::NotAssignable {
                    base_target: expected_type,
                    lhs: e.expected,
                    rhs: e.actual,
                    adjustment: e.adjustment,
                    expr: call_site,
                }
                .to_diagnostic(db),
            );
        }
    } else {
        // Non-generic path: use the original coercion logic
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
}

/// Returns true if validation succeeded, false if it failed (and errors were emitted)
fn validate_generic_type_args<'db>(
    db: &'db dyn WorkspaceDataBase,
    callable: &crate::hir_ty::ty::CallableType<'db>,
    func_call: FuncCall<'db>,
    ctx: &mut BodyInferenceResult<'db>,
    resolver: Resolver<'db>,
) -> bool {
    let generics = callable.generics(db);
    if generics.is_empty() {
        return true;
    }

    let type_args = func_call.type_args(db);
    let call_site = CallSite::from_scoped(db, &func_call.path(db));
    let callable_name = callable.get_name_ident(db);
    let callable_scope = callable.get_scope_id(db);

    // Try to infer generic types from arguments if not explicitly provided
    if type_args.is_empty() {
        let inferred_types = infer_generic_types_from_args(db, callable, func_call, ctx, resolver);

        if inferred_types.len() == generics.len() {
            // Successfully inferred all generic types — validate and store
            if !validate_and_store_generic_substitutions(
                db,
                generics,
                &inferred_types,
                callable_scope,
                call_site,
                ctx,
            ) {
                return false;
            }
            return true;
        }

        // E0313: Could not infer types - require explicit type arguments
        ctx.errors.push(
            TypeError::MissingTypeArguments {
                func_name: callable_name,
                call_site,
            }
            .to_diagnostic(db),
        );
        return false;
    }

    // E0314: Wrong number of type arguments
    if type_args.len() != generics.len() {
        ctx.errors.push(
            TypeError::WrongTypeArgumentArity {
                func_name: callable_name,
                expected: generics.len(),
                actual: type_args.len(),
                call_site,
            }
            .to_diagnostic(db),
        );
        return false;
    }

    // Resolve concrete types from type argument specs
    // Note: We use Type::resolve_spec directly because type argument specs
    // are not processed during signature inference (they're in the body, not variable declarations),
    // so Spec::infer() would return Type::Never.
    let concrete_types: Vec<_> = type_args
        .iter()
        .map(|s| Type::resolve_spec(db, *s))
        .collect();

    // Validate and store
    validate_and_store_generic_substitutions(
        db,
        generics,
        &concrete_types,
        callable_scope,
        call_site,
        ctx,
    )
}

/// Validate all constraints (type bounds + INTO), then store the substitution map.
/// Uses the signature's pre-computed `constraint_of_generic` — a single source of truth.
/// Returns true if all constraints passed, false if any failed.
fn validate_and_store_generic_substitutions<'db>(
    db: &'db dyn WorkspaceDataBase,
    generics: &[crate::hir_def::pous::generics::GenericParam<'db>],
    concrete_types: &[Type<'db>],
    callable_scope: crate::hir_def::scope::ScopeId<'db>,
    call_site: CallSite<'db>,
    ctx: &mut BodyInferenceResult<'db>,
) -> bool {
    use crate::hir_ty::head::signature::{Constraint, infer_signature};

    // Build the substitution map first (needed for GenericParameter constraints)
    for (generic_param, concrete_type) in generics.iter().zip(concrete_types.iter()) {
        ctx.generic_substitutions
            .insert(generic_param.name(db), *concrete_type);
    }

    // Validate ALL constraints from the signature in one pass
    let signature = infer_signature(db, callable_scope);
    let mut ok = true;

    for (generic_param, concrete_type) in generics.iter().zip(concrete_types.iter()) {
        let param_name = generic_param.name(db);

        let constraints = match signature.constraint_of_generic.get(&param_name) {
            Some(c) => c,
            None => continue,
        };

        for constraint in constraints {
            match constraint {
                Constraint::TypeBound(any) => {
                    // Main type bound (T: ANY_INT) — E0315
                    if !type_satisfies_any_constraint(concrete_type, *any) {
                        ctx.errors.push(
                            TypeError::TypeArgumentConstraintMismatch {
                                concrete_type: *concrete_type,
                                param_name,
                                constraint: *any,
                                call_site,
                            }
                            .to_diagnostic(db),
                        );
                        ok = false;
                    }
                }
                Constraint::GenericParameter(other_param_name) => {
                    // INTO<U> — cross-parameter constraint — E0316
                    if let Some(&other_concrete) = ctx.generic_substitutions.get(other_param_name)
                        && !type_satisfies_into_constraint(db, concrete_type, &other_concrete)
                    {
                        ctx.errors.push(
                            TypeError::TypeArgumentIntoConstraintMismatch {
                                type_arg: *concrete_type,
                                into_target: other_concrete,
                                param_name,
                                call_site,
                            }
                            .to_diagnostic(db),
                        );
                        ok = false;
                    }
                }
            }
        }
    }

    ok
}

/// Infer generic type arguments from function call arguments.
/// Returns a vector of inferred types, one for each generic parameter.
fn infer_generic_types_from_args<'db>(
    db: &'db dyn WorkspaceDataBase,
    callable: &crate::hir_ty::ty::CallableType<'db>,
    func_call: FuncCall<'db>,
    ctx: &mut BodyInferenceResult<'db>,
    resolver: Resolver<'db>,
) -> Vec<Type<'db>> {
    let generics = callable.generics(db);
    if generics.is_empty() {
        return vec![];
    }

    // Map from generic parameter index to inferred type
    let mut inferred: FxHashMap<usize, Type<'db>> = FxHashMap::default();

    let params = func_call.params(db);
    let def_map = callable.def_map(db);
    let mut formal_idx = 0;

    for parameter in params {
        match parameter.kind(db) {
            ParamAssignKind::NonFormal { value } => {
                let var = def_map.local_variables.values().nth(formal_idx);
                if let Some(var) = var {
                    let expected_type = var.spec(db).infer(db);
                    let actual_type = infer_expr_type_for_inference(db, resolver, value, ctx);
                    unify_types(db, expected_type, actual_type, generics, &mut inferred);
                }
                formal_idx += 1;
            }
            ParamAssignKind::FormalInput { param, value } => {
                let var = def_map.local_variables.get(&param.ident);
                if let Some(var) = var {
                    let expected_type = var.spec(db).infer(db);
                    let actual_type = infer_expr_type_for_inference(db, resolver, value, ctx);
                    unify_types(db, expected_type, actual_type, generics, &mut inferred);
                }
            }
            ParamAssignKind::FormalOutput { .. } => continue,
        }
    }

    // Convert the map to a vector, in the order of generic parameters
    let mut result = Vec::with_capacity(generics.len());
    for (idx, _) in generics.iter().enumerate() {
        match inferred.get(&idx) {
            Some(&typ) => result.push(typ),
            None => return vec![], // Could not infer this parameter
        }
    }
    result
}

/// Helper function to infer the type of an expression for type inference
/// This is a lightweight version that doesn't do full type checking
fn infer_expr_type_for_inference<'db>(
    db: &'db dyn WorkspaceDataBase,
    resolver: Resolver<'db>,
    expr: Expr<'db>,
    ctx: &mut BodyInferenceResult<'db>,
) -> Type<'db> {
    // Check if we already resolved this expression
    if let Some(&typ) = ctx.type_of_expr.get(&expr) {
        // Normalize Infer types to concrete types
        return normalize_for_inference(db, typ);
    }

    // Resolve the expression to infer its type
    let mut infer_ctx = InferExprCtx::new(resolver);
    infer_ctx.resolve_expr(db, expr, ctx);

    // Get the inferred type and normalize it
    let typ = ctx.type_of_expr.get(&expr).copied().unwrap_or(Type::Never);
    normalize_for_inference(db, typ)
}

/// Normalize types to their concrete elementary types for inference
fn normalize_for_inference<'db>(db: &'db dyn WorkspaceDataBase, typ: Type<'db>) -> Type<'db> {
    match typ {
        Type::Infer(infer) => infer.to_ty(db),
        Type::Variable((var, _)) => var.spec(db).infer(db).normalize(db),
        _ => typ.normalize(db),
    }
}

/// Unify expected and actual types to infer generic parameter types
fn unify_types<'db>(
    db: &'db dyn WorkspaceDataBase,
    expected: Type<'db>,
    actual: Type<'db>,
    generics: &[crate::hir_def::pous::generics::GenericParam<'db>],
    inferred: &mut FxHashMap<usize, Type<'db>>,
) {
    match expected {
        Type::Generic(param) => {
            // Find the index of this generic parameter
            let param_name = param.name(db);
            if let Some(idx) = generics.iter().position(|p| p.name(db) == param_name) {
                // Check if we already inferred a type for this parameter
                if let Some(&existing) = inferred.get(&idx) {
                    // Type must match - if it doesn't, inference fails
                    // We'll let the caller handle this by checking if all params were inferred
                    if existing != actual {
                        // Conflicting inference - leave it to fail later
                    }
                } else {
                    // Record the inferred type
                    inferred.insert(idx, actual);
                }
            }
        }
        Type::Array(arr) => {
            // If the expected type is an array of generic element, unify recursively
            if let Type::Array(actual_arr) = actual {
                let expected_elem = arr.of_type(db).infer(db);
                let actual_elem = actual_arr.of_type(db).infer(db);
                unify_types(db, expected_elem, actual_elem, generics, inferred);
            }
        }
        _ => {
            // Other types don't help with inference
        }
    }
}

/// Check if a concrete type satisfies an INTO<target> constraint.
/// The concrete type must either be the same as the target, or implicitly castable to it.
fn type_satisfies_into_constraint<'db>(
    db: &'db dyn WorkspaceDataBase,
    concrete: &Type<'db>,
    into_target: &Type<'db>,
) -> bool {
    // Same type always satisfies
    if concrete == into_target {
        return true;
    }

    // Both must be elementary types for implicit cast checking
    let concrete_elem = match concrete {
        Type::Elementary(e) => e,
        _ => return false,
    };

    let target_elem = match into_target {
        Type::Elementary(e) => e,
        _ => return false,
    };

    // Check if concrete can be implicitly cast to target
    // target.implicit_cast(source) returns Some if source can be implicitly cast to target
    target_elem.implicit_cast(*concrete_elem).is_some()
}

fn type_satisfies_any_constraint(
    typ: &Type,
    constraint: crate::hir_def::pous::generics::AnyGeneric,
) -> bool {
    use crate::hir_def::pous::generics::AnyGeneric;
    match typ {
        Type::Elementary(elem) => constraint.contains(*elem),
        // ANY matches all types (elementary + derived)
        _ if matches!(constraint, AnyGeneric::ANY) => true,
        _ => false,
    }
}
