use auto_lsp::default::db::BaseDatabase;

use crate::{hir::{expression::Expr, visibility::Modifiers}, solver::fq_name::{NamespaceAccess, SpannedNamespaceAccess}, to_proto::{IterToProto, ToProto}};

#[salsa::tracked(debug)]
pub struct Class<'db> {
    pub extends: Option<SpannedNamespaceAccess>,

    pub implements: Option<Vec<SpannedNamespaceAccess>>,

    pub modifiers: Modifiers,
}

impl<'db> IterToProto<'db> for Class<'db> {
    fn iter(&'db self, db: &'db dyn BaseDatabase) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        std::iter::empty()
    }
}
