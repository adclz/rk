use auto_lsp::default::db::BaseDatabase;

use crate::{hir::{variable::Variable, visibility::Modifiers}, solver::fq_name::SpannedNamespaceAccess, to_proto::{IterToProto, ToProto}};


#[salsa::tracked]
pub struct FunctionBlock<'db> {
    pub extends: Option<SpannedNamespaceAccess>,

    pub implements: Option<Vec<SpannedNamespaceAccess>>,

    #[returns(ref)]
    pub variables: Vec<Variable<'db>>,

    pub modifiers: Modifiers,
}


impl<'db> IterToProto<'db> for FunctionBlock<'db> {
    fn iter(&'db self, db: &'db dyn BaseDatabase) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        self.variables(db).iter().map(|v| v as _)
    }
}