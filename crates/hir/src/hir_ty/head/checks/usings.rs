// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use std::hash::{BuildHasher, Hash, Hasher};

use db::WorkspaceDataBase;
use rustc_hash::{FxBuildHasher, FxHashMap};

use crate::{
    CallSite, HirNodeInfo,
    check::errors::{ToIdeDiagnostic, e01_duplicates::DuplicateError, e02_resolve::ResolveError},
    hir_def::semantic_index::get_scope,
    hir_ty::head::init_inference::InitInference,
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
            using.path(db).path(db).fragments(db).iter().for_each(|f| {
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
                        .to_diagnostic(db, self.scope.file(db)),
                    );
                })
                .or_insert(*using);

            let written = using.path(db).path(db);
            if let Some(candidates) =
                crate::hir_ty::index_graphs::namespace_path_ambiguity(db, self.scope, written)
            {
                self.errors.push(
                    ResolveError::AmbiguousNamespace {
                        span: CallSite::from_scoped(db, using).get_span(db),
                        written: using.path(db).path_with_case,
                        candidates,
                    }
                    .to_diagnostic(db, self.scope.file(db)),
                );
                continue;
            }
            let path =
                crate::hir_ty::index_graphs::absolute_namespace_path(db, self.scope, written);
            // What this scope's code sees: a library's, its own namespaces
            // and the standard library's.
            let seen = crate::hir_ty::library::Seen::From(self.scope.file(db));
            let decls = crate::hir_ty::index_graphs::namespace_index_seen(db, path, seen);
            if decls.is_empty()
                && !crate::hir_ty::index_graphs::namespace_exists_seen(db, path, seen)
            {
                let call_site = CallSite::from_scoped(db, using);
                match crate::hir_ty::resolver::name::namespace_reached_outside(
                    db, self.scope, written,
                ) {
                    Some(reached) => self.errors.push(
                        crate::check::errors::e10_visibility::VisibilityError::OutsideTheLibrary {
                            text: using.path(db).path_with_case.to_string(db).into(),
                            span: call_site.get_span(db),
                            reached,
                        }
                        .to_diagnostic(db, self.scope.file(db)),
                    ),
                    None => self.errors.push(
                        ResolveError::UsingNamespaceNotFound {
                            path: using.path(db).path_with_case,
                            call_site,
                        }
                        .to_diagnostic(db, self.scope.file(db)),
                    ),
                }
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
