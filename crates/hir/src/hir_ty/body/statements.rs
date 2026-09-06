use db::WorkspaceDataBase;

use crate::{
    CallSite, HirNodeInfo,
    check::errors::{
        ToIdeDiagnostic, e5_inheritance::InheritanceError,
        e10_control_flow::ControlFlowError,
    },
    hir_def::{
        expressions::{
            expression::{
                BooleanOperatorKind, ComparisonOperatorKind, Elementary, Expr, ExprKind,
                PrimaryExpr, RefValue, UnaryOperatorKind,
            },
            spec::ElementarySpec,
            statement::{CaseKind, Stmt, StmtKind},
        },
        pous::variable::{VariableDecl, VariableKind},
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{
        body::{Adjust, BodyInferenceResult, CaseLabelValue, NullState},
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
/// Evaluate a CASE label, recording its value and refusing it if it has none
/// (E1006).
///
/// IEC: `Case_List_Elem : Subrange | Constant_Expr`, where a constant
/// expression is any expression that evaluates to a constant AT COMPILE TIME.
/// That is a semantic rule, not a shape one, so this evaluates rather than
/// pattern-matches: literals and signs, a named CONSTANT's initializer, and
/// arithmetic over those. "Known at compile time" is wider than "written as a
/// literal" — the same principle `interval_nanos` states for a TASK period.
///
/// The value is RECORDED, because checking a label and evaluating it are the
/// same act. Lowering reads it instead of lowering the label and inspecting
/// the result, which is how a label MIR could not fold became an internal
/// compiler error on source that checked clean.
fn check_case_label_constant<'db>(
    db: &'db dyn WorkspaceDataBase,
    label: Expr<'db>,
    // Whether a non-integer constant is acceptable here. A single label may
    // be a string or an enum variant; a SUBRANGE bound may not — `'a'..'z'`
    // denotes a lexicographic set the compiler has no representation for, and
    // ordering is the whole point of a range.
    allow_non_integer: bool,
    ctx: &mut BodyInferenceResult<'db>,
) {
    // An enum label's value is its variant's ordinal, which lowering reads
    // from the variant table; there is nothing to record here.
    match ctx.get_type_of_expr(label).normalize(db) {
        Type::Enum(_) | Type::EnumVariant(..) if allow_non_integer => return,
        Type::Elementary(ElementarySpec::String) if allow_non_integer => {
            // A string LITERAL is constant; a STRING variable is not. Record
            // the DECODED bytes, so `STRING#'a'` and `'a'` are one label and
            // an escape is compared by what it denotes.
            if let ExprKind::PrimaryExpr(PrimaryExpr::Literal(
                Elementary::String(ident) | Elementary::Char(ident),
            )) = label.expr(db)
                && let Ok(bytes) = ident.as_single_string(db)
            {
                ctx.case_label_value
                    .insert(label, CaseLabelValue::Str(bytes));
                return;
            }
        }
        _ => {}
    }
    match crate::hir_ty::infer::const_eval::const_int(db, label, ctx) {
        Some(value) => {
            ctx.case_label_value
                .insert(label, CaseLabelValue::Int(value));
        }
        None => ctx.errors.push(
            ControlFlowError::CaseLabelNotConstant {
                label: CallSite::from_scoped(db, &label),
                as_range_bound: !allow_non_integer,
            }
            .to_diagnostic(db, ctx.scope.file(db)),
        ),
    }
}


/// What a condition proves about references, per outcome.
///
/// Each entry says "in this branch, that reference IS / IS NOT null".
/// `AND` proves both halves when it HOLDS and nothing when it fails; `OR` is
/// the mirror. Keeping the two directions apart is what lets a compound
/// condition narrow the branch it actually establishes.
#[derive(Default)]
struct NullGuards<'db> {
    when_true: Vec<(VariableDecl<'db>, bool)>,
    when_false: Vec<(VariableDecl<'db>, bool)>,
}

impl<'db> NullGuards<'db> {
    fn swapped(self) -> Self {
        NullGuards {
            when_true: self.when_false,
            when_false: self.when_true,
        }
    }
}

/// Read a condition as null guards: `p = NULL` / `p <> NULL` in either operand
/// order, and the `AND` / `OR` / `NOT` combinations of those.
///
/// Without this the analysis was assignment-only, so the one idiomatic way to
/// write a safe dereference — `IF p <> NULL THEN p^` — was refused, and no
/// pragma could silence it because E1003 is a compiler error.
fn null_guards<'db>(
    db: &'db dyn WorkspaceDataBase,
    condition: Expr<'db>,
    ctx: &BodyInferenceResult<'db>,
) -> NullGuards<'db> {
    match condition.expr(db) {
        ExprKind::ComparisonOperator {
            left,
            operator,
            right,
        } => {
            let null_when_true = match operator {
                ComparisonOperatorKind::Eq => true,
                ComparisonOperatorKind::Ne => false,
                _ => return NullGuards::default(),
            };
            let is_null = |e: &Expr<'db>| {
                matches!(
                    e.expr(db),
                    ExprKind::PrimaryExpr(PrimaryExpr::RefValue {
                        value: RefValue::Null
                    })
                )
            };
            let var_of = |e: &Expr<'db>| match ctx.get_type_of_expr(*e) {
                Type::Variable((v, _)) => Some(v),
                _ => None,
            };
            let var = if is_null(right) {
                var_of(left)
            } else if is_null(left) {
                var_of(right)
            } else {
                None
            };
            match var {
                Some(v) => NullGuards {
                    when_true: vec![(v, null_when_true)],
                    when_false: vec![(v, !null_when_true)],
                },
                None => NullGuards::default(),
            }
        }
        ExprKind::BooleanOperator {
            left,
            operator,
            right,
        } => {
            let mut l = null_guards(db, *left, ctx);
            let mut r = null_guards(db, *right, ctx);
            match operator {
                // Both halves hold when the AND does; a failure names neither.
                BooleanOperatorKind::And => {
                    l.when_true.append(&mut r.when_true);
                    NullGuards {
                        when_true: l.when_true,
                        when_false: Vec::new(),
                    }
                }
                BooleanOperatorKind::Or => {
                    l.when_false.append(&mut r.when_false);
                    NullGuards {
                        when_true: Vec::new(),
                        when_false: l.when_false,
                    }
                }
                _ => NullGuards::default(),
            }
        }
        ExprKind::UnaryOperator { expr, operator } => match operator {
            UnaryOperatorKind::Not => null_guards(db, *expr, ctx).swapped(),
            _ => NullGuards::default(),
        },
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => {
            null_guards(db, *expr, ctx)
        }
        _ => NullGuards::default(),
    }
}

