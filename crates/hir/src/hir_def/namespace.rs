use auto_lsp::core::span::Span;
use auto_lsp::default::db::BaseDatabase;

use crate::hir_def::interned::namespace::NamespacePath;
use crate::hir_def::pous::pou::PouDecl;
use crate::hir_def::scope::ScopeId;
use crate::{AstId, HirNodeInfo};

#[salsa::tracked(debug)]
pub struct NamespaceDecl<'db> {
    #[returns(ref)]
    pub path: NamespacePath,

    #[returns(ref)]
    pub pous: Vec<PouDecl<'db>>,

    #[returns(ref)]
    pub namespaces: Vec<NamespaceDecl<'db>>,

    #[tracked]
    #[no_eq]
    pub id: AstId,

    #[tracked]
    #[no_eq]
    pub name_id: AstId,

    pub scope_id: ScopeId<'db>,
}

impl<'db> NamespaceDecl<'db> {
    pub fn name_span(&self, db: &'db dyn BaseDatabase) -> Span {
        self.get_name_span(db).unwrap()
    }
}

impl<'db> HirNodeInfo<'db> for NamespaceDecl<'db> {
    fn get_id(&self, db: &'db dyn BaseDatabase) -> crate::AstId {
        self.id(db)
    }

    fn get_name_id(&'db self, db: &'db dyn BaseDatabase) -> Option<AstId> {
        Some(self.name_id(db))
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}
