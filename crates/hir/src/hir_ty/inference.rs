use ast::generated::{PrimaryExpression, Subrange};
use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

use crate::{
    HirNodeInfo,
    builder::interface,
    check::errors::path_error::AccessError,
    hir_def::{
        expressions::{
            expression::{
                BeginPathExpr, Expr, ExprKind, FuncCall, InitExpr, ParamAssign, ParamAssignKind,
                PathExpr, PrimaryExpr, VariableAccess, VariableAccessKind,
            },
            invocation::{self, Invocation, InvocationKind},
            spec::{Array, ElementarySpec, Enum, Spec, SpecKind, Struct, StructElement, SubRange},
            statement::{CaseKind, Stmt, StmtKind},
        },
        interned::identifier::Ident,
        pous::{
            class::{Class, MethodDecl},
            function::Function,
            function_block::FunctionBlock,
            interface::{Interface, MethodPrototype},
            pou::{Pou, PouDecl},
            variable::VariableDecl,
        },
        scope::{ScopeId, ScopeKind},
        semantic_index::{HirNode, get_scope},
    },
    hir_ty::{
        def_map::LocalDefMap,
        flatten::{self, Flatten, PathExprWalkStep},
        inheritance_solver::MethodRef,
        name_res::{pou_names_res, resolve_namespace_access},
        ty::Ty,
        ty_var_access_resolver::CallSite,
        ty2::Type,
    },
};

#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum WalkError<'db> {
    NoItemInScope {
        expr: PathExpr<'db>,
        scope: ScopeId<'db>,
    },
    NoSuchField {
        expr: PathExpr<'db>,
        ident: Ident,
        ty: Type<'db>,
    },
    DerefNonRefType {
        expr: PathExpr<'db>,
        ty: Type<'db>,
    },
    IndexNonArrayType {
        expr: PathExpr<'db>,
        ty: Type<'db>,
    },
    SuperBodyOnIncompatiblePou {
        call_site: CallSite<'db>,
    },
    SuperOnIncompatiblePou {
        call_site: CallSite<'db>,
    },
    ThisOnIncompatiblePou {
        call_site: CallSite<'db>,
    },
    ContinueOutsideLoop {
        stmt: Stmt<'db>,
    },
    ExitOutsideLoop {
        stmt: Stmt<'db>,
    },
}

#[salsa::tracked(returns(ref))]
pub fn infer_scope<'db>(db: &'db dyn BaseDatabase, scope: ScopeId<'db>) -> InferenceResult<'db> {
    let mut result = InferenceResult::new(scope);
    let ctx = InferCtx::new(db, scope);

    ctx.resolve_statements(
        StatementSource::Scope(scope),
        NestedScope::None,
        &mut result,
    );

    result
}

#[derive(Debug, PartialEq, Eq, salsa::Update)]
pub struct InferenceResult<'db> {
    // Scope where this InferenceResult was emitted
    pub scope: ScopeId<'db>,

    // Mapping from path exression to variables
    pub variable_of_path_expr: FxHashMap<PathExpr<'db>, VariableDecl<'db>>,

    // Mapping from parameter assignments to variables
    pub variable_of_param: FxHashMap<ParamAssign<'db>, VariableDecl<'db>>,

    // Mapping from invocations to their resolved types.
    pub type_of_invocation: FxHashMap<Invocation<'db>, Type<'db>>,

    // Mapping from path expressions to their resolved types.
    pub type_of_begin_path_expr: FxHashMap<BeginPathExpr<'db>, Type<'db>>,

    // Mapping from path expressions to their resolved types.
    pub type_of_path_expr: FxHashMap<PathExpr<'db>, Type<'db>>,

    // Mapping from expressions to their resolved types.
    pub type_of_expr: FxHashMap<Expr<'db>, Type<'db>>,

    // Mapping from path expressions to their adjustment sequences.
    pub path_expr_adjustments: FxHashMap<PathExpr<'db>, Vec<Adjustment<'db>>>,

    pub errors: Vec<WalkError<'db>>,
}

impl<'db> InferenceResult<'db> {
    pub fn new(scope: ScopeId<'db>) -> Self {
        InferenceResult {
            scope,
            variable_of_path_expr: FxHashMap::default(),
            variable_of_param: FxHashMap::default(),
            type_of_invocation: FxHashMap::default(),
            type_of_expr: FxHashMap::default(),
            type_of_begin_path_expr: FxHashMap::default(),
            type_of_path_expr: FxHashMap::default(),
            path_expr_adjustments: FxHashMap::default(),
            errors: Vec::new(),
        }
    }

