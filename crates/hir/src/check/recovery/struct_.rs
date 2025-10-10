use auto_lsp::default::db::BaseDatabase;

use crate::{
    check::recovery::pou::FuzzyResult,
    hir_ty::ty::{ty_for_struct_field, Ty, TyKind},
    query_string::query::{NamedSymbol, Query, SymbolIndex, SymbolKind},
};

pub fn fuzzy_struct_fields<'db>(
    db: &'db dyn BaseDatabase,
    ztruct: Ty<'db>,
    query: &str,
) -> FuzzyResult<'db> {
    let mut results = FuzzyResult::default();

    let mut indexes = vec![];
    if let TyKind::Struct { spec: _, elements } = ztruct.kind(db) {
        elements.iter().for_each(|(name, ty)| {
            indexes.push(NamedSymbol {
                name: name.text(db).to_string(),
                kind: SymbolKind::StructField(ty_for_struct_field(db, *ty)),
            })
        });
    }

    let index = vec![SymbolIndex::create(db, indexes.into_boxed_slice())];

    let mut fast_query = Query::new(query.to_string());
    fast_query.fuzzy();

    fast_query.search(db, index, |symbol| {
        match symbol.kind {
            SymbolKind::Variable(ty) => results.variables.push(symbol.clone()),
            SymbolKind::StructField(ty) => results.struct_fields.push(symbol.clone()),
            _ => results.pou.push(symbol.clone()),
        }

        std::ops::ControlFlow::Continue::<()>(())
    });

    results
}
