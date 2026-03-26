// # Index Graphs — Cross-File Name Resolution
//
// ## Original problem (pre-2026-03-09)
//
// This module was originally implemented using workspace-wide salsa queries
// (`workspace_pou_index`, `workspace_namespace_index`, etc.) that aggregated
// all files into HashMaps. The problem was:
//
// Adrien Clauzel:
// > Ideally, we do not need indexes, we should instead use the semantic index
// > directly to resolve names. Therefore it is necessary to add namespaces to
// > ScopeDefMap and iterate over them when resolving names. Such operation
// > would be O(N) where N is, at worst, the number of files (we use the def
// > maps to check if a pou/namespace is declared inside a scope, which is
// > O(1) by simply looking at an interned identifier key).
// >
// > That means the invalidation of a cross-file dependency is dependent on
// > the found item itself, but with indexes they have to be invalidated
// > whenever a file is changed, which is not ideal.
// >
// > This is very similar to how rust-analyzer implements the all_crates()
// > query. But in the case of RA, this query is a salsa::input and the
// > comment above it states that it should not be used by HIR crates because
// > it is always invalidated.
// >
// > Creating nested queries (those exposed publicly in this module) so the
// > invalidation stops propagating when a pou has not changed creates
// > non-deterministic behavior and thus leaks salsa structs.
//
// ## 2026-03-09 — Per-file extraction queries (Adrien + Claude)
//
// The workspace-wide `#[salsa::tracked]` queries were removed and replaced
// with simple iteration over `semantic_index(db, file)` for each file,
// as the original comment above suggested.
//
// Claude proposed adding intermediate per-file extraction queries
// (`file_global_pous`, `file_namespaces`, etc.) as an "Eq firewall"
// between `semantic_index` (which is `no_eq`) and downstream consumers —
// similar to how ruff/ty uses `place_table(scope)` and `use_def_map(scope)`.
//
// Adrien pointed out that salsa's "twist" (backdating) should handle this
// at the tracked struct level without the extra layer: even though
// `semantic_index` is `no_eq`, the tracked structs it produces have stable
// identity, so downstream queries like `infer_signature(pou)` shouldn't
// need to re-execute.
//
// This turned out to be partially correct: backdating at the tracked struct
// level works for *field-level* dependencies (e.g. reading `pou.name(db)`).
// However, the lookup functions in this module (`pou_index`, etc.) are
// regular functions — not salsa queries — so when `infer_signature(main)`
// calls `pou_index()` which reads `semantic_index(file0)`, salsa records a
// direct dependency from `infer_signature(main)` to `semantic_index(file0)`.
// Since `semantic_index` is `no_eq`, this always marks the downstream query
// as dirty, bypassing the tracked struct backdating entirely.
//
// The extraction queries solve this by interposing a salsa query with Eq
// between `semantic_index` and the lookup functions. When file0's body
// changes, `file_global_pous(file0)` re-executes but returns the same
// `Vec<Pou>`, salsa backdates it, and `infer_signature(main)` (which now
// depends on `file_global_pous(file0)` instead of `semantic_index(file0)`)
// is NOT re-executed.
//
// Confirmed by the incremental test suite (`src/tests/incremental.rs`).

use std::sync::Arc;

use auto_lsp::default::db::file::File;
use db::WorkspaceDataBase;

use crate::{
    HasName,
    hir_def::{
        config::{ConfigDecl, ConfigResource},
        interned::{identifier::Ident, namespace::NamespacePath},
        namespace::NamespaceDecl,
        pous::{function::Function, pou::Pou, variable::VariableDecl},
        program::ProgramDecl,
        semantic_index::semantic_index,
    },
};

// ---------------------------------------------------------------------------
// Per-file extraction queries (Eq firewall)
// ---------------------------------------------------------------------------

/// Extracts global POUs from a file's semantic index.
///
/// This query supports Eq (unlike `semantic_index` which is `no_eq`),
/// so body-only edits that produce the same POUs will backdate and
/// not invalidate downstream lookups.
#[salsa::tracked(returns(ref))]
pub fn file_global_pous<'db>(db: &'db dyn WorkspaceDataBase, file: File) -> Arc<Vec<Pou<'db>>> {
    Arc::clone(&semantic_index(db, file).global_pous)
}

/// Extracts namespace declarations from a file's semantic index.
#[salsa::tracked(returns(ref))]
pub fn file_namespaces<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: File,
) -> Arc<Vec<NamespaceDecl<'db>>> {
    Arc::clone(&semantic_index(db, file).namespaces)
}

/// Extracts program declarations from a file's semantic index.
#[salsa::tracked(returns(ref))]
pub fn file_programs<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: File,
) -> Arc<Vec<ProgramDecl<'db>>> {
    Arc::clone(&semantic_index(db, file).programs)
}

