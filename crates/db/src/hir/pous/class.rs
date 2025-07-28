use auto_lsp::default::db::BaseDatabase;

use crate::{
    hir::{
        interned::namespace::SpannedNamespaceAccess, scopes::scope::ScopeId,
        semantic_index::SemanticIndex, visibility::Modifiers,
    },
    to_proto::{IterToProto, ToProto},
};

#[salsa::tracked(debug)]
pub struct Class<'db> {
    pub extends: Option<SpannedNamespaceAccess>,

    pub implements: Option<Vec<SpannedNamespaceAccess>>,

    pub modifiers: Modifiers,

    pub scope_id: ScopeId,
}

impl<'db> IterToProto<'db> for Class<'db> {
    fn iter(
        &'db self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        Box::new(std::iter::empty())
    }
}
