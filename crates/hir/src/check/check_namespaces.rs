use std::collections::hash_map::Entry;

use auto_lsp::default::db::{BaseDatabase, file::File};
use rustc_hash::FxHashMap;

use crate::{
    check::errors::{analysis_error::AnalysisError, duplicates::DuplicateError},
    hir_def::{interned::identifier::Ident, namespace::NamespaceDecl, pous::pou::PouDecl},
    hir_ty::name_res::shared_namespaces,
};

#[salsa::tracked(returns(ref), no_eq)]
pub fn check_duplicate_namespaces<'db>(
    db: &'db dyn BaseDatabase,
    namespace: NamespaceDecl<'db>,
) -> FxHashMap<File, Vec<AnalysisError<'db>>> {
    let mut errors: FxHashMap<File, Vec<AnalysisError<'db>>> = FxHashMap::default();
    let mut seen_pous: FxHashMap<Ident, PouDecl<'db>> = FxHashMap::default();

    // Collect POUs and process duplicates in a single pass
    shared_namespaces(db, *namespace.path(db))
        .iter()
        .for_each(|ns| {
            ns.pous(db).iter().for_each(|decl| {
                let name = *decl.name(db);
                match seen_pous.entry(name) {
                    Entry::Occupied(entry) => {
                        // Found a duplicate - create error
                        let original = *entry.get();
                        match errors.entry(decl.scope_id(db).file(db)) {
                            Entry::Occupied(mut err_entry) => {
                                err_entry.get_mut().push(
                                    DuplicateError::Pou {
                                        pou1: *decl,
                                        pou2: original,
                                    }
                                    .into(),
                                );
                            }
                            Entry::Vacant(err_entry) => {
                                err_entry.insert(vec![
                                    DuplicateError::Pou {
                                        pou1: *decl,
                                        pou2: original,
                                    }
                                    .into(),
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
