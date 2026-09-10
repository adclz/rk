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

use rustc_hash::FxHashMap;
use std::sync::Arc;

use auto_lsp::default::db::file::File;
use db::WorkspaceDataBase;

use crate::{
    HasName,
    hir_def::{
        config::ConfigDecl,
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

/// Helper to iterate over all workspace + library files.
fn all_files<'db>(db: &'db dyn WorkspaceDataBase) -> impl Iterator<Item = File> + 'db {
    db.get_files()
        .iter()
        .map(|e| *e)
        .chain(db.get_library_files().iter().map(|e| *e))
}

/// The workspace's OWN files, without the library's.
///
/// A library is code, not a PLC. Its POUs exist to be used and must resolve
/// from anywhere, but a CONFIGURATION it declares describes the machine its
/// author was building, not this one. Counted as the workspace's, it spent the
/// single configuration a workspace may have before the user had written a
/// line, and E0242 then refused them their own.
fn workspace_files<'db>(db: &'db dyn WorkspaceDataBase) -> impl Iterator<Item = File> + 'db {
    db.get_files().iter().map(|e| *e)
}

// ---------------------------------------------------------------------------
// Public lookup functions
// ---------------------------------------------------------------------------

/// A file's namespace declarations keyed by case-folded path, built with
/// the file's semantic index. Per file on purpose: the file set is read
/// outside salsa, so a workspace-wide map would not learn about a file added
/// after it was built, while this one invalidates with the file it describes
/// and, like [`file_namespaces`], backdates when the file changed elsewhere.
#[salsa::tracked(returns(ref))]
pub fn file_namespace_map<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: File,
) -> Arc<FxHashMap<NamespacePath, Vec<NamespaceDecl<'db>>>> {
    Arc::clone(&semantic_index(db, file).namespace_map)
}

/// Returns all namespace declarations matching a given path across all files.
pub fn namespace_index<'db>(
    db: &'db dyn WorkspaceDataBase,
    path: NamespacePath,
) -> Vec<NamespaceDecl<'db>> {
    // Folded once; every file is then a hash probe.
    let key = path.caseless(db);
    let mut result = vec![];
    for file in all_files(db) {
        if let Some(found) = file_namespace_map(db, file).get(&key) {
            result.extend(found.iter().copied());
        }
    }
    result
}

/// A written namespace path, made absolute from where it was written: the
/// enclosing namespaces are tried innermost first (`Impl` inside `NAMESPACE
/// Lib` means `Lib.Impl`), then the path as written. The first spelling any
/// declaration answers to wins; a path nobody declares comes back as
/// written, so the caller's "not found" report names what the user typed.
pub fn absolute_namespace_path<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: crate::hir_def::scope::ScopeId<'db>,
    written: NamespacePath,
) -> NamespacePath {
    namespace_path_candidates(db, scope, written)
        .find(|candidate| !namespace_index(db, *candidate).is_empty())
        .unwrap_or(written)
}

/// Every spelling a written path may mean from `scope`, in resolution order:
/// under the innermost enclosing namespace first, outward, then as written.
/// Lazy, so [`absolute_namespace_path`] interns nothing past its first hit;
/// an IDE feature completing a partial path walks the whole sequence, because
/// a prefix nobody declares yet still names where the user is typing.
pub fn namespace_path_candidates<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: crate::hir_def::scope::ScopeId<'db>,
    written: NamespacePath,
) -> impl Iterator<Item = NamespacePath> + 'db {
    let enclosing = crate::hir_ty::resolver::name::enclosing_namespace_path(db, scope)
        .map(|p| p.fragments(db).clone())
        .unwrap_or_default();
    (1..=enclosing.len())
        .rev()
        .map(move |depth| {
            let mut candidate = enclosing[..depth].to_vec();
            candidate.extend(written.fragments(db).iter().copied());
            NamespacePath::new(db, candidate)
        })
        .chain(std::iter::once(written))
}

/// Returns the canonical POU for a given name within a namespace path.
#[tracing::instrument(level = "trace", skip(db))]
pub fn namespace_pou_index<'db>(
    db: &'db dyn WorkspaceDataBase,
    path: NamespacePath,
    name: Ident,
) -> Option<Pou<'db>> {
    for ns in namespace_index(db, path) {
        if let Some(pou) = ns.scope_id(db).def_map(db).local_pous.get(&name.caseless(db)) {
            return Some(*pou);
        }
    }
    None
}

/// Finds a globally declared POU by name across all files.
#[tracing::instrument(level = "trace", skip(db))]
pub fn pou_index<'db>(db: &'db dyn WorkspaceDataBase, name: Ident) -> Option<Pou<'db>> {
    for file in all_files(db) {
        for p in file_global_pous(db, file).iter() {
            if p.get_name_ident(db).caseless(db) == name.caseless(db) {
                return Some(*p);
            }
        }
    }
    None
}

