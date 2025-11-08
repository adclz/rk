use std::ops::ControlFlow;

use auto_lsp::default::db::BaseDatabase;

use crate::{
    HirNodeInfo, hir_def::{
        expressions::{
            expression::{Expr, ExprKind, PrimaryExpr, RefValue, VarAccess, VariableAccess},
            spec::{Spec, SpecKind},
            statement::{Stmt, StmtKind},
        },
        interned::namespace::{SpanNamespaceAccess, SpanNamespaceAccessContext},
        namespace::NamespaceDecl,
        pous::{
            class::MethodDecl, pou::{Pou, PouDecl}, variable::VariableDecl
        },
        semantic_index::{HirNode, SemanticIndex, get_scope, semantic_index},
        using::Using,
    }, hir_ty::{
        func_call_resolver::ResolvedFuncCall,
        inheritance_solver::MethodRef,
        init_expr_resolver::{ResolvedInitExpr, ResolvedInitExprKind, resolve_init_expr},
        param_resolver::{ResolvedParam, ResolvedParamKind, resolve_parameters},
        ty::TyKind,
        ty_var_access_resolver::{LookUp, ResolvedAccess},
        using_resolver::resolve_using,
        walk::{ResolvedPath, ResolvedPathKind, ResolvedPathResult},
    }
};

pub trait WalkHir<'db> {
    fn walk_hir<F>(&self, db: &'db dyn BaseDatabase, f: &mut F) -> ControlFlow<()>
    where
        F: FnMut(HirNode<'db>) -> ControlFlow<()>;
}

impl<'db> WalkHir<'db> for SemanticIndex<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        for pou in &self.global_pous {
            pou.walk_hir(db, f)?;
        }
        for namespace in &self.namespaces {
            namespace.walk_hir(db, f)?;
        }
        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for Using<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::ResolvedUsing(*resolve_using(db, *self)))
    }
}

impl<'db> WalkHir<'db> for NamespaceDecl<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::Namespace(*self))?;

        let scope = get_scope(db, self.scope_id(db));

        for using in &scope.usings {
            using.walk_hir(db, f)?;
        }

        for namespace in self.namespaces(db) {
            namespace.walk_hir(db, f)?;
        }

        for pou in self.pous(db) {
            pou.walk_hir(db, f)?;
        }
        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for PouDecl<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::PouDecl(*self))?;

        let scope = get_scope(db, self.scope_id(db));
        for using in &scope.usings {
            using.walk_hir(db, f)?;
        }

        match self.pou(db) {
            Pou::Function(function) => {
                if let Some(ret) = function.return_type(db) {
                    ret.walk_hir(db, f)?;
                }

                for var in function.variables(db) {
                    var.walk_hir(db, f)?;
                }

                for stmt in function.statements(db) {
                    stmt.walk_hir(db, f)?;
                }
            }
            Pou::FunctionBlock(fb) => {
                if let Some(extends) = fb.extends(db) {
                    f(HirNode::SpanNamespaceAccess(SpanNamespaceAccessContext::Extends(extends)))?;
                }

                for implements in fb.implements(db) {
                    f(HirNode::SpanNamespaceAccess(SpanNamespaceAccessContext::Implements(implements)))?;
                }

                for var in fb.variables(db) {
                    var.walk_hir(db, f)?;
                }

                for method in fb.methods(db) {
                    MethodRef::from(method).walk_hir(db, f)?;
                }

                for stmt in fb.statements(db) {
                    stmt.walk_hir(db, f)?;
                }
            }
            Pou::Class(class) => {
                if let Some(extends) = class.extends(db) {
                    f(HirNode::SpanNamespaceAccess(SpanNamespaceAccessContext::Extends(extends)))?;
                }

                for implements in class.implements(db) {
                    f(HirNode::SpanNamespaceAccess(SpanNamespaceAccessContext::Implements(implements)))?;
                }

                for var in class.variables(db) {
                    var.walk_hir(db, f)?;
                }
                for method in class.methods(db) {
                    MethodRef::from(method).walk_hir(db, f)?;
                }
            }
            Pou::Interface(it) => {
                if let Some(extends) = it.extends(db) {
                    for implements in extends {
                        f(HirNode::SpanNamespaceAccess(SpanNamespaceAccessContext::Implements(implements)))?
                    }
                }

                for method in it.methods(db) {
                    MethodRef::from(method).walk_hir(db, f)?;
                }
            }
            Pou::DataType(dt) => {
                f(HirNode::Spec(dt.spec(db)))?;
                if let SpecKind::Struct(st) = dt.spec(db).kind(db) {
                    for field in &st.elements(db) {
                        f(HirNode::StructElement(*field))?;
                    }
                }

                if let Some(init_expr) = dt.init(db) {
                    f(HirNode::ResolvedInitExpr(*resolve_init_expr(
                        db,
                        dt.spec(db).to_ty(db),
                        init_expr,
                    )))?;
                }
            }
        }
        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for VariableDecl<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::VariableDecl(*self))?;
        f(HirNode::Spec(self.spec(db)))?;
        if let Some(init_expr) = self.init(db) {
            resolve_init_expr(db, self.spec(db).to_ty(db), *init_expr).walk_hir(db, f)?;
        }
        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for MethodRef<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::MethodRef(*self))?;
        for var in self.variables(db) {
            var.walk_hir(db, f)?;
        }

        if let Some(ret) = self.return_type(db) {
            ret.walk_hir(db, f)?;
        }

        if let MethodRef::Declared(m) = self {
            for stmt in m.stmts(db) {
                stmt.walk_hir(db, f)?;
            }
        }
        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for Spec<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::Spec(*self))
    }
}

