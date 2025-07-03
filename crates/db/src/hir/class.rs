use auto_lsp::default::db::BaseDatabase;

use crate::{hir::{expression::{Expr, PrimaryExpr}, visibility::Modifiers}, to_proto::{IterToProto, SymbolInfo, ToProto}};

#[salsa::tracked(debug)]
pub struct Class<'db> {
    pub extends: Option<Expr<'db>>,

    pub implements: Option<Vec<Expr<'db>>>,

    pub modifiers: Modifiers,
}

impl<'db> IterToProto<'db> for Class<'db> {
    fn iter(&'db self, db: &'db dyn BaseDatabase) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        std::iter::empty()
    }
}
