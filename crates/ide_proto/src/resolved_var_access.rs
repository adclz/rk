
use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{
        GotoDefinitionResponse, Hover,
        request::GotoDeclarationResponse,
    },
};
use hir::hir_ty::ty_var_access_resolver::ResolvedAccess;

use crate::ToProtocol;

impl<'db> ToProtocol<'db> for ResolvedAccess<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Option<Hover> {
        self.resolved(db).ok()?.hover(db, offset)
    }

    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
        self.resolved(db).ok()?.declaration(db)
    }

    fn definition(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDefinitionResponse> {
        self.resolved(db).ok()?.definition(db)
    }
}
