use auto_lsp::default::db::{BaseDatabase, file::File};
use rustc_hash::FxHashMap;

use crate::{
    check::errors::{analysis_error::AnalysisError, duplicates::DuplicateError},
    hir_def::{interned::identifier::Ident, pous::pou::PouDecl, semantic_index::semantic_index},
};

#[salsa::tracked(returns(ref), no_eq)]
pub fn check_duplicate_pous<'db>(db: &'db dyn BaseDatabase, file: File) -> Vec<AnalysisError<'db>> {
    let mut errors: Vec<AnalysisError<'db>> = vec![];
    let self_pous = global_pous_in_file(db, file);

    // Multiple local POUs with the same name in this file
    for (name, pous) in global_pous_in_file(db, file) {
        if pous.len() > 1 {
            for i in 0..pous.len() {
                for j in (i + 1)..pous.len() {
                    errors.push(
                        DuplicateError::Pou {
                            pou1: pous[j],
                            pou2: pous[0],
                        }
                        .into(),
                    );
                }
            }
        }
    }

    // Collect POUs and process duplicates in a single pass
    db.get_files()
        .iter()
        .filter(|f| (**f) != file)
        .for_each(|file| {
            for (name, pous) in global_pous_in_file(db, *file) {
                if let Some(self_pous) = self_pous.get(name) {
                    // We have a duplicate POU name
                    for self_pou in self_pous {
                        for pou in pous {
                            errors.push(
                                DuplicateError::Pou {
                                    pou1: *self_pou,
                                    pou2: *pou,
                                }
                                .into(),
                            );
                        }
                    }
                }
            }
        });
    errors
}

#[salsa::tracked(returns(ref), no_eq)]
pub fn global_pous_in_file<'db>(
    db: &'db dyn BaseDatabase,
    file: File,
) -> FxHashMap<Ident, Vec<PouDecl<'db>>> {
    let mut map: std::collections::HashMap<Ident, Vec<PouDecl<'db>>, rustc_hash::FxBuildHasher> =
        FxHashMap::default();
    semantic_index(db, file).global_pous.iter().for_each(|pou| {
        map.entry(*pou.name(db)).or_default().push(*pou);
    });
    map
}
