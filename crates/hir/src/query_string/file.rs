// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::default::db::file::File;
use db::WorkspaceDataBase;

use crate::{
    HasName,
    hir_def::semantic_index::semantic_index,
    query_string::query::{NamedSymbol, SymbolIndex, SymbolKind},
};

/// Library symbol index containing all POUs from library files
/// tracked because std lib files have high durability and are not expected to change often
#[tracing::instrument(level = "debug", skip_all)]
#[salsa::tracked(no_eq)]
pub fn library_symbol_index<'db>(db: &'db dyn WorkspaceDataBase) -> SymbolIndex<'db> {
    let mut items = Vec::new();

    for file in db.get_library_files().iter() {
        symbols_of_file(db, *file, &mut items);
    }

    SymbolIndex::create(db, items.into_boxed_slice())
}

/// The symbols of one file, each under its absolute namespace: a library's
/// top-level POUs are under the library's root, not global, and its
/// namespaces are named with the root before them.
fn symbols_of_file<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: File,
    items: &mut Vec<NamedSymbol<'db>>,
) {
    let sema = semantic_index(db, file);
    let root = crate::hir_ty::library::library_root(db, file).map(|root| root.path);

    // Top-level POUs
    for pou in sema.global_pous.iter() {
        items.push(NamedSymbol {
            name: pou.get_name_with_case(db).text(db).to_string(),
            namespace: root,
            kind: SymbolKind::Pou(*pou),
        });
        methods_of(db, *pou, root, items);
    }

    // Namespaces and their POUs
    for ns in sema.namespaces.iter() {
        items.push(NamedSymbol {
            name: ns.absolute_path_with_case(db).to_string(db),
            namespace: None,
            kind: SymbolKind::Namespace(*ns),
        });

        for pou in ns.pous(db).iter() {
            items.push(NamedSymbol {
                name: pou.get_name_with_case(db).text(db).to_string(),
                namespace: Some(ns.absolute_path(db)),
                kind: SymbolKind::Pou(*pou),
            });
            methods_of(db, *pou, Some(ns.absolute_path(db)), items);
        }
    }
}

// Construct a symbol index for all POUs in the given file
#[tracing::instrument(level = "debug", skip_all)]
#[salsa::tracked(no_eq)]
pub fn file_symbol_index<'db>(db: &'db dyn WorkspaceDataBase, file: File) -> SymbolIndex<'db> {
    let mut items = Vec::new();
    symbols_of_file(db, file, &mut items);
    SymbolIndex::create(db, items.into_boxed_slice())
}

/// A POU's own METHODs. A symbol search has to reach them: `Spin` is not
/// findable through the block that declares it, and the index held only
/// POUs and namespaces. An inherited method belongs to the POU that
/// declares it and is indexed there.
fn methods_of<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: crate::hir_def::pous::pou::Pou<'db>,
    namespace: Option<crate::hir_def::interned::namespace::NamespacePath>,
    items: &mut Vec<NamedSymbol<'db>>,
) {
    use crate::hir_def::pous::pou::Pou;
    use crate::hir_ty::oop::MethodRef;

    let declared: Vec<MethodRef<'db>> = match pou {
        Pou::FunctionBlock(fb) => fb
            .methods(db)
            .iter()
            .copied()
            .map(MethodRef::Declared)
            .collect(),
        Pou::Class(class) => class
            .methods(db)
            .iter()
            .copied()
            .map(MethodRef::Declared)
            .collect(),
        Pou::Interface(itf) => itf
            .methods(db)
            .iter()
            .copied()
            .map(MethodRef::Prototype)
            .collect(),
        _ => return,
    };

    for method in declared {
        items.push(NamedSymbol {
            name: method.get_name_with_case(db).text(db).to_string(),
            namespace,
            kind: SymbolKind::Method(method),
        });
    }
}
