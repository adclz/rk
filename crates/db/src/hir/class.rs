use crate::{ident::Ident, to_proto::{SymbolInfo, ToProto}};

#[salsa::tracked(debug)]
pub struct Class<'db> {}