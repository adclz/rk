
use crate::def::{
        expressions::statement::Stmt, interned::namespace::SpannedNamespaceAccess,
        pous::variable::VariableDecl, scope::FileScopeId,
        visibility::Modifiers,
    };

#[salsa::tracked(debug)]
pub struct FunctionBlock<'db> {
    #[tracked]
    #[returns(as_ref)]
    pub extends: Option<SpannedNamespaceAccess<'db>>,

    #[tracked]
    #[returns(ref)]
    pub implements: Vec<SpannedNamespaceAccess<'db>>,

    #[tracked]
    #[returns(ref)]
    pub variables: Vec<VariableDecl<'db>>,

    #[tracked]
    #[returns(ref)]
    pub statements: Vec<Stmt<'db>>,

    pub modifiers: Modifiers,

    pub scope_id: FileScopeId<'db>,
}
