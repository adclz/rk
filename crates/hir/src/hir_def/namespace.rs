use auto_lsp::core::span::Span;
use db::WorkspaceDataBase;

use crate::hir_def::interned::namespace::NamespacePath;
use crate::hir_def::pous::pou::Pou;
use crate::hir_def::scope::ScopeId;
use crate::hir_def::semantic_index::semantic_index;
use crate::{AstId, HirNodeInfo};

#[salsa::tracked(debug)]
pub struct NamespaceDecl<'db> {
    #[returns(ref)]
    pub path: NamespacePath,

    #[returns(ref)]
    pub pous: Vec<Pou<'db>>,

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
    pub fn name_span(&'db self, db: &'db dyn WorkspaceDataBase) -> Span {
        semantic_index(db, self.get_scope_id(db).file(db))
            .ast
            .get(self.name_id(db).0)
            .unwrap_or_else(|| {
                panic!(
                    "Invalid name ID {} when attempting to retrieve name span",
                    self.name_id(db).0
                )
            })
            .get_span()
    }
}

impl<'db> HirNodeInfo<'db> for NamespaceDecl<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> crate::AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}