impl<'db> WalkHir<'db> for ResolvedAccess<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::ResolvedAccess(self.clone()))?;
        for elem in &self.elements {
            if let ResolvedPathResult::Ok(path) = &elem {
                path.walk_hir(db, f)?;
            }
        }
        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for ResolvedPath<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::ResolvedPath(self.clone()))
    }
}

impl<'db> WalkHir<'db> for Expr<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::Expr(*self))?;
        match self.expr(db) {
            ExprKind::AddOperator { left, right, .. } => {
                left.walk_hir(db, f)?;
                right.walk_hir(db, f)?;
            }
            ExprKind::BooleanOperator { left, right, .. } => {
                left.walk_hir(db, f)?;
                right.walk_hir(db, f)?;
            }
            ExprKind::ComparisonOperator { left, right, .. } => {
                left.walk_hir(db, f)?;
                right.walk_hir(db, f)?;
            }
            ExprKind::MultOperator { left, right, .. } => {
                left.walk_hir(db, f)?;
                right.walk_hir(db, f)?;
            }
            ExprKind::UnaryOperator { expr, .. } => {
                expr.walk_hir(db, f)?;
            }
            ExprKind::PowerOperator { left, right, .. } => {
                left.walk_hir(db, f)?;
                right.walk_hir(db, f)?;
            }
            ExprKind::PrimaryExpr(expr) => match expr {
                PrimaryExpr::VariableAccess(variable) => {
                    let var_access = variable.lookup(db);
                    var_access.walk_hir(db, f)?;
                }
                PrimaryExpr::FuncCall(func) => {
                    let resolved_call = func.resolve_func_call(db);
                    resolved_call.walk_hir(db, f)?;
                }
                _ => {}
            },
        }
        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for ResolvedInitExpr<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::ResolvedInitExpr(*self))?;
        match self.kind(db) {
            ResolvedInitExprKind::ArrayInit { values }
            | ResolvedInitExprKind::ArrayIndexedElement { values, .. }
            | ResolvedInitExprKind::StructInit { values } => {
                for v in values {
                    v.walk_hir(db, f)?;
                }
            }
            ResolvedInitExprKind::StructElement { value, .. } => {
                value.walk_hir(db, f)?;
            }
            ResolvedInitExprKind::ConstantExpr(expr) => {
                expr.walk_hir(db, f)?;
            }
            ResolvedInitExprKind::Error(_) => {}
        }
        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for ResolvedParam<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::ResolvedParam(self.clone()))?;
        match self.kind {
            ResolvedParamKind::NonFormal {
                resolved_param,
                value,
            } => {
                if let Some(var) = resolved_param {
                    var.walk_hir(db, f)?;
                }
                value.walk_hir(db, f)
            }
            ResolvedParamKind::FormalInput {
                param,
                resolved_param,
                value,
            } => {
                if let Some(var) = resolved_param {
                    var.walk_hir(db, f)?;
                }
                value.walk_hir(db, f)
            }
            ResolvedParamKind::FormalOutput {
                not,
                param,
                resolved_param,
                variable,
            } => {
                if let Some(ty) = resolved_param {
                    ty.walk_hir(db, f)?;
                }
                variable.lookup(db).walk_hir(db, f)
            }
        }
    }
}

impl<'db> WalkHir<'db> for ResolvedFuncCall<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        self.target.walk_hir(db, f)?;

        match self.target.fully_resolved(db) {
            Ok(r) => {
                for param in resolve_parameters(db, r.target_scope_id(db), &self.params) {
                    param.walk_hir(db, f)?;
                }
            }
            _ => {}
        }
        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for VariableAccess<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::ResolvedAccess(self.lookup(db)))
    }
}

impl<'db> WalkHir<'db> for Stmt<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        match self.stmt(db) {
            StmtKind::EmptyPathExpression(path) => {
                path.lookup(db).walk_hir(db, f)?;
            }
            StmtKind::Assignment { target, var } => {
                var.walk_hir(db, f)?;
                target.walk_hir(db, f)?;
            }
            StmtKind::AssignmentAttempt { var, target } => {
                var.walk_hir(db, f)?;
                target.walk_hir(db, f)?;
            }
            StmtKind::If {
                condition,
                then,
                else_if,
                else_,
            } => {
                condition.walk_hir(db, f)?;
                if let Some(then_block) = then {
                    for stmt in then_block {
                        stmt.walk_hir(db, f)?;
                    }
                }

                for (cond, block) in else_if {
                    cond.walk_hir(db, f)?;
                    for stmt in block {
                        stmt.walk_hir(db, f)?;
                    }
                }

                if let Some(else_block) = else_ {
                    for stmt in else_block {
                        stmt.walk_hir(db, f)?;
                    }
                }
            }
            StmtKind::For {
                start,
                end,
                step,
                body,
                ..
            } => {
                start.walk_hir(db, f)?;
                end.walk_hir(db, f)?;
                if let Some(step) = step {
                    step.walk_hir(db, f)?;
                }
                for stmt in body {
                    stmt.walk_hir(db, f)?;
                }
            }
            StmtKind::While { condition, body } => {
                condition.walk_hir(db, f)?;
                for stmt in body {
                    stmt.walk_hir(db, f)?;
                }
            }
            StmtKind::Repeat { condition, body } => {
                condition.walk_hir(db, f)?;
                for stmt in body {
                    stmt.walk_hir(db, f)?;
                }
            }
            StmtKind::FuncCall(func_call) => {
                func_call.resolve_func_call(db).walk_hir(db, f)?;
            }
            _ => {}
        }
        ControlFlow::Continue(())
    }
}
