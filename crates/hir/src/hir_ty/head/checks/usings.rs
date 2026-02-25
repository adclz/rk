use std::hash::{BuildHasher, Hash, Hasher};

use db::WorkspaceDataBase;
use rustc_hash::{FxBuildHasher, FxHashMap};

use crate::{
    CallSite,
    check::errors::{
        analysis_error::ToIdeDiagnostic, e1_duplicates::DuplicateError, e2_resolve::ResolveError,
    },
    hir_def::semantic_index::get_scope,
    hir_ty::{head::init_inference::InitInference, index_graphs::namespace_index},
};

impl<'db> InitInference<'db> {
    pub(crate) fn check_usings(&mut self, db: &'db dyn WorkspaceDataBase) {
        let scope = get_scope(db, self.scope);
        let usings = &scope.usings;

        let mut seen = FxHashMap::default();

        for using in usings.iter() {
            let mut hasher = FxBuildHasher.build_hasher();

            using.path(db).fragments(db).iter().for_each(|f| {
                f.hash(&mut hasher);
            });

            let frag_hash = hasher.finish();

            seen.entry(frag_hash)
                .and_modify(|prev| {
                    self.errors.push(
                        DuplicateError::Using {
                            other: *prev,
                            using: *using,
                        }
                        .to_diagnostic(db),
                    );
                })
                .or_insert(*using);

            if namespace_index(db, using.path(db).path).is_empty() {
                self.errors.push(
                    ResolveError::UsingNamespaceNotFound {
                        path: using.path(db).path,
                        call_site: CallSite::from_scoped(db, using),
                    }
                    .to_diagnostic(db),
                )
            }
        }
    }
}
