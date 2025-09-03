use std::ops::ControlFlow;

use auto_lsp::default::db::BaseDatabase;

use crate::{
    def::{
        namespace::NamespaceDecl,
        pous::pou::{Pou, PouDecl},
        semantic_index::{HirNode, SemanticIndex},
    },
    ty::{
        expr_resolver::ResolvedExpr,
        stmt_resolver::{ResolvedStmt, ResolvedStmtKind, resolve_stmt},
        ty::{Ty, ty_for_pou},
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

        let stmts = match self.pou(db) {
            Pou::Function(f) => Some(f.statements(db)),
            Pou::FunctionBlock(fb) => Some(fb.statements(db)),
            _ => None,
        };

        for stmt in stmts.unwrap_or(&vec![]) {
            resolve_stmt(db, *stmt).walk_hir(db, f)?;
        }
        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for Ty<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::Ty(*self))?;
        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for ResolvedVarResult<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::ResolvedVarResult(*self))?;
        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for ResolvedPathResult<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::ResolvedPathResult(*self))?;
        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for ResolvedExpr<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::ResolvedExpr(*self))?;
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
            ResolvedStmtKind::FuncCall { target, params } => {
                target.walk_hir(db, f)?;
            }
            // … same for While/Repeat/etc
            _ => {}
        }
        ControlFlow::Continue(())
    }
}
