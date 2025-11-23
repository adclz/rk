use auto_lsp::default::db::BaseDatabase;

use crate::{
    AstId, HirNodeInfo, check::errors::body_inference::{BodyInferenceError, TypeError}, hir_def::{
        expressions::{
            expression::{Expr, FuncCall, ParamAssignKind},
            invocation::{Invocation, InvocationKind},
            statement::{CaseKind, Stmt, StmtKind},
        },
        pous::pou::Pou,
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    }, hir_ty::{
        body_inference::BodyInferenceResult, infer::expr::InferExprCtx, ty::Type
    }
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum NestedScope {
    Loop,
    None,
}

pub struct InferCtx<'db> {
    pub scope: ScopeId<'db>,
    pub nested_scope: NestedScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct CallSite<'db> {
    pub scope: ScopeId<'db>,
    pub id: AstId,
}

impl<'db> CallSite<'db> {
    pub fn new(scope: ScopeId<'db>, id: AstId) -> Self {
        Self { scope, id }
    }
}

impl<'db> HirNodeInfo<'db> for CallSite<'db> {
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        self.id
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
        self.scope
    }
}

impl<'db> InferCtx<'db> {
    pub fn new(scope: ScopeId<'db>) -> Self {
        InferCtx {
            scope,
            nested_scope: NestedScope::None,
        }
    }

    pub fn resolve_invocation(
        db: &'db dyn BaseDatabase,
        scope: ScopeId<'db>,
        invocation: Invocation<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        match get_scope(db, scope).kind {
            ScopeKind::MethodDecl(m) => {
                let scope = get_scope(db, m.scope_id(db));
                let parent = scope
                    .parent
                    .expect("A method scope always has a parent scope");
                Self::resolve_invocation(db, parent, invocation, ctx);
            }
            ScopeKind::Pou(pou) => {
                /*
                CLASS:

                7Access reference
                9a THIS: Reference to own methods
                9b SUPER: Access reference to method in base class

                FUNCTION BLOCKS:

                Access reference
                10a THIS:  Reference to own methods
                10b SUPER:  Access reference to method in base function block
                10c SUPER():  Access reference to body in base function block
                */
                match invocation.kind(db) {
                    InvocationKind::SuperBody => match pou.pou(db) {
                        Pou::FunctionBlock(fb) => {
                            ctx.type_of_invocation
                                .insert(invocation, Type::new_pou(db, pou));
                        }
                        _ => {
                            ctx.errors.push(BodyInferenceError::SuperBodyOnIncompatiblePou {
                                call_site: CallSite::new(scope, invocation.keyword_id(db)),
                            });
                        }
                    },
                    InvocationKind::Super => match pou.pou(db) {
                        // fixme: SUPER only gives access to base methods from EXTENDS, not all implemented interfaces
                        Pou::FunctionBlock(_) | Pou::Class(_) => {
                            ctx.type_of_invocation
                                .insert(invocation, Type::new_pou(db, pou));
                        }
                        _ => {
                            ctx.errors.push(BodyInferenceError::SuperOnIncompatiblePou {
                                call_site: CallSite::new(scope, invocation.keyword_id(db)),
                            });
                        }
                    },
                    InvocationKind::This => match pou.pou(db) {
                        Pou::FunctionBlock(_) | Pou::Class(_) => {
                            ctx.type_of_invocation
                                .insert(invocation, Type::new_pou(db, pou));
                        }
                        _ => {
                            ctx.errors.push(BodyInferenceError::ThisOnIncompatiblePou {
                                call_site: CallSite::new(scope, invocation.keyword_id(db)),
                            });
                        }
                    },
                }
            }
            _ => unreachable!("An invocation will always be in a POU scope"),
        }
    }

