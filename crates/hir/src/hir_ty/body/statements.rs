use db::WorkspaceDataBase;

use crate::{
    CallSite, HasName, HirNodeInfo,
    check::errors::{
        ToIdeDiagnostic, e2_resolve::ResolveError, e3_type::TypeError,
        e10_control_flow::ControlFlowError,
    },
    hir_def::{
        expressions::{
            expression::{Elementary, Expr, ExprKind, PrimaryExpr, UnaryOperatorKind},
            spec::{ElementarySpec, Spec, SpecKind},
            statement::{CaseKind, Stmt, StmtKind},
        },
        interned::identifier::Ident,
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{
        body::{Adjust, BodyInferenceResult, NullState},
        infer::expr::InferExprCtx,
        resolver::{Resolver, func_call::resolve_func_call},
        ty::Type,
    },
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum NestedScope {
    Loop,
    None,
}

pub struct StmtsResolverCtx<'db> {
    pub scope: ScopeId<'db>,
    pub nested_scope: NestedScope,
}

/// Try to extract a constant integer value from a literal expression.
/// Handles plain literals and unary minus on literals.
fn try_extract_integer(db: &dyn WorkspaceDataBase, expr: Expr<'_>) -> Option<i64> {
    match expr.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(Elementary::InferInteger(v))) => {
            v.as_i64(db).ok()
        }
        ExprKind::UnaryOperator {
            expr: inner,
            operator,
        } => match operator {
            UnaryOperatorKind::Minus => try_extract_integer(db, *inner).map(|v| -v),
            UnaryOperatorKind::Plus => try_extract_integer(db, *inner),
            _ => None,
        },
        _ => None,
    }
}

impl<'db> StmtsResolverCtx<'db> {
    pub fn new(scope: ScopeId<'db>) -> Self {
        StmtsResolverCtx {
            scope,
            nested_scope: NestedScope::None,
        }
    }

    pub fn check_statements(
        &self,
        db: &'db dyn WorkspaceDataBase,
        resolver: Resolver<'db>,
        statements: &'db [Stmt<'db>],
        nested_scope: NestedScope,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        let mut infer = InferExprCtx::new(resolver);

