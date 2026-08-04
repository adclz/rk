use auto_lsp::default::db::file::File;
use db::WorkspaceDataBase;

use crate::{
    HasName,
    hir_def::semantic_index::semantic_index,
    query_string::query::{NamedSymbol, SymbolIndex, SymbolKind},
};

/// Library symbol index containing all POUs from library files
/// tracked because std lib files have high durability and are not expected to change often
#[tracing::instrument(skip_all)]
#[salsa::tracked(no_eq)]
pub fn library_symbol_index<'db>(db: &'db dyn WorkspaceDataBase) -> SymbolIndex<'db> {
    let mut items = Vec::new();

    for file in db.get_library_files().iter() {
        let sema = semantic_index(db, *file);

        // Global POUs
        for pou in sema.global_pous.iter() {
            items.push(NamedSymbol {
                name: pou.get_name_ident(db).text(db).to_string(),
                namespace: None,
                kind: SymbolKind::Pou(*pou),
            });
        }

        // Namespaces and their POUs
        for ns in sema.namespaces.iter() {
            items.push(NamedSymbol {
                name: ns.path(db).to_string(db),
                namespace: None,
                kind: SymbolKind::Namespace(*ns),
            });

            for pou in ns.pous(db).iter() {
                items.push(NamedSymbol {
                    name: pou.get_name_ident(db).text(db).to_string(),
                    namespace: Some(*ns.path(db)),
                    kind: SymbolKind::Pou(*pou),
                });
            }
        }
    }

    SymbolIndex::create(db, items.into_boxed_slice())
}

// Construct a symbol index for all POUs in the given file
#[tracing::instrument(skip_all)]
#[salsa::tracked(no_eq)]
pub fn file_symbol_index<'db>(db: &'db dyn WorkspaceDataBase, file: File) -> SymbolIndex<'db> {
    let sema = semantic_index(db, file);
    let mut items = Vec::new();

    // Global POUs
    for pou in sema.global_pous.iter() {
        items.push(NamedSymbol {
            name: pou.get_name_ident(db).text(db).to_string(),
            namespace: None,
            kind: SymbolKind::Pou(*pou),
        });
    }

    // Namespaces and their POUs
    for ns in sema.namespaces.iter() {
        items.push(NamedSymbol {
            name: ns.path(db).to_string(db),
            namespace: None,
            kind: SymbolKind::Namespace(*ns),
        });

        for pou in ns.pous(db).iter() {
            items.push(NamedSymbol {
                name: pou.get_name_ident(db).text(db).to_string(),
                namespace: Some(*ns.path(db)),
                kind: SymbolKind::Pou(*pou),
            });
        }
    }

    SymbolIndex::create(db, items.into_boxed_slice())
}
