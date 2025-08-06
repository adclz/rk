use auto_lsp::default::db::BaseDatabase;

use crate::{
    hir::{
        interned::namespace::SpannedNamespaceAccess, pous::variable::Variable,
        scopes::scope::{Scope, ScopeId}, semantic_index::SemanticIndex, visibility::Modifiers,
    },
    to_proto::{IterToProto, ToProto},
};

#[salsa::tracked(debug)]
pub struct Class<'db> {
    #[tracked]
    #[returns(as_ref)]
    pub extends: Option<SpannedNamespaceAccess>,

    #[tracked]
    #[returns(as_ref)]
    pub implements: Option<Vec<SpannedNamespaceAccess>>,

    #[tracked]
    #[returns(ref)]
    pub variables: Vec<Variable<'db>>,

    pub modifiers: Modifiers,

    pub scope_id: ScopeId,
}

impl<'db> IterToProto<'db> for Class<'db> {
    fn iter(
        &self,
        db: &'db dyn BaseDatabase,
        sema: &'db SemanticIndex,
    ) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        let scope = sema.get_scope(self.scope_id(db));

        scope
            .usings
            .iter()
            .flat_map(move |u| u.iter(db, sema))
            .chain(
                self.variables(db)
                    .iter()
                    .flat_map(move |v| v.iter(db, sema)),
            )
    }
}