/// Apply a guard to the state a branch is entered with.
///
/// Only the non-null direction narrows: when a condition PROVES the reference
/// null, the nullable state already on record is the accurate one, and a
/// dereference under it must still be reported.
fn apply_guard<'db>(
    states: &mut rustc_hash::FxHashMap<VariableDecl<'db>, NullState<'db>>,
    narrowings: &[(VariableDecl<'db>, bool)],
) {
    for (var, is_null) in narrowings {
        if !*is_null && states.contains_key(var) {
            states.insert(*var, NullState::NonNull);
        }
    }
}

/// Whether a branch runs off its end into the code after the IF.
///
/// A branch that returns cannot reach the statements that follow, so its state
/// must not join into theirs: `IF p = NULL THEN RETURN; END_IF;` leaves only
/// the non-null path alive.
fn falls_through<'db>(db: &'db dyn WorkspaceDataBase, stmts: &[Stmt<'db>]) -> bool {
    !matches!(
        stmts.last().map(|s| s.stmt(db)),
        Some(StmtKind::Return | StmtKind::Exit | StmtKind::Continue)
    )
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

                    let assignable =
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

                    // The target's type was resolved above, so it can DIRECT
                    // the right-hand side: a RETURN-overloaded call picks the
                    // overload whose return the target expects.
                    infer.resolve_expr_expecting(db, *target, ctx, Some(base_typ));
                    infer.check_expr(db, *target, ctx);

                    if assignable
                        && let Err(err) = infer.coerce_var_access_with_expr(db, *var, *target, ctx)
                    {
                        ctx.errors.push(err.into_non_assignable(
                            db,
                            base_typ,
                            CallSite::from_scoped(db, target),
                        ));
                    }

                    // A string literal must FIT the destination. The store
                    // runs at the destination's capacity — 80 unless the spec
                    // says otherwise — so an over-long literal was silently
                    // cut there: a 149-byte literal read back as its first 80
                    // bytes, from a compile that said nothing. The
                    // initializer door has always refused this; the
                    // assignment door now matches it.
                    check_string_literal_fits(db, base_typ, *target, ctx);
                    if let Some(err) = ctx.ref_subrange_mismatch(db, base_typ, *target) {
                        ctx.errors.push(err.to_diagnostic(db, ctx.scope.file(db)));
                    }

                    // A reference to this call's own storage, handed back to
                    // the caller. It does not fault: an address-taken local
                    // sits at a fixed address, so the reference quietly reads
                    // whatever the NEXT call leaves in that slot.
                    if matches!(base_typ, Type::Function(_) | Type::MethodDecl(_))
                        && let ExprKind::PrimaryExpr(PrimaryExpr::RefValue {
                            value: RefValue::Address(path),
                        }) = target.expr(db)
                        && let Some(path_expr) = path.expr(db)
                        && let Type::Variable((referenced, _)) =
                            ctx.get_type_of_path_expr(db, path_expr)
                        && referenced.get_scope_id(db) == ctx.scope
                        && matches!(
                            referenced.kind(db),
                            VariableKind::Var | VariableKind::Temp | VariableKind::Input
                        )
                    {
                        ctx.errors.push(
                            crate::check::errors::e2_resolve::ResolveError::ReturnsReferenceToLocal {
                                var: referenced,
                                site: CallSite::from_scoped(db, target),
                            }
                            .to_diagnostic(db, ctx.scope.file(db)),
                        );
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
                                NullState::Null(crate::hir_ty::body::NullOrigin {
                                    site: stmt.as_call_site(db),
                                    var: var_decl,
                                })
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

                    // Snapshot null state before branches. `fallthrough` is
                    // the state reaching the next branch: every condition
                    // tested so far was false, which is itself information
                    // about a reference (`p = NULL` being false proves it is
                    // not).
                    let pre_if_state = ctx.ref_null_state.clone();
                    let mut fallthrough = pre_if_state.clone();

                    // check branches

                    // THEN
                    let guard = null_guards(db, *condition, ctx);
                    apply_guard(&mut ctx.ref_null_state, &guard.when_true);
                    if let Some(then) = then {
                        self.check_statements(db, resolver, then, nested_scope, ctx);
                    }
                    let then_state = ctx.ref_null_state.clone();
                    apply_guard(&mut fallthrough, &guard.when_false);

                    // Collect branch states for joining
                    let mut branch_states = vec![];
                    if then.as_deref().is_none_or(|s| falls_through(db, s)) {
                        branch_states.push(then_state);
                    }

                    // ELSE IFs
                    for (condition, stmts) in else_if {
                        // Reached only when every earlier condition was false
                        ctx.ref_null_state = fallthrough.clone();

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

                        let guard = null_guards(db, *condition, ctx);
                        apply_guard(&mut ctx.ref_null_state, &guard.when_true);
                        self.check_statements(db, resolver, stmts, nested_scope, ctx);
                        if falls_through(db, stmts) {
                            branch_states.push(ctx.ref_null_state.clone());
                        }
                        apply_guard(&mut fallthrough, &guard.when_false);
                    }

                    // ELSE
                    if let Some(else_) = else_ {
                        ctx.ref_null_state = fallthrough.clone();
                        self.check_statements(db, resolver, else_, nested_scope, ctx);
                        if falls_through(db, else_) {
                            branch_states.push(ctx.ref_null_state.clone());
                        }
                    } else {
                        // No ELSE: falling past the IF is a possible path
                        branch_states.push(fallthrough);
                    }

                    // Every branch returned: nothing reaches the code below,
                    // so keep the state the IF was entered with.
                    if branch_states.is_empty() {
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

                StmtKind::While { condition, body } => {
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

                    // The body runs only where the condition held, so
                    // `WHILE p <> NULL DO p^` is as guarded as the IF form.
                    let guard = null_guards(db, *condition, ctx);
                    apply_guard(&mut ctx.ref_null_state, &guard.when_true);
                    self.check_statements(db, resolver, body, NestedScope::Loop, ctx);
                }

                StmtKind::Repeat { condition, body } => {
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

                    // REPEAT tests AFTER the body, so the condition proves
                    // nothing about the first pass — no narrowing here.
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

                    // IEC's grammar: `control_variable ::= identifier`. A
                    // path (`r.i`), an index (`a[k]`), a deref or a bit access
                    // is not a counter — other toolchains rejects them too. A bare name
                    // resolving to an FB/PROGRAM member is fine: the rule is
                    // about the syntax, not where the variable lives. Without
                    // this check the shapes sailed through to MIR, which
                    // rejected them with an unlocated "unsupported" error —
                    // `rk check` said one thing and `rk compile` another.
                    if !for_control_is_bare_identifier(db, *control_variable) {
                        ctx.errors.push(
                            crate::check::errors::e10_control_flow::ControlFlowError::ForControlNotAVariable {
                                access: CallSite::from_scoped(db, control_variable),
                            }
                            .to_diagnostic(db, ctx.scope.file(db)),
                        );
                    }

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
                        infer.coerce_var_access_compared_with_expr(db, *control_variable, *end, ctx)
                    {
                        ctx.errors.push(err.into_non_comparable(
                            db,
                            ctx.get_type_of_variable_access(db, *control_variable),
                            CallSite::from_scoped(db, end),
                        ));
                    }

                    if let Some(step) = step {
                        self.infer_and_check_expr(db, &mut infer, *step, ctx);

                        if let Err(err) = infer.coerce_var_access_compared_with_expr(
                            db,
                            *control_variable,
                            *step,
                            ctx,
                        ) {
                            ctx.errors.push(err.into_non_comparable(
                                db,
                                ctx.get_type_of_variable_access(db, *control_variable),
                                CallSite::from_scoped(db, step),
                            ));
                        }

                        // The step's SIGN picks the exit comparison at compile
                        // time, so the step must fold — and to a nonzero
                        // value, since BY 0 never advances the counter.
                        match crate::hir_ty::infer::const_eval::const_int(db, *step, ctx) {
                            Some(0) => ctx.errors.push(
                                crate::check::errors::e10_control_flow::ControlFlowError::ForStepInvalid {
                                    step: CallSite::from_scoped(db, step),
                                    zero: true,
                                    decl: None,
                                }
                                .to_diagnostic(db, ctx.scope.file(db)),
                            ),
                            Some(v) => {
                                ctx.for_step_value.insert(*step, v);
                            }
                            None => {
                                // Point the fix at the declaration only when
                                // the step IS a bare non-CONSTANT variable:
                                // qualifying it CONSTANT is then the fix. For
                                // `arr[j]` or `f()` no declaration change makes
                                // the step fold, so no advice is offered.
                                let decl = match step.expr(db) {
                                    ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(va))
                                        if for_control_is_bare_identifier(db, *va) =>
                                    {
                                        match ctx.type_of_variable_access_with_adjustments(db, *va)
                                        {
                                            Type::Variable((var, None))
                                                if !var
                                                    .qualifier(db)
                                                    .contains(crate::Qualifier::CONSTANT) =>
                                            {
                                                Some(var)
                                            }
                                            _ => None,
                                        }
                                    }
                                    _ => None,
                                };
                                ctx.errors.push(
                                    crate::check::errors::e10_control_flow::ControlFlowError::ForStepInvalid {
                                        step: CallSite::from_scoped(db, step),
                                        zero: false,
                                        decl,
                                    }
                                    .to_diagnostic(db, ctx.scope.file(db)),
                                )
                            }
                        }
                    }

                    // Check for mismatched step sign. `const_int`, not a
                    // literal match: a CONSTANT bound or a folding expression
                    // walks the wrong way just as surely.
                    if let (Some(start_val), Some(end_val)) = (
                        crate::hir_ty::infer::const_eval::const_int(db, *start, ctx),
                        crate::hir_ty::infer::const_eval::const_int(db, *end, ctx),
                    ) {
                        let step_val = step
                            .as_ref()
                            .and_then(|s| ctx.for_step_value.get(s).copied())
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
                    resolve_func_call(db, resolver, *call, ctx, None);

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
                                    check_case_label_constant(db, *expr, true, ctx);

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
                                    check_case_label_constant(db, *lower, false, ctx);
                                    check_case_label_constant(db, *upper, false, ctx);

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

                // Nothing to infer: the pragma only marks the next statement
                // for the linter.
                StmtKind::AllowPragma(_) => {}

                StmtKind::WasmPragma(wasm_decl) => {
                    use crate::hir_def::interned::identifier::SpanIdent;
                    use crate::hir_def::pous::pou::Pou;
                    use crate::hir_def::scope::ScopeKind;
                    use crate::hir_def::semantic_index::get_scope;
                    // Only FUNCTION bodies lower a wasm pragma; anywhere else
                    // it was silently dropped and the body compiled as if it
                    // were not there.
                    let function = match get_scope(db, self.scope).kind {
                        ScopeKind::Pou(Pou::Function(f)) => Some(f),
                        _ => None,
                    };
                    if function.is_none() {
                        ctx.errors.push(
                            crate::check::errors::e2_resolve::ResolveError::WasmPragmaOutsideFunction {
                                span: wasm_decl.instruction_span,
                            }
                            .to_diagnostic(db, ctx.scope.file(db)),
                        );
                    } else {
                        // An unknown name used to fall through to
                        // `unreachable`, or to a silent identity on the
                        // conversion shape.
                        let name = wasm_decl.instruction.as_str();
                        let ok = if wasm_decl.type_ref.is_some() {
                            crate::check::wasm_instructions::known_with_type_basis(name)
                        } else {
                            crate::check::wasm_instructions::known(name)
                        };
                        if !ok {
                            ctx.errors.push(
                                crate::check::errors::e2_resolve::ResolveError::UnknownWasmInstruction {
                                    name: wasm_decl.instruction.clone(),
                                    span: wasm_decl.instruction_span,
                                }
                                .to_diagnostic(db, ctx.scope.file(db)),
                            );
                        }
                    }
                    // Every operand is a declared variable (marked used, so
                    // the unused-variable lint stays quiet) or the return.
                    // The lowering reads and writes exactly these names; an
                    // unknown one used to be ignored while the FUNCTION's own
                    // parameter list was lowered instead.
                    let def_map = self.scope.def_map(db);
                    let mut operand = |ident: &SpanIdent<'db>| {
                        let key = ident.ident.caseless(db);
                        if let Some(var) = def_map
                            .local_variables
                            .get(&key)
                            .or_else(|| def_map.global_variables.get(&key))
                        {
                            ctx.variables_used.insert(*var);
                            return;
                        }
                        let Some(f) = function else { return };
                        if f.name(db).caseless(db) == key && f.return_type(db).is_some() {
                            return;
                        }
                        ctx.errors.push(
                            crate::check::errors::e2_resolve::ResolveError::UnknownWasmOperand {
                                name: ident.ident.text(db).clone(),
                                span: ident.get_span(db),
                            }
                            .to_diagnostic(db, ctx.scope.file(db)),
                        );
                    };
                    if let Some(t) = &wasm_decl.type_ref {
                        operand(t);
                    }
                    for p in &wasm_decl.params {
                        operand(p);
                    }
                    if let Some(r) = &wasm_decl.result {
                        operand(r);
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

/// Whether a FOR control access is a single bare identifier — no path steps
/// past the root, no indexing, no dereference, no partial (bit) access.
fn for_control_is_bare_identifier<'db>(
    db: &'db dyn WorkspaceDataBase,
    access: crate::hir_def::expressions::expression::VariableAccess<'db>,
) -> bool {
    use crate::hir_def::expressions::expression::{PathExprKind, VariableAccessKind};

    if access.multibits(db).is_some() {
        return false;
    }
    let VariableAccessKind::Symbolic(begin) = access.kind(db) else {
        // A directly represented variable (%MW0) is not an identifier.
        return false;
    };
    // THIS.x etc. — an invocation prefix is already more than an identifier.
    if begin.invocation(db).is_some() {
        return false;
    }
    let Some(path) = begin.expr(db) else {
        return false;
    };
    matches!(path.expr(db), PathExprKind::VarAccess(_))
}

/// Refuse a string literal wider than the destination it is assigned to.
///
/// The capacity comes from the destination's SPEC ([`declared_string_capacity`]
/// follows alias hops), falling back to the default every plain `STRING`
/// stores at. Only literal right-hand sides are measured: a runtime string is
/// clamped by the runtime's copy, which cannot be seen from here.
///
/// [`declared_string_capacity`]: crate::hir_ty::infer::normalize::declared_string_capacity
fn check_string_literal_fits<'db>(
    db: &'db dyn WorkspaceDataBase,
    base_typ: Type<'db>,
    target: Expr<'db>,
    ctx: &mut BodyInferenceResult<'db>,
) {
    use crate::hir_def::expressions::expression::{ExprKind, PrimaryExpr};
    use crate::check::errors::e3_type::InferLiteralError;

    let mut spec = match base_typ {
        Type::Variable((var, None)) => var.spec(db),
        Type::StructElement(el) => el.spec(db),
        _ => return,
    };
    // A subscripted destination still resolves to the ARRAY variable, so the
    // element is where the capacity lives: `a[1] := <literal>` for an
    // `ARRAY OF STRING[4]` measured nothing and cut the value at 4 silently,
    // while the same literal into a plain `STRING[4]` was refused.
    let mut depth = 0;
    while let Type::Array(array) = spec.infer(db).normalize(db) {
        spec = array.of_type(db);
        depth += 1;
        if depth > 16 {
            return;
        }
    }
    if !matches!(
        spec.infer(db).normalize(db),
        Type::Elementary(crate::hir_def::expressions::spec::ElementarySpec::String)
    ) {
        return;
    }
    let capacity = crate::hir_ty::infer::normalize::declared_string_capacity(db, spec)
        .map(u64::from)
        .unwrap_or(80);

    let ExprKind::PrimaryExpr(PrimaryExpr::Literal(
        crate::hir_def::expressions::expression::Elementary::String(lit),
    )) = target.expr(db)
    else {
        return;
    };
    let Ok(bytes) = lit.as_single_string(db) else {
        return;
    };
    if bytes.len() as u64 > capacity {
        let err = InferLiteralError::Invalid_STRING_Length {
            max: capacity,
            got: bytes.len(),
        };
        ctx.errors.push(
            crate::check::errors::e3_type::TypeError::InferLiteralError {
                expr: target,
                source: None,
                target: Type::Elementary(
                    crate::hir_def::expressions::spec::ElementarySpec::String,
                ),
                err,
            }
            .to_diagnostic(db, ctx.scope.file(db)),
        );
    }
}
