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
        program::ProgramDecl,
        scope::{ScopeId, ScopeKind},
        semantic_index::semantic_index,
    },
};


// Note for later:
// Ideally, we do not need indexes, we should instead use the semantic index directly to resolve names. 
// Therefore it is necessary to add namespaces to ScopeDefMap and iterate over them when resolving names.
// Such operation woud be O(N) where N is, at worst, the number of files (we use the def maps to check if a pou/namespace is declared inside a scope,
// which is O(1) by simply looking at an interned identifier key)

// That means the invalidation of a cross-file dependency is dependent on the found item itself,
// But with indexes they have to be invalidated whenever a file is changed, which is not ideal.

// This is very similar to how rust-analyze implements the all_crates() query (https://github.com/rust-lang/rust-analyzer/blob/d8e0e96766968bfebca2360ae4cb8f08d7bbab18/crates/base-db/src/lib.rs#L266)
// But in the case of RA, this query is a salsa::input and the comment above it states that it should not be used by HIR crates because it is always invalidated.
// I am not sure if this applies to our case, needs further investigation.

// Creating nested queries (those exposed publicly in this module) so the invalidation stop propagating when a pou has not changed
// creates non deterministic behavior and thus leaks salsa structs

/// Returns namespaces from workspace files only.
#[tracing::instrument(skip_all)]
#[salsa::tracked(returns(ref))]
fn workspace_namespace_index<'db>(
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

    eprintln!("size of stdlib namespace index: {}", db.get_std_lib_files().len());
    for file in db.get_std_lib_files().iter() {
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
    workspace_namespace_index(db)
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
pub fn namespace_pou_index<'db>(
    db: &'db dyn WorkspaceDataBase,
    path: NamespacePath,
    name: Ident,
) -> Option<Pou<'db>> {
    global_namespace_pou_index(db, path).get(&name).copied()
}

/// Returns globally declared POUs from workspace files only.
#[tracing::instrument(skip_all)]
#[salsa::tracked(returns(ref))]
fn workspace_pou_index<'db>(db: &'db dyn WorkspaceDataBase) -> FxHashMap<Ident, Pou<'db>> {
    let mut result = FxHashMap::default();

    for file in db.get_files().iter() {
        for p in semantic_index(db, *file).global_pous.iter() {
            result.insert(p.get_name_ident(db), *p);
        }
    };
    
    for file in db.get_std_lib_files().iter() {
        for p in semantic_index(db, *file).global_pous.iter() {
            result.insert(p.get_name_ident(db), *p);
        }
    };
    result
}

#[tracing::instrument(skip(db))]
pub fn pou_index<'db>(db: &'db dyn WorkspaceDataBase, name: Ident) -> Option<Pou<'db>> {
    workspace_pou_index(db).get(&name).copied()
}

/// Returns globally declared programs from workspace files only.
#[tracing::instrument(skip_all)]
#[salsa::tracked(returns(ref))]
fn workspace_program_index<'db>(
    db: &'db dyn WorkspaceDataBase,
) -> FxHashMap<Ident, ProgramDecl<'db>> {
    let mut result = FxHashMap::default();
    for file in db.get_files().iter() {
        for p in semantic_index(db, *file).programs.iter() {
            result.insert(p.get_name_ident(db), *p);
        }
    };


    for file in db.get_std_lib_files().iter() {
        for p in semantic_index(db, *file).programs.iter() {
            result.insert(p.get_name_ident(db), *p);
        }
    };

    result
}

#[tracing::instrument(skip(db))]
pub fn program_index<'db>(db: &'db dyn WorkspaceDataBase, name: Ident) -> Option<ProgramDecl<'db>> {
    workspace_program_index(db).get(&name).copied()
}

#[tracing::instrument(skip_all)]
/// Resolve a namespace access to a POU declaration.
pub(crate) fn resolve_namespace_access<'db>(
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
            find_in_parent_pous(db, name, scope).or_else(|| pou_index(db, name))
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