    pub fn type_of_expr_with_adjustments(
        &self,
        db: &'db dyn BaseDatabase,
        expr: Expr<'db>,
    ) -> Option<Type<'db>> {
        match expr.expr(db) {
            ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(var)) => match var.kind(db) {
                VariableAccessKind::Symbolic(sym) => {
                    self.type_of_begin_expr_with_adjustments(db, sym)
                }
                _ => self.type_of_expr.get(&expr).copied(),
            },
            _ => self.type_of_expr.get(&expr).copied(),
        }
    }

    pub fn type_of_variable_access_with_adjustments(
        &self,
        db: &'db dyn BaseDatabase,
        var_access: VariableAccess<'db>,
    ) -> Option<Type<'db>> {
        match var_access.kind(db) {
            VariableAccessKind::Symbolic(sym) => self.type_of_begin_expr_with_adjustments(db, sym),
            _ => None,
        }
    }

    pub fn type_of_begin_expr_with_adjustments(
        &self,
        db: &'db dyn BaseDatabase,
        begin: BeginPathExpr<'db>,
    ) -> Option<Type<'db>> {
        match begin.expr(db) {
            Some(expr) => self.type_of_path_expr_with_adjustments(expr),
            None => match begin.invocation(db) {
                Some(invocation) => self.type_of_invocation.get(&invocation).copied(),
                None => None,
            },
        }
    }

    pub fn type_of_path_expr_with_adjustments(&self, expr: PathExpr<'db>) -> Option<Type<'db>> {
        match self
            .path_expr_adjustments
            .get(&expr)
            .and_then(|adjustements| adjustements.last())
        {
            Some(adjustment) => Some(adjustment.target),
            None => self.type_of_path_expr.get(&expr).copied(),
        }
    }

    pub fn variable_for_path_expr(&self, expr: PathExpr<'db>) -> Option<VariableDecl<'db>> {
        self.variable_of_path_expr.get(&expr).copied()
    }

    pub fn variable_for_param(&self, param: ParamAssign<'db>) -> Option<VariableDecl<'db>> {
        self.variable_of_param.get(&param).copied()
    }

    pub fn variable_for_var_access(
        &self,
        db: &'db dyn BaseDatabase,
        var_access: VariableAccess<'db>,
    ) -> Option<VariableDecl<'db>> {
        match var_access.kind(db) {
            VariableAccessKind::Direct { .. } => todo!(),
            VariableAccessKind::Symbolic(path_expr) => None,
        }
    }

    pub fn path_expr_adjustments(&self, expr: PathExpr<'db>) -> Option<&[Adjustment<'db>]> {
        self.path_expr_adjustments.get(&expr).map(|it| &**it)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Adjust {
    Deref,
    Ref,
    Index,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub struct Adjustment<'db> {
    pub kind: Adjust,
    pub target: Type<'db>,
}

impl<'db> Adjustment<'db> {
    pub fn new_deref(db: &'db dyn BaseDatabase, ty: Type<'db>) -> Self {
        Adjustment {
            kind: Adjust::Deref,
            target: ty,
        }
    }

    pub fn new_ref(db: &'db dyn BaseDatabase, ty: Type<'db>) -> Self {
        Adjustment {
            kind: Adjust::Ref,
            target: ty,
        }
    }

    pub fn new_index(db: &'db dyn BaseDatabase, ty: Type<'db>) -> Self {
        Adjustment {
            kind: Adjust::Index,
            target: ty,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum StatementSource<'db> {
    Scope(ScopeId<'db>),
    List(&'db [Stmt<'db>]),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum NestedScope {
    Loop,
    None,
}

pub struct InferCtx<'db> {
    pub db: &'db dyn BaseDatabase,
    pub scope: ScopeId<'db>,
    pub nested_scope: NestedScope,
}

impl<'db> InferCtx<'db> {
    pub fn new(db: &'db dyn BaseDatabase, scope: ScopeId<'db>) -> Self {
        InferCtx {
            db,
            scope,
            nested_scope: NestedScope::None,
        }
    }

    pub fn resolve_invocation(
        db: &'db dyn BaseDatabase,
        scope: ScopeId<'db>,
        invocation: Invocation<'db>,
        ctx: &mut InferenceResult<'db>,
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
                            ctx.errors.push(WalkError::SuperBodyOnIncompatiblePou {
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
                            ctx.errors.push(WalkError::SuperOnIncompatiblePou {
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
                            ctx.errors.push(WalkError::ThisOnIncompatiblePou {
                                call_site: CallSite::new(scope, invocation.keyword_id(db)),
                            });
                        }
                    },
                }
            }
            _ => unreachable!("An invocation will always be in a POU scope"),
        }
    }

    fn resolve_func_call(
        &self,
        scope_typ: Type<'db>,
        func_call: FuncCall<'db>,
        ctx: &mut InferenceResult<'db>,
    ) {
        scope_typ.walk_begin_path_expr(self.db, func_call.path(self.db), ctx);
        if let Some(typ) = ctx
            .type_of_begin_path_expr
            .get(&func_call.path(self.db))
            .copied()
            && let Some(callable) = typ.as_callable()
        {
            let mut formal_idx = 0;
            for parameter in func_call.params(self.db) {
                match parameter.kind(self.db) {
                    ParamAssignKind::NonFormal { value } => {
                        // Try to get the param by index
                        let var = callable
                            .def_map(self.db)
                            .local_variables
                            .values()
                            .nth(formal_idx);

                        if let Some(var) = var {
                            ctx.variable_of_param.insert(parameter, *var);
                        }
                        formal_idx += 1;
                    }
                    ParamAssignKind::FormalInput { param, value } => {
                        let var = callable.def_map(self.db).local_variables.get(&param.ident);

                        if let Some(var) = var {
                            ctx.variable_of_param.insert(parameter, *var);
                        }
                    }
                    ParamAssignKind::FormalOutput {
                        not,
                        param,
                        variable,
                    } => {
                        let var = callable.def_map(self.db).local_variables.get(&param.ident);
                        typ.walk_variable_access(self.db, variable, ctx);

                        if let Some(var) = var {
                            ctx.variable_of_param.insert(parameter, *var);
                        }
                    }
                }
            }
        }
    }

    /// Resolve all expression statements for a given scope
    pub fn resolve_statements(
        &self,
        source: StatementSource<'db>,
        nested_scope: NestedScope,
        ctx: &mut InferenceResult<'db>,
    ) {
        let scope_typ = match get_scope(self.db, self.scope).kind {
            ScopeKind::Pou(pou) => match pou.pou(self.db) {
                Pou::Function(f) => Type::Function(*f),
                Pou::FunctionBlock(fb) => Type::FunctionBlock(*fb),
                _ => return,
            },
            ScopeKind::MethodDecl(m) => Type::MethodDecl(m.into()),
            _ => return,
        };

        let statements = match source {
            StatementSource::List(stmts) => stmts,
            StatementSource::Scope(scope) => match get_scope(self.db, self.scope).kind {
                ScopeKind::Pou(pou) => match pou.pou(self.db) {
                    Pou::Function(f) => f.statements(self.db),
                    Pou::FunctionBlock(fb) => fb.statements(self.db),
                    _ => return,
                },
                ScopeKind::MethodDecl(m) => m.stmts(self.db),
                _ => return,
            },
        };

        for stmt in statements.iter() {
            match stmt.stmt(self.db) {
                StmtKind::EmptyPathExpression(expr) => {
                    scope_typ.walk_begin_path_expr(self.db, *expr, ctx);
                }
                StmtKind::Assignment { var, target } => {
                    scope_typ.walk_variable_access(self.db, *var, ctx);
                    self.resolve_expr(*target, scope_typ, ctx);
                }
                StmtKind::AssignmentAttempt { var, target } => {
                    scope_typ.walk_variable_access(self.db, *var, ctx);
                    self.resolve_expr(*target, scope_typ, ctx);
                }
                StmtKind::If {
                    condition,
                    then,
                    else_if,
                    else_,
                } => {
                    self.resolve_expr(*condition, scope_typ, ctx);

                    if let Some(then) = then.as_ref() {
                        InferCtx::new(self.db, self.scope).resolve_statements(
                            StatementSource::List(then),
                            NestedScope::None,
                            ctx,
                        );
                    }

                    for (expr, stmt) in else_if {
                        self.resolve_expr(*expr, scope_typ, ctx);

                        InferCtx::new(self.db, self.scope).resolve_statements(
                            StatementSource::List(stmt),
                            NestedScope::None,
                            ctx,
                        );
                    }

                    if let Some(else_) = else_.as_ref() {
                        InferCtx::new(self.db, self.scope).resolve_statements(
                            StatementSource::List(else_),
                            NestedScope::None,
                            ctx,
                        );
                    }
                }
                StmtKind::For {
                    control_variable,
                    start,
                    end,
                    step,
                    body,
                } => {
                    scope_typ.walk_variable_access(self.db, *control_variable, ctx);
                    self.resolve_expr(*start, scope_typ, ctx);
                    self.resolve_expr(*end, scope_typ, ctx);
                    if let Some(step) = step {
                        self.resolve_expr(*step, scope_typ, ctx);
                    }

                    InferCtx::new(self.db, self.scope).resolve_statements(
                        StatementSource::List(body),
                        NestedScope::Loop,
                        ctx,
                    );
                }
                StmtKind::While { condition, body } => {
                    self.resolve_expr(*condition, scope_typ, ctx);

                    InferCtx::new(self.db, self.scope).resolve_statements(
                        StatementSource::List(body),
                        NestedScope::Loop,
                        ctx,
                    );
                }
                StmtKind::Repeat { condition, body } => {
                    self.resolve_expr(*condition, scope_typ, ctx);

                    InferCtx::new(self.db, self.scope).resolve_statements(
                        StatementSource::List(body),
                        NestedScope::Loop,
                        ctx,
                    );
                }
                StmtKind::FuncCall(f) => self.resolve_func_call(scope_typ, *f, ctx),
                StmtKind::Case { condition, cases, else_ } => {
                    self.resolve_expr(*condition, scope_typ, ctx);

                    for (exprs, stmts) in cases {
                        for expr in exprs {
                            match expr {
                                CaseKind::Expression(expr) => {
                                    self.resolve_expr(*expr, scope_typ, ctx);
                                }
                                CaseKind::Subrange { lower, upper } => {
                                    self.resolve_expr(*lower, scope_typ, ctx);
                                    self.resolve_expr(*upper, scope_typ, ctx);
                                }
                            }
                        }

                        InferCtx::new(self.db, self.scope).resolve_statements(
                            StatementSource::List(stmts),
                            NestedScope::None,
                            ctx,
                        );
                    }

                    if let Some(else_) = else_.as_ref() {
                        InferCtx::new(self.db, self.scope).resolve_statements(
                            StatementSource::List(else_),
                            NestedScope::None,
                            ctx,
                        );
                    }
                },
                StmtKind::Continue => {
                    if nested_scope != NestedScope::Loop {
                        ctx.errors.push(WalkError::ContinueOutsideLoop {
                            stmt: *stmt,
                        });
                    }
                },
                StmtKind::Exit => {
                    if nested_scope != NestedScope::Loop {
                        ctx.errors.push(WalkError::ExitOutsideLoop {
                            stmt: *stmt,
                        });
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

    pub fn resolve_expr(&self, expr: Expr<'db>, scope: Type<'db>, ctx: &mut InferenceResult<'db>) {
        match expr.expr(self.db) {
            ExprKind::AddOperator {
                left,
                operator,
                right,
            } => {
                self.resolve_expr(*left, scope, ctx);
                self.resolve_expr(*right, scope, ctx);
            }
            ExprKind::BooleanOperator {
                left,
                operator,
                right,
            } => {
                self.resolve_expr(*left, scope, ctx);
                self.resolve_expr(*right, scope, ctx);
            }
            ExprKind::ComparisonOperator {
                left,
                operator,
                right,
            } => {
                self.resolve_expr(*left, scope, ctx);
                self.resolve_expr(*right, scope, ctx);
            }
            ExprKind::MultOperator {
                left,
                operator,
                right,
            } => {
                self.resolve_expr(*left, scope, ctx);
                self.resolve_expr(*right, scope, ctx);
            }
            ExprKind::PowerOperator { left, right } => {
                self.resolve_expr(*left, scope, ctx);
                self.resolve_expr(*right, scope, ctx);
            }
            ExprKind::UnaryOperator { expr, operator } => {
                self.resolve_expr(*expr, scope, ctx);
            }
            ExprKind::PrimaryExpr(p) => match p {
                PrimaryExpr::VariableAccess(v) => scope.walk_variable_access(self.db, *v, ctx),
                PrimaryExpr::FuncCall(f) => self.resolve_func_call(scope, *f, ctx),
                PrimaryExpr::Literal(elem) => {
                    ctx.type_of_expr.insert(expr, Type::ElementaryValue(*elem));
                }
                PrimaryExpr::ParenthesizedExpr { expr } => {
                    self.resolve_expr(*expr, scope, ctx);
                }
                PrimaryExpr::EnumValue { name, variant } => {
                    // resolve enum first
                    //ctx.type_of_expr.insert(expr, Type::EnumVariant(**variant));
                    todo!()
                }
                PrimaryExpr::RefValue { value } => {
                    // find reference
                    todo!()
                }
            },
        }
    }
}
