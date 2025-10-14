use auto_lsp::default::db::BaseDatabase;

use crate::{
    hir_def::{
        expressions::spec::Spec,
        interned::{identifier::Ident, namespace::SpanNamespaceAccess},
        pous::variable::VariableDecl,
        scope::FileScopeId, visibility::Visibility,
    },
    AstId, HirNodeInfo,
};

#[salsa::tracked(debug)]
pub struct Interface<'db> {
    #[returns(as_ref)]
    pub extends: Option<Vec<SpanNamespaceAccess<'db>>>,

    #[returns(ref)]
    pub methods: Vec<MethodPrototype<'db>>,

    pub scope_id: FileScopeId<'db>,
}

#[salsa::tracked(debug)]
pub struct MethodPrototype<'db> {
    #[returns(ref)]
    pub name: Ident,

    #[returns(as_ref)]
    pub return_type: Option<Spec<'db>>,

    #[returns(ref)]
    pub variables: Vec<VariableDecl<'db>>,

    #[tracked]
    #[no_eq]
    pub id: AstId,

    #[tracked]
    #[no_eq]
    pub name_id: AstId,

    pub scope_id: FileScopeId<'db>,
}

impl<'db> HirNodeInfo<'db> for MethodPrototype<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> crate::AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.scope_id(db)
    }

    fn get_name_id(&'db self, db: &'db dyn BaseDatabase) -> Option<AstId> {
        Some(self.name_id(db))
    }
}
