use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

use crate::{
    HasName, HirNodeInfo,
    hir_def::{
        interned::{
            identifier::{Ident, SpanIdent},
            namespace::{NamespaceAccess, NamespacePath},
        },
        namespace::NamespaceDecl,
        pous::pou::Pou,
        scope::{ScopeId, ScopeKind},
        semantic_index::semantic_index,
    },
};

// todo: for both indexes, use salsa::par_map to parallelize the construction

/// Returns all Namespaces.
#[tracing::instrument(skip_all)]
#[salsa::tracked(returns(ref))]
fn global_namespace_index<'db>(
    db: &'db dyn WorkspaceDataBase,
) -> FxHashMap<NamespacePath, Vec<NamespaceDecl<'db>>> {
    let mut result = FxHashMap::default();
    for file in db.get_files().iter() {
        for ns in semantic_index(db, *file).global_namespaces.iter() {
            result
                .entry(*ns.path(db))
                .or_insert_with(Vec::new)
                .push(*ns);
        }
    }
    result
}

#[tracing::instrument(skip(db))]
#[salsa::tracked(returns(ref))]
pub fn namespace_index<'db>(
    db: &'db dyn WorkspaceDataBase,
    path: NamespacePath,
) -> Vec<NamespaceDecl<'db>> {
    global_namespace_index(db)
        .get(&path)
        .cloned()
        .unwrap_or_default()
}

/// Returns all POUs within namespaces that share the same path, indexed by name.
#[tracing::instrument(skip_all)]
#[salsa::tracked(returns(ref))]
fn global_namespace_pou_index<'db>(
    db: &'db dyn WorkspaceDataBase,
    path: NamespacePath,
) -> FxHashMap<Ident, Pou<'db>> {
    namespace_index(db, path)
        .iter()
        .flat_map(|ns| ns.pous(db).iter().map(|p| (p.get_name_ident(db), *p)))
        .collect()
}

/// Returns the canonical POU for a given name within a namespace path.
#[tracing::instrument(skip(db))]
#[salsa::tracked(returns(ref))]
pub fn namespace_pou_index<'db>(
    db: &'db dyn WorkspaceDataBase,
    path: NamespacePath,
    name: Ident,
) -> Option<Pou<'db>> {
    global_namespace_pou_index(db, path).get(&name).copied()
}

/// Returns all POUs *globally declared*.
#[tracing::instrument(skip_all)]
#[salsa::tracked(returns(ref))]
fn global_pou_index<'db>(db: &'db dyn WorkspaceDataBase) -> FxHashMap<Ident, Pou<'db>> {
    db.get_files()
        .iter()
        .flat_map(|file| {
            semantic_index(db, *file)
                .global_pous
                .iter()
                .map(|p| (p.get_name_ident(db), *p))
        })
        .collect()
}

#[tracing::instrument(skip(db))]
#[salsa::tracked(returns(ref))]
pub fn pou_index<'db>(db: &'db dyn WorkspaceDataBase, name: Ident) -> Option<Pou<'db>> {
    global_pou_index(db).get(&name).copied()
}

#[tracing::instrument(skip_all)]
/// Resolve a namespace access to a POU declaration.
pub fn resolve_namespace_access<'db>(
    db: &'db dyn WorkspaceDataBase,
    access: &NamespaceAccess<'db>,
) -> Option<Pou<'db>> {
    let target = &access.target;

    match &access.namespace {
        // There's a namespace specified, so we look for it
        Some(path) => namespace_index(db, **path)
            .iter()
            .find_map(|ns| pou_names_res(db, target.ident, ns.scope_id(db))),
        // None, look for the POU in the current scope
        None => pou_names_res(db, target.ident, target.scope_id),
    }
}

#[tracing::instrument(skip_all)]
pub fn find_in_parent_pous<'db>(
    db: &'db dyn WorkspaceDataBase,
    name: Ident,
    scope: ScopeId<'db>,
) -> Option<Pou<'db>> {
    let it = semantic_index(db, scope.file(db)).scope_iterator(db, scope);
    for scope in it {
        // Find POUs in all shared namespaces
        if let ScopeKind::Namespace(ns) = scope.kind {
            for ns in namespace_index(db, *ns.path(db)).iter() {
                if let Some(p) = ns.scope_id(db).def_map(db).local_pous.get(&name) {
                    return Some(*p);
                }
            }
        }

        // Find POUs in all USING directives
        for using in &scope.usings {
            for ns in namespace_index(db, *using.path(db)).iter() {
                if let Some(p) = ns.scope_id(db).def_map(db).local_pous.get(&name) {
                    return Some(*p);
                }
            }
        }
    }

    None
}

pub fn pou_names_res<'db>(
    db: &'db dyn WorkspaceDataBase,
    name: Ident,
    scope: ScopeId<'db>,
) -> Option<Pou<'db>> {
    // Checks for POUs declared in the current scope
    scope
        .def_map(db)
        .local_pous
        .get(&name)
        .copied()
        .or_else(|| {
            // Checks for parent POUs and those imported via USING directives
            find_in_parent_pous(db, name, scope).or_else(|| *pou_index(db, name))
        })
}

#[cfg(debug_assertions)]
pub fn pou_name_res_from_scope<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: impl HirNodeInfo<'db>,
    name: &str,
) -> Option<Pou<'db>> {
    let span_ident = SpanIdent {
        id: crate::AstId(0),
        ident: Ident::from_slice(db, name),
        scope_id: scope.get_scope_id(db),
    };
    pou_names_res(db, span_ident.ident, scope.get_scope_id(db))
}
