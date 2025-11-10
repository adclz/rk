use auto_lsp::default::db::BaseDatabase;

use crate::{
    AstId, HirNodeInfo, hir_def::{interned::namespace::{NamespacePath, SpanNamespacePath}, scope::ScopeId}
};

#[salsa::tracked(debug)]
pub struct Using<'db> {
    pub path: SpanNamespacePath<'db>,

    #[tracked]
    #[no_eq]
    pub id: AstId,

    pub scope_id: ScopeId<'db>,
}

impl<'db> HirNodeInfo<'db> for Using<'db> {
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}
