use std::hash::{BuildHasher, Hash, Hasher};

use db::WorkspaceDataBase;
use rustc_hash::{FxBuildHasher, FxHashMap};

use crate::{
    CallSite,
    check::errors::{ToIdeDiagnostic, e01_duplicates::DuplicateError, e02_resolve::ResolveError},
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

            // Folded: `USING Tools` and `USING tools` name one namespace, so
            // importing both is importing it twice.
            using.path(db).fragments(db).iter().for_each(|f| {
                f.caseless(db).hash(&mut hasher);
            });

            let frag_hash = hasher.finish();

            seen.entry(frag_hash)
                .and_modify(|prev| {
                    self.errors.push(
                        DuplicateError::Using {
                            other: *prev,
                            using: *using,
                        }
                        .to_diagnostic(db, self.scope.file(db)),
                    );
                })
                .or_insert(*using);

            let path = crate::hir_ty::index_graphs::absolute_namespace_path(
                db,
                self.scope,
                using.path(db).path,
            );
            let decls = namespace_index(db, path);
            if decls.is_empty() {
                self.errors.push(
                    ResolveError::UsingNamespaceNotFound {
                        path: using.path(db).path,
                        call_site: CallSite::from_scoped(db, using),
                    }
                    .to_diagnostic(db, self.scope.file(db)),
                )
            }
            // USING an INTERNAL namespace from outside its enclosing one: the
            // whole import is refused, not each imported name later.
            for ns in decls {
                if ns.internal(db)
                    && let Some(violated) =
                        crate::hir_ty::resolver::visibility::internal_namespace_violated(
                            db, self.scope, ns,
                        )
                {
                    self.errors.push(
                        crate::check::errors::e10_visibility::VisibilityError::InternalNamespace {
                            call_site: CallSite::from_scoped(db, using),
                            namespace: violated,
                        }
                        .to_diagnostic(db, self.scope.file(db)),
                    );
                    break;
                }
            }
        }
    }
}
