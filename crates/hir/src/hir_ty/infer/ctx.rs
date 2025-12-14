use auto_lsp::default::db::BaseDatabase;

use crate::{
    AstId, CallSite, HirNodeInfo,
    check::errors::{
        analysis_error::ToIdeDiagnostic,
        body_inference::{BodyInferenceError, TypeError},
    },
    hir_def::{
        expressions::{
            expression::{Expr, FuncCall, ParamAssignKind},
            invocation::{Invocation, InvocationKind},
            statement::{CaseKind, Stmt, StmtKind},
        },
        pous::{pou::Pou, variable::VariableDecl},
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{
        body_inference::BodyInferenceResult, infer::expr::InferExprCtx, resolver::Resolver,
        ty::Type,
    },
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
    ) -> Option<Pou<'db>> {
        match get_scope(db, scope).kind {
            ScopeKind::MethodDecl(m) => {
                let scope = get_scope(db, m.scope_id(db));
                let parent = scope
                    .parent
                    .expect("A method scope always has a parent scope");
                return Self::resolve_invocation(db, parent, invocation, ctx);
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
                    InvocationKind::SuperBody => match pou {
                        Pou::FunctionBlock(fb) => {
                            ctx.type_of_invocation
                                .insert(invocation, Type::new_pou(db, pou));
                            return Some(pou);
                        }
                        _ => {
                            ctx.errors.push(
                                BodyInferenceError::SuperBodyOnIncompatiblePou {
                                    call_site: CallSite::new(scope, invocation.keyword_id(db)),
                                }
                                .to_diagnostic(db),
                            );
                        }
                    },
                    InvocationKind::Super => match pou {
                        // fixme: SUPER only gives access to base methods from EXTENDS, not all implemented interfaces
                        Pou::FunctionBlock(_) | Pou::Class(_) => {
                            ctx.type_of_invocation
                                .insert(invocation, Type::new_pou(db, pou));
                            return Some(pou);
                        }
                        _ => {
                            ctx.errors.push(
                                BodyInferenceError::SuperOnIncompatiblePou {
                                    call_site: CallSite::new(scope, invocation.keyword_id(db)),
                                }
                                .to_diagnostic(db),
                            );
                        }
                    },
                    InvocationKind::This => match pou {
                        Pou::FunctionBlock(_) | Pou::Class(_) => {
                            ctx.type_of_invocation
                                .insert(invocation, Type::new_pou(db, pou));
                            return Some(pou);
                        }
                        _ => {
                            ctx.errors.push(
                                BodyInferenceError::ThisOnIncompatiblePou {
                                    call_site: CallSite::new(scope, invocation.keyword_id(db)),
                                }
                                .to_diagnostic(db),
                            );
                        }
                    },
                }
            }
            _ => unreachable!("An invocation will always be in a POU scope"),
        }
        None
    }

    pub fn resolve_func_call(
        db: &'db dyn BaseDatabase,
        resolver: Resolver<'db>,
        func_call: FuncCall<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) -> Type<'db> {
        let typ = resolver.resolve_begin_path_expr(db, func_call.path(db), ctx);

        if let Some(callable) = typ.shallow_as_callable(db) {
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
                            let mut expr_ctx = InferExprCtx::new(resolver);
                            let typ = expr_ctx.infer_expr(db, value, ctx);
                            expr_ctx
                                .inference_table
                                .resolve_completly(db, resolver, ctx);

                            if let Err(e) = Type::new_var(db, *var).coerce_with(db, typ, resolver) {
                                ctx.errors.push(
                                    TypeError::NotAssignable {
                                        base_target: Type::new_var(db, *var),
                                        target: e.expected,
                                        value: e.actual,
                                        expr: value.into(),
                                    }
                                    .to_diagnostic(db),
                                );
                            }

                            if var.is_output(db) {
                                ctx.errors.push(
                                    BodyInferenceError::OutputParameterUsedAsInput {
                                        func: callable,
                                        var: *var,
                                        expr: value,
                                        param: formal_idx,
                                    }
                                    .to_diagnostic(db),
                                );
                            }
                            ctx.variable_of_param.insert(parameter, *var);
                        } else {
                            ctx.errors.push(
                                BodyInferenceError::UnknownNonFormalParameter {
                                    func: callable,
                                    expr: value,
                                    param: formal_idx,
                                }
                                .to_diagnostic(db),
                            );
                        }
                        formal_idx += 1;
                    }
                    ParamAssignKind::FormalInput { param, value } => {
                        let var = callable.def_map(db).local_variables.get(&param.ident);

                        if let Some(var) = var {
                            ctx.variable_of_param.insert(parameter, *var);
                        } else {
                            ctx.errors.push(
                                BodyInferenceError::UnknownInputParameter {
                                    func: callable,
                                    param: param,
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
                        let var = callable.def_map(db).local_variables.get(&param.ident);
                        let ty = resolver.resolve_variable_access(db, variable, ctx);

                        if let Some(var) = var {
                            ctx.variable_of_param.insert(parameter, *var);
                        } else {
                            ctx.errors.push(
                                BodyInferenceError::UnknownOutputParameter {
                                    func: callable,
                                    param: param,
                                }
                                .to_diagnostic(db),
                            );
                        }
                        return ty;
                    }
                }
            }
            return typ;
        } else {
            // not a callable type
            ctx.errors.push(
                BodyInferenceError::CallNonCallableType {
                    typ: typ,
                    func_call: func_call,
                }
                .to_diagnostic(db),
            );
        }
        Type::Never
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
                    let var_access = resolver.resolve_variable_access(db, *var, ctx);
                    // Additional checks for variable assignments
                    if let Type::Variable(variable) = var_access {
                        // a variable of kind INPUT cannot be assigned to
                        if variable.is_input(db) {
                            ctx.errors.push(
                                BodyInferenceError::IsVarInput {
                                    var: variable,
                                    access: *var,
                                }
                                .to_diagnostic(db),
                            );
                        }

                        // a variable of callable type cannot be assigned to
                        if let Some(callable_typ) =
                            Type::new_var(db, variable).shallow_as_callable(db)
                        {
                            ctx.errors.push(
                                BodyInferenceError::AssignCallableType {
                                    typ: callable_typ,
                                    access: *var,
                                }
                                .to_diagnostic(db),
                            );
                            continue;
                        }
                    }
                    // type is not a variable
                    else {
                        // function and methods can be assigned IF they are the same
                        let ok = match var_access {
                            Type::Function(f) => f.get_scope_id(db) == var.get_scope_id(db),
                            Type::MethodDecl(m) => m.get_scope_id(db) == var.get_scope_id(db),
                            _ => false,
                        };
                        if !ok {
                            ctx.errors.push(
                                BodyInferenceError::DirectType {
                                    expr: *var,
                                    typ: var_access,
                                }
                                .to_diagnostic(db),
                            );
                            continue;
                        }
                    }
                    self.check_assign(db, var_access, *target, &mut infer_ctx, ctx);
                }

                StmtKind::AssignmentAttempt { var, target } => {
                    // fixme: assignment attempts should only be REF_TO
                    self.check_assign(
                        db,
                        resolver.resolve_variable_access(db, *var, ctx),
                        *target,
                        &mut infer_ctx,
                        ctx,
                    );
                }
                StmtKind::If {
                    condition,
                    then,
                    else_if,
                    else_,
                } => {
                    self.check_assign(db, Type::new_bool(), *condition, &mut infer_ctx, ctx);

                    // Analyze THEN block
                    if let Some(then) = then.as_ref() {
                        self.resolve_statements(db, resolver, then, NestedScope::None, ctx);
                    }

                    // Analyze ELSE IF blocks
                    for (condition, stmt) in else_if {
                        self.check_assign(db, Type::new_bool(), *condition, &mut infer_ctx, ctx);

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
                    self.check_assign(
                        db,
                        resolver.resolve_variable_access(db, *control_variable, ctx),
                        *start,
                        &mut infer_ctx,
                        ctx,
                    );

                    self.check_assign(
                        db,
                        resolver.resolve_variable_access(db, *control_variable, ctx),
                        *end,
                        &mut infer_ctx,
                        ctx,
                    );

                    if let Some(step) = step {
                        self.check_assign(
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
                    self.check_assign(db, Type::new_bool(), *condition, &mut infer_ctx, ctx);

                    self.resolve_statements(db, resolver, body, NestedScope::Loop, ctx);
                }
                StmtKind::Repeat { condition, body } => {
                    self.check_assign(db, Type::new_bool(), *condition, &mut infer_ctx, ctx);

                    self.resolve_statements(db, resolver, body, NestedScope::Loop, ctx);
                }
                StmtKind::FuncCall(f) => {
                    let typ = Self::resolve_func_call(db, resolver, *f, ctx);
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

    fn check_assign(
        &self,
        db: &'db dyn BaseDatabase,
        target: Type<'db>,
        expr: Expr<'db>,
        infer_ctx: &mut InferExprCtx<'db>,
        ctx: &mut BodyInferenceResult<'db>,
    ) {
        infer_ctx.infer_expr(db, expr, ctx);
        infer_ctx.inference_table.set_resolved(expr, target);
        infer_ctx
            .inference_table
            .resolve_completly(db, infer_ctx.resolver, ctx);

        let value = ctx.type_of_expr.get(&expr).copied().unwrap_or_default();

        if let Err(err) = target.coerce_with(db, value, infer_ctx.resolver) {
            ctx.errors.push(
                TypeError::NotAssignable {
                    base_target: target,
                    target: err.expected,
                    value: err.actual,
                    expr: expr.into(),
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
        if let Err(err) = lhs.coerce_with(db, rhs, infer_ctx.resolver) {
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
