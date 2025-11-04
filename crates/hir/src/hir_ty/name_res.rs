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
    hir_ty::{def_map, using_resolver::resolve_using},
};

// todo: for both indexes, use salsa::par_map to parallelize the construction


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
    access: &NamespaceAccess<'db>,
) -> Option<PouDecl<'db>> {
    let target = &access.target;

    match access.namespace {
        // There's a namespace specified, so we look for it
        Some(path) => global_namespace_index(db)
            .get(&path)?
            .iter()
            .find_map(|ns| pou_names_res(db, &target)),
        // None, look for the POU in the current scope
        None => pou_names_res(db, target),
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
                    if let Some(p) = ns.scope_id(db).def_map(db).local_pous.get(&name.ident) {
                        return Some(*p);
                    }
                }
            }
        }

        for using in &scope.usings {
            if let Some(namespaces) = global_namespace_index(db).get(&using.path(db)) {
                for ns in namespaces.iter() {
                    if let Some(p) = ns.scope_id(db).def_map(db).local_pous.get(&name.ident) {
                        return Some(*p);
                    }
                }
            }
        }
    }

    None
}

pub fn pou_names_res<'db>(
    db: &'db dyn BaseDatabase,
    span_ident: &SpanIdent<'db>,
) -> Option<PouDecl<'db>> {
    // Checks for POUs declared in the current scope
    span_ident
        .scope_id
        .def_map(db)
        .local_pous
        .get(span_ident)
        .copied()
        .or_else(|| {
            // Checks for parent POUs and those imported via USING directives
            find_in_parent_pous(db, span_ident)
                .or_else(|| global_pou_index(db).get(&span_ident.ident).copied())
        })
}
