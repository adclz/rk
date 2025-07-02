use auto_lsp::default::db::BaseDatabase;

use crate::to_proto::{IterToProto, SymbolInfo, ToProto};

#[salsa::tracked(debug)]
pub struct Interface<'db> {}

impl<'db> IterToProto<'db> for Interface<'db> {
    fn iter(&'db self, db: &'db dyn BaseDatabase) -> impl Iterator<Item = &'db dyn ToProto<'db>> {
        std::iter::empty()
    }
}