/// Extracts configuration declarations from a file's semantic index.
#[salsa::tracked(returns(ref))]
pub fn file_configs<'db>(db: &'db dyn WorkspaceDataBase, file: File) -> Arc<Vec<ConfigDecl<'db>>> {
    Arc::clone(&semantic_index(db, file).configs)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Helper to iterate over all workspace + stdlib files.
fn all_files<'db>(db: &'db dyn WorkspaceDataBase) -> impl Iterator<Item = File> + 'db {
    db.get_files()
        .iter()
        .map(|e| *e)
        .chain(db.get_std_lib_files().iter().map(|e| *e))
}

// ---------------------------------------------------------------------------
// Public lookup functions
// ---------------------------------------------------------------------------

/// Returns all namespace declarations matching a given path across all files.
#[tracing::instrument(skip(db))]
pub fn namespace_index<'db>(
    db: &'db dyn WorkspaceDataBase,
    path: NamespacePath,
) -> Vec<NamespaceDecl<'db>> {
    let mut result = vec![];
    for file in all_files(db) {
        for ns in file_namespaces(db, file).iter() {
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
        for p in file_global_pous(db, file).iter() {
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
        for p in file_programs(db, file).iter() {
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
        for c in file_configs(db, file).iter() {
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
        for config in file_configs(db, file).iter() {
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

// ---------------------------------------------------------------------------
// Test discovery
// ---------------------------------------------------------------------------

/// A discovered test item with its qualified name.
#[derive(Debug, Clone)]
pub enum TestItem<'db> {
    Function(Function<'db>, String),
    Program(ProgramDecl<'db>, String),
}

impl<'db> TestItem<'db> {
    pub fn qualified_name(&self) -> &str {
        match self {
            TestItem::Function(_, name) => name,
            TestItem::Program(_, name) => name,
        }
    }
}

/// Discover all {test}-annotated POUs and programs across the workspace.
///
/// Returns a list of test items with their fully-qualified names
/// (e.g. `"test_abs"` for global, `"Std.Math.test_sqrt"` for namespaced).
pub fn discover_all_tests<'db>(db: &'db dyn WorkspaceDataBase) -> Vec<TestItem<'db>> {
    let mut tests = vec![];

    for file in all_files(db) {
        // Global test functions
        for pou in file_global_pous(db, file).iter() {
            if let Pou::Function(f) = pou
                && f.is_test(db) {
                    tests.push(TestItem::Function(*f, f.name(db).text(db).to_string()));
                }
        }

        // Global test programs
        for prog in file_programs(db, file).iter() {
            if prog.is_test(db) {
                tests.push(TestItem::Program(*prog, prog.name(db).text(db).to_string()));
            }
        }

        // Namespaced test functions
        for ns in file_namespaces(db, file).iter() {
            discover_tests_in_namespace(db, *ns, &mut tests);
        }
    }

    tests
}

fn discover_tests_in_namespace<'db>(
    db: &'db dyn WorkspaceDataBase,
    ns: NamespaceDecl<'db>,
    tests: &mut Vec<TestItem<'db>>,
) {
    let ns_prefix = ns.path(db).to_string(db);

    for pou in ns.pous(db).iter() {
        if let Pou::Function(f) = pou
            && f.is_test(db) {
                tests.push(TestItem::Function(
                    *f,
                    format!("{}.{}", ns_prefix, f.name(db).text(db)),
                ));
            }
    }

    for child_ns in ns.namespaces(db).iter() {
        discover_tests_in_namespace(db, *child_ns, tests);
    }
}

/// Find a specific test by its qualified name (e.g. `"Std.Math.test_sqrt"` or `"test_abs"`).
///
/// Returns `Some` if the name resolves to a {test}-annotated POU or program, `None` otherwise.
pub fn find_test<'db>(
    db: &'db dyn WorkspaceDataBase,
    qualified_name: &str,
) -> Option<TestItem<'db>> {
    let parts: Vec<&str> = qualified_name.split('.').collect();

    if parts.len() == 1 {
        // Global scope: check functions then programs
        let name = Ident::from_slice(db, parts[0]);
        if let Some(Pou::Function(f)) = pou_index(db, name)
            && f.is_test(db) {
                return Some(TestItem::Function(f, qualified_name.to_string()));
            }
        if let Some(prog) = program_index(db, name)
            && prog.is_test(db) {
                return Some(TestItem::Program(prog, qualified_name.to_string()));
            }
        None
    } else {
        // Namespaced: split into namespace path + item name
        let ns_parts = &parts[..parts.len() - 1];
        let item_name = parts[parts.len() - 1];

        let ns_idents: Vec<Ident> = ns_parts.iter().map(|s| Ident::from_slice(db, s)).collect();
        let ns_path = NamespacePath::new(db, ns_idents);
        let name = Ident::from_slice(db, item_name);

        if let Some(Pou::Function(f)) = namespace_pou_index(db, ns_path, name)
            && f.is_test(db) {
                return Some(TestItem::Function(f, qualified_name.to_string()));
            }
        None
    }
}
