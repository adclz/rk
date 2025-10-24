use auto_lsp::{
    default::db::BaseDatabase,
    lsp_types::{
        request::GotoDeclarationResponse, CompletionItem, GotoDefinitionResponse, Hover
    },
};
use hir::hir_ty::expr_resolver::ResolvedRefValue;

use crate::ToProtocol;

impl<'db> ToProtocol<'db> for ResolvedRefValue<'db> {
    fn hover(&'db self, db: &'db dyn BaseDatabase, offset: usize) -> Option<Hover> {
        match self {
            ResolvedRefValue::Null(_, _) => None,
            ResolvedRefValue::Adress(constant) => constant.hover(db, offset),
        }
    }

    fn declaration(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDeclarationResponse> {
        match self {
            ResolvedRefValue::Null(_, _) => None,
            ResolvedRefValue::Adress(constant) => constant.declaration(db),
        }
    }

    fn definition(&'db self, db: &'db dyn BaseDatabase) -> Option<GotoDefinitionResponse> {
        match self {
            ResolvedRefValue::Null(_, _) => None,
            ResolvedRefValue::Adress(constant) => constant.definition(db),
        }
    }

    fn completion(
            &'db self,
            db: &'db dyn BaseDatabase,
            offset: usize,
        ) -> Option<Vec<CompletionItem>> {
        match self {
            ResolvedRefValue::Null(_, _) => None,
            ResolvedRefValue::Adress(constant) => constant.completion(db, offset),
        }
    }
}
