use crate::def::{
    expressions::statement::Stmt, interned::namespace::SpanNamespaceAccess, modifier::Modifier,
    pous::variable::VariableDecl, scope::FileScopeId, visibility::Visibility,
};

#[salsa::tracked(debug)]
pub struct FunctionBlock<'db> {
    #[tracked]
    #[returns(as_ref)]
    pub extends: Option<SpanNamespaceAccess<'db>>,

    #[tracked]
    #[returns(ref)]
    pub implements: Vec<SpanNamespaceAccess<'db>>,

    #[tracked]
    #[returns(ref)]
    pub variables: Vec<VariableDecl<'db>>,

    #[tracked]
    #[returns(ref)]
    pub statements: Vec<Stmt<'db>>,

    pub modifier: Modifier,

    pub scope_id: FileScopeId<'db>,
}
