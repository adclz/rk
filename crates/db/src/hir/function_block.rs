use auto_lsp::default::db::BaseDatabase;

use crate::{
    hir::{namespace::Using, variable::Variable, visibility::Modifiers},
    solver::fq_name::SpannedNamespaceAccess,
    to_proto::{ToProto, IterToProto},
};

#[salsa::tracked(debug)]
pub struct FunctionBlock<'db> {
    pub extends: Option<SpannedNamespaceAccess>,

    pub implements: Option<Vec<SpannedNamespaceAccess>>,

    #[tracked]
    #[returns(ref)]
    pub using: Vec<Using<'db>>,

    #[returns(ref)]
    pub variables: Vec<Variable<'db>>,

    pub modifiers: Modifiers,
}

impl<'db> IterToProto<'db> for FunctionBlock<'db> {
    fn iter(&'db self, db: &'db dyn BaseDatabase) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        self.using(db).iter().map(move |u| u as _)
            .chain(self.variables(db).iter().map(move |v| v as _))
    }
}
