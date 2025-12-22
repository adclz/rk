use std::ops::ControlFlow;

use auto_lsp::default::db::BaseDatabase;

use hir::{
    HirNodeInfo, hir_def::{
        expressions::{
            expression::{
                BeginPathExpr, Expr, ExprKind, ParamAssign, PathExpr, PrimaryExpr, VariableAccess,
            },
            spec::{Spec, SpecKind},
            statement::{CaseKind, Stmt, StmtKind},
        },
        namespace::NamespaceDecl,
        pous::{
            pou::Pou,
            variable::VariableDecl,
        },
        semantic_index::{SemanticIndex, get_scope},
        using::Using,
    }, hir_ty::{inheritance_solver::MethodRef, init_inference::{infer_data_type, infer_variable}, ty::Type}
};

use crate::to_proto::hir_node::{HirNode, SpanNamespaceAccessContext};

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
        f(HirNode::Using(*self))
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

impl<'db> WalkHir<'db> for Pou<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::PouDecl(*self))?;

        let scope = get_scope(db, self.get_scope_id(db));

        match self {
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
                    f(HirNode::SpanNamespaceAccess(
                        SpanNamespaceAccessContext::Extends(extends),
                    ))?;
                }

                for implements in fb.implements(db) {
                    f(HirNode::SpanNamespaceAccess(
                        SpanNamespaceAccessContext::Implements(implements),
                    ))?;
                }

                for using in &scope.usings {
                    using.walk_hir(db, f)?;
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
                    f(HirNode::SpanNamespaceAccess(
                        SpanNamespaceAccessContext::Extends(extends),
                    ))?;
                }

                for implements in class.implements(db) {
                    f(HirNode::SpanNamespaceAccess(
                        SpanNamespaceAccessContext::Implements(implements),
                    ))?;
                }

                for using in &scope.usings {
                    using.walk_hir(db, f)?;
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
                        f(HirNode::SpanNamespaceAccess(
                            SpanNamespaceAccessContext::Implements(implements),
                        ))?
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
                    let infer = infer_data_type(db, *dt);
                    for (init_expr, typ) in &infer.type_of_expr {
                        f(HirNode::InitExprWithType((*init_expr, *typ).into()))?;
                    }
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
            let infer = infer_variable(db, *self);
            for (init_expr, typ) in &infer.type_of_expr {
                f(HirNode::InitExprWithType((*init_expr, *typ).into()))?;
            }
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

impl<'db> WalkHir<'db> for Expr<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::Expr(*self))?;

        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for BeginPathExpr<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::BeginPathExpr(*self))?;

        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for PathExpr<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::PathExpr(*self))?;

        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for VariableAccess<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::VariableAccess(*self))?;

        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for ParamAssign<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::Param(*self))?;

        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for Stmt<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        match self.stmt(db) {
            StmtKind::EmptyPathExpression(path) => {}
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
                func_call.path(db).walk_hir(db, f)?;
                for param in func_call.params(db) {
                    param.walk_hir(db, f)?;
                }
            }
            StmtKind::Case {
                condition,
                cases,
                else_,
            } => {
                condition.walk_hir(db, f)?;
                for (case_exprs, stmts) in cases {
                    for case in case_exprs {
                        match case {
                            CaseKind::Expression(expr) => {
                                expr.walk_hir(db, f)?;
                            }
                            CaseKind::Subrange { lower, upper } => {
                                lower.walk_hir(db, f)?;
                                upper.walk_hir(db, f)?;
                            }
                        }
                    }
                    for stmt in stmts {
                        stmt.walk_hir(db, f)?;
                    }
                }
                if let Some(else_block) = else_ {
                    for stmt in else_block {
                        stmt.walk_hir(db, f)?;
                    }
                }
            }
            StmtKind::Continue | StmtKind::Exit | StmtKind::Return => {}
        }
        ControlFlow::Continue(())
    }
}
