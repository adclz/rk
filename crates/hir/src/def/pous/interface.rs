use auto_lsp::{core::span::Span, default::db::BaseDatabase};

use crate::{
    def::{
        expressions::spec::Spec,
        interned::{identifier::Ident, namespace::SpannedNamespaceAccess},
        pous::variable::VariableDecl,
        scope::FileScopeId,
        semantic_index::{SemanticIndex, semantic_index},
    },
    to_proto::{AstId, ToProto},
};

#[salsa::tracked(debug)]
pub struct Interface<'db> {
    #[returns(as_ref)]
    pub extends: Option<Vec<SpannedNamespaceAccess>>,

    #[returns(ref)]
    pub methods: Vec<MethodPrototype<'db>>,

    pub scope_id: FileScopeId,
}

#[salsa::tracked(debug)]
pub struct MethodPrototype<'db> {
    pub name: Ident,

    #[returns(as_ref)]
    pub return_type: Option<Spec<'db>>,

    #[returns(ref)]
    pub variables: Vec<VariableDecl<'db>>,

    pub id: AstId,

    pub name_id: AstId,

    pub scope_id: FileScopeId,
}

impl<'db> ToProto<'db> for MethodPrototype<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> crate::to_proto::AstId {
        self.id(db)
    }

    fn get_scope_id(&'db self, db: &'db dyn BaseDatabase) -> FileScopeId {
        self.scope_id(db)
    }
}
