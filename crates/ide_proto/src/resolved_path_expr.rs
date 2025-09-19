use auto_lsp::default::db::BaseDatabase;
use hir::hir_ty::{TyInfo, ty_path_expr_resolver::ResolvedPathResult};

use crate::ToProtocol;

impl<'db> ToProtocol<'db> for ResolvedPathResult<'db> {
    fn declaration(
        &'db self,
        db: &'db dyn BaseDatabase,
    ) -> Option<auto_lsp::lsp_types::request::GotoDeclarationResponse> {
        if let Ok(ty) = self.ty(db) {
            ty.declaration(db)
        } else {
            None
        }
    }

    fn definition(
        &'db self,
        db: &'db dyn BaseDatabase,
    ) -> Option<auto_lsp::lsp_types::GotoDefinitionResponse> {
        if let Ok(ty) = self.ty(db) {
            ty.definition(db)
        } else {
            None
        }
    }
}
