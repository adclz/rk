use auto_lsp::default::db::BaseDatabase;

use crate::{
    def::{
        expressions::statement::Stmt, interned::namespace::SpannedNamespaceAccess,
        pous::variable::VariableDecl, scope::FileScopeId, semantic_index::SemanticIndex,
        visibility::Modifiers,
    },
    to_proto::{ToProto},
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
    pub variables: Vec<VariableDecl<'db>>,

    #[tracked]
    #[returns(ref)]
    pub statements: Vec<Stmt<'db>>,

    pub modifiers: Modifiers,

    pub scope_id: FileScopeId,
}
