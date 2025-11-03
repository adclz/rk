use auto_lsp::default::db::BaseDatabase;
use rustc_hash::FxHashMap;

use crate::{
    HirNodeInfo,
    hir_def::{
        interned::{
            identifier::{Ident, SpanIdent},
            namespace::{NamespaceAccess, NamespacePath},
        },
        namespace::NamespaceDecl,
        pous::pou::PouDecl,
        scope::{ScopeId, ScopeKind},
        semantic_index::{get_scope, semantic_index},
    },
    hir_ty::using_resolver::resolve_using,
};

/// Returns all Namespaces.
#[salsa::tracked(returns(ref), no_eq)]
pub fn global_namespace_index<'db>(
    db: &'db dyn BaseDatabase,
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

/// Returns all POUs *globally declared*.
#[salsa::tracked(returns(ref), no_eq)]
pub fn global_pou_index<'db>(db: &'db dyn BaseDatabase) -> FxHashMap<Ident, PouDecl<'db>> {
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

#[tracing::instrument(skip_all)]
/// Resolve a namespace access to a POU declaration.
pub fn resolve_namespace_access<'db>(
    db: &'db dyn BaseDatabase,
    access: NamespaceAccess,
) -> Option<PouDecl<'db>> {
    let target = access.target(db);

    match access.namespace(db) {
        // There's a namespace specified, so we look for it
        Some(path) => global_namespace_index(db)
            .get(&path)?
            .iter()
            .find_map(|ns| pou_names_res(db, &target)),
        // None, look for the POU in the current scope
        None => pou_names_res(db, &target),
    }
}

#[tracing::instrument(skip_all)]
pub fn find_in_parent_pous<'db>(
    db: &'db dyn BaseDatabase,
    name: &SpanIdent<'db>,
) -> Option<PouDecl<'db>> {
    let it = semantic_index(db, name.scope_id.file(db)).scope_iterator(db, name.scope_id);
    for scope in it {
        if let ScopeKind::Namespace(ns) = scope.kind {
            if let Some(namespaces) = global_namespace_index(db).get(ns.path(db)) {
                for ns in namespaces.iter() {
                    if let Some(pou) = ns.pous(db).iter().find(|p| p.name(db) == &name.ident) {
                        return Some(*pou);
                    }
                }
            }
        }

        for using in &scope.usings {
            if let Some(namespaces) = global_namespace_index(db).get(&using.path(db)) {
                for ns in namespaces.iter() {
                    if let Some(pou) = ns.pous(db).iter().find(|p| p.name(db) == &name.ident) {
                        return Some(*pou);
                    }
                }
            }
        }
    }

    None
}

pub fn pou_names_res<'db>(
    db: &'db dyn BaseDatabase,
    pou: &SpanIdent<'db>,
) -> Option<PouDecl<'db>> {
    // Checks for POUs declared in the current scope
    pou.scope_id
        .def_map(db)
        .local_pous
        .get(pou)
        .copied()
        // Checks for parent POUs and those imported via USING directives
        .or_else(|| find_in_parent_pous(db, pou))
        // Checks for POUs declared globally
        .or_else(|| global_pou_index(db).get(pou).copied())
}
