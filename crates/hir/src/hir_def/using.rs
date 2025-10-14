use auto_lsp::default::db::BaseDatabase;

use crate::{
    hir_def::{interned::namespace::NamespacePath, scope::FileScopeId},
    {AstId, HirNodeInfo},
};

#[salsa::tracked(debug)]
pub struct Using<'db> {
    pub path: NamespacePath,

    #[tracked]
    #[no_eq]    
    pub id: AstId,

    pub scope_id: FileScopeId<'db>,
}

impl<'db> HirNodeInfo<'db> for Using<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.scope_id(db)
    }
}
