use std::ops::ControlFlow;

use auto_lsp::default::db::BaseDatabase;

use crate::{
    def::{
        namespace::NamespaceDecl,
        pous::pou::PouDecl,
        semantic_index::{HirNode, SemanticIndex},
    },
    ty::{
        expr_resolver::{ResolvedExpr, ResolvedExprKind},
        stmt_resolver::{ResolveStmtsResult, ResolvedStmt, ResolvedStmtKind, resolve_stmt},
        ty::{Ty, ty_for_pou},
    },
};

pub trait WalkHir<'db> {
    fn walk_hir<F>(&'db self, db: &'db dyn BaseDatabase, f: &mut F) -> ControlFlow<()>
    where
        F: FnMut(HirNode<'db>) -> ControlFlow<()>;
}

impl<'db> WalkHir<'db> for SemanticIndex<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &'db self,
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

impl<'db> WalkHir<'db> for ResolveStmtsResult<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &'db self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        self.stmt(db).as_ref().map(|s| s.walk_hir(db, f));
        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for NamespaceDecl<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &'db self,
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
        &'db self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::Ty(ty_for_pou(db, *self)))?;

        if let Some(stmts) = self.get_stmts(db) {
            for stmt in stmts {
                resolve_stmt(db,  *stmt, self.scope_id(db)).walk_hir(db, f)?;
            }
        }
        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for Ty<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &'db self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::Ty(*self))?;
        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for ResolvedExpr<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &'db self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::ResolvedExpr(*self))?;

        if let ResolvedExprKind::FuncCall(ty) = self.kind(db) {
            ty.walk_hir(db, f)?;
        }
        ControlFlow::Continue(())
    }
}

impl<'db> WalkHir<'db> for ResolvedStmt<'db> {
    fn walk_hir<F: FnMut(HirNode<'db>) -> ControlFlow<()>>(
        &'db self,
        db: &'db dyn BaseDatabase,
        f: &mut F,
    ) -> ControlFlow<()> {
        f(HirNode::ResolvedStmt(*self))?;

        match self.kind(db) {
            ResolvedStmtKind::Assignment { target, var } => {
                //var.walk_hir(db, f)?;
                target.walk_hir(db, f)?;
            }
            ResolvedStmtKind::AssignmentAttempt { var, target } => {
                //var.walk_hir(db, f)?;
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
            // … same for While/Repeat/etc
            _ => {}
        }
        ControlFlow::Continue(())
    }
}
