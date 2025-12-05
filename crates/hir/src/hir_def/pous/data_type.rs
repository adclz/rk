use auto_lsp::default::db::BaseDatabase;

use crate::{AstId, HasName, HirNodeInfo, hir_def::{
    expressions::{expression::InitExpr, spec::Spec}, interned::identifier::Ident, scope::ScopeId
}};

#[salsa::tracked(debug)]
pub struct DataType<'db> {
    pub name: Ident,

    #[tracked]
    #[no_eq]
    pub name_id: AstId,

    pub spec: Spec<'db>,

    #[tracked]
    #[no_eq]
    pub init: Option<InitExpr<'db>>,

    #[tracked]
    #[no_eq]
    pub id: AstId,

    pub scope_id: ScopeId<'db>,
}

impl<'db> HirNodeInfo<'db> for DataType<'db> {
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}


impl<'db> HasName<'db> for DataType<'db> {
    fn get_name_ident(&self, db: &'db dyn BaseDatabase) -> Ident {
        self.name(db)
    }

    fn get_name_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        self.name_id(db)
    }
}