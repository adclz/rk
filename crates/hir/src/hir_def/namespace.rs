use auto_lsp::default::db::BaseDatabase;

use crate::hir_def::interned::namespace::NamespacePath;
use crate::hir_def::pous::pou::PouDecl;
use crate::hir_def::scope::FileScopeId;
use crate::{AstId, HirNodeInfo};

#[salsa::tracked(debug)]
pub struct NamespaceDecl<'db> {
    #[returns(ref)]
    pub path: NamespacePath,

    #[returns(ref)]
    pub pous: Vec<PouDecl<'db>>,

    pub id: AstId,

    pub name_id: AstId,

    pub scope_id: FileScopeId<'db>,
}

impl<'db> HirNodeInfo<'db> for NamespaceDecl<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> crate::AstId {
        self.id(db)
    }

    fn get_name_id(&'db self, db: &'db dyn BaseDatabase) -> Option<AstId> {
        Some(self.name_id(db))
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.scope_id(db)
    }
}
