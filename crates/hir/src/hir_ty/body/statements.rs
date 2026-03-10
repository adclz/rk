use db::WorkspaceDataBase;

use crate::{
    CallSite, HirNodeInfo,
    check::errors::{ToIdeDiagnostic, e3_type::TypeError, e10_control_flow::ControlFlowError},
    hir_def::{
        expressions::{
            expression::{Elementary, Expr, ExprKind, PrimaryExpr, UnaryOperatorKind},
            statement::{CaseKind, Stmt, StmtKind},
        },
        scope::ScopeId,
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
                        && ctx.ref_null_state.contains_key(&var_decl) {
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
                        self.check_statements(db, resolver, then, NestedScope::None, ctx);
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

                        self.check_statements(db, resolver, stmts, NestedScope::None, ctx);
                        branch_states.push(ctx.ref_null_state.clone());
                    }

                    // ELSE
                    if let Some(else_) = else_ {
                        ctx.ref_null_state = pre_if_state.clone();
                        self.check_statements(db, resolver, else_, NestedScope::None, ctx);
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

                    if typ.with_return_type(db).is_some() {
                        ctx.unused_return_types.push((*stmt, typ));
                    }
                }

                StmtKind::Continue | StmtKind::Exit => {
                    if nested_scope != NestedScope::Loop {
                        ctx.errors.push(
                            ControlFlowError::ContinueOutsideLoop { stmt: *stmt }.to_diagnostic(db),
                        );
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

                        self.check_statements(db, resolver, stmts, NestedScope::None, ctx);
                    }

                    // check else
                    if let Some(else_) = else_ {
                        self.check_statements(db, resolver, else_, NestedScope::None, ctx);
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
