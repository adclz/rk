use db::WorkspaceDataBase;

use crate::{
    CallSite, HirNodeInfo,
    check::errors::{
        ToIdeDiagnostic, e5_inheritance::InheritanceError,
        e10_control_flow::ControlFlowError,
    },
    hir_def::{
        expressions::{
            expression::{Elementary, Expr, ExprKind, PrimaryExpr},
            spec::ElementarySpec,
            statement::{CaseKind, Stmt, StmtKind},
        },
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
