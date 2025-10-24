use std::ops::ControlFlow;

use auto_lsp::default::db::BaseDatabase;

use crate::{
    hir_def::{
        expressions::{expression::{Expr, VarAccess, VariableAccess}, spec::{Spec, SpecKind}, statement::{Stmt, StmtKind}},
        interned::namespace::SpanNamespaceAccess,
        namespace::NamespaceDecl,
        pous::{
            pou::{Pou, PouDecl},
            variable::VariableDecl,
        },
        semantic_index::{semantic_index, HirNode, SemanticIndex},
        using::Using,
    },
    hir_ty::{
        expr_resolver::{resolve_expr, ResolvedExpr, ResolvedExprKind, ResolvedRefValue},
        func_call_resolver::{ResolvedFuncCall, ResolvedParam, ResolvedParamKind},
        inheritance_solver::MethodRef,
        init_expr_resolver::{resolve_init_expr, ResolvedInitExpr, ResolvedInitExprKind},
        invocation_resolver::{ResolvedInvocationResult, ResolvedMethodKind},
        ty_var_access_resolver::{resolve_local_path_expr, resolve_var_access, ResolvedAccess},
        using_resolver::resolve_using,
        walk::{ResolvedPath, ResolvedPathResult},
    },
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

impl<'db> WalkHir<'db> for SpanNamespaceAccess<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::SpanNamespaceAccess(*self))
    }
}

impl<'db> WalkHir<'db> for NamespaceDecl<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::Namespace(*self))?;

        let sema = semantic_index(db, self.scope_id(db).file(db));
        let scope = sema.get_scope(db, self.scope_id(db));

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

        let scope = semantic_index(db, self.scope_id(db).file(db)).get_scope(db, self.scope_id(db));
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
                    extends.walk_hir(db, f)?;
                }

                for implements in fb.implements(db) {
                    implements.walk_hir(db, f)?;
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
                    extends.walk_hir(db, f)?;
                }

                for implements in class.implements(db) {
                    implements.walk_hir(db, f)?;
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
                        implements.walk_hir(db, f)?;
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
        f(HirNode::ResolvedAccess(*self))?;
        for elem in &self.elements(db) {
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
        // A path element does not derive Copy, but it is small enough to be cheaply cloned.
        f(HirNode::ResolvedPath(self.clone()))
    }
}

impl<'db> WalkHir<'db> for ResolvedExpr<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::ResolvedExpr(*self))?;
        match self.kind(db) {
            ResolvedExprKind::BooleanExpression(lhs, rhs) => {
                lhs.walk_hir(db, f)?;
                rhs.walk_hir(db, f)?;
            }
            ResolvedExprKind::Compare(lhs, rhs) => {
                lhs.walk_hir(db, f)?;
                rhs.walk_hir(db, f)?;
            }
            ResolvedExprKind::Math(lhs, rhs) => {
                lhs.walk_hir(db, f)?;
                rhs.walk_hir(db, f)?;
            }
            ResolvedExprKind::Parenthesized(expr) => {
                expr.walk_hir(db, f)?;
            }
            ResolvedExprKind::FuncCall(func) => {
                func.walk_hir(db, f)?;
            }
            ResolvedExprKind::Invocation(inv) => {
                inv.walk_hir(db, f)?;
            }
            ResolvedExprKind::VarAccess(var) => {
                var.walk_hir(db, f)?;
            }
            ResolvedExprKind::RefValue(expr) => {
                expr.walk_hir(db, f)?;
            }
            _ => {}
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
                resolve_expr(db, expr).walk_hir(db, f)?;
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
        f(HirNode::ResolvedParam(*self))?;
        match self.kind(db) {
            ResolvedParamKind::NonFormal {
                resolved_param,
                value,
            } => {
                if let Some(ty) = resolved_param {
                    ty.walk_hir(db, f)?;
                }
                resolve_expr(db, value).walk_hir(db, f)
            }
            ResolvedParamKind::FormalInput {
                param,
                resolved_param,
                value,
            } => {
                if let Some(ty) = resolved_param {
                    ty.walk_hir(db, f)?;
                }
                resolve_expr(db, value).walk_hir(db, f)
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
                resolve_var_access(db, variable).walk_hir(db, f)
            }
        }
    }
}

impl<'db> WalkHir<'db> for ResolvedInvocationResult<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        match self.target.kind {
            ResolvedMethodKind::InheritedMethod { target, method }
            | ResolvedMethodKind::DeclaredMethod { target, method } => {
                target.walk_hir(db, f)?;
                method.walk_hir(db, f)
            }
            _ => ControlFlow::Continue(()),
        }?;

        for param in &self.params {
            param.walk_hir(db, f)?;
        }
        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for ResolvedFuncCall<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        self.target.walk_hir(db, f)?;

        for param in &self.params {
            param.walk_hir(db, f)?;
        }
        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for ResolvedRefValue<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::ResolvedRefValue(self.clone()))
    }
}

impl<'db> WalkHir<'db> for VariableAccess<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::ResolvedAccess(resolve_var_access(db, *self)))
    }
}

impl<'db> WalkHir<'db> for Expr<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::ResolvedExpr(resolve_expr(db, *self)))
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
                resolve_local_path_expr(db, *path).walk_hir(db, f)?;
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
            StmtKind::Invocation(invocation) => {
                //invocation.resolve_invocation(db, scope).walk_hir(db, f)?;
            }
            _ => {}
        }
        ControlFlow::Continue(())
    }
}
