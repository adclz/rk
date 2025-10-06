use std::ops::ControlFlow;

use auto_lsp::default::db::BaseDatabase;

use crate::{
    hir_def::{
        expressions::spec::{SpecKind, Struct}, namespace::NamespaceDecl, pous::{
            class::MethodDecl,
            interface::MethodPrototype,
            pou::{Pou, PouDecl},
            variable::VariableDecl,
        }, semantic_index::{HirNode, SemanticIndex}
    },
    hir_ty::{
        expr_resolver::ResolvedExpr,
        func_call_resolver::{ResolvedFuncCall, ResolvedParam, ResolvedParamKind},
        init_expr_resolver::{resolve_init_expr, ResolvedInitExpr, ResolvedInitExprKind},
        invocation_resolver::{ResolvedInvocationResult, ResolvedMethodKind},
        stmt_resolver::{resolve_stmt, ResolvedStmt, ResolvedStmtKind},
        ty::{ty_for_method_decl, ty_for_method_prot, ty_for_pou, ty_for_struct_field, ty_for_variable},
        ty_path_expr_resolver::ResolvedPathResult,
        ty_var_access_resolver::ResolvedVarResult,
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

impl<'db> WalkHir<'db> for NamespaceDecl<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::Namespace(*self))?;

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
        f(HirNode::Ty(ty_for_pou(db, *self)))?;

        match self.pou(db) {
            Pou::Function(function) => {
                for var in function.variables(db) {
                    var.walk_hir(db, f)?;
                }

                for stmt in function.statements(db) {
                    resolve_stmt(db, *stmt).walk_hir(db, f)?;
                }
            }
            Pou::FunctionBlock(fb) => {
                for var in fb.variables(db) {
                    var.walk_hir(db, f)?;
                }

                for method in fb.methods(db) {
                    method.walk_hir(db, f)?;
                }

                for stmt in fb.statements(db) {
                    resolve_stmt(db, *stmt).walk_hir(db, f)?;
                }
            }
            Pou::Class(class) => {
                for var in class.variables(db) {
                    var.walk_hir(db, f)?;
                }
                for method in class.methods(db) {
                    method.walk_hir(db, f)?;
                }
            }
            Pou::Interface(it) => {
                for method in it.methods(db) {
                    method.walk_hir(db, f)?;
                }
            }
            Pou::DataType(dt) => {
                match dt.spec(db).kind(db) {
                    SpecKind::Struct(st) => {
                        for field in &st.elements {
                            f(HirNode::Ty(ty_for_struct_field(db, *field)))?;
                        }
                    }
                    _ => {}
                }

                if let Some(init_expr) = dt.init(db) {
                    f(HirNode::ResolvedInitExpr(*resolve_init_expr(db, ty_for_pou(db, *self), init_expr)))?;
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
        f(HirNode::Ty(ty_for_variable(db, *self)))?;
        f(HirNode::Ty(
            *self.spec(db).to_ty(db, ty_for_variable(db, *self).decl(db)),
        ))?;
        if let Some(init_expr) = self.init(db) {
            resolve_init_expr(db, ty_for_variable(db, *self), *init_expr).walk_hir(db, f)?;
        }
        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for MethodDecl<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::Ty(ty_for_method_decl(db, *self)))
    }
}

impl<'db> WalkHir<'db> for MethodPrototype<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::Ty(ty_for_method_prot(db, *self)))
    }
}

impl<'db> WalkHir<'db> for ResolvedVarResult<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::ResolvedVarResult(*self))
    }
}

impl<'db> WalkHir<'db> for ResolvedPathResult<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::ResolvedPathResult(*self))
    }
}

impl<'db> WalkHir<'db> for ResolvedExpr<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::ResolvedExpr(*self))
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
            },
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
                value.walk_hir(db, f)
            }
            ResolvedParamKind::FormalInput {
                param,
                resolved_param,
                value,
            } => {
                if let Some(ty) = resolved_param {
                    ty.walk_hir(db, f)?;
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
                variable.walk_hir(db, f)
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

impl<'db> WalkHir<'db> for ResolvedStmt<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::ResolvedStmt(*self))?;

        match self.kind(db) {
            ResolvedStmtKind::Assignment { target, var } => {
                var.walk_hir(db, f)?;
                target.walk_hir(db, f)?;
            }
            ResolvedStmtKind::AssignmentAttempt { var, target } => {
                var.walk_hir(db, f)?;
                target.walk_hir(db, f)?;
            }
            ResolvedStmtKind::If {
                condition,
                then,
                else_if,
                else_,
            } => {
                condition.walk_hir(db, f)?;
                for stmt in then {
                    stmt.walk_hir(db, f)?;
                }
                for (cond, block) in else_if {
                    cond.walk_hir(db, f)?;
                    for stmt in block {
                        stmt.walk_hir(db, f)?;
                    }
                }
                for stmt in else_ {
                    stmt.walk_hir(db, f)?;
                }
            }
            ResolvedStmtKind::For {
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
            ResolvedStmtKind::While { condition, body } => {
                condition.walk_hir(db, f)?;
                for stmt in body {
                    stmt.walk_hir(db, f)?;
                }
            }
            ResolvedStmtKind::Repeat { condition, body } => {
                condition.walk_hir(db, f)?;
                for stmt in body {
                    stmt.walk_hir(db, f)?;
                }
            }
            ResolvedStmtKind::FuncCall(func_call) => {
                func_call.walk_hir(db, f)?;
            }
            ResolvedStmtKind::Invocation(invocation) => {
                invocation.walk_hir(db, f)?;
            }
            _ => {}
        }
        ControlFlow::Continue(())
    }
}
