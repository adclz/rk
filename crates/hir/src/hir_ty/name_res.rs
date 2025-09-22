use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

use crate::hir_def::{
    interned::{
        identifier::Ident,
        namespace::{NamespaceAccess, NamespacePath},
    },
    namespace::NamespaceDecl,
    pous::{
        pou::{Pou, PouDecl},
        variable::VariableDecl,
    },
    scope::{FileScopeId, ScopeKind},
    semantic_index::semantic_index,
    using::Using,
};

/// Find all namespaces in all files that match a given namespace path.
#[salsa::tracked(returns(ref), no_eq)]
pub fn shared_namespaces<'db>(
    db: &'db dyn BaseDatabase,
    path: NamespacePath,
) -> Vec<NamespaceDecl<'db>> {
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
#[tracing::instrument(skip_all, name = "imported_namespaces_for_using")]
fn imported_namespaces<'db>(
    db: &'db dyn BaseDatabase,
    using: Using<'db>,
) -> FxHashMap<NamespacePath, NamespaceDecl<'db>> {
    let mut result = FxHashMap::default();
    let path = using.path(db);

    let matching_namespaces = shared_namespaces(db, path);

    if matching_namespaces.is_empty() {
        return result;
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
    scope: FileScopeId<'db>,
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
        None => pou_names_res(db, &target.ident, scope),
    }
}

/// Returns all POU declarations *globally declared*.
#[salsa::tracked(returns(ref))]
pub fn all_global_pous<'db>(db: &'db dyn BaseDatabase) -> FxHashMap<Ident, PouDecl<'db>> {
    db.get_files()
        .iter()
        .flat_map(|file| {
            semantic_index(db, *file)
                .global_pous
                .iter()
                .map(|p| (*p.name(db), *p))
        })
        .collect()
}

/// Returns all POU declarations *globally declared*.
#[salsa::tracked(returns(ref))]
pub fn all_local_pous<'db>(
    db: &'db dyn BaseDatabase,
    scope_id: FileScopeId<'db>,
) -> FxHashMap<Ident, PouDecl<'db>> {
    let sema = semantic_index(db, scope_id.file(db));
    let scope = sema.get_scope(db, scope_id);

    match scope.kind {
        ScopeKind::Namespace(ns) => ns.pous(db).iter().map(|p| (*p.name(db), *p)).collect(),
        _ => FxHashMap::default(),
    }
}

/// Returns all POU declarations *globally declared*.
#[salsa::tracked(returns(ref))]
pub fn all_imported_pous<'db>(
    db: &'db dyn BaseDatabase,
    scope_id: FileScopeId<'db>,
) -> FxHashMap<Ident, PouDecl<'db>> {
   let sema = semantic_index(db, scope_id.file(db));
    let scope = sema.get_scope(db, scope_id);

    let mut result = FxHashMap::default();

    for using in &scope.usings {
        let namespaces = imported_namespaces(db, *using);
        for (_, ns) in namespaces {
            result.extend(ns.pous(db).iter().map(|p| (*p.name(db), *p)));
        }
    }

    result
}

// Returns all POU declarations *globally declared*.
#[salsa::tracked(returns(ref))]
pub fn all_inherited_pous<'db>(
    db: &'db dyn BaseDatabase,
    scope_id: FileScopeId<'db>,
) -> FxHashMap<Ident, PouDecl<'db>> {
    let sema = semantic_index(db, scope_id.file(db));
    let mut result = FxHashMap::default();

    let it = sema.scope_iterator(db, scope_id);
    for scope in it {
        if let ScopeKind::Namespace(ns) = scope.kind {
            shared_namespaces(db, *ns.path(db)).iter().for_each(|ns| {
                result.extend(ns.pous(db).iter().map(|p| (*p.name(db), *p)));
            });
        }
    }

    result
}

pub fn pou_names_res<'db>(
    db: &'db dyn BaseDatabase,
    pou: &Ident,
    scope_id: FileScopeId<'db>,
) -> Option<PouDecl<'db>> {
    all_local_pous(db, scope_id).get(pou)
        .or_else(|| all_imported_pous(db, scope_id).get(pou))
        .or_else(|| all_inherited_pous(db, scope_id).get(pou))
        .or_else(|| all_global_pous(db).get(pou))
        .copied()
}

#[salsa::tracked(returns(ref))]
pub fn variables_in_scope<'db>(
    db: &'db dyn BaseDatabase,
    scope_id: FileScopeId<'db>,
) -> FxHashMap<Ident, VariableDecl<'db>> {
    let sema = semantic_index(db, scope_id.file(db));
    let scope = sema.get_scope(db, scope_id);

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
