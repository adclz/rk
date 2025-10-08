use crate::hir_def::{
    expressions::{expression::InitExpr, spec::Spec},
    scope::FileScopeId,
};

#[salsa::tracked(debug)]
pub struct DataType<'db> {
    pub spec: Spec<'db>,

    pub init: Option<InitExpr<'db>>,

    pub scope_id: FileScopeId<'db>,
}
