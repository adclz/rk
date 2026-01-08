use auto_lsp::default::db::BaseDatabase;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    AstId, CallSite, HirNodeInfo,
    check::errors::{
        analysis_error::ToIdeDiagnostic,
        body_inference::{BodyInferenceError, TypeError},
    },
    hir_def::{
        expressions::{
            expression::{Expr, FuncCall, ParamAssignKind, VariableAccess},
            invocation::{Invocation, InvocationKind},
            statement::{CaseKind, Stmt, StmtKind},
        },
        pous::{pou::Pou, variable::VariableDecl},
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{
        body_inference::BodyInferenceResult,
        infer::{expr::InferExprCtx, inference_table::InferenceTable},
        resolver::{Resolver, func_call::resolve_func_call},
        ty::Type,
    },
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum NestedScope {
    Loop,
    None,
}

pub struct InferenceCtx<'db> {
    pub scope: ScopeId<'db>,
    pub nested_scope: NestedScope,
}

impl<'db> InferenceCtx<'db> {
    pub fn new(scope: ScopeId<'db>) -> Self {
        InferenceCtx {
            scope,
            nested_scope: NestedScope::None,
        }
    }

    pub fn check_statements(
        &self,
        db: &'db dyn BaseDatabase,
        resolver: Resolver<'db>,
        statements: &'db [Stmt<'db>],
        nested_scope: NestedScope,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        let mut infer = InferExprCtx::new(resolver);

        for stmt in statements {
            match stmt.stmt(db) {
                StmtKind::EmptyPathExpression(expr) => {
                    let _ = resolver.resolve_begin_path_expr(db, *expr, ctx);
                }

                StmtKind::AssignmentAttempt { var, target } => { /* todo */ }

                StmtKind::Assignment { var, target } => {
                    resolver.resolve_variable_access(db, *var, ctx);
                    let base_typ = ctx
                        .get_type_of_variable_access(db, *var)
                        .unwrap_or_default();

                    base_typ.check_assignable(db, CallSite::from_var_access(db, *var), ctx);

                    self.infer_and_check_expr(db, &mut infer, *target, ctx);

                    if let Err(err) = infer.coerce_var_access_with_expr(db, *var, *target, ctx) {
                        ctx.errors.push(err.into_non_assignable(
                            db,
                            base_typ,
                            CallSite::from_expr(db, *target),
                        ));
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
                            CallSite::from_expr(db, *condition),
                        ));
                    }

                    // check branches

                    // THEN
                    if let Some(then) = then {
                        self.check_statements(db, resolver, then, NestedScope::None, ctx);
                    }

                    // ELSE IFs
                    for (cond, stmts) in else_if {
                        self.infer_and_check_expr(db, &mut infer, *cond, ctx);

                        if let Err(err) =
                            infer.coerce_type_with_expr(db, Type::new_bool(), *condition, ctx)
                        {
                            ctx.errors.push(err.into_non_assignable(
                                db,
                                Type::new_bool(),
                                CallSite::from_expr(db, *condition),
                            ));
                        }

                        self.check_statements(db, resolver, stmts, NestedScope::None, ctx);
                    }

                    // ELSE
                    if let Some(else_) = else_ {
                        self.check_statements(db, resolver, else_, NestedScope::None, ctx);
                    }
                }

                StmtKind::While { condition, body } | StmtKind::Repeat { condition, body } => {
                    self.infer_and_check_expr(db, &mut infer, *condition, ctx);

                    if let Err(err) =
                        infer.coerce_type_with_expr(db, Type::new_bool(), *condition, ctx)
                    {
                        ctx.errors.push(err.into_non_assignable(
                            db,
                            Type::new_bool(),
                            CallSite::from_expr(db, *condition),
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
                    let control_typ = ctx
                        .type_of_variable_access_with_adjustments(db, *control_variable)
                        .unwrap_or_default();

                    control_typ.check_assignable(
                        db,
                        CallSite::from_var_access(db, *control_variable),
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
                            CallSite::from_expr(db, *start),
                        ));
                    }

                    if let Err(err) =
                        infer.coerce_var_access_with_expr(db, *control_variable, *end, ctx)
                    {
                        ctx.errors.push(
                            err.into_non_comparable(
                                db,
                                ctx.get_type_of_variable_access(db, *control_variable)
                                    .unwrap_or_default(),
                                CallSite::from_expr(db, *end),
                            ),
                        );
                    }

                    if let Some(step) = step {
                        self.infer_and_check_expr(db, &mut infer, *step, ctx);

                        if let Err(err) =
                            infer.coerce_var_access_with_expr(db, *control_variable, *step, ctx)
                        {
                            ctx.errors.push(
                                err.into_non_comparable(
                                    db,
                                    ctx.get_type_of_variable_access(db, *control_variable)
                                        .unwrap_or_default(),
                                    CallSite::from_expr(db, *step),
                                ),
                            );
                        }
                    }

                    self.check_statements(db, resolver, body, NestedScope::Loop, ctx);
                }

                StmtKind::FuncCall(call) => {
                    resolve_func_call(db, resolver, *call, ctx);

                    let typ = ctx
                        .type_of_begin_expr_with_adjustments(db, call.path(db))
                        .unwrap_or_default();

                    if typ.with_return_type(db).is_some() {
                        ctx.errors.push(
                            TypeError::UnusedReturnType { typ, expr: *stmt }.to_diagnostic(db),
                        );
                    }
                }

                StmtKind::Continue | StmtKind::Exit => {
                    if nested_scope != NestedScope::Loop {
                        ctx.errors.push(
                            BodyInferenceError::ContinueOutsideLoop { stmt: *stmt }
                                .to_diagnostic(db),
                        );
                    }
                }

                StmtKind::Case {
                    condition,
                    cases,
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
                            CallSite::from_expr(db, *condition),
                        ));
                    }

                    // check cases
                    for (case_kind, stmts) in cases { /* todo */ }

                    // check else
                    if let Some(else_) = else_ {
                        self.check_statements(db, resolver, else_, NestedScope::None, ctx);
                    }
                }

                StmtKind::Return => {}
            }
        }
    }

    fn infer_and_check_expr(
        &self,
        db: &'db dyn BaseDatabase,
        infer: &mut InferExprCtx<'db>,
        expr: Expr<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        infer.resolve_expr(db, expr, ctx);
        infer.check_expr(db, expr, ctx);
    }
}
