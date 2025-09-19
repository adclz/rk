use auto_lsp::{default::db::BaseDatabase, lsp_types::request::GotoDeclarationResponse};
use hir::{
    hir_def::expressions::expression::InitExprKind,
    hir_ty::{expr_resolver::resolve_expr, init_expr_resolver::ResolvedInitExpr},
};

use crate::ToProtocol;

impl<'db> ToProtocol<'db> for ResolvedInitExpr<'db> {
    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
        if let InitExprKind::ConstantExpr(expr) = self.expr(db).kind(db) {
            resolve_expr(db, expr).declaration(db)
        } else {
            None
        }
    }

    fn definition(
        &'db self,
        db: &'db dyn BaseDatabase,
    ) -> Option<auto_lsp::lsp_types::GotoDefinitionResponse> {
        if let InitExprKind::ConstantExpr(expr) = self.expr(db).kind(db) {
            resolve_expr(db, expr).definition(db)
        } else {
            None
        }
    }

    fn hover(
        &'db self,
        db: &'db dyn BaseDatabase,
        offset: Option<usize>,
    ) -> Option<auto_lsp::lsp_types::Hover> {
        if let InitExprKind::ConstantExpr(expr) = self.expr(db).kind(db) {
            resolve_expr(db, expr).hover(db, None)
        } else {
            None
        }
    }
}
