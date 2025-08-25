use std::ops::ControlFlow;

use auto_lsp::default::db::BaseDatabase;

use crate::{def::{namespace::NamespaceDecl, pous::pou::PouDecl, semantic_index::{HirNode, SemanticIndex}}, ty::{expr_resolver::{ResolvedExpr, ResolvedExprKind}, stmt_resolver::{resolve_stmts, ResolveStmtsResult, ResolvedStmt, ResolvedStmtKind}}};


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
        for stmt in self.stmts(db) {
            stmt.walk_hir(db, f)?;
        }
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
        f(HirNode::PouDecl(*self))?;

        if let Some(stmts) = self.get_stmts(db) {
            resolve_stmts(db, stmts, self.scope_id(db)).walk_hir(db, f)?;
        }
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

        match self.kind(db) {
            ResolvedExprKind::FuncCall(ty) => {
                //ty.walk_hir(db, f);
            }
            _ => {}
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
            ResolvedStmtKind::Assignment { target, .. } => {
                target.walk_hir(db, f)?;
            }
            ResolvedStmtKind::If {
                condition,
                then,
                else_if,
                else_,
            } => {
                condition.walk_hir(db, f)?;
                if let Some(then) = then {
                    then.walk_hir(db, f)?;
                }
                for (cond, block) in else_if {
                    cond.walk_hir(db, f)?;
                    block.walk_hir(db, f)?;
                }
                if let Some(else_) = else_ {
                    else_.walk_hir(db, f)?;
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
                body.walk_hir(db, f)?;
            }
            // … same for While/Repeat/etc
            _ => {}
        }
        ControlFlow::Continue(())
    }
}
