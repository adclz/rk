use auto_lsp::default::db::BaseDatabase;

use crate::{ident::Ident, to_proto::{IterToProto, SymbolInfo, ToProto}};

#[salsa::tracked(debug)]
pub struct Class<'db> {}

impl<'db> IterToProto<'db> for Class<'db> {
    fn iter(&'db self, db: &'db dyn BaseDatabase) -> impl Iterator<Item = SymbolInfo<'db>> {
        std::iter::empty()
    }
}