    pub fn resolve_func_call(
        db: &'db dyn BaseDatabase,
        scope_typ: Type<'db>,
        func_call: FuncCall<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) -> Type<'db> {
        let typ = scope_typ.walk_begin_path_expr(db, func_call.path(db), ctx);
        if let Some(callable) = typ.as_callable() {
            let mut formal_idx = 0;
            for parameter in func_call.params(db) {
                match parameter.kind(db) {
                    ParamAssignKind::NonFormal { value } => {
                        // Try to get the param by index
                        let var = callable
                            .def_map(db)
                            .local_variables
                            .values()
                            .nth(formal_idx);

                        if let Some(var) = var {
                            ctx.variable_of_param.insert(parameter, *var);
                        }
                        formal_idx += 1;
                    }
                    ParamAssignKind::FormalInput { param, value } => {
                        let var = callable.def_map(db).local_variables.get(&param.ident);

                        if let Some(var) = var {
                            ctx.variable_of_param.insert(parameter, *var);
                        }
                    }
                    ParamAssignKind::FormalOutput {
                        not,
                        param,
                        variable,
                    } => {
                        let var = callable.def_map(db).local_variables.get(&param.ident);
                        let ty = typ.walk_variable_access(db, variable, ctx);

                        if let Some(var) = var {
                            ctx.variable_of_param.insert(parameter, *var);
                        }
                        return ty;
                    }
                }
            }
            return typ;
        }
        Type::Never
    }

    /// Resolve all expression statements for a given scope
    pub fn resolve_statements(
        &self,
        db: &'db dyn BaseDatabase,
        scope_typ: Type<'db>,
        statements: &'db [Stmt<'db>],
        nested_scope: NestedScope,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        let infer_ctx = InferExprCtx::new(self.scope, scope_typ);

        for stmt in statements.iter() {
            match stmt.stmt(db) {
                StmtKind::EmptyPathExpression(expr) => {
                    let _ = scope_typ.walk_begin_path_expr(db, *expr, ctx);
                }
                StmtKind::Assignment { var, target } => {
                    self.check_assign(
                        db,
                        scope_typ.walk_variable_access(db, *var, ctx),
                        *target,
                        &infer_ctx,
                        ctx,
                    );
                }

                StmtKind::AssignmentAttempt { var, target } => {
                    // fixme: assignment attempts should only be REF_TO
                    self.check_assign(
                        db,
                        scope_typ.walk_variable_access(db, *var, ctx),
                        *target,
                        &infer_ctx,
                        ctx,
                    );
                }
                StmtKind::If {
                    condition,
                    then,
                    else_if,
                    else_,
                } => {
                    self.check_assign(db, Type::new_bool(), *condition, &infer_ctx, ctx);

                    // Analyze THEN block
                    if let Some(then) = then.as_ref() {
                        self.resolve_statements(db, scope_typ, then, NestedScope::None, ctx);
                    }

                    // Analyze ELSE IF blocks
                    for (condition, stmt) in else_if {
                        self.check_assign(db, Type::new_bool(), *condition, &infer_ctx, ctx);

                        self.resolve_statements(db, scope_typ, stmt, NestedScope::None, ctx);
                    }

                    // Analyze ELSE block
                    if let Some(else_) = else_.as_ref() {
                        self.resolve_statements(db, scope_typ, else_, NestedScope::None, ctx);
                    }
                }
                StmtKind::For {
                    control_variable,
                    start,
                    end,
                    step,
                    body,
                } => {
                    self.check_assign(
                        db,
                        scope_typ.walk_variable_access(db, *control_variable, ctx),
                        *start,
                        &infer_ctx,
                        ctx,
                    );

                    self.check_assign(
                        db,
                        scope_typ.walk_variable_access(db, *control_variable, ctx),
                        *end,
                        &infer_ctx,
                        ctx,
                    );

                    if let Some(step) = step {
                        self.check_assign(
                            db,
                            scope_typ.walk_variable_access(db, *control_variable, ctx),
                            *step,
                            &infer_ctx,
                            ctx,
                        );
                    }

                    self.resolve_statements(db, scope_typ, body, NestedScope::Loop, ctx);
                }
                StmtKind::While { condition, body } => {
                    self.check_assign(db, Type::new_bool(), *condition, &infer_ctx, ctx);

                    self.resolve_statements(db, scope_typ, body, NestedScope::Loop, ctx);
                }
                StmtKind::Repeat { condition, body } => {
                    self.check_assign(db, Type::new_bool(), *condition, &infer_ctx, ctx);

                    self.resolve_statements(db, scope_typ, body, NestedScope::Loop, ctx);
                }
                StmtKind::FuncCall(f) => {
                    let typ = Self::resolve_func_call(db, scope_typ, *f, ctx);
                    if typ.with_return_type(db).is_some() {
                        // unused return type of function call
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
                                    self.check_compare(db, cond, *expr, &infer_ctx, ctx);
                                }
                                CaseKind::Subrange { lower, upper } => {
                                    self.check_compare(db, cond, *lower, &infer_ctx, ctx);
                                    self.check_compare(db, cond, *upper, &infer_ctx, ctx);
                                }
                            }
                        }

                        self.resolve_statements(db, scope_typ, stmts, NestedScope::None, ctx);
                    }

                    if let Some(else_) = else_.as_ref() {
                        self.resolve_statements(db, scope_typ, else_, NestedScope::None, ctx);
                    }
                }
                StmtKind::Continue => {
                    if nested_scope != NestedScope::Loop {
                        ctx.errors
                            .push(BodyInferenceError::ContinueOutsideLoop { stmt: *stmt });
                    }
                }
                StmtKind::Exit => {
                    if nested_scope != NestedScope::Loop {
                        ctx.errors
                            .push(BodyInferenceError::ExitOutsideLoop { stmt: *stmt });
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

    fn check_assign(
        &self,
        db: &'db dyn BaseDatabase,
        target: Type<'db>,
        expr: Expr<'db>,
        infer_ctx: &InferExprCtx<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        let value = infer_ctx.infer_expr(db, expr, ctx);
        if !target.coerce_with(db, value, self.scope) {
            ctx.errors.push(
                TypeError::NotAssignable {
                    target,
                    value,
                    expr: expr.into(),
                }
                .into(),
            )
        };
    }

    fn check_compare(
        &self,
        db: &'db dyn BaseDatabase,
        lhs: Type<'db>,
        expr: Expr<'db>,
        infer_ctx: &InferExprCtx<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        let rhs = infer_ctx.infer_expr(db, expr, ctx);
        if !lhs.coerce_with(db, rhs, self.scope) {
            ctx.errors
                .push(TypeError::NotComparable { lhs, rhs, expr }.into())
        };
    }
}
