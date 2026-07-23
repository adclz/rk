use db::WorkspaceDataBase;

use crate::{
    CallSite, HasName, HirNodeInfo,
    check::errors::{
        ToIdeDiagnostic, e2_resolve::ResolveError, e5_inheritance::InheritanceError,
        e10_control_flow::ControlFlowError,
    },
    hir_def::{
        expressions::{
            expression::{Elementary, Expr, ExprKind, PrimaryExpr, UnaryOperatorKind},
            spec::ElementarySpec,
            statement::{CaseKind, Stmt, StmtKind},
        },
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{
        body::{Adjust, BodyInferenceResult, NullState},
        infer::{Infer, expr::InferExprCtx},
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
                    // Rule 2 (IEC 6.6.7.2.9): SUPER() shall occur once in the FB
                    // body. Track the first; a second occurrence is E0519,
                    // pointing back to the first. (SUPER() in a method is E0518,
                    // handled in resolve_invocation, so restrict to FB bodies.)
                    if expr.invocation(db).map(|i| i.kind(db))
                        == Some(crate::hir_def::expressions::invocation::InvocationKind::SuperBody)
                        && matches!(
                            get_scope(db, self.scope).kind,
                            ScopeKind::Pou(crate::hir_def::pous::pou::Pou::FunctionBlock(_))
                        )
                    {
                        // Rule 2: SUPER() shall not be in a loop.
                        if nested_scope == NestedScope::Loop {
                            ctx.errors.push(
                                InheritanceError::SuperBodyInLoop {
                                    call_site: stmt.as_call_site(db),
                                }
                                .to_diagnostic(db, ctx.scope.file(db)),
                            );
                        }
                        match ctx.first_super_body {
                            Some(first) => ctx.errors.push(
                                InheritanceError::SuperBodyMultiple {
                                    call_site: stmt.as_call_site(db),
                                    first: first.as_call_site(db),
                                }
                                .to_diagnostic(db, ctx.scope.file(db)),
                            ),
                            None => ctx.first_super_body = Some(*stmt),
                        }
                    }
                    ctx.effectless_statements.push(*stmt);
                }

                StmtKind::Assignment { var, target } => {
                    resolver.resolve_variable_access(db, *var, ctx);
                    let base_typ = ctx.get_type_of_variable_access(db, *var);

                    if ctx.is_constant_access(db, *var) {
                        ctx.errors.push(
                            ControlFlowError::AssignToConstant {
                                access: CallSite::from_scoped(db, var),
                            }
                            .to_diagnostic(db, ctx.scope.file(db)),
                        );
                    }

                    base_typ.check_assignable(db, CallSite::from_scoped(db, var), ctx);

                    // Design 1: an interface parameter is a fixed binding to the
                    // concrete type the caller supplied; reassigning it would
                    // break monomorphization (see E0517).
                    if let Type::Variable((var_decl, _)) = base_typ
                        && matches!(
                            var_decl.spec(db).infer(db).normalize(db),
                            Type::Interface(_)
                        )
                    {
                        ctx.errors.push(
                            InheritanceError::InterfaceParamNotAssignable {
                                var: var_decl,
                                access: *var,
                            }
                            .to_diagnostic(db, ctx.scope.file(db)),
                        );
                    }

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
                        ctx.errors.push(err.to_diagnostic(db, ctx.scope.file(db)));
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

                StmtKind::Raise { message } => {
                    // The message expression must be STRING
                    self.infer_and_check_expr(db, &mut infer, *message, ctx);
                    let string_ty = Type::Elementary(ElementarySpec::String);
                    if let Err(err) = infer.coerce_type_with_expr(db, string_ty, *message, ctx) {
                        ctx.errors.push(err.into_non_assignable(
                            db,
                            string_ty,
                            CallSite::from_scoped(db, message),
                        ));
                    }
                    for dead in &statements[i + 1..] {
                        ctx.dead_code_statements.push(*dead);
                    }
                    break;
                }

                StmtKind::ExternPragma(extern_decl) => {
                    let def_map = self.scope.def_map(db);
                    let scope_kind = crate::hir_def::semantic_index::get_scope(db, self.scope).kind;

                    let lookup_var = |ident: &crate::hir_def::interned::identifier::Ident| {
                        def_map
                            .local_variables
                            .get(ident)
                            .or_else(|| def_map.global_variables.get(ident))
                            .copied()
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

                    // Check that each param variable exists in scope; mark
                    // resolved ones as used so the unused-variable lint
                    // doesn't fire on them.
                    for param in &extern_decl.params {
                        if let Some(var) = lookup_var(&param.ident) {
                            ctx.variables_used.insert(var);
                        } else if !is_pou_name(&param.ident) {
                            ctx.errors.push(
                                ResolveError::ExternVariableNotFound {
                                    ident: *param,
                                    scope: self.scope,
                                }
                                .to_diagnostic(db, ctx.scope.file(db)),
                            );
                        }
                    }

                    // Result: same treatment. (POU-name results don't
                    // map to a VariableDecl, so nothing to mark.)
                    if let Some(result) = &extern_decl.result {
                        if let Some(var) = lookup_var(&result.ident) {
                            ctx.variables_used.insert(var);
                        } else if !is_pou_name(&result.ident) {
                            ctx.errors.push(
                                ResolveError::ExternVariableNotFound {
                                    ident: *result,
                                    scope: self.scope,
                                }
                                .to_diagnostic(db, ctx.scope.file(db)),
                            );
                        }
                    }
                }
                StmtKind::WasmPragma(wasm_decl) => {
                    // Wasm intrinsic doesn't need type inference, but we
                    // still need to mark referenced variables as used so
                    // the unused-variable lint doesn't flag them.
                    let def_map = self.scope.def_map(db);
                    let mut mark = |ident: &crate::hir_def::interned::identifier::Ident| {
                        if let Some(var) = def_map
                            .local_variables
                            .get(ident)
                            .or_else(|| def_map.global_variables.get(ident))
                        {
                            ctx.variables_used.insert(*var);
                        }
                    };
                    if let Some(t) = &wasm_decl.type_ref {
                        mark(&t.ident);
                    }
                    for p in &wasm_decl.params {
                        mark(&p.ident);
                    }
                    if let Some(r) = &wasm_decl.result {
                        mark(&r.ident);
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
