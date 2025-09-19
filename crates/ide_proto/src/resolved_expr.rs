use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{GotoDefinitionResponse, Hover, request::GotoDeclarationResponse},
};
use hir::hir_ty::expr_resolver::{ResolvedExpr, ResolvedExprKind};

use crate::ToProtocol;

impl<'db> ToProtocol<'db> for ResolvedExpr<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, offset: Option<usize>) -> Option<Hover> {
        match self.kind(db) {
            ResolvedExprKind::PathExpr(resolved) => resolved.hover(db, None),
            ResolvedExprKind::VarAccess(resolved) => resolved.hover(db, None),
            _ => None,
        }
    }

    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
        match self.kind(db) {
            ResolvedExprKind::PathExpr(resolved) => resolved.declaration(db),
            ResolvedExprKind::VarAccess(resolved) => resolved.declaration(db),
            _ => None,
        }
    }

    fn definition(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDefinitionResponse> {
        match self.kind(db) {
            ResolvedExprKind::PathExpr(resolved) => resolved.definition(db),
            ResolvedExprKind::VarAccess(resolved) => resolved.definition(db),
            _ => None,
        }
    }
}