        for (i, stmt) in statements.iter().enumerate() {
            match stmt.stmt(db) {
                StmtKind::EmptyPathExpression(expr) => {
                    resolver.resolve_begin_path_expr(db, *expr, None, ctx);
                    ctx.effectless_statements.push(*stmt);
                }

                StmtKind::AssignmentAttempt { var, target } => {
                    resolver.resolve_variable_access(db, *var, ctx);
                    let base_typ = ctx.get_type_of_variable_access(db, *var);
                    let lhs_typ = base_typ.normalize(db);

                    base_typ.check_assignable(db, CallSite::from_scoped(db, var), ctx);

                    // LHS must be REF_TO
                    if !matches!(lhs_typ, Type::RefTo(_) | Type::Never) {
                        ctx.errors.push(
                            TypeError::AssignAttemptRequiresRef {
                                typ: lhs_typ,
                                call_site: CallSite::from_scoped(db, var),
                            }
                            .to_diagnostic(db),
                        );
                    }

                    self.infer_and_check_expr(db, &mut infer, *target, ctx);

                    // Check RHS is REF_TO or Interface
                    if matches!(lhs_typ, Type::RefTo(_)) {
                        let rhs_typ = ctx.get_type_of_expr(*target).normalize(db);
                        if lhs_typ.coerce_assign_attempt(db, rhs_typ).is_err() {
                            ctx.errors.push(
                                TypeError::AssignAttemptInvalidRhs {
                                    lhs: lhs_typ,
                                    rhs: rhs_typ,
                                    call_site: CallSite::from_scoped(db, target),
                                }
                                .to_diagnostic(db),
                            );
                        }
                    }
                }

                StmtKind::Assignment { var, target } => {
                    resolver.resolve_variable_access(db, *var, ctx);
                    let base_typ = ctx.get_type_of_variable_access(db, *var);

                    if ctx.is_constant_access(db, *var) {
                        ctx.errors.push(
                            ControlFlowError::AssignToConstant {
                                access: CallSite::from_scoped(db, var),
                            }
                            .to_diagnostic(db),
                        );
                    }

                    base_typ.check_assignable(db, CallSite::from_scoped(db, var), ctx);

                    self.infer_and_check_expr(db, &mut infer, *target, ctx);

                    if let Err(err) = infer.coerce_var_access_with_expr(db, *var, *target, ctx) {
                        ctx.errors.push(err.into_non_assignable(
                            db,
                            base_typ,
                            CallSite::from_scoped(db, target),
                        ));
                    }

                    // Update null state for REF_TO variables
                    if let Type::Variable((var_decl, _)) = base_typ
                        && ctx.ref_null_state.contains_key(&var_decl)
                    {
                        // Only update if this is a direct assignment (no deref on the LHS)
                        let has_deref =
                            ctx.adjustments_of_var_access(db, *var).is_some_and(|adjs| {
                                adjs.iter().any(|a| matches!(a.kind, Adjust::Deref))
                            });
                        if !has_deref {
                            let rhs_type = ctx.get_type_of_expr(*target);
                            let new_state = if matches!(rhs_type, Type::Null) {
                                NullState::Null(stmt.as_call_site(db))
                            } else if let Type::Variable((rhs_var, _)) = rhs_type {
                                // Propagate null state from RHS variable
                                ctx.ref_null_state
                                    .get(&rhs_var)
                                    .copied()
                                    .unwrap_or(NullState::NonNull)
                            } else {
                                NullState::NonNull
                            };
                            ctx.ref_null_state.insert(var_decl, new_state);
                        }
                    }
                }

                StmtKind::If {
                    condition,
                    then,
                    else_if,
                    else_,
                } => {
                    // check condition
                    self.infer_and_check_expr(db, &mut infer, *condition, ctx);
                    if let Err(err) =
                        infer.coerce_type_with_expr(db, Type::new_bool(), *condition, ctx)
                    {
                        ctx.errors.push(err.into_non_assignable(
                            db,
                            Type::new_bool(),
                            CallSite::from_scoped(db, condition),
                        ));
                    }

                    // Snapshot null state before branches
                    let pre_if_state = ctx.ref_null_state.clone();

                    // check branches

                    // THEN
                    if let Some(then) = then {
                        self.check_statements(db, resolver, then, nested_scope, ctx);
                    }
                    let then_state = ctx.ref_null_state.clone();

                    // Collect branch states for joining
                    let mut branch_states = vec![then_state];

                    // ELSE IFs
                    for (condition, stmts) in else_if {
                        // Reset to pre-IF state for each branch
                        ctx.ref_null_state = pre_if_state.clone();

                        self.infer_and_check_expr(db, &mut infer, *condition, ctx);

                        if let Err(err) =
                            infer.coerce_type_with_expr(db, Type::new_bool(), *condition, ctx)
                        {
                            ctx.errors.push(err.into_non_assignable(
                                db,
                                Type::new_bool(),
                                CallSite::from_scoped(db, condition),
                            ));
                        }

                        self.check_statements(db, resolver, stmts, nested_scope, ctx);
                        branch_states.push(ctx.ref_null_state.clone());
                    }

                    // ELSE
                    if let Some(else_) = else_ {
                        ctx.ref_null_state = pre_if_state.clone();
                        self.check_statements(db, resolver, else_, nested_scope, ctx);
                        branch_states.push(ctx.ref_null_state.clone());
                    } else {
                        // No ELSE means the pre-IF state is a possible path
                        branch_states.push(pre_if_state);
                    }

                    // Join all branch states
                    let mut joined = branch_states.remove(0);
                    for branch in branch_states {
                        for (var, state) in &branch {
                            if let Some(existing) = joined.get(var) {
                                joined.insert(*var, existing.join(*state));
                            }
                        }
                    }
                    ctx.ref_null_state = joined;
                }

                StmtKind::While { condition, body } | StmtKind::Repeat { condition, body } => {
                    self.infer_and_check_expr(db, &mut infer, *condition, ctx);

                    if let Err(err) =
                        infer.coerce_type_with_expr(db, Type::new_bool(), *condition, ctx)
                    {
                        ctx.errors.push(err.into_non_assignable(
                            db,
                            Type::new_bool(),
                            CallSite::from_scoped(db, condition),
                        ));
                    }

                    self.check_statements(db, resolver, body, NestedScope::Loop, ctx);
                }

                StmtKind::For {
                    control_variable,
                    start,
                    end,
                    step,
                    body,
                } => {
                    resolver.resolve_variable_access(db, *control_variable, ctx);
                    let control_typ =
                        ctx.type_of_variable_access_with_adjustments(db, *control_variable);

                    control_typ.check_assignable(
                        db,
                        CallSite::from_scoped(db, control_variable),
                        ctx,
                    );

                    self.infer_and_check_expr(db, &mut infer, *start, ctx);
                    self.infer_and_check_expr(db, &mut infer, *end, ctx);

                    if let Err(err) =
                        infer.coerce_var_access_with_expr(db, *control_variable, *start, ctx)
                    {
                        ctx.errors.push(err.into_non_assignable(
                            db,
                            control_typ,
                            CallSite::from_scoped(db, start),
                        ));
                    }

                    if let Err(err) =
                        infer.coerce_var_access_with_expr(db, *control_variable, *end, ctx)
                    {
                        ctx.errors.push(err.into_non_comparable(
                            db,
                            ctx.get_type_of_variable_access(db, *control_variable),
                            CallSite::from_scoped(db, end),
                        ));
                    }

                    if let Some(step) = step {
                        self.infer_and_check_expr(db, &mut infer, *step, ctx);

                        if let Err(err) =
                            infer.coerce_var_access_with_expr(db, *control_variable, *step, ctx)
                        {
                            ctx.errors.push(err.into_non_comparable(
                                db,
                                ctx.get_type_of_variable_access(db, *control_variable),
                                CallSite::from_scoped(db, step),
                            ));
                        }
                    }

                    // Check for mismatched step sign (only with literal values)
                    if let (Some(start_val), Some(end_val)) = (
                        try_extract_integer(db, *start),
                        try_extract_integer(db, *end),
                    ) {
                        let step_val = step
                            .as_ref()
                            .and_then(|s| try_extract_integer(db, *s))
                            .unwrap_or(1);
                        let ascending = end_val > start_val;
                        if (ascending && step_val < 0)
                            || (!ascending && step_val > 0 && start_val != end_val)
                        {
                            ctx.mismatched_for_step.push(*stmt);
                        }
                    }

                    self.check_statements(db, resolver, body, NestedScope::Loop, ctx);
                }

                StmtKind::FuncCall(call) => {
                    resolve_func_call(db, resolver, *call, ctx);

                    let typ = ctx.type_of_begin_expr_with_adjustments(db, call.path(db));

                    let callable = match typ {
                        Type::CallableType(ct) => Some(ct),
                        _ => typ.as_callable(db),
                    };
                    if let Some(callable) = callable
                        && callable.inner_callable().with_return_type(db).is_some()
                    {
                        ctx.unused_return_types.push((*stmt, typ));
                    }
                }

                StmtKind::Continue | StmtKind::Exit => {
                    if nested_scope != NestedScope::Loop {
                        let err = match stmt.stmt(db) {
                            StmtKind::Continue => {
                                ControlFlowError::ContinueOutsideLoop { stmt: *stmt }
                            }
                            _ => ControlFlowError::ExitOutsideLoop { stmt: *stmt },
                        };
                        ctx.errors.push(err.to_diagnostic(db));
                    }

                    // Remaining statements in this block are unreachable
                    for dead in &statements[i + 1..] {
                        ctx.dead_code_statements.push(*dead);
                    }
                    break;
                }

                StmtKind::Case {
                    condition,
                    cases,
                    else_,
                } => {
                    // check condition
                    self.infer_and_check_expr(db, &mut infer, *condition, ctx);

                    let condition_typ = ctx.get_type_of_expr(*condition);

                    // check cases
                    for (case_kind, stmts) in cases {
                        for case in case_kind {
                            match case {
                                CaseKind::Expression(expr) => {
                                    self.infer_and_check_expr(db, &mut infer, *expr, ctx);

                                    if let Err(err) =
                                        infer.coerce_type_with_expr(db, condition_typ, *expr, ctx)
                                    {
                                        ctx.errors.push(err.into_non_comparable(
                                            db,
                                            condition_typ,
                                            CallSite::from_scoped(db, expr),
                                        ));
                                    }
                                }
                                CaseKind::Subrange { lower, upper } => {
                                    self.infer_and_check_expr(db, &mut infer, *lower, ctx);
                                    self.infer_and_check_expr(db, &mut infer, *upper, ctx);

                                    if let Err(err) =
                                        infer.coerce_type_with_expr(db, condition_typ, *lower, ctx)
                                    {
                                        ctx.errors.push(err.into_non_comparable(
                                            db,
                                            condition_typ,
                                            CallSite::from_scoped(db, lower),
                                        ));
                                    }

                                    if let Err(err) =
                                        infer.coerce_type_with_expr(db, condition_typ, *upper, ctx)
                                    {
                                        ctx.errors.push(err.into_non_comparable(
                                            db,
                                            condition_typ,
                                            CallSite::from_scoped(db, upper),
                                        ));
                                    }
                                }
                            }
                        }

                        self.check_statements(db, resolver, stmts, nested_scope, ctx);
                    }

                    // check else
                    if let Some(else_) = else_ {
                        self.check_statements(db, resolver, else_, nested_scope, ctx);
                    } else {
                        ctx.case_without_else.push(*stmt);
                    }
                }

                StmtKind::Return => {
                    // Remaining statements in this block are unreachable
                    for dead in &statements[i + 1..] {
                        ctx.dead_code_statements.push(*dead);
                    }
                    break;
                }

                StmtKind::ExternPragma(extern_decl) => {
                    let def_map = self.scope.def_map(db);
                    let scope_kind = crate::hir_def::semantic_index::get_scope(db, self.scope).kind;

                    let is_known_var = |ident: &crate::hir_def::interned::identifier::Ident| {
                        def_map.local_variables.contains_key(ident)
                            || def_map.global_variables.contains_key(ident)
                    };

                    // Check if the name is the enclosing POU's own name (return variable)
                    let is_pou_name =
                        |ident: &crate::hir_def::interned::identifier::Ident| match scope_kind {
                            crate::hir_def::scope::ScopeKind::Pou(pou) => {
                                pou.get_name_ident(db) == *ident
                            }
                            crate::hir_def::scope::ScopeKind::MethodDecl(m) => m.name(db) == *ident,
                            _ => false,
                        };

                    // Check that each param variable exists in scope
                    for param in &extern_decl.params {
                        if !is_known_var(&param.ident) && !is_pou_name(&param.ident) {
                            ctx.errors.push(
                                ResolveError::ExternVariableNotFound {
                                    ident: *param,
                                    scope: self.scope,
                                }
                                .to_diagnostic(db),
                            );
                        }
                    }

                    // Check that result variable exists in scope
                    // (can be a local variable or the POU name for return value)
                    if let Some(result) = &extern_decl.result
                        && !is_known_var(&result.ident)
                        && !is_pou_name(&result.ident)
                    {
                        ctx.errors.push(
                            ResolveError::ExternVariableNotFound {
                                ident: *result,
                                scope: self.scope,
                            }
                            .to_diagnostic(db),
                        );
                    }
                }
                StmtKind::WasmPragma(_) => {
                    // Wasm intrinsic, no type inference needed
                }
                StmtKind::PreprocessIf { branches } => {
                    for branch in branches {
                        self.check_statements(
                            db,
                            resolver,
                            branch.body.as_slice(),
                            nested_scope,
                            ctx,
                        );
                    }
                }
            }
        }
    }

    fn infer_and_check_expr(
        &self,
        db: &'db dyn WorkspaceDataBase,
        infer: &mut InferExprCtx<'db>,
        expr: Expr<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        infer.resolve_expr(db, expr, ctx);
        infer.check_expr(db, expr, ctx);
    }
}

