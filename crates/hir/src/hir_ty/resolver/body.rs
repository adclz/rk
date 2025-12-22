use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

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

pub struct BodyResolverCtx<'db> {
    pub scope: ScopeId<'db>,
    pub nested_scope: NestedScope,
}

impl<'db> BodyResolverCtx<'db> {
    pub fn new(scope: ScopeId<'db>) -> Self {
        BodyResolverCtx {
            scope,
            nested_scope: NestedScope::None,
        }
    }

    /// Resolve all expression statements for a given scope
    pub fn resolve_statements(
        &self,
        db: &'db dyn BaseDatabase,
        resolver: Resolver<'db>,
        statements: &'db [Stmt<'db>],
        nested_scope: NestedScope,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        let mut infer_ctx = InferExprCtx::new(resolver);

        for stmt in statements.iter() {
            match stmt.stmt(db) {
                StmtKind::EmptyPathExpression(expr) => {
                    let _ = resolver.resolve_begin_path_expr(db, *expr, ctx);
                }
                StmtKind::Assignment { var, target } => {
                    let var_lhs = resolver.resolve_variable_access(db, *var, ctx);
                    let call_site = CallSite::from_var_access(db, *var);

                    // is the variable assignable?
                    var_lhs.is_assignable(db, call_site, ctx);

                    // type coercion
                    var_lhs
                        .coerce_with_expression(db, call_site, *target, resolver, ctx)
                        .map_err(|err| {
                            ctx.errors
                                .push(err.into_non_assignable(db, var_lhs, *target))
                        })
                        .ok();
                }

                StmtKind::AssignmentAttempt { var, target } => {
                    // fixme: assignment attempts should only be REF_TO
                    // todo!
                }
                StmtKind::If {
                    condition,
                    then,
                    else_if,
                    else_,
                } => {
                    Type::new_bool()
                        .coerce_with_expression(
                            db,
                            CallSite::from_expr(db, *condition),
                            *condition,
                            resolver,
                            ctx,
                        )
                        .map_err(|err| {
                            ctx.errors.push(err.into_non_assignable(
                                db,
                                Type::new_bool(),
                                *condition,
                            ))
                        })
                        .ok();

                    // Analyze THEN block
                    if let Some(then) = then.as_ref() {
                        self.resolve_statements(db, resolver, then, NestedScope::None, ctx);
                    }

                    // Analyze ELSE IF blocks
                    for (condition, stmt) in else_if {
                        Type::new_bool()
                            .coerce_with_expression(
                                db,
                                CallSite::from_expr(db, *condition),
                                *condition,
                                resolver,
                                ctx,
                            )
                            .map_err(|err| {
                                ctx.errors.push(err.into_non_assignable(
                                    db,
                                    Type::new_bool(),
                                    *condition,
                                ))
                            })
                            .ok();

                        self.resolve_statements(db, resolver, stmt, NestedScope::None, ctx);
                    }

                    // Analyze ELSE block
                    if let Some(else_) = else_.as_ref() {
                        self.resolve_statements(db, resolver, else_, NestedScope::None, ctx);
                    }
                }
                StmtKind::For {
                    control_variable,
                    start,
                    end,
                    step,
                    body,
                } => {
                    Self::check_expr_assign(
                        db,
                        resolver.resolve_variable_access(db, *control_variable, ctx),
                        *start,
                        &mut infer_ctx,
                        ctx,
                    );

                    Self::check_expr_assign(
                        db,
                        resolver.resolve_variable_access(db, *control_variable, ctx),
                        *end,
                        &mut infer_ctx,
                        ctx,
                    );

                    if let Some(step) = step {
                        Self::check_expr_assign(
                            db,
                            resolver.resolve_variable_access(db, *control_variable, ctx),
                            *step,
                            &mut infer_ctx,
                            ctx,
                        );
                    }

                    self.resolve_statements(db, resolver, body, NestedScope::Loop, ctx);
                }
                StmtKind::While { condition, body } => {
                    Self::check_expr_assign(db, Type::new_bool(), *condition, &mut infer_ctx, ctx);

                    self.resolve_statements(db, resolver, body, NestedScope::Loop, ctx);
                }
                StmtKind::Repeat { condition, body } => {
                    Self::check_expr_assign(db, Type::new_bool(), *condition, &mut infer_ctx, ctx);

                    self.resolve_statements(db, resolver, body, NestedScope::Loop, ctx);
                }
                StmtKind::FuncCall(f) => {
                    let typ = resolve_func_call(db, resolver, *f, ctx);
                    if typ.with_return_type(db).is_some() {
                        ctx.errors.push(
                            TypeError::UnusedReturnType { typ, expr: *stmt }.to_diagnostic(db),
                        );
                    }
                }
                StmtKind::Case {
                    condition,
                    cases,
                    else_,
                } => {
                    let cond = infer_ctx.infer_expr(db, *condition, ctx);

                    for (cases, stmts) in cases {
                        for case in cases {
                            match case {
                                CaseKind::Expression(expr) => {
                                    self.check_compare(db, cond, *expr, &mut infer_ctx, ctx);
                                }
                                CaseKind::Subrange { lower, upper } => {
                                    self.check_compare(db, cond, *lower, &mut infer_ctx, ctx);
                                    self.check_compare(db, cond, *upper, &mut infer_ctx, ctx);
                                }
                            }
                        }

                        self.resolve_statements(db, resolver, stmts, NestedScope::None, ctx);
                    }

                    if let Some(else_) = else_.as_ref() {
                        self.resolve_statements(db, resolver, else_, NestedScope::None, ctx);
                    }
                }
                StmtKind::Continue => {
                    if nested_scope != NestedScope::Loop {
                        ctx.errors.push(
                            BodyInferenceError::ContinueOutsideLoop { stmt: *stmt }
                                .to_diagnostic(db),
                        );
                    }
                }
                StmtKind::Exit => {
                    if nested_scope != NestedScope::Loop {
                        ctx.errors.push(
                            BodyInferenceError::ExitOutsideLoop { stmt: *stmt }.to_diagnostic(db),
                        );
                    }
                }
                StmtKind::Return => {
                    // Nothing to do for return statements yet
                    // We could return a warning if RETURN is the followed by other statements
                    // But this seems to be the job of the MIR layer
                }
            }
        }
    }

    fn check_expr_assign(
        db: &'db dyn BaseDatabase,
        target: Type<'db>,
        expr: Expr<'db>,
        infer_ctx: &mut InferExprCtx<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        infer_ctx.infer_expr(db, expr, ctx);
        infer_ctx
            .inference_table
            .set_target_type(db, CallSite::from_expr(db, expr), target);

        infer_ctx
            .inference_table
            .resolve_completly(db, infer_ctx.resolver, ctx);

        let value = ctx
            .type_of_expr_with_adjustments(db, expr)
            .unwrap_or_default();

        if let Err(err) = target.coerce_with_type(db, value, infer_ctx.resolver) {
            ctx.errors.push(
                TypeError::NotAssignable {
                    base_target: target,
                    target: err.expected,
                    value: err.actual,
                    expr: CallSite::from_expr(db, expr),
                }
                .to_diagnostic(db),
            )
        };
    }

    fn check_compare(
        &self,
        db: &'db dyn BaseDatabase,
        lhs: Type<'db>,
        expr: Expr<'db>,
        infer_ctx: &mut InferExprCtx<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        let rhs = infer_ctx.infer_expr(db, expr, ctx);
        infer_ctx
            .inference_table
            .resolve_completly(db, infer_ctx.resolver, ctx);
        if let Err(err) = lhs.coerce_with_type(db, rhs, infer_ctx.resolver) {
            ctx.errors.push(
                TypeError::NotComparable {
                    lhs: err.expected,
                    rhs: err.actual,
                    expr,
                }
                .to_diagnostic(db),
            )
        };
    }
}
