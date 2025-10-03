use auto_lsp::{default::db::BaseDatabase, lsp_types::request::GotoDeclarationResponse};
use hir::{
    hir_def::expressions::expression::InitExprKind,
    hir_ty::{expr_resolver::resolve_expr, init_expr_resolver::{ResolvedInitExpr, ResolvedInitExprKind}},
};

use crate::ToProtocol;

impl<'db> ToProtocol<'db> for ResolvedInitExpr<'db> {
    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
        match self.kind(db) {
            ResolvedInitExprKind::ConstantExpr(expr) => expr.declaration(db),
            _ => None,
        }
    }

    fn definition(
        &'db self,
        db: &'db dyn BaseDatabase,
    ) -> Option<auto_lsp::lsp_types::GotoDefinitionResponse> {
        match self.kind(db) {
            ResolvedInitExprKind::ConstantExpr(expr) => expr.definition(db),
            _ => None,
        }
    }

    fn hover(
        &'db self,
        db: &'db dyn BaseDatabase,
        offset: usize,
    ) -> Option<auto_lsp::lsp_types::Hover> {
        match self.kind(db) {
            ResolvedInitExprKind::ConstantExpr(expr) => expr.hover(db, offset),
            _ => None,
        }
    }
}
