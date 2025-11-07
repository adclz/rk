use std::{cmp::Ordering, ops::ControlFlow};

use auto_lsp::default::db::{BaseDatabase, file::File};
use rayon::slice::ParallelSliceMut;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    hir_def::{interned::{identifier::Ident, namespace::NamespacePath}, pous::pou::PouDecl, scope::{ScopeId, ScopeKind}, semantic_index::semantic_index}, hir_ty::name_res::global_namespace_index, query_string::query::{NamedSymbol, Query, SymbolIndex, SymbolKind}
};

// Construct a symbol index for all POUs in the given file
#[tracing::instrument(skip_all)]
#[salsa::tracked(no_eq)]
pub fn file_symbol_index<'db>(db: &'db dyn BaseDatabase, file: File) -> SymbolIndex<'db> {
    let sema = semantic_index(db, file);
    let mut items = Vec::new();

    // Global POUs
    for pou in sema.global_pous.iter() {
        items.push(NamedSymbol {
            name: pou.name(db).text(db).to_string(),
            namespace: None,
            kind: SymbolKind::Pou(*pou),
        });
    }

    // Namespaces POUs
    for ns in sema.namespaces.iter() {
        for pou in ns.pous(db).iter() {
            items.push(NamedSymbol {
                name: pou.name(db).text(db).to_string(),
                namespace: Some(*ns.path(db)),
                kind: SymbolKind::Pou(*pou),
            });
        }
    }

    SymbolIndex::new(db, items.into_boxed_slice())
}
