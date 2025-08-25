use auto_lsp::default::db::BaseDatabase;

use crate::{
    def::{
        expressions::{spec::Spec, statement::Stmt},
        interned::{identifier::Ident, namespace::SpannedNamespaceAccess},
        pous::variable::VariableDecl,
        scope::FileScopeId,
        semantic_index::SemanticIndex,
        visibility::Modifiers,
    },
    to_proto::{AstId, ToProto},
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
