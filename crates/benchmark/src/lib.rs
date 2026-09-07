//! Benchmark helpers for the IEC 61131-3 compiler.
//!
//! The fixture is a real corpus tracked in this repository: `stdlib/` (the
//! standard library — 11 files, ~6.5k lines, checks clean and compiles to
//! WASM). Every benchmark asserts its diagnostic count against the baselines
//! below, so a benchmark can never silently drift into measuring an empty or
//! error-flooded analysis. `cargo test -p rk-benchmark` verifies the
//! baselines without running the benchmarks.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use auto_lsp::{
    core::document::Document,
    default::db::{FileManager, file::File},
    lsp_types::Url,
    salsa::Setter,
};
use db::RootDatabase;
use hir::{
    HirNodeInfo,
    check::diagnostics_for_file,
    hir_def::{namespace::NamespaceDecl, scope::ScopeId, semantic_index::semantic_index},
    hir_ty::{
        body::infer_body,
        head::{init_inference::infer_initialization, signature::infer_signature},
    },
};

// ---------------------------------------------------------------------------
// Diagnostic baselines
//
// The expected number of HIR diagnostics (`diagnostics_for_file`) and lint
// diagnostics per corpus. When a corpus source changes legitimately, update
// the constant — the `corpus_baselines` test states the fresh value.
// ---------------------------------------------------------------------------

pub const STDLIB_EXPECTED_DIAGNOSTICS: usize = 0;
pub const STDLIB_EXPECTED_LINTS: usize = 329;

// ---------------------------------------------------------------------------
// Corpora
// ---------------------------------------------------------------------------

/// A named set of real source files loaded from the repository.
pub struct Corpus {
    pub name: &'static str,
    /// (workspace-relative path, source text), sorted by path.
    pub files: Vec<(String, String)>,
}

impl Corpus {
    pub fn total_lines(&self) -> usize {
        self.files.iter().map(|(_, s)| s.lines().count()).sum()
    }
}

impl std::fmt::Display for Corpus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name)
    }
}

fn workspace_root() -> &'static Path {
    // crates/benchmark → crates → workspace root
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
}

fn collect_st_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("corpus directory missing") {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_st_files(&path, out);
        } else if db::loader::is_st_file(&path) {
            out.push(path);
        }
    }
}

fn load_dirs(name: &'static str, dirs: &[&str]) -> Corpus {
    let root = workspace_root();
    let mut paths = Vec::new();
    for dir in dirs {
        collect_st_files(&root.join(dir), &mut paths);
    }
    paths.sort();
    let files = paths
        .into_iter()
        .map(|p| {
            // `/`-separated: the key is matched against `Edit::file`
            // constants written in source, so it cannot carry the host's
            // separator.
            let rel = p
                .strip_prefix(root)
                .unwrap()
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            // A Windows checkout smudges the corpus to CRLF, and the edit
            // needles are `\n`-joined; the measurement must not depend on
            // what git did to the working tree.
            let source = std::fs::read_to_string(&p).unwrap().replace("\r\n", "\n");
            (rel, source)
        })
        .collect();
    Corpus { name, files }
}

/// The standard library: 13 files, ~9.5k lines, checks clean.
pub fn stdlib_corpus() -> Corpus {
    load_dirs("stdlib", &["stdlib"])
}

pub fn load_corpus(name: &str) -> Corpus {
    match name {
        "stdlib" => stdlib_corpus(),
        other => panic!("unknown corpus {other}"),
    }
}

pub fn expected_diagnostics(corpus: &str) -> usize {
    match corpus {
        "stdlib" => STDLIB_EXPECTED_DIAGNOSTICS,
        other => panic!("unknown corpus {other}"),
    }
}

pub fn expected_lints(corpus: &str) -> usize {
    match corpus {
        "stdlib" => STDLIB_EXPECTED_LINTS,
        other => panic!("unknown corpus {other}"),
    }
}

// ---------------------------------------------------------------------------
// Database setup
// ---------------------------------------------------------------------------

/// Create a fresh database and parse every corpus file into it.
pub fn setup_db(corpus: &Corpus) -> (RootDatabase, Vec<File>) {
    let mut db = RootDatabase::default();
    let mut files = Vec::with_capacity(corpus.files.len());

    for (rel, source) in &corpus.files {
        let url = Url::parse(&format!("file:///{rel}")).unwrap();

        let file = File::from_string()
            .db(&db)
            .parsers(&ast::RK_PARSER)
            .url(&url)
            .source(source.clone())
            .call()
            .unwrap();

        db.add_file(file).unwrap();
        files.push(file);
    }

    (db, files)
}

