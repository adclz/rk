// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use db::WorkspaceDataBase;

use crate::check::errors::e04_init::InitError;
use crate::check::errors::e15_pragma::PragmaError;
use crate::{
    CallSite, HirNodeInfo,
    check::errors::{ToIdeDiagnostic, e11_oop::OopError, e12_control_flow::ControlFlowError},
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
        infer::{Infer, expr::InferExprCtx, table::InferenceTable},
        resolver::{Resolver, func_call::resolve_func_call},
        ty::{InferType, Type},
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
/// (E1205).
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
    // on a STRING denotes a lexicographic set the compiler has no
    // representation for, and ordering is the whole point of a range.
    allow_non_integer: bool,
    // The selector's type: the value is recorded as the selector holds it.
    selector: Type<'db>,
    ctx: &mut BodyInferenceResult<'db>,
) {
    // On a CHAR, a label is its code point, which orders `'a'..'z'` as the
    // selector compares.
    if matches!(
        selector.normalize(db),
        Type::Elementary(ElementarySpec::Char)
    ) && let ExprKind::PrimaryExpr(PrimaryExpr::Literal(
        Elementary::InferString(ident) | Elementary::Char(ident),
    )) = label.expr(db)
        && let Ok(bytes) = ident.as_single_string(db)
        && let Ok(code) = crate::hir_ty::infer::literals::char_literal_code_point(&bytes)
    {
        ctx.case_label_value
            .insert(label, CaseLabelValue::Int(i128::from(code)));
        return;
    }
    // An enum label's value is its variant's ordinal, which lowering reads
    // from the variant table; there is nothing to record here.
    match ctx.get_type_of_expr(label).normalize(db) {
        Type::Enum(_) | Type::EnumVariant(..) if allow_non_integer => return,
        // The label is evaluated BEFORE it coerces to the selector, so a bare
        // literal is still untyped here.
        Type::Elementary(ElementarySpec::String) | Type::Infer(InferType::String(_))
            if allow_non_integer =>
        {
            // A string LITERAL is constant; a STRING variable is not. Record
            // the DECODED bytes, so `STRING#'a'` and `'a'` are one label and
            // an escape is compared by what it denotes.
            if let ExprKind::PrimaryExpr(PrimaryExpr::Literal(
                Elementary::String(ident)
                | Elementary::InferString(ident)
                | Elementary::Char(ident),
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
            let value = crate::hir_ty::infer::const_eval::held_as(db, value, selector);
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
/// pragma could silence it because E0902 is a compiler error.
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
        ExprKind::UnaryOperator {
            expr,
            operator: UnaryOperatorKind::Not,
        } => null_guards(db, *expr, ctx).swapped(),
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
/// the non-null path alive. Any terminator among the branch's own statements
/// ends it, since what follows one is dead: read from the last statement
/// alone, `RETURN; x := 1;` fell through, and `__RAISE` never counted.
fn falls_through<'db>(db: &'db dyn WorkspaceDataBase, stmts: &[Stmt<'db>]) -> bool {
    !stmts.iter().any(|s| terminates(db, *s))
}

/// A statement nothing in its block can follow: the ones after it are dead.
fn terminates<'db>(db: &'db dyn WorkspaceDataBase, stmt: Stmt<'db>) -> bool {
    matches!(
        stmt.stmt(db),
        StmtKind::Return | StmtKind::Exit | StmtKind::Continue | StmtKind::Raise { .. }
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

        // The null state the block's first terminator was reached with. The
        // statements after it are dead, inferred and checked like the
        // others so that an undeclared name or a mismatch there is reported
        // and lowering finds their types, but what they do to a reference
        // never happens: the state is put back once the block ends.
        let mut state_at_terminator = None;

        for (i, stmt) in statements.iter().enumerate() {
            if terminates(db, *stmt) && state_at_terminator.is_none() {
                for dead in &statements[i + 1..] {
                    ctx.dead_code_statements.push(*dead);
                }
                state_at_terminator = Some(ctx.ref_null_state.clone());
            }
            match stmt.stmt(db) {
                StmtKind::EmptyPathExpression(expr) => {
                    resolver.resolve_begin_path_expr(db, *expr, None, ctx);
                    // Rule 2 (IEC 6.6.7.2.9): SUPER() shall occur once in the FB
                    // body. Track the first; a second occurrence is E1110,
                    // pointing back to the first. (SUPER() in a method is E1109,
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
                                OopError::SuperBodyInLoop {
                                    call_site: stmt.as_call_site(db),
                                }
                                .to_diagnostic(db, ctx.scope.file(db)),
                            );
                        }
                        match ctx.first_super_body {
                            Some(first) => ctx.errors.push(
                                OopError::SuperBodyMultiple {
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

                    // A field or an element of a constant is the constant.
                    // A CONSTANT variable itself is refused by
                    // `check_assignable`, below.
                    if ctx.is_constant_place(db, *var) && !is_constant_variable(db, base_typ) {
                        ctx.errors.push(
                            InitError::AssignToConstant {
                                access: CallSite::from_scoped(db, var),
                                constant: ctx.constant_root(db, *var),
                            }
                            .to_diagnostic(db, ctx.scope.file(db)),
                        );
                    }

                    refuse_edge_input_as_storage(
                        db,
                        *var,
                        crate::check::errors::e02_resolve::EdgeUse::Written,
                        ctx,
                    );
                    let assignable =
                        base_typ.check_assignable(db, CallSite::from_scoped(db, var), ctx);

                    // Design 1: an interface parameter is a fixed binding to the
                    // concrete type the caller supplied; reassigning it would
                    // break monomorphization (see E1124).
                    if let Type::Variable((var_decl, _)) = base_typ
                        && matches!(
                            var_decl.spec(db).infer(db).normalize(db),
                            Type::Interface(_)
                        )
                    {
                        ctx.errors.push(
                            OopError::InterfaceParamNotAssignable {
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
                    } else if assignable {
                        // An instance is copied whole. One with a member
                        // declared `AT %I*` holds the address of its channel
                        // there, and the copy would write the other
                        // instance's over it (E1427).
                        check_copy_keeps_location(
                            db,
                            ctx.type_of_variable_access_with_adjustments(db, *var),
                            CallSite::from_scoped(db, var),
                            ctx,
                        );
                    }

                    // A string literal must FIT the destination. The store
                    // runs at the destination's capacity — 80 unless the spec
                    // says otherwise — so an over-long literal was silently
                    // cut there: a 149-byte literal read back as its first 80
                    // bytes, from a compile that said nothing. The
                    // initializer door has always refused this; the
                    // assignment door now matches it.
                    check_string_literal_fits(db, base_typ, *var, *target, ctx);
                    if let Some(err) = ctx.ref_subrange_mismatch(db, base_typ, *target) {
                        ctx.errors.push(err.to_diagnostic(db, ctx.scope.file(db)));
                    }

                    // A reference to this call's own storage, handed back to
                    // the caller. It does not fault: an address-taken local
                    // sits at a fixed address, so the reference quietly reads
                    // whatever the NEXT call leaves in that slot.
                    if ctx.writes_result(db, *var)
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
                            crate::check::errors::e09_reference::ReferenceError::ReturnsReferenceToLocal {
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
                    // is not a counter. A bare name
                    // resolving to an FB/PROGRAM member is fine: the rule is
                    // about the syntax, not where the variable lives. Without
                    // this check the shapes sailed through to MIR, which
                    // rejected them with an unlocated "unsupported" error —
                    // `rk check` said one thing and `rk compile` another.
                    if !for_control_is_bare_identifier(db, *control_variable) {
                        ctx.errors.push(
                            crate::check::errors::e12_control_flow::ControlFlowError::ForControlNotAVariable {
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
                    // IEC counts in ANY_INT; a subrange or an alias of an
                    // integer normalizes to it. A bit of a wider address is a
                    // BOOL, so this refuses it too.
                    let counted = control_typ.normalize(db);
                    if !control_typ.is_never()
                        && !counted.is_never()
                        && !(counted.is_signed_integer() || counted.is_unsigned_integer())
                    {
                        ctx.errors.push(
                            crate::check::errors::e12_control_flow::ControlFlowError::ForControlNotInteger {
                                access: CallSite::from_scoped(db, control_variable),
                                ty: control_typ,
                            }
                            .to_diagnostic(db, ctx.scope.file(db)),
                        );
                    }

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
                        // value, since BY 0 never advances the counter. It is
                        // the value the counter adds.
                        match crate::hir_ty::infer::const_eval::const_int(db, *step, ctx)
                            .map(|v| crate::hir_ty::infer::const_eval::held_as(db, v, control_typ))
                        {
                            Some(0) => ctx.errors.push(
                                crate::check::errors::e12_control_flow::ControlFlowError::ForStepInvalid {
                                    step: CallSite::from_scoped(db, step),
                                    zero: true,
                                    decl: None,
                                }
                                .to_diagnostic(db, ctx.scope.file(db)),
                            ),
                            // The counter's 64 bits, as lowering emits them.
                            Some(v) => {
                                ctx.for_step_value.insert(*step, v as i64);
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
                                    crate::check::errors::e12_control_flow::ControlFlowError::ForStepInvalid {
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
                    let bound = |bound: Expr<'db>, ctx: &BodyInferenceResult<'db>| {
                        crate::hir_ty::infer::const_eval::const_int(db, bound, ctx)
                            .map(|v| crate::hir_ty::infer::const_eval::held_as(db, v, control_typ))
                    };
                    if let (Some(start_val), Some(end_val)) = (bound(*start, ctx), bound(*end, ctx))
                    {
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
                }

                StmtKind::Case {
                    condition,
                    cases,
                    else_,
                } => {
                    // check condition
                    self.infer_and_check_expr(db, &mut infer, *condition, ctx);

                    // A literal selector, `CASE 5 OF` or `CASE 'bd' OF`, takes
                    // its type from the labels, as the two sides of `=` do:
                    // `CASE 5 OF DINT#5:` compares as DINT, and with only
                    // literals for labels the selector is an INT or a STRING.
                    // Left a literal, every label was refused as not
                    // comparable with it. The labels are inferred here for
                    // that, and not again below.
                    let literal = ctx.type_of_expr_with_adjustments(db, *condition);
                    let literal_selector = literal.has_infer();
                    if literal_selector {
                        let mut table = InferenceTable::new();
                        table.add_type(db, *condition, literal, resolver);
                        for kind in cases.iter().flat_map(|(kinds, _)| kinds) {
                            let label_exprs = match kind {
                                CaseKind::Expression(expr) => [Some(*expr), None],
                                CaseKind::Subrange { lower, upper } => [Some(*lower), Some(*upper)],
                            };
                            for label in label_exprs.into_iter().flatten() {
                                self.infer_and_check_expr(db, &mut infer, label, ctx);
                                let ty = ctx.type_of_expr_with_adjustments(db, label);
                                table.add_type(db, label, ty, resolver);
                            }
                        }
                        table.resolve_completly(db, resolver, ctx);
                    }

                    // After its adjustments: `CASE names[i] OF` selects on
                    // an element, not on the array.
                    let condition_typ = ctx.type_of_expr_with_adjustments(db, *condition);

                    // CASE branches on an integer, a bit string, a CHAR, an
                    // enum or a STRING. Another selector is refused once, and
                    // its labels are not checked against it.
                    let declared = ctx.type_of_expr_with_adjustments(db, *condition);
                    let selector = declared.normalize(db);
                    let selectable = selector.is_never()
                        || selector.is_signed_integer()
                        || selector.is_unsigned_integer()
                        || selector.is_binary_integer()
                        || matches!(
                            selector,
                            Type::Enum(_)
                                | Type::EnumVariant(..)
                                | Type::Elementary(ElementarySpec::Char | ElementarySpec::String)
                        );
                    if !selectable {
                        ctx.errors.push(
                            crate::check::errors::e12_control_flow::ControlFlowError::CaseSelectorNotSupported {
                                selector: CallSite::from_scoped(db, condition),
                                ty: declared,
                            }
                            .to_diagnostic(db, ctx.scope.file(db)),
                        );
                    }

                    // check cases
                    for (case_kind, stmts) in cases {
                        for case in case_kind {
                            match case {
                                CaseKind::Expression(expr) if !selectable => {
                                    if !literal_selector {
                                        self.infer_and_check_expr(db, &mut infer, *expr, ctx);
                                    }
                                }
                                CaseKind::Subrange { lower, upper } if !selectable => {
                                    if !literal_selector {
                                        self.infer_and_check_expr(db, &mut infer, *lower, ctx);
                                        self.infer_and_check_expr(db, &mut infer, *upper, ctx);
                                    }
                                }
                                CaseKind::Expression(expr) => {
                                    if !literal_selector {
                                        self.infer_and_check_expr(db, &mut infer, *expr, ctx);
                                    }
                                    check_case_label_constant(db, *expr, true, condition_typ, ctx);

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
                                    if !literal_selector {
                                        self.infer_and_check_expr(db, &mut infer, *lower, ctx);
                                        self.infer_and_check_expr(db, &mut infer, *upper, ctx);
                                    }
                                    check_case_label_constant(
                                        db,
                                        *lower,
                                        false,
                                        condition_typ,
                                        ctx,
                                    );
                                    check_case_label_constant(
                                        db,
                                        *upper,
                                        false,
                                        condition_typ,
                                        ctx,
                                    );
                                    // Both bounds as the selector holds them: a
                                    // reversed range holds no value.
                                    if let (
                                        Some(CaseLabelValue::Int(low)),
                                        Some(CaseLabelValue::Int(high)),
                                    ) = (
                                        ctx.case_label_value.get(lower),
                                        ctx.case_label_value.get(upper),
                                    ) && low > high
                                    {
                                        let (lower_value, upper_value) = (*low, *high);
                                        ctx.errors.push(
                                            crate::check::errors::e12_control_flow::ControlFlowError::CaseRangeEmpty {
                                                range: CallSite::from_scoped(db, lower),
                                                lower: lower_value,
                                                upper: upper_value,
                                                chars: matches!(
                                                    selector,
                                                    Type::Elementary(ElementarySpec::Char)
                                                ),
                                            }
                                            .to_diagnostic(db, ctx.scope.file(db)),
                                        );
                                    }

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

                StmtKind::Return => {}

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
                }

                // Nothing to infer: the pragma only marks the next statement
                // for the linter.
                StmtKind::AllowPragma(_) => {}

                StmtKind::WasmPragma(wasm_decl) => {
                    use crate::check::wasm_instructions as wasm;
                    use crate::hir_def::interned::identifier::{Ident, SpanIdent};
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
                    let mut instruction_known = false;
                    if function.is_none() {
                        ctx.errors.push(
                            crate::check::errors::e15_pragma::PragmaError::WasmPragmaOutsideFunction {
                                span: wasm_decl.instruction_span,
                            }
                            .to_diagnostic(db, ctx.scope.file(db)),
                        );
                    } else {
                        // An unknown name used to fall through to
                        // `unreachable`, or to a silent identity on the
                        // conversion shape.
                        let name = wasm_decl.instruction.as_str();
                        instruction_known = if wasm_decl.type_ref.is_some() {
                            wasm::known_with_type_basis(name)
                        } else {
                            wasm::known(name)
                        };
                        if !instruction_known {
                            ctx.errors.push(
                                crate::check::errors::e15_pragma::PragmaError::UnknownWasmInstruction {
                                    name: wasm_decl.instruction.clone(),
                                    span: wasm_decl.instruction_span,
                                }
                                .to_diagnostic(db, ctx.scope.file(db)),
                            );
                        }
                    }
                    // Every operand is a declared variable (marked used, so
                    // the unused-variable lint stays quiet) or the return,
                    // and carries its type to the signature check below.
                    // The lowering reads and writes exactly these names; an
                    // unknown one used to be ignored while the FUNCTION's own
                    // parameter list was lowered instead.
                    let def_map = self.scope.def_map(db);
                    let mut all_known = true;
                    let mut operand = |ident: &SpanIdent<'db>| -> Option<(Ident, Type<'db>)> {
                        let key = ident.ident(db);
                        if let Some(var) = def_map
                            .local_variables
                            .get(&key)
                            .or_else(|| def_map.global_variables.get(&key))
                        {
                            ctx.variables_used.insert(*var);
                            // Named as written: only the refusal below shows them.
                            return Some((var.name_with_case(db), var.spec(db).infer(db)));
                        }
                        let Some(f) = function else {
                            all_known = false;
                            return None;
                        };
                        if f.name(db) == key
                            && let Some(ret) = f.return_type(db)
                        {
                            return Some((f.name_with_case(db), ret.infer(db)));
                        }
                        all_known = false;
                        ctx.errors.push(
                            crate::check::errors::e15_pragma::PragmaError::UnknownWasmOperand {
                                name: ident.with_case,
                                span: ident.get_span(db),
                            }
                            .to_diagnostic(db, ctx.scope.file(db)),
                        );
                        None
                    };
                    let basis = match &wasm_decl.type_ref {
                        Some(t) => operand(t),
                        None => None,
                    };
                    let mut params: Vec<(Ident, Type<'db>)> = Vec::new();
                    for p in &wasm_decl.params {
                        if let Some(typed) = operand(p) {
                            params.push(typed);
                        }
                    }
                    let result = match &wasm_decl.result {
                        Some(r) => operand(r),
                        None => None,
                    };
                    // The operands against the instruction: lanes, count and
                    // result. The module validator used to be the first to
                    // say so, at load.
                    if function.is_some()
                        && instruction_known
                        && all_known
                        && let Some(refusal) = wasm::check_signature(
                            db,
                            wasm_decl.instruction.as_str(),
                            basis,
                            &params,
                            result,
                        )
                    {
                        let error = match refusal {
                            wasm::Refusal::Mismatch {
                                instruction,
                                expected,
                                actual,
                            } => PragmaError::WasmSignatureMismatch {
                                instruction,
                                expected,
                                actual,
                                span: wasm_decl.instruction_span,
                            },
                            wasm::Refusal::Unknown(name) => PragmaError::UnknownWasmInstruction {
                                name,
                                span: wasm_decl.instruction_span,
                            },
                        };
                        ctx.errors.push(error.to_diagnostic(db, ctx.scope.file(db)));
                    }
                }
            }
        }

        if let Some(state) = state_at_terminator {
            ctx.ref_null_state = state;
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

/// The edge input a path names in its own block: by its bare name or as
/// `THIS.name`, where it stands for its edge, a value computed for the call,
/// which has no storage to write, reference or bind by reference (E0211).
/// Through an instance, `fb.start` is the input as it was given. `ty` is the
/// path's type.
pub(crate) fn edge_input_named<'db>(
    db: &'db dyn WorkspaceDataBase,
    begin: crate::hir_def::expressions::expression::BeginPathExpr<'db>,
    ty: Type<'db>,
) -> Option<VariableDecl<'db>> {
    use crate::hir_def::expressions::expression::PathExprKind;
    use crate::hir_def::expressions::invocation::InvocationKind;
    if begin
        .invocation(db)
        .is_some_and(|invocation| invocation.kind(db) != InvocationKind::This)
        || !matches!(begin.expr(db)?.expr(db), PathExprKind::VarAccess(_))
    {
        return None;
    }
    let Type::Variable((var, _)) = ty else {
        return None;
    };
    // Anywhere else, E0210 already refused the declaration.
    let in_instance = matches!(
        get_scope(db, var.get_scope_id(db)).kind,
        ScopeKind::Program(_) | ScopeKind::Pou(crate::hir_def::pous::pou::Pou::FunctionBlock(_))
    );
    (var.is_edge_input(db) && in_instance).then_some(var)
}

/// E0211 when `access`, written or bound by reference, names an edge input of
/// its own block.
pub(crate) fn refuse_edge_input_as_storage<'db>(
    db: &'db dyn WorkspaceDataBase,
    access: crate::hir_def::expressions::expression::VariableAccess<'db>,
    usage: crate::check::errors::e02_resolve::EdgeUse,
    ctx: &mut BodyInferenceResult<'db>,
) {
    use crate::hir_def::expressions::expression::VariableAccessKind;
    let VariableAccessKind::Symbolic(begin) = access.kind(db) else {
        return;
    };
    if access.multibits(db).is_some() {
        return;
    }
    let ty = ctx.get_type_of_variable_access(db, access);
    if let Some(var) = edge_input_named(db, begin, ty) {
        ctx.errors.push(
            crate::check::errors::e02_resolve::ResolveError::EdgeInputAsStorage {
                access: CallSite::from_scoped(db, &access),
                var,
                usage,
            }
            .to_diagnostic(db, ctx.scope.file(db)),
        );
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

/// Whether `ty` is a CONSTANT variable written whole, which
/// `check_assignable` refuses.
pub(crate) fn is_constant_variable<'db>(db: &'db dyn WorkspaceDataBase, ty: Type<'db>) -> bool {
    matches!(ty, Type::Variable((var, _)) if var.qualifier(db).contains(crate::Qualifier::CONSTANT))
}

/// Refuse a string literal wider than the destination it is assigned to.
///
/// The capacity comes from the spec the store writes to: the destination's
/// declaration, then an element's for each `[i]` and the referenced one's for
/// each `^`, with aliases followed. Only literal right-hand sides are
/// measured: a runtime string is clamped by the runtime's copy, which cannot
/// be seen from here.
fn check_string_literal_fits<'db>(
    db: &'db dyn WorkspaceDataBase,
    base_typ: Type<'db>,
    access: crate::hir_def::expressions::expression::VariableAccess<'db>,
    target: Expr<'db>,
    ctx: &mut BodyInferenceResult<'db>,
) {
    let Some(spec) = stored_spec(
        db,
        base_typ,
        ctx.adjustments_of_var_access(db, access)
            .unwrap_or_default(),
    ) else {
        return;
    };
    if let Some(err) =
        crate::hir_ty::head::checks::variables::string_literal_overflow(db, spec, target)
    {
        ctx.errors.push(
            crate::check::errors::e03_type::TypeError::InferLiteralError {
                expr: target,
                source: None,
                target: Type::Elementary(ElementarySpec::String),
                err,
            }
            .to_diagnostic(db, ctx.scope.file(db)),
        );
    }
}

/// The spec a store through an access of type `base_typ` writes to: the
/// declaration's, then for each adjustment an element's or a referenced
/// one's. A subscripted destination still resolves to the ARRAY variable,
/// so past the adjustments the element is where the capacity lives.
fn stored_spec<'db>(
    db: &'db dyn WorkspaceDataBase,
    base_typ: Type<'db>,
    adjustments: &[crate::hir_ty::body::Adjustment<'db>],
) -> Option<crate::hir_def::expressions::spec::Spec<'db>> {
    use crate::hir_def::expressions::spec::SpecKind;
    use crate::hir_ty::body::Adjust;
    let mut spec = match base_typ {
        Type::Variable((var, None)) => var.spec(db),
        Type::StructElement(el) => el.spec(db),
        Type::ReturnValue(callable) => *callable.return_type(db)?,
        _ => return None,
    };
    for adjustment in adjustments {
        let named = match (spec.kind(db), spec.infer(db)) {
            (SpecKind::Target(_), Type::DataType(dt)) => dt.spec(db),
            _ => spec,
        };
        spec = match (&adjustment.kind, named.kind(db)) {
            (Adjust::Index, SpecKind::Array(array)) => array.of_type(db),
            (Adjust::Deref, SpecKind::Ref(inner)) => *inner,
            _ => return None,
        };
    }
    Some(crate::hir_ty::head::checks::variables::innermost_element(
        db, spec,
    ))
}

/// E1427 for a copy over `target`, an instance with a member declared
/// `AT %I*`, `%Q*` or `%M*`: the member is the address of its channel, and
/// the copy would write the source instance's over it.
pub(crate) fn check_copy_keeps_location<'db>(
    db: &'db dyn WorkspaceDataBase,
    target: Type<'db>,
    site: CallSite<'db>,
    ctx: &mut BodyInferenceResult<'db>,
) {
    let Some((member, var)) = crate::hir_ty::head::instances::partly_located_in_copy(db, target)
    else {
        return;
    };
    ctx.errors.push(
        crate::check::errors::e14_config::ConfigError::PartlyLocatedCopied {
            site,
            member,
            address: var
                .location(db)
                .map(|dv| compact_str::CompactString::from(dv.to_address(db)))
                .unwrap_or_default(),
        }
        .to_diagnostic(db, ctx.scope.file(db)),
    );
}
