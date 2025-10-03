use auto_lsp::{default::db::BaseDatabase, lsp_types::Hover};
use hir::hir_ty::ty_var_access_resolver::ResolvedVarResult;

use crate::{ty::TyHover, ToProtocol};

impl<'db> ToProtocol<'db> for ResolvedVarResult<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, _offset: usize) -> Option<Hover> {
        match self.ty(db) {
            Ok(ty) => ty.hover_decl(db),
            Err(_) => None,
        }
    }

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
