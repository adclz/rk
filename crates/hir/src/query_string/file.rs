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
        let sema = semantic_index(db, *file);

        // Global POUs
        for pou in sema.global_pous.iter() {
            items.push(NamedSymbol {
                name: pou.get_name_ident(db).text(db).to_string(),
                namespace: None,
                kind: SymbolKind::Pou(*pou),
            });
            methods_of(db, *pou, None, &mut items);
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
                methods_of(db, *pou, Some(*ns.path(db)), &mut items);
            }
        }
    }

    SymbolIndex::create(db, items.into_boxed_slice())
}

// Construct a symbol index for all POUs in the given file
#[tracing::instrument(level = "debug", skip_all)]
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
        methods_of(db, *pou, None, &mut items);
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
            methods_of(db, *pou, Some(*ns.path(db)), &mut items);
        }
    }

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
    use crate::hir_ty::head::inheritance::MethodRef;

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
            name: method.get_name_ident(db).text(db).to_string(),
            namespace,
            kind: SymbolKind::Method(method),
        });
    }
}
