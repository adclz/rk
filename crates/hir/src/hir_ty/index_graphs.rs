use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

use crate::{
    HasName,
    hir_def::{
        config::{ConfigDecl, ConfigResource},
        interned::{
            identifier::Ident,
            namespace::NamespacePath,
        },
        namespace::NamespaceDecl,
        pous::{pou::Pou, variable::VariableDecl},
        program::ProgramDecl,
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

    eprintln!(
        "size of stdlib namespace index: {}",
        db.get_std_lib_files().len()
    );
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
    }

    for file in db.get_std_lib_files().iter() {
        for p in semantic_index(db, *file).global_pous.iter() {
            result.insert(p.get_name_ident(db), *p);
        }
    }
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
    }

    for file in db.get_std_lib_files().iter() {
        for p in semantic_index(db, *file).programs.iter() {
            result.insert(p.get_name_ident(db), *p);
        }
    }

    result
}

#[tracing::instrument(skip(db))]
pub fn program_index<'db>(db: &'db dyn WorkspaceDataBase, name: Ident) -> Option<ProgramDecl<'db>> {
    workspace_program_index(db).get(&name).copied()
}

/// Returns globally declared configurations from workspace files.
#[tracing::instrument(skip_all)]
#[salsa::tracked(returns(ref))]
fn workspace_config_index<'db>(
    db: &'db dyn WorkspaceDataBase,
) -> FxHashMap<Ident, ConfigDecl<'db>> {
    let mut result = FxHashMap::default();
    for file in db.get_files().iter() {
        for c in semantic_index(db, *file).configs.iter() {
            result.insert(c.get_name_ident(db), *c);
        }
    }
    for file in db.get_std_lib_files().iter() {
        for c in semantic_index(db, *file).configs.iter() {
            result.insert(c.get_name_ident(db), *c);
        }
    }
    result
}

#[tracing::instrument(skip(db))]
pub fn config_index<'db>(db: &'db dyn WorkspaceDataBase, name: Ident) -> Option<ConfigDecl<'db>> {
    workspace_config_index(db).get(&name).copied()
}

/// Collects all VAR_GLOBAL variables from every CONFIGURATION and RESOURCE in the workspace.
///
/// Used to validate VAR_EXTERNAL declarations: any VAR_EXTERNAL must reference a name
/// that exists in at least one VAR_GLOBAL across all configs/resources.
#[tracing::instrument(skip_all)]
#[salsa::tracked(returns(ref))]
fn workspace_config_globals<'db>(
    db: &'db dyn WorkspaceDataBase,
) -> FxHashMap<Ident, VariableDecl<'db>> {
    let mut result: FxHashMap<Ident, VariableDecl<'db>> = FxHashMap::default();

    let all_files = db.get_files().iter().chain(db.get_std_lib_files().iter());
    for file in all_files {
        for config in semantic_index(db, *file).configs.iter() {
            for v in config.variables(db).iter() {
                result.insert(v.get_name_ident(db), *v);
            }
            for res in config.resources(db).iter() {
                if let ConfigResource::Resource(r) = res {
                    for v in r.variables.iter() {
                        result.insert(v.get_name_ident(db), *v);
                    }
                }
            }
        }
    }
    result
}

/// Looks up a VAR_GLOBAL by name across all configs/resources in the workspace.
pub fn external_var_lookup<'db>(
    db: &'db dyn WorkspaceDataBase,
    var_name: Ident,
) -> Option<VariableDecl<'db>> {
    workspace_config_globals(db).get(&var_name).copied()
}

