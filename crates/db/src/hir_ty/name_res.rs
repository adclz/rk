use auto_lsp::default::db::{file::File, BaseDatabase};
use rustc_hash::FxHashMap;

use crate::{
    check::errors::semantic_errors::{duplicate_using_declaration, namespace_not_found},
    hir::{
        interned::{
            identifier::Ident,
            namespace::{NamespaceAccess, NamespacePath},
        },
        namespace::Namespace,
        pous::{
            pou::{Pou, PouDecl},
            variable::Variable,
        },
        scope::{FileScopeId, ScopeKind},
        semantic_index::semantic_index,
        using::Using,
    }, to_proto::ToProto,
};

/// Find all namespaces in all files that match a given namespace path.
#[salsa::tracked(returns(ref), no_eq)]
pub fn shared_namespaces<'db>(
    db: &'db dyn BaseDatabase,
    path: NamespacePath,
) -> Vec<Namespace<'db>> {
    db.get_files()
        .iter()
        .flat_map(|file| {
            semantic_index(db, *file)
                .namespaces
                .iter()
                .filter_map(move |ns| (ns.path(db) == &path).then_some(*ns))
        })
        .collect()
}

/// Rules for resolving USING directives
///
/// We first search if the namespace matches any of the namespaces declared in all files (via [`shared_namespaces`]).
///
/// The, we check if the directive is not declared multiple times in the same scope.
#[tracing::instrument(skip_all, name = "imported_namespaces_for_using")]
fn imported_namespaces<'db>(
    db: &'db dyn BaseDatabase,
    file: File,
    using: Using<'db>,
) -> FxHashMap<NamespacePath, Namespace<'db>> {
    let mut result = FxHashMap::default();
    let path = using.path(db);

    let matching_namespaces = shared_namespaces(db, path);

    if matching_namespaces.is_empty() {
        namespace_not_found(db, using.get_span(db).clone(), path);
        return result;
    }

    // Check for duplicate `USING` in the same top-level scope
    let sema = semantic_index(db, file);
    let scope = sema.get_scope(using.scope_id(db));
    for other in &scope.usings {
        if *other != using && other.path(db) == path {
            duplicate_using_declaration(db, file, using, *other);
        }
    }

    for ns in matching_namespaces {
        result.insert(*ns.path(db), *ns);
    }

    result
}

#[tracing::instrument(skip_all)]
/// Resolve a namespace access to a POU declaration.
pub fn resolve_namespace_access<'db>(
    db: &'db dyn BaseDatabase,
    file: File,
    scope: FileScopeId,
    access: NamespaceAccess,
) -> Option<PouDecl<'db>> {
    let target = access.target(db);

    match access.namespace(db) {
        // There's a namespace specified, so we look for it
        Some(path) => shared_namespaces(db, path).iter().find_map(|ns| {
            ns.pous(db)
                .iter()
                .find(|pou| *pou.name(db) == target.ident)
                .copied()
        }),
        // None, look for the POU in the current scope
        None => pous_in_scope(db, file, scope).get(&target.ident).copied(),
    }
}

/// Returns all POU declarations *globally declared*.
#[salsa::tracked(returns(ref))]
fn global_pous<'db>(db: &'db dyn BaseDatabase) -> Vec<PouDecl<'db>> {
    db.get_files()
        .iter()
        .flat_map(|file| semantic_index(db, *file).global_pous.clone())
        .collect()
}

/// Returns all POU declarations *locally declared* in this scope.
#[salsa::tracked(returns(ref))]
fn local_pous_in_scope<'db>(
    db: &'db dyn BaseDatabase,
    file: File,
    scope_id: FileScopeId,
) -> Vec<PouDecl<'db>> {
    let sema = semantic_index(db, file);
    let scope = sema.get_scope(scope_id);

    match scope.kind {
        ScopeKind::Namespace(ns) => ns.pous(db).to_vec(),
        _ => Vec::new(),
    }
}

/// Returns all POU declarations *imported* into this scope via USING directives.
#[salsa::tracked(returns(ref))]
fn imported_pous_in_scope<'db>(
    db: &'db dyn BaseDatabase,
    file: File,
    scope_id: FileScopeId,
) -> Vec<PouDecl<'db>> {
    let sema = semantic_index(db, file);
    let scope = sema.get_scope(scope_id);

    let mut result = Vec::new();

    for using in &scope.usings {
        let namespaces = imported_namespaces(db, file, *using);
        for (_, ns) in namespaces {
            result.extend_from_slice(ns.pous(db));
        }
    }

    result
}

/// Returns all POU declarations from parent (ancestor) scopes, including shared namespaces the global scope.
#[salsa::tracked(returns(ref))]
fn inherited_pous<'db>(
    db: &'db dyn BaseDatabase,
    file: File,
    scope_id: FileScopeId,
) -> Vec<PouDecl<'db>> {
    let sema = semantic_index(db, file);
    let mut result = Vec::new();

    let it = sema.scope_iterator(scope_id);
    for scope in it {
        if scope.is_global() {
            result.extend_from_slice(&sema.global_pous);
        } else if let ScopeKind::Namespace(ns) = scope.kind {
            shared_namespaces(db, *ns.path(db)).iter().for_each(|ns| {
                result.extend_from_slice(ns.pous(db));
            });
        }
    }

    result
}

/// Returns all visible POU declarations in the given scope:
/// - Imported via USING
/// - Locally declared
/// - Inherited from ancestor scopes
#[salsa::tracked(returns(ref))]
pub fn pous_in_scope<'db>(
    db: &'db dyn BaseDatabase,
    file: File,
    scope_id: FileScopeId,
) -> FxHashMap<Ident, PouDecl<'db>> {
    let mut map = FxHashMap::default();

    // Note that the order of these calls matters:
    // 1 Global POU declarations
    // 2 Inherited POU declarations
    // 3 Imported POU declarations
    // 4 Local POU declarations

    // If a same name is found in multiple sources, the last one will shadow the previous ones.

    for pou in global_pous(db) {
        map.insert(*pou.name(db), *pou);
    }

    for pou in inherited_pous(db, file, scope_id) {
        map.insert(*pou.name(db), *pou);
    }

    for pou in imported_pous_in_scope(db, file, scope_id) {
        map.insert(*pou.name(db), *pou);
    }

    for pou in local_pous_in_scope(db, file, scope_id) {
        map.insert(*pou.name(db), *pou);
    }

    map
}

#[salsa::tracked(returns(ref))]
pub fn variables_in_scope<'db>(
    db: &'db dyn BaseDatabase,
    file: File,
    scope_id: FileScopeId,
) -> FxHashMap<Ident, Variable<'db>> {
    let sema = semantic_index(db, file);
    let scope = sema.get_scope(scope_id);

    let mut map = FxHashMap::default();

    if let ScopeKind::Pou(pou) = scope.kind {
        match pou.pou(db) {
            Pou::Function(f) => {
                for var in f.variables(db) {
                    map.insert(*var.name(db), *var);
                }
            }
            Pou::FunctionBlock(fb) => {
                for var in fb.variables(db) {
                    map.insert(*var.name(db), *var);
                }
            }
            _ => {}
        }
    }

    map
}
