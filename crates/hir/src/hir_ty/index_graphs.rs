use db::WorkspaceDataBase;

use crate::{
    HasName,
    hir_def::{
        config::{ConfigDecl, ConfigResource},
        interned::{identifier::Ident, namespace::NamespacePath},
        namespace::NamespaceDecl,
        pous::{pou::Pou, variable::VariableDecl},
        program::ProgramDecl,
        semantic_index::semantic_index,
    },
};

/// Helper to iterate over all workspace + stdlib files.
fn all_files<'db>(
    db: &'db dyn WorkspaceDataBase,
) -> impl Iterator<Item = auto_lsp::default::db::file::File> + 'db {
    db.get_files()
        .iter()
        .map(|e| *e)
        .chain(db.get_std_lib_files().iter().map(|e| *e))
}

/// Returns all namespace declarations matching a given path across all files.
#[tracing::instrument(skip(db))]
pub fn namespace_index<'db>(
    db: &'db dyn WorkspaceDataBase,
    path: NamespacePath,
) -> Vec<NamespaceDecl<'db>> {
    let mut result = vec![];
    for file in all_files(db) {
        for ns in semantic_index(db, file).global_namespaces.iter() {
            if *ns.path(db) == path {
                result.push(*ns);
            }
        }
    }
    result
}

/// Returns the canonical POU for a given name within a namespace path.
#[tracing::instrument(skip(db))]
pub fn namespace_pou_index<'db>(
    db: &'db dyn WorkspaceDataBase,
    path: NamespacePath,
    name: Ident,
) -> Option<Pou<'db>> {
    for ns in namespace_index(db, path) {
        if let Some(pou) = ns.scope_id(db).def_map(db).local_pous.get(&name) {
            return Some(*pou);
        }
    }
    None
}

/// Finds a globally declared POU by name across all files.
#[tracing::instrument(skip(db))]
pub fn pou_index<'db>(db: &'db dyn WorkspaceDataBase, name: Ident) -> Option<Pou<'db>> {
    for file in all_files(db) {
        for p in semantic_index(db, file).global_pous.iter() {
            if p.get_name_ident(db) == name {
                return Some(*p);
            }
        }
    }
    None
}

/// Finds a globally declared program by name across all files.
#[tracing::instrument(skip(db))]
pub fn program_index<'db>(db: &'db dyn WorkspaceDataBase, name: Ident) -> Option<ProgramDecl<'db>> {
    for file in all_files(db) {
        for p in semantic_index(db, file).programs.iter() {
            if p.get_name_ident(db) == name {
                return Some(*p);
            }
        }
    }
    None
}

/// Finds a globally declared configuration by name across all files.
#[tracing::instrument(skip(db))]
pub fn config_index<'db>(db: &'db dyn WorkspaceDataBase, name: Ident) -> Option<ConfigDecl<'db>> {
    for file in all_files(db) {
        for c in semantic_index(db, file).configs.iter() {
            if c.get_name_ident(db) == name {
                return Some(*c);
            }
        }
    }
    None
}

/// Looks up a VAR_GLOBAL by name across all configs/resources in the workspace.
///
/// Used to validate VAR_EXTERNAL declarations: any VAR_EXTERNAL must reference a name
/// that exists in at least one VAR_GLOBAL across all configs/resources.
pub fn external_var_lookup<'db>(
    db: &'db dyn WorkspaceDataBase,
    var_name: Ident,
) -> Option<VariableDecl<'db>> {
    for file in all_files(db) {
        for config in semantic_index(db, file).configs.iter() {
            for v in config.variables(db).iter() {
                if v.get_name_ident(db) == var_name {
                    return Some(*v);
                }
            }
            for res in config.resources(db).iter() {
                if let ConfigResource::Resource(r) = res {
                    for v in r.variables(db).iter() {
                        if v.get_name_ident(db) == var_name {
                            return Some(*v);
                        }
                    }
                }
            }
        }
    }
    None
}
