use auto_lsp::default::db::BaseDatabase;

use crate::{AstId, HirNodeInfo, hir_def::scope::ScopeId};

#[salsa::tracked(debug)]
pub struct Invocation<'db> {
    #[tracked]
    #[no_eq]
    pub id: AstId,

    #[tracked]
    #[no_eq]
    pub keyword_id: AstId,

    pub scope_id: ScopeId<'db>,

    pub kind: InvocationKind,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum InvocationKind {
    This,
    Super,
    SuperBody,
}

impl<'db> HirNodeInfo<'db> for Invocation<'db> {
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}