/// Returns EVERY globally-declared POU with the given name, in a stable
/// discovery order. Where [`pou_index`] returns the first match, this surfaces
/// all same-name POUs so overload-aware callers (call resolution, the duplicate
/// check) can pick among FUNCTION overloads by signature. A non-overloaded name
/// yields a single-element vec.
#[tracing::instrument(level = "trace", skip(db))]
pub fn pou_candidates<'db>(db: &'db dyn WorkspaceDataBase, name: Ident) -> Vec<Pou<'db>> {
    let mut result = Vec::new();
    for file in all_files(db) {
        for p in file_global_pous(db, file).iter() {
            if p.get_name_ident(db).caseless(db) == name.caseless(db) {
                result.push(*p);
            }
        }
    }
    result
}

/// Namespace-scoped counterpart of [`pou_candidates`]: every POU with the given
/// name declared directly in the namespace `path` (across files that reopen it).
#[tracing::instrument(level = "trace", skip(db))]
pub fn namespace_pou_candidates<'db>(
    db: &'db dyn WorkspaceDataBase,
    path: NamespacePath,
    name: Ident,
) -> Vec<Pou<'db>> {
    let mut result = Vec::new();
    for ns in namespace_index(db, path) {
        for p in ns.pous(db).iter() {
            if p.get_name_ident(db).caseless(db) == name.caseless(db) {
                result.push(*p);
            }
        }
    }
    result
}

/// Finds a globally declared program by name across all files.
#[tracing::instrument(level = "trace", skip(db))]
pub fn program_index<'db>(db: &'db dyn WorkspaceDataBase, name: Ident) -> Option<ProgramDecl<'db>> {
    for file in all_files(db) {
        for p in file_programs(db, file).iter() {
            if p.get_name_ident(db).caseless(db) == name.caseless(db) {
                return Some(*p);
            }
        }
    }
    None
}

/// Every CONFIGURATION the workspace declares — for reporting that it
/// declares more than one (E0242), not for choosing among them.
///
/// The WORKSPACE's, not the library's: see [`workspace_files`].
///
/// A workspace has one configuration; this is how the check sees that it has
/// two, and where they are. Order is whatever the file maps yield, so it can
/// answer "how many" and "which ones" and never "which one".
pub fn declared_configs<'db>(db: &'db dyn WorkspaceDataBase) -> Vec<ConfigDecl<'db>> {
    let mut out = Vec::new();
    for file in workspace_files(db) {
        out.extend(file_configs(db, file).iter().copied());
    }
    out
}

/// Every block of the named CONFIGURATION, across the workspace's files.
///
/// The library's are not fragments of this workspace's configuration even when
/// they share its name: see [`workspace_files`].
///
/// Same-named blocks are FRAGMENTS of one configuration — the GVL model: a
/// file of VAR_GLOBALs here, the resources there. Order is whatever the file
/// maps yield; callers that report or lower must order fragments themselves.
#[tracing::instrument(level = "trace", skip(db), ret)]
pub fn config_fragments<'db>(db: &'db dyn WorkspaceDataBase, name: Ident) -> Vec<ConfigDecl<'db>> {
    let mut out = Vec::new();
    for file in workspace_files(db) {
        out.extend(
            file_configs(db, file)
                .iter()
                .filter(|c| c.get_name_ident(db).caseless(db) == name.caseless(db))
                .copied(),
        );
    }
    out
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
                if v.get_name_ident(db).caseless(db) == var_name.caseless(db) {
                    return Some(*v);
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
}

impl<'db> TestItem<'db> {
    pub fn qualified_name(&self) -> &str {
        match self {
            TestItem::Function(_, name) => name,
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
                && crate::hir_def::pous::pragma::is_test(db, f.pragmas(db))
            {
                tests.push(TestItem::Function(*f, f.name(db).text(db).to_string()));
            }
        }


        // Namespaced test functions. The namespace list is FLAT (nested
        // included), so each namespace contributes its own tests exactly
        // once; recursing into children here discovered every nested test
        // twice.
        for ns in file_namespaces(db, file).iter() {
            let ns_prefix = ns.path(db).to_string(db);
            for pou in ns.pous(db).iter() {
                if let Pou::Function(f) = pou
                    && crate::hir_def::pous::pragma::is_test(db, f.pragmas(db))
                {
                    tests.push(TestItem::Function(
                        *f,
                        format!("{}.{}", ns_prefix, f.name(db).text(db)),
                    ));
                }
            }
        }
    }

    tests
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
        // Global scope: a test is a FUNCTION (E0252 refuses the pragma
        // anywhere else).
        let name = Ident::from_slice(db, parts[0]);
        if let Some(Pou::Function(f)) = pou_index(db, name)
            && crate::hir_def::pous::pragma::is_test(db, f.pragmas(db))
        {
            return Some(TestItem::Function(f, qualified_name.to_string()));
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
            && crate::hir_def::pous::pragma::is_test(db, f.pragmas(db))
        {
            return Some(TestItem::Function(f, qualified_name.to_string()));
        }
        None
    }
}
