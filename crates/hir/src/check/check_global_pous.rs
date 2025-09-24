use std::collections::hash_map::Entry;

use auto_lsp::default::db::{BaseDatabase, file::File};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    check::errors::{duplicates::DuplicateError, sem_errors::AnalysisError},
    hir_def::{interned::identifier::Ident, pous::pou::PouDecl, semantic_index::semantic_index},
    hir_ty::ty::ty_for_pou,
};

#[salsa::tracked(returns(ref), no_eq)]
pub fn check_duplicate_pous<'db>(
    db: &'db dyn BaseDatabase,
) -> FxHashMap<File, Vec<AnalysisError<'db>>> {
    let mut errors: FxHashMap<File, Vec<AnalysisError<'db>>> = FxHashMap::default();
    let mut seen_pous: FxHashMap<Ident, PouDecl<'db>> = FxHashMap::default();

    // Collect POUs and process duplicates in a single pass
    db.get_files().iter().for_each(|file| {
        semantic_index(db, *file)
            .global_pous
            .iter()
            .for_each(|decl| {
                let name = *decl.name(db);
                match seen_pous.entry(name) {
                    Entry::Occupied(entry) => {
                        // Found a duplicate - create error
                        let original = *entry.get();
                        match errors.entry(decl.scope_id(db).file(db)) {
                            Entry::Occupied(mut err_entry) => {
                                err_entry.get_mut().push(
                                    DuplicateError::Pou {
                                        pou1: ty_for_pou(db, *decl),
                                        pou2: ty_for_pou(db, original),
                                    }
                                    .into(),
                                );
                            }
                            Entry::Vacant(err_entry) => {
                                err_entry.insert(vec![
                                    DuplicateError::Pou {
                                        pou1: ty_for_pou(db, *decl),
                                        pou2: ty_for_pou(db, original),
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