/// Validates `{#if}` conditions and `{wasm}` pragmas inside the function
/// body.
///
/// Three validations on every `{#if x is T}` cond, regardless of whether
/// it sits inside an enclosing chain:
/// - **E0325** `x` doesn't resolve in scope.
/// - **E0326** `x` resolves but is concrete (the chain through `INTO(...)`
///   bottoms out at a non-`ANY_*` spec) — the branch can never narrow.
/// - **E0327** `T` isn't a variant of `x`'s `ANY_*` bound.
///
/// One validation on `{wasm}` pragmas inside a chain:
/// - **E0328** a referenced ident's anchor isn't pinned by the enclosing
///   `{#if}`. Wasm intrinsics emit a single concrete instruction, so an
///   unpinned ANY_* would not have a type to encode against.
///
/// `{extern}` pragmas are intentionally untouched — those are host
/// imports and the host can polymorphic-dispatch lazily (the
/// `generic-extern` lint flags the case informationally).
pub(crate) fn check_preprocess<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    statements: &[Stmt<'db>],
    fixed: &rustc_hash::FxHashMap<Ident, ElementarySpec>,
    inside_if: bool,
    ctx: &mut BodyInferenceResult<'db>,
) {
    let def_map = scope.def_map(db);
    for stmt in statements {
        match stmt.stmt(db) {
            StmtKind::WasmPragma(decl) if inside_if => {
                if let Some(t) = &decl.type_ref {
                    check_wasm_ident(db, scope, &def_map, fixed, t, ctx);
                }
                for p in &decl.params {
                    check_wasm_ident(db, scope, &def_map, fixed, p, ctx);
                }
                if let Some(r) = &decl.result {
                    check_wasm_ident(db, scope, &def_map, fixed, r, ctx);
                }
            }
            StmtKind::PreprocessIf { branches } => {
                for branch in branches {
                    let mut child = fixed.clone();
                    if let Some(anchor_spec) =
                        check_cond(db, scope, &def_map, branch, ctx)
                        && let Type::Elementary(e) =
                            Type::resolve_spec(db, branch.cond.expected)
                    {
                        child.insert(anchor_spec, e);
                    }
                    check_preprocess(db, scope, &branch.body, &child, true, ctx);
                }
            }
            // Recurse into structured control flow — pragmas can hide
            // arbitrarily deep, even if it's an unusual style.
            StmtKind::If {
                then,
                else_if,
                else_,
                ..
            } => {
                if let Some(stmts) = then {
                    check_preprocess(db, scope, stmts, fixed, inside_if, ctx);
                }
                for (_, body) in else_if {
                    check_preprocess(db, scope, body, fixed, inside_if, ctx);
                }
                if let Some(stmts) = else_ {
                    check_preprocess(db, scope, stmts, fixed, inside_if, ctx);
                }
            }
            StmtKind::Case { cases, else_, .. } => {
                for (_, body) in cases {
                    check_preprocess(db, scope, body, fixed, inside_if, ctx);
                }
                if let Some(stmts) = else_ {
                    check_preprocess(db, scope, stmts, fixed, inside_if, ctx);
                }
            }
            StmtKind::For { body, .. }
            | StmtKind::While { body, .. }
            | StmtKind::Repeat { body, .. } => {
                check_preprocess(db, scope, body, fixed, inside_if, ctx);
            }
            _ => {}
        }
    }
}

