use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

use crate::{
    hir_def::{
        interned::{
            identifier::Ident,
            namespace::{NamespaceAccess, NamespacePath},
        },
        namespace::NamespaceDecl,
        pous::pou::PouDecl,
        scope::{FileScopeId, ScopeKind},
        semantic_index::semantic_index,
    },
    hir_ty::using_resolver::resolve_using,
};

/// Find all namespaces in all files that match a given namespace path.
#[salsa::tracked(returns(ref))]
pub fn shared_namespaces<'db>(
    db: &'db dyn BaseDatabase,
    path: NamespacePath,
) -> Vec<NamespaceDecl<'db>> {
    db.get_files()
        .iter()
        .flat_map(|file| {
            semantic_index(db, *file)
                .global_namespaces
                .iter()
                .filter_map(move |ns| (ns.path(db) == &path).then_some(*ns))
        })
        .collect()
}

#[tracing::instrument(skip_all)]
#[salsa::tracked]
/// Resolve a namespace access to a POU declaration.
pub fn resolve_namespace_access<'db>(
    db: &'db dyn BaseDatabase,
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
        None => pou_names_res(db, &target.ident, access.target(db).scope_id),
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
    let mut result = FxHashMap::default();
    let sema = semantic_index(db, scope_id.file(db));

    // A scope always refers to the current scope of the element.
    // But in the case of NAMESPACE, POUs have access to the USING directives of the parent namespace.
    // It is then necessary to check both the POU's directives AND the parent's directives.
    let scope = match sema.get_scope(db, scope_id).kind {
        // Inside Global Scope, just check the current scope.
        ScopeKind::Global => sema.get_scope(db, scope_id),
        // Same, NAMESPACES do not have access to the USING directives of the parent namespace.
        ScopeKind::Namespace(_) => sema.get_scope(db, scope_id),
        // POUs must check both their own USING directives and the USING directives of their parent namespace.
        ScopeKind::Pou(_) => {
            let scope = sema.get_scope(db, scope_id);
            for using in &scope.usings {
                let namespaces = resolve_using(db, *using);
                for ns in namespaces.namespaces(db) {
                    result.extend(ns.pous(db).iter().map(|p| (*p.name(db), *p)));
                }
            }
            if let Some(parent) = scope.parent {
                let parent_scope = sema.get_scope(db, parent);
                for using in &parent_scope.usings {
                    let namespaces = resolve_using(db, *using);
                    for ns in namespaces.namespaces(db) {
                        result.extend(ns.pous(db).iter().map(|p| (*p.name(db), *p)));
                    }
                }
            }
            return result;
        }
    };

    for using in &scope.usings {
        let namespaces = resolve_using(db, *using);
        for ns in namespaces.namespaces(db) {
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
    all_local_pous(db, scope_id)
        .get(pou)
        .or_else(|| all_imported_pous(db, scope_id).get(pou))
        .or_else(|| all_inherited_pous(db, scope_id).get(pou))
        .or_else(|| all_global_pous(db).get(pou))
        .copied()
}


pub fn all_pous_in_scope<'db>(
    db: &'db dyn BaseDatabase,
    scope_id: FileScopeId<'db>,
) -> FxHashMap<Ident, PouDecl<'db>> {
    let mut result = FxHashMap::default();
    result.extend(all_local_pous(db, scope_id));
    result.extend(all_imported_pous(db, scope_id));
    result.extend(all_inherited_pous(db, scope_id));
    result.extend(all_global_pous(db));
    result
}