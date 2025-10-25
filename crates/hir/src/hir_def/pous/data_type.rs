use crate::hir_def::{
    expressions::{expression::InitExpr, spec::Spec},
    scope::ScopeId,
};

#[salsa::tracked(debug)]
pub struct DataType<'db> {
    pub spec: Spec<'db>,

    #[tracked]
    #[no_eq]
    pub init: Option<InitExpr<'db>>,

    pub scope_id: ScopeId<'db>,
}
