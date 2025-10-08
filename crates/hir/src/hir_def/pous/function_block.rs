use crate::hir_def::{
    expressions::statement::Stmt,
    interned::namespace::SpanNamespaceAccess,
    modifier::Modifier,
    pous::{class::MethodDecl, variable::VariableDecl},
    scope::FileScopeId,
};

#[salsa::tracked(debug)]
pub struct FunctionBlock<'db> {
    #[returns(as_ref)]
    pub extends: Option<SpanNamespaceAccess<'db>>,

    #[returns(ref)]
    pub implements: Vec<SpanNamespaceAccess<'db>>,

    #[returns(ref)]
    pub variables: Vec<VariableDecl<'db>>,

    #[returns(ref)]
    pub methods: Vec<MethodDecl<'db>>,

    #[returns(ref)]
    pub statements: Vec<Stmt<'db>>,

    pub modifier: Modifier,

    pub scope_id: FileScopeId<'db>,
}
