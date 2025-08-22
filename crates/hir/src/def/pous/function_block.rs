use auto_lsp::default::db::BaseDatabase;

use crate::{
    def::{
        expressions::statement::Stmt, interned::namespace::SpannedNamespaceAccess,
        pous::variable::Variable, scope::FileScopeId, semantic_index::SemanticIndex,
        visibility::Modifiers,
    },
    to_proto::{IterToProto, ToProto},
};

#[salsa::tracked(debug)]
pub struct FunctionBlock<'db> {
    #[tracked]
    #[returns(as_ref)]
    pub extends: Option<SpannedNamespaceAccess>,

    #[tracked]
    #[returns(ref)]
    pub implements: Vec<SpannedNamespaceAccess>,

    #[tracked]
    #[returns(ref)]
    pub variables: Vec<Variable<'db>>,

    #[tracked]
    #[returns(ref)]
    pub statements: Vec<Stmt<'db>>,

    pub modifiers: Modifiers,

    pub scope_id: FileScopeId,
}

impl<'db> IterToProto<'db> for FunctionBlock<'db> {
    fn iter(
        &'db self,
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
