use auto_lsp::{
    default::db::{file::File, BaseDatabase},
    lsp_types::{DiagnosticRelatedInformation, DiagnosticTag, Location},
};
use rustc_hash::FxHashMap;
use salsa::Accumulator;

use crate::{
    diagnostics::{diagnostic_builder::diag, DiagnosticAccumulator},
    hir::{
        interned::{
            namespace::{NamespaceAccess, NamespacePath},
        },
        scopes::{
            iterators::{ScopedMap},
            scope::{ScopeId, ScopeKind, ScopedNamespaceId, ScopedPouId},
        },
        semantic_index::{semantic_index},
        using::Using,
    },
};

/// Finds all namespaces that match the given path.
#[salsa::tracked(returns(ref), no_eq)]
pub fn find_namespaces<'db>(
    db: &'db dyn BaseDatabase,
    path: NamespacePath,
) -> Vec<ScopedNamespaceId> {
    db.get_files()
        .iter()
        .filter_map(|file| {
            semantic_index(db, *file)
                .namespace_keys
                .iter()
                .find_map(|(key, ns)| {
                    if ns.path(db) == &path {
                        Some(ScopedNamespaceId(*key, *file))
                    } else {
                        None
                    }
                })
        })
        .collect()
}

/// Finds all exported items in a given scope.
#[salsa::tracked(returns(ref))]
pub fn exported_items_in_scope<'db>(
    db: &'db dyn BaseDatabase,
    file: File,
    scope_id: ScopeId,
) -> ScopedMap {
    let sema = semantic_index(db, file);
    let scope = sema.get_scope(scope_id);

    let mut namespaces = FxHashMap::default();
    let mut pous = FxHashMap::default();

    scope.usings.iter().for_each(|using| {
        let exported_namespaces = exported_namespaces(db, file, *using);

        for (path, ns) in exported_namespaces {
            namespaces.insert(path, ns);

            let sema = semantic_index(db, ns.1);

            for pou in sema.get_namespace(ns.0).pous(db).iter() {
                pous.insert(sema.pou_keys[pou].name(db).clone(), ScopedPouId(*pou, ns.1));
            }
        }
    });
    ScopedMap { namespaces, pous }
}

// Rules for resolving Using directives

/// 1 - We first search if the namespace matches any of the namespaces declared in all files.
///
/// 2 - Then, we check if the namespace is not shadowed by any other namespace in the current scope.
///
/// 2.5 - Check if the directive is not declared multiple times in the same scope.
///
/// 3 - We then see if the visibility allows the namespace to be used in the current scope.
fn exported_namespaces<'db>(
    db: &'db dyn BaseDatabase,
    file: File,
    using: Using<'db>,
) -> FxHashMap<NamespacePath, ScopedNamespaceId> {
    let mut results = FxHashMap::default();
    // 1: Check if the namespace exists
    let accross = find_namespaces(db, using.path(db));

    if accross.is_empty() {
        let diag = diag()
            .message(format!(
                "namespace '{}' not found",
                using.path(db).to_string(db)
            ))
            .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
            .range(using.span(db).clone())
            .call();

        DiagnosticAccumulator::accumulate(diag.into(), db);
        return results;
    }

    let sema = semantic_index(db, file);

    // Check if this directive is not declared multiple times in the same scope
    sema.get_scope(using.scope_id(db))
        .usings
        .iter()
        .for_each(|u| {
            if u.path(db) == using.path(db) && *u != using {
                let diag = diag()
                    .message(format!(
                        "duplicate declarations of using directive '{}'",
                        using.path(db).to_string(db)
                    ))
                    .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                    .tags(vec![DiagnosticTag::UNNECESSARY])
                    .related_information(vec![DiagnosticRelatedInformation {
                        location: Location {
                            uri: file.url(db).clone(),
                            range: u.span(db).into(),
                        },
                        message: format!(
                            "namespace '{}' is already imported here",
                            using.path(db).to_string(db)
                        ),
                    }])
                    .range(using.span(db).clone())
                    .call();

                DiagnosticAccumulator::accumulate(diag.into(), db);
            }
        });

    for scoped_ns in accross {
        let ns_file = scoped_ns.1;
        let ns_id = scoped_ns.0;

        let sema = semantic_index(db, ns_file);
        let ns = sema.get_namespace(ns_id);

        // 2: Check if the namespace is not shadowed by any other namespace in the current scope
        if ns_file == file {
            let mut ancestors = sema.ancestor_scopes(ns.scope_id(db));
            if let Some(parent) = ancestors.find_map(|sc| {
                if let ScopeKind::Namespace(ns_id) = sc.kind {
                    let ns_path = sema.get_namespace(ns_id).path(db);
                    if *ns_path == using.path(db) {
                        Some(sema.get_namespace(ns_id))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }) {
                let diag = diag()
                    .message(format!(
                        "'{}' is already in scope",
                        using.path(db).to_string(db)
                    ))
                    .severity(auto_lsp::lsp_types::DiagnosticSeverity::WARNING)
                    .tags(vec![DiagnosticTag::UNNECESSARY])
                    .related_information(vec![DiagnosticRelatedInformation {
                        location: Location {
                            uri: file.url(db).clone(),
                            range: parent.span(db).into(),
                        },
                        message: format!(
                            "namespace '{}' is defined here",
                            using.path(db).to_string(db)
                        ),
                    }])
                    .range(using.span(db).clone())
                    .call();

                DiagnosticAccumulator::accumulate(diag.into(), db);
            }
        }
        results.insert(*ns.path(db), ScopedNamespaceId(ns_id, file));
    }
    results
}

pub fn resolve_access<'db>(
    db: &'db dyn BaseDatabase,
    file: File,
    scope: ScopeId,
    access: NamespaceAccess,
) -> Option<ScopedPouId> {
    let target = access.target(db);

    // Namespace is optional
    match access.namespace(db) {
        Some(ns) => {
            let namespaces = find_namespaces(db, ns);
            namespaces.iter().find_map(|ns| {
                let sema = semantic_index(db, ns.1);
                let pou = sema
                    .get_namespace(ns.0)
                    .pous(db)
                    .iter()
                    .find(|pou| *sema.pou_keys[*pou].name(db) == target.ident)?;
                Some(ScopedPouId(*pou, ns.1))
            })
        }
        None => {
            let sema = semantic_index(db, file);
            let exported = exported_items_in_scope(db, file, scope);
            exported.pous.get(&target.ident).copied()
        }
    }
}