/// Validate one `{#if x is T}` cond. Returns the canonical anchor ident
/// (i.e. the variable that owns the `ANY_*` bound) on success so the
/// caller can extend its `fixed` map. Returns `None` when any of E0325,
/// E0326, or E0327 fired.
fn check_cond<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    def_map: &crate::hir_ty::def_map::LocalDefMap<'db>,
    branch: &crate::hir_def::expressions::statement::PreprocessBranch<'db>,
    ctx: &mut BodyInferenceResult<'db>,
) -> Option<Ident> {
    let span_ident = &branch.cond.ident;
    let ident = span_ident.ident;

    let Some(_) = lookup_spec(db, scope, def_map, ident) else {
        ctx.errors.push(
            TypeError::PreprocessIdentNotFound {
                ident: ident.text(db).to_string(),
                site: CallSite::from_scoped(db, span_ident),
            }
            .to_diagnostic(db),
        );
        return None;
    };

    let Some(anchor) = canonical_anchor(db, scope, def_map, ident) else {
        // Spec exists but isn't rooted in ANY_* (concrete or non-Into chain).
        let actual_spec = lookup_spec(db, scope, def_map, ident)
            .expect("just checked existence");
        ctx.errors.push(
            TypeError::PreprocessIdentNotGeneric {
                ident: ident.text(db).to_string(),
                actual_spec,
                site: CallSite::from_scoped(db, span_ident),
            }
            .to_diagnostic(db),
        );
        return None;
    };

    let bound = lookup_any_bound(db, scope, def_map, anchor)
        .expect("canonical_anchor returned ident with ANY_* bound");

    let expected_ty = Type::resolve_spec(db, branch.cond.expected);
    match expected_ty {
        Type::Elementary(e) if bound.accepts(e) => Some(anchor),
        // Type::Never propagates from a previous resolution failure
        // already reported elsewhere — don't pile on.
        Type::Never => None,
        _ => {
            ctx.errors.push(
                TypeError::PreprocessTypeNotInBound {
                    expected_ty,
                    bound,
                    spec: branch.cond.expected,
                }
                .to_diagnostic(db),
            );
            None
        }
    }
}

