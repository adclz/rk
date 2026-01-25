use auto_lsp::default::db::file::File;
use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    HasName, HirNodeInfo,
    check::errors::{analysis_error::ToIdeDiagnostic, e1_duplicates::DuplicateError},
    hir_def::{namespace::NamespaceDecl, pous::pou::Pou, semantic_index::semantic_index},
    hir_ty::name_res::{namespace_pou_index, pou_index},
};

/// Check for duplicate global POU names.
/// A POU is a duplicate if it differs from the one in the index.
pub fn check_duplicate_pous(db: &dyn WorkspaceDataBase, file: File) -> Vec<IdeDiagnostic> {
    semantic_index(db, file)
        .global_pous
        .iter()
        .filter_map(|pou| {
            let indexed = (*pou_index(db, pou.get_name_ident(db)))?;
            // If this POU is not the indexed one, it's a duplicate
            if *pou != indexed {
                Some(
                    DuplicateError::Pou {
                        pou1: *pou,
                        pou2: indexed,
                    }
                    .to_diagnostic(db),
                )
            } else {
                None
            }
        })
        .collect()
}

/// Check for duplicate POU names within a namespace.
/// A POU is a duplicate if it differs from the one in the namespace index.
pub fn check_duplicate_namespaces<'db>(
    db: &'db dyn WorkspaceDataBase,
    namespace: NamespaceDecl<'db>,
) -> FxHashMap<File, Vec<IdeDiagnostic>> {
    let mut errors: FxHashMap<File, Vec<IdeDiagnostic>> = FxHashMap::default();
    let path = *namespace.path(db);

    for pou in namespace.pous(db).iter() {
        if let Some(indexed) = *namespace_pou_index(db, path, pou.get_name_ident(db)) {
            // If this POU is not the indexed one, it's a duplicate
            if *pou != indexed {
                errors
                    .entry(pou.get_scope_id(db).file(db))
                    .or_default()
                    .push(
                        DuplicateError::Pou {
                            pou1: *pou,
                            pou2: indexed,
                        }
                        .to_diagnostic(db),
                    );
            }
        }
    }

    errors
}