/// Update the source of a file in-place via `set_document`.
///
/// This preserves the Salsa `File` identity so that downstream query caches
/// are *invalidated* rather than orphaned — enabling true incremental
/// re-computation. The reparse is intentionally part of this function: an
/// editor pays it on every keystroke, so incremental benchmarks time it.
pub fn edit_file(db: &mut RootDatabase, file: File, new_source: &str) {
    let parsers = file.parsers(db);
    let tree = parsers
        .parser
        .write()
        .parse(new_source.as_bytes(), None)
        .expect("tree-sitter parse failed");
    let document = Document::new(new_source.to_string(), tree, None);
    file.set_document(db).to(Arc::new(document));
}

// ---------------------------------------------------------------------------
// Pipeline stages
//
// Each helper runs exactly one stage for every file. Benchmarks isolate a
// stage by priming all *earlier* stages in their (untimed) setup, so the
// timed region contains only the stage under measurement plus whatever that
// stage intrinsically demands (e.g. name resolution during inference).
// ---------------------------------------------------------------------------

/// Every POU scope in a file, including POUs nested in namespaces — the
/// corpora are almost entirely NAMESPACE-wrapped, so forgetting recursion
/// here would leave the stage benchmarks measuring an empty scope list.
pub fn all_pou_scopes<'db>(db: &'db dyn db::WorkspaceDataBase, file: File) -> Vec<ScopeId<'db>> {
    fn collect_ns<'db>(
        db: &'db dyn db::WorkspaceDataBase,
        ns: &'db NamespaceDecl<'db>,
        scopes: &mut Vec<ScopeId<'db>>,
    ) {
        for pou in ns.pous(db).iter() {
            scopes.push(pou.get_scope_id(db));
        }
        for child in ns.namespaces(db).iter() {
            collect_ns(db, child, scopes);
        }
    }

    let sema = semantic_index(db, file);
    let mut scopes = Vec::new();

    for pou in sema.global_pous.iter() {
        scopes.push(pou.get_scope_id(db));
    }

    for program in sema.programs.iter() {
        scopes.push(program.get_scope_id(db));
    }

    for ns in sema.namespaces.iter() {
        collect_ns(db, ns, &mut scopes);
    }

    scopes
}

pub fn index_all(db: &dyn db::WorkspaceDataBase, files: &[File]) {
    for file in files {
        let _ = semantic_index(db, *file);
    }
}

pub fn signatures_all(db: &dyn db::WorkspaceDataBase, files: &[File]) {
    for file in files {
        for scope in all_pou_scopes(db, *file) {
            let _ = infer_signature(db, scope);
        }
    }
}

pub fn initializations_all(db: &dyn db::WorkspaceDataBase, files: &[File]) {
    for file in files {
        for scope in all_pou_scopes(db, *file) {
            let _ = infer_initialization(db, scope);
        }
    }
}

pub fn bodies_all(db: &dyn db::WorkspaceDataBase, files: &[File]) {
    for file in files {
        for scope in all_pou_scopes(db, *file) {
            let _ = infer_body(db, scope);
        }
    }
}

/// Full HIR diagnostics for the whole workspace; returns the total count.
pub fn check_all(db: &dyn db::WorkspaceDataBase, files: &[File]) -> usize {
    files
        .iter()
        .map(|file| diagnostics_for_file(db, *file).len())
        .sum()
}

/// A linter configuration with every rule enabled.
pub fn all_rules_config() -> db::config_file::LinterConfig {
    let rules = linter::rules::ALL_RULE_NAMES
        .iter()
        .map(|name| (name.to_string(), true))
        .collect();
    db::config_file::LinterConfig {
        select: Some(db::config_file::Select::All),
        rules: Some(rules),
    }
}

/// Run every lint rule over the whole workspace; returns the total count.
pub fn lint_all(
    db: &RootDatabase,
    files: &[File],
    config: &db::config_file::LinterConfig,
) -> usize {
    let mut total = 0;
    for file in files {
        let mut lints = Vec::new();
        linter::lint_file(db, *file, config, &mut lints);
        total += lints.len();
    }
    total
}

// ---------------------------------------------------------------------------
// Incremental edit scenarios
//
// Each edit targets `stdlib/Edge.st`: `R_TRIG` is used from `Counters.st`,
// so the workspace re-check after an edit exercises the cross-file
// invalidation story — the thing incrementality exists for. Needles must
// match exactly once, so corpus drift breaks the benchmark loudly instead
// of quietly changing what it measures.
// ---------------------------------------------------------------------------

