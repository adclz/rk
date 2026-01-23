use std::collections::hash_map::Entry;

use auto_lsp::default::db::file::File;
use db::WorkspaceDataBase;
use ide_diagnostic::IdeDiagnostic;
use rustc_hash::FxHashMap;

use crate::{
    HasName, HirNodeInfo,
    check::errors::{analysis_error::ToIdeDiagnostic, e1_duplicates::DuplicateError},
    hir_def::{interned::identifier::Ident, namespace::NamespaceDecl, pous::pou::Pou},
    hir_ty::name_res::namespace_index,
};

#[salsa::tracked(returns(ref), no_eq)]
pub fn check_duplicate_namespaces<'db>(
    db: &'db dyn WorkspaceDataBase,
    namespace: NamespaceDecl<'db>,
) -> FxHashMap<File, Vec<IdeDiagnostic>> {
    let mut errors: FxHashMap<File, Vec<IdeDiagnostic>> = FxHashMap::default();
    let mut seen_pous: FxHashMap<Ident, Pou<'db>> = FxHashMap::default();

    // Collect POUs and process duplicates in a single pass
    namespace_index(db, *namespace.path(db))
        .iter()
        .for_each(|ns| {
            ns.pous(db).iter().for_each(|decl| {
                let name = decl.get_name_ident(db);
                match seen_pous.entry(name) {
                    Entry::Occupied(entry) => {
                        // Found a duplicate - create error
                        let original = *entry.get();
                        match errors.entry(decl.get_scope_id(db).file(db)) {
                            Entry::Occupied(mut err_entry) => {
                                err_entry.get_mut().push(
                                    DuplicateError::Pou {
                                        pou1: *decl,
                                        pou2: original,
                                    }
                                    .to_diagnostic(db),
                                );
                            }
                            Entry::Vacant(err_entry) => {
                                err_entry.insert(vec![
                                    DuplicateError::Pou {
                                        pou1: *decl,
                                        pou2: original,
                                    }
                                    .to_diagnostic(db),
                                ]);
                            }
                        };
                    }
                    Entry::Vacant(entry) => {
                        // First time seeing this POU name
                        entry.insert(*decl);
                    }
                }
            });
        });

    errors
}
