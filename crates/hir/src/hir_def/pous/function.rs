use auto_lsp::default::db::BaseDatabase;

use crate::{
    AstId, HasName, HirNodeInfo,
    hir_def::{
        expressions::{spec::Spec, statement::Stmt},
        interned::identifier::Ident,
        pous::variable::VariableDecl,
        scope::ScopeId,
    },
};

#[salsa::tracked(debug)]
pub struct Function<'db> {
    pub name: Ident,

    #[tracked]
    #[no_eq]
    pub name_id: AstId,

    #[returns(ref)]
    pub variables: Vec<VariableDecl<'db>>,

    #[returns(ref)]
    pub statements: Vec<Stmt<'db>>,

    #[returns(as_ref)]
    pub return_type: Option<Spec<'db>>,

    #[tracked]
    #[no_eq]
    pub id: AstId,

    pub scope_id: ScopeId<'db>,
}

impl<'db> HirNodeInfo<'db> for Function<'db> {
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
        self.scope_id(db)
    }
}

impl<'db> HasName<'db> for Function<'db> {
    fn get_name_ident(&self, db: &'db dyn BaseDatabase) -> Ident {
        self.name(db)
    }

    fn get_name_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        self.name_id(db)
    }
}
