use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

use crate::hir_def::{expressions::spec::{Struct, StructElement}, interned::identifier::Ident};

#[salsa::tracked]
impl<'db> Struct<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn resolve_elements(
        self,
        db: &'db dyn BaseDatabase,
    ) -> FxHashMap<Ident, StructElement<'db>> {
        self.elements(db)
            .iter()
            .map(|element| (*element.name(db), *element))
            .collect()
    }
}