fn check_wasm_ident<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    def_map: &crate::hir_ty::def_map::LocalDefMap<'db>,
    fixed: &rustc_hash::FxHashMap<Ident, ElementarySpec>,
    span_ident: &crate::hir_def::interned::identifier::SpanIdent<'db>,
    ctx: &mut BodyInferenceResult<'db>,
) {
    let Some(anchor) = canonical_anchor(db, scope, def_map, span_ident.ident) else {
        return;
    };
    if fixed.contains_key(&anchor) {
        return;
    }
    let Some(bound) = lookup_any_bound(db, scope, def_map, anchor) else {
        return;
    };
    ctx.errors.push(
        TypeError::WasmUnresolvedGeneric {
            param: span_ident.ident.text(db).to_string(),
            bound,
            site: CallSite::from_scoped(db, span_ident),
        }
        .to_diagnostic(db),
    );
}

/// Walk `INTO(X)` chains until we hit a bare `ANY_*` spec; return the
/// canonical anchor ident (the variable that owns the bound). `None` if
/// the chain resolves to a concrete type or can't be resolved.
fn canonical_anchor<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    def_map: &crate::hir_ty::def_map::LocalDefMap<'db>,
    ident: Ident,
) -> Option<Ident> {
    let mut current = ident;
    for _ in 0..16 {
        let spec = lookup_spec(db, scope, def_map, current)?;
        match spec.kind(db) {
            SpecKind::Simple(e) if e.is_any() => return Some(current),
            SpecKind::Into(target) => current = target.ident,
            _ => return None,
        }
    }
    None
}

fn lookup_any_bound<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    def_map: &crate::hir_ty::def_map::LocalDefMap<'db>,
    ident: Ident,
) -> Option<ElementarySpec> {
    let spec = lookup_spec(db, scope, def_map, ident)?;
    match spec.kind(db) {
        SpecKind::Simple(e) if e.is_any() => Some(*e),
        _ => None,
    }
}

/// Resolve an ident to its declaring spec — local var, global var, or the
/// enclosing POU's return type when the ident matches the POU/method name.
fn lookup_spec<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    def_map: &crate::hir_ty::def_map::LocalDefMap<'db>,
    ident: Ident,
) -> Option<Spec<'db>> {
    if let Some(v) = def_map
        .local_variables
        .get(&ident)
        .or_else(|| def_map.global_variables.get(&ident))
    {
        return Some(v.spec(db));
    }
    let pou_match = match get_scope(db, scope).kind {
        ScopeKind::Pou(pou) => pou.get_name_ident(db) == ident,
        ScopeKind::MethodDecl(m) => m.name(db) == ident,
        _ => false,
    };
    if pou_match {
        scope.return_type(db).copied()
    } else {
        None
    }
}
