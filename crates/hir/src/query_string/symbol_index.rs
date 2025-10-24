use std::cmp::Ordering;

use auto_lsp::default::db::{BaseDatabase, file::File};
use rayon::slice::ParallelSliceMut;

use crate::{
    hir_def::semantic_index::semantic_index,
    query_string::query::{NamedSymbol, SymbolIndex, SymbolKind},
};

#[tracing::instrument(skip_all)]
#[salsa::tracked(no_eq)]
pub fn file_pou_symbol_index<'db>(db: &'db dyn BaseDatabase, file: File) -> SymbolIndex<'db> {
    let mut pous = vec![];
    let sema = semantic_index(db, file);

    sema.global_pous.iter().for_each(|pou| {
        pous.push(NamedSymbol {
            name: pou.name(db).text(db).to_string(),
            kind: SymbolKind::Pou(*pou),
        });
    });

    sema.namespaces.iter().for_each(|ns| {
        ns.pous(db).iter().for_each(|pou| {
            pous.push(NamedSymbol {
                name: pou.name(db).text(db).to_string(),
                kind: SymbolKind::Pou(*pou),
            });
        });
    });

    SymbolIndex::create(db, pous.into_boxed_slice())
}

#[tracing::instrument(skip_all)]
#[salsa::tracked(no_eq)]
pub fn file_namespace_symbol_index<'db>(db: &'db dyn BaseDatabase, file: File) -> SymbolIndex<'db> {
    let mut pous = vec![];
    let sema = semantic_index(db, file);

    sema.global_pous.iter().for_each(|pou| {
        pous.push(NamedSymbol {
            name: pou.name(db).text(db).to_string(),
            kind: SymbolKind::Pou(*pou),
        });
    });

    sema.namespaces.iter().for_each(|ns| {
        ns.pous(db).iter().for_each(|pou| {
            pous.push(NamedSymbol {
                name: pou.name(db).text(db).to_string(),
                kind: SymbolKind::Pou(*pou),
            });
        });
    });

    SymbolIndex::create(db, pous.into_boxed_slice())
}
