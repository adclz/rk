use auto_lsp::{default::db::BaseDatabase};

use crate::{
    def::{
        expressions::{spec::Spec, statement::Stmt},
        interned::{identifier::Ident, namespace::SpannedNamespaceAccess},
        pous::variable::VariableDecl,
        scope::FileScopeId,
        semantic_index::SemanticIndex,
        visibility::Modifiers,
    },
    to_proto::{AstId, IterToProto, ToProto},
};

#[salsa::tracked(debug)]
pub struct Class<'db> {
    #[tracked]
    #[returns(as_ref)]
    pub extends: Option<SpannedNamespaceAccess>,

    #[tracked]
    #[returns(ref)]
    pub implements: Vec<SpannedNamespaceAccess>,

    #[tracked]
    #[returns(ref)]
    pub variables: Vec<VariableDecl<'db>>,

    pub methods: Vec<MethodDecl<'db>>,

    pub modifiers: Modifiers,

    pub scope_id: FileScopeId,
}

impl<'db> IterToProto<'db> for Class<'db> {
    fn iter(
        &self,
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

#[salsa::tracked(debug)]
pub struct MethodDecl<'db> {
    #[tracked]
    #[returns(ref)]
    pub name: Ident,

    #[tracked]
    #[returns(ref)]
    pub variables: Vec<VariableDecl<'db>>,

    #[tracked]
    #[returns(ref)]
    pub return_type: Option<Spec<'db>>,

    pub modifiers: Modifiers,

    pub _override: bool,

    pub body: Vec<Stmt<'db>>,

    pub id: AstId,

    pub scope_id: FileScopeId,
}
