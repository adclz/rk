use std::ops::ControlFlow;

use auto_lsp::default::db::file::File;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{
                BeginPathExpr, Expr, InitExpr, InitExprKind, ParamAssign, VariableAccess,
                VariableAccessKind,
            },
            spec::{Spec, SpecKind},
            statement::{CaseKind, Stmt, StmtKind},
        },
        namespace::NamespaceDecl,
        pous::{pou::Pou, variable::VariableDecl},
        semantic_index::{SemanticIndex, get_scope, semantic_index},
        using::Using,
    },
    hir_ty::{expr_store::InitExprIterator, head::inheritance::MethodRef},
};

use crate::hir_node::{HirNode, PathExprRoot};

pub fn descendant_at<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: File,
    offset: usize,
) -> Option<HirNode<'db>> {
    let mut best_match: Option<HirNode<'db>> = None;

    let _ = semantic_index(db, file).walk_hir(db, &mut |node| {
        let range = node.get_span(db);
        // Only consider nodes that contain the offset
        if range.start_byte <= offset && offset <= range.end_byte {
            // Always update the best match when we find a containing node
            // This ensures we get the deepest (last visited) node in the tree
            best_match = Some(node);
        }
        if range.start_byte > offset {
            return ControlFlow::Break(());
        }
        ControlFlow::Continue(())
    });

    best_match
}

pub trait WalkHir<'db> {
    fn walk_hir<F>(&self, db: &'db dyn WorkspaceDataBase, f: &mut F) -> ControlFlow<()>
    where
        F: FnMut(HirNode<'db>) -> ControlFlow<()>;
}

impl<'db> WalkHir<'db> for SemanticIndex<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn WorkspaceDataBase,
        f: &mut F,
    ) -> ControlFlow<()> {
        for using in self.scope.usings(db) {
            using.walk_hir(db, f)?;
        }
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
        _db: &'db dyn WorkspaceDataBase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::Using(*self))
    }
}

impl<'db> WalkHir<'db> for NamespaceDecl<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn WorkspaceDataBase,
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
        db: &'db dyn WorkspaceDataBase,
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
                    f(HirNode::NamespaceAccess(extends.clone()))?;
                }

                for implements in fb.implements(db) {
                    f(HirNode::NamespaceAccess(implements.clone()))?;
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
                    f(HirNode::NamespaceAccess(extends.clone()))?;
                }

                for implements in class.implements(db) {
                    f(HirNode::NamespaceAccess(implements.clone()))?;
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
                        f(HirNode::NamespaceAccess(implements.clone()))?;
                    }
                }

                for method in it.methods(db) {
                    MethodRef::from(method).walk_hir(db, f)?;
                }
            }
            Pou::DataType(dt) => {
                dt.spec(db).walk_hir(db, f)?;

                if let Some(init_expr) = dt.init(db) {
                    init_expr.walk_hir(db, f)?;
                }
            }
        }
        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for VariableDecl<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn WorkspaceDataBase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::VariableDecl(*self))?;
        self.spec(db).walk_hir(db, f)?;
        if let Some(init_expr) = self.init(db) {
            init_expr.walk_hir(db, f)?;
        }
        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for MethodRef<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn WorkspaceDataBase,
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
        db: &'db dyn WorkspaceDataBase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::Spec(*self))?;
        if let SpecKind::Struct(st) = self.kind(db) {
            for field in &st.elements(db) {
                f(HirNode::StructElement(*field))?;
                field.spec(db).walk_hir(db, f)?;
                if let Some(init_expr) = field.init(db) {
                    init_expr.walk_hir(db, f)?;
                }
            }
        }
        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for InitExpr<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn WorkspaceDataBase,
        f: &mut F,
    ) -> ControlFlow<()> {
        let exprs = self.flatten(db);

        let mut prev = *self;
        for init in InitExprIterator::new(&exprs[0]) {
            f(HirNode::InitExpr {
                prev,
                curr: *init.get_expr(),
            })?;
            if let InitExprKind::ConstantExpr(expr) = init.get_expr().kind(db) {
                expr.walk_hir(db, f)?;
            }
            prev = *init.get_expr();
        }
        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for Expr<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        _db: &'db dyn WorkspaceDataBase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::Expr(*self))?;

        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for BeginPathExpr<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn WorkspaceDataBase,
        f: &mut F,
    ) -> ControlFlow<()> {
        if let Some(invocation) = self.invocation(db) {
            f(HirNode::Invocation(invocation))?;
        }

        if let Some(expr) = self.expr(db) {
            let flat = expr.flatten(db);

            // we need keep track of the previous expession, in order to walk up the the PathExpr correctly
            // e.g: in 'a.b.c', if this is a completion request and the cursor is on c,
            // chances are that c is not valid, so we want to go back to b and show its fields instead.
            let mut prev = match self.invocation(db) {
                Some(inv) => PathExprRoot::Invocation(inv),
                None => PathExprRoot::PathExpr(expr),
            };
            for path in flat {
                f(HirNode::PathExpr {
                    prev,
                    curr: path.get_expr(db),
                })?;
                prev = PathExprRoot::PathExpr(path.get_expr(db));
            }
        }

        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for VariableAccess<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn WorkspaceDataBase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::VariableAccess(*self))?;
        match self.kind(db) {
            VariableAccessKind::Direct(_) => { /* HW Bindings */ }
            VariableAccessKind::Symbolic(v) => {
                if let Some(expr) = v.expr(db) {
                    let flat = expr.flatten(db);

                    // we need keep track of the previous expession, in order to walk up the the PathExpr correctly
                    // e.g: in 'a.b.c', if this is a completion request and the cursor is on c,
                    // chances are that c is not valid, so we want to go back to b and show its fields instead.
                    let mut prev = PathExprRoot::VariableAccess(*self);
                    for path in flat {
                        f(HirNode::PathExpr {
                            prev,
                            curr: path.get_expr(db),
                        })?;
                        prev = PathExprRoot::PathExpr(path.get_expr(db));
                    }
                }
            }
        }

        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for ParamAssign<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        _db: &'db dyn WorkspaceDataBase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::Param(*self))?;

        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for Stmt<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn WorkspaceDataBase,
        f: &mut F,
    ) -> ControlFlow<()> {
        match self.stmt(db) {
            StmtKind::EmptyPathExpression(path) => path.walk_hir(db, f)?,
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
