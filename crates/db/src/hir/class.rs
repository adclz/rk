use auto_lsp::default::db::BaseDatabase;

use crate::{
    hir::{namespace::Using, visibility::Modifiers},
    solver::fq_name::SpannedPath,
    to_proto::{IterToProto, ToProto},
};

#[salsa::tracked(debug)]
pub struct Class<'db> {
    pub extends: Option<SpannedPath>,

    #[tracked]
    #[returns(ref)]
    pub using: Vec<Using<'db>>,

    pub implements: Option<Vec<SpannedPath>>,

    pub modifiers: Modifiers,
}

impl<'db> IterToProto<'db> for Class<'db> {
    fn iter(&'db self, db: &'db dyn BaseDatabase) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        Box::new(std::iter::empty())
    }
}
