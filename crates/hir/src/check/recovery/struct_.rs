use auto_lsp::default::db::BaseDatabase;

use crate::{
    hir_def::{
        query_string::{NamedSymbol, Query, SymbolIndex, SymbolKind},
    },
    hir_ty::ty::{Ty, TyKind},
};

pub fn fuzzy_struct_fields<'db>(
    db: &'db dyn BaseDatabase,
    ztruct: Ty<'db>,
    query: &str,
) -> Vec<String> {
    let mut indexes = vec![];
    if let TyKind::Struct { spec: _, elements } = ztruct.kind(db) {
        elements.iter().for_each(|(name, ty)| {
            indexes.push(NamedSymbol {
                name: name.text(db).to_string(),
                kind: SymbolKind::StructField(*ty),
            })
        });
    }

    let index = vec![SymbolIndex::create(db, indexes.into_boxed_slice())];

    let mut fast_query = Query::new(query.to_string());
    fast_query.fuzzy();

    let mut results = vec![];

    fast_query.search(db, &index, |symbol| {
        results.push(symbol.name.clone());

        std::ops::ControlFlow::Continue::<()>(())
    });

    results
}
