use crate::hir_def::{
    expressions::{spec::Spec, statement::Stmt},
    pous::variable::VariableDecl,
    scope::FileScopeId,
};

#[salsa::tracked(debug)]
pub struct Function<'db> {
    #[returns(ref)]
    pub variables: Vec<VariableDecl<'db>>,

    #[tracked]
    #[no_eq]
    #[returns(ref)]
    pub statements: Vec<Stmt<'db>>,

    #[returns(as_ref)]
    pub return_type: Option<Spec<'db>>,

    pub scope_id: FileScopeId<'db>,
}