pub struct Edit {
    pub name: &'static str,
    /// Workspace-relative path of the file the edit applies to.
    pub file: &'static str,
    needle: &'static str,
    replacement: &'static str,
}

impl Edit {
    /// The edited full source text, and the corpus index of the edited file.
    pub fn apply(&self, corpus: &Corpus) -> (usize, String) {
        let idx = corpus
            .files
            .iter()
            .position(|(rel, _)| rel == self.file)
            .unwrap_or_else(|| panic!("{} not in corpus {}", self.file, corpus.name));
        let source = &corpus.files[idx].1;
        assert_eq!(
            source.matches(self.needle).count(),
            1,
            "edit needle for `{}` must match exactly once in {}",
            self.name,
            self.file
        );
        (idx, source.replacen(self.needle, self.replacement, 1))
    }
}

impl std::fmt::Display for Edit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name)
    }
}

pub static EDITS: &[Edit] = &[
    // A statement inside R_TRIG's body changes. Signatures are untouched, so
    // dependent files must NOT re-infer — this is the cheap common case.
    Edit {
        name: "body_edit",
        file: "stdlib/Edge.st",
        needle: "Q := CLK AND NOT M;",
        replacement: "Q := NOT M AND CLK;",
    },
    // A new POU appears in the namespace. The global name indexes rebuild
    // and resolution re-validates — the expensive structural case.
    Edit {
        name: "add_pou",
        file: "stdlib/Edge.st",
        needle: "END_NAMESPACE\nEND_NAMESPACE",
        replacement: "END_NAMESPACE\n\n\tFUNCTION_BLOCK BENCH_PROBE\n\t\tVAR_INPUT\n\t\t\tCLK: BOOL;\n\t\tEND_VAR\n\t\tVAR_OUTPUT\n\t\t\tQ: BOOL;\n\t\tEND_VAR\n\t\tQ := CLK;\n\tEND_FUNCTION_BLOCK\nEND_NAMESPACE",
    },
    // Only a comment changes: every span below it shifts but no semantics
    // do. `semantic_index` is `no_eq`, so the edited file itself is always
    // fully re-analyzed — this pins the per-edit floor (reparse + one file's
    // re-analysis). `body_edit` sitting AT this floor is the proof that
    // dependent files reuse their caches; `add_pou` sits above it.
    Edit {
        name: "comment_edit",
        file: "stdlib/Edge.st",
        needle: "# Standard Edge Detection Function Blocks",
        replacement: "# Standard Edge Detection Function Blocks (edited)",
    },
];

// ---------------------------------------------------------------------------
// Baseline guard — runs under `cargo nextest`, not only when benchmarking.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpus_baselines() {
        for name in ["stdlib"] {
            let corpus = load_corpus(name);
            let (db, files) = setup_db(&corpus);
            assert_eq!(
                check_all(&db, &files),
                expected_diagnostics(name),
                "HIR diagnostic baseline drifted for corpus `{name}` — update the constant"
            );
            assert_eq!(
                lint_all(&db, &files, &all_rules_config()),
                expected_lints(name),
                "lint baseline drifted for corpus `{name}` — update the constant"
            );
            for file in &files {
                formatter::format(&db, *file).expect("corpus file must format");
            }
            let scopes: usize = files.iter().map(|f| all_pou_scopes(&db, *f).len()).sum();
            assert!(
                scopes > 100,
                "stage helpers see only {scopes} POU scopes in `{name}` — namespace recursion broken?"
            );
        }

        // The codegen benchmarks require the stdlib to lower end-to-end.
        let corpus = stdlib_corpus();
        let (db, files) = setup_db(&corpus);
        let indices: Vec<_> = files
            .iter()
            .map(|f| hir::hir_def::semantic_index::semantic_index(&db, *f))
            .collect();
        let module = mir::lower::lower_module::lower_modules(&db, &indices)
            .expect("stdlib must lower to MIR");
        let wasm = wasm_codegen::generate_wasm(&db, &module).finish();
        assert!(!wasm.is_empty());
    }

    #[test]
    fn edits_apply_and_preserve_diagnostics() {
        let corpus = stdlib_corpus();
        for edit in EDITS {
            let (idx, edited) = edit.apply(&corpus);
            let (mut db, files) = setup_db(&corpus);
            assert_eq!(check_all(&db, &files), STDLIB_EXPECTED_DIAGNOSTICS);
            edit_file(&mut db, files[idx], &edited);
            assert_eq!(
                check_all(&db, &files),
                STDLIB_EXPECTED_DIAGNOSTICS,
                "edit `{}` must not change the diagnostic count",
                edit.name
            );
        }
    }
}
