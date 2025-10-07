use auto_lsp::{default::db::BaseDatabase, lsp_types::Hover};
use hir::hir_ty::ty_var_access_resolver::ResolvedVarResult;

use crate::{ToProtocol, ty::TyHover};

impl<'db> ToProtocol<'db> for ResolvedVarResult<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, _offset: usize) -> Option<Hover> {
        self.ty(db).ok().and_then(|ty| ty.hover_decl(db))
    }

    fn declaration(
        &'db self,
        db: &'db dyn BaseDatabase,
    ) -> Option<auto_lsp::lsp_types::request::GotoDeclarationResponse> {
        self.ty(db).ok().and_then(|ty| ty.declaration(db))
    }

    fn definition(
        &'db self,
        db: &'db dyn BaseDatabase,
    ) -> Option<auto_lsp::lsp_types::GotoDefinitionResponse> {
        self.ty(db).ok().and_then(|ty| ty.definition(db))
    }
}
