//! Benchmark helpers for the IEC 61131-3 compiler.
//!
//! Provides [`TestCase`] definitions and database setup functions
//! used by the divan benchmark harnesses in `benches/`.

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
    hir_def::{scope::ScopeId, semantic_index::semantic_index},
    hir_ty::{body::infer_body, head::signature::infer_signature},
};

// ---------------------------------------------------------------------------
// Test cases
// ---------------------------------------------------------------------------

/// A named source fixture for benchmarking.
pub struct TestCase {
    name: &'static str,
    /// Source files (one or more .st sources).
    sources: &'static [&'static str],
    /// An edited variant of the first source file for incremental benchmarks.
    /// Typically a body-only change so that signatures remain cached.
    edited_source: &'static str,
}

impl TestCase {
    pub const fn new(
        name: &'static str,
        sources: &'static [&'static str],
        edited_source: &'static str,
    ) -> Self {
        Self {
            name,
            sources,
            edited_source,
        }
    }

    pub fn sources(&self) -> &[&str] {
        self.sources
    }

    pub fn edited_source(&self) -> &str {
        self.edited_source
    }
}

impl std::fmt::Display for TestCase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name)
    }
}

// ---------------------------------------------------------------------------
// Starter fixtures — intentionally small; real samples added over time.
// ---------------------------------------------------------------------------

pub static CASES: &[TestCase] = &[
    TestCase::new(
        "minimal",
        &[r#"
FUNCTION add : INT
VAR_INPUT
    a : INT;
    b : INT;
END_VAR
    add := a + b;
END_FUNCTION
"#],
        // edited: body change only (add a local variable + assignment)
        r#"
FUNCTION add : INT
VAR_INPUT
    a : INT;
    b : INT;
END_VAR
VAR
    tmp : INT;
END_VAR
    tmp := a + b;
    add := tmp;
END_FUNCTION
"#,
    ),
    TestCase::new(
        "medium_fb",
        &[r#"
FUNCTION_BLOCK Motor
VAR_INPUT
    start : BOOL;
    stop  : BOOL;
    speed_ref : REAL;
END_VAR
VAR_OUTPUT
    running : BOOL;
    actual_speed : REAL;
END_VAR
VAR
    ramp : REAL;
    fault : BOOL;
    timer_count : INT;
    max_speed : REAL := 1500.0;
    accel_rate : REAL := 10.0;
END_VAR
    IF stop THEN
        running := FALSE;
        ramp := 0.0;
    ELSIF start AND NOT fault THEN
        running := TRUE;
        IF ramp < speed_ref THEN
            ramp := ramp + accel_rate;
            IF ramp > max_speed THEN
                ramp := max_speed;
            END_IF;
        END_IF;
    END_IF;

    CASE timer_count OF
        0: timer_count := 1;
        1: timer_count := 2;
        2: timer_count := 0;
    END_CASE;

    actual_speed := ramp;
END_FUNCTION_BLOCK
"#],
        // edited: body change — add a line
        r#"
FUNCTION_BLOCK Motor
VAR_INPUT
    start : BOOL;
    stop  : BOOL;
    speed_ref : REAL;
END_VAR
VAR_OUTPUT
    running : BOOL;
    actual_speed : REAL;
END_VAR
VAR
    ramp : REAL;
    fault : BOOL;
    timer_count : INT;
    max_speed : REAL := 1500.0;
    accel_rate : REAL := 10.0;
END_VAR
    IF stop THEN
        running := FALSE;
        ramp := 0.0;
        fault := FALSE;
    ELSIF start AND NOT fault THEN
        running := TRUE;
        IF ramp < speed_ref THEN
            ramp := ramp + accel_rate;
            IF ramp > max_speed THEN
                ramp := max_speed;
            END_IF;
        END_IF;
    END_IF;

    CASE timer_count OF
        0: timer_count := 1;
        1: timer_count := 2;
        2: timer_count := 0;
    END_CASE;

    actual_speed := ramp;
END_FUNCTION_BLOCK
"#,
    ),
    TestCase::new(
        "multi_file",
        &[
            // File 0: function block
            r#"
FUNCTION_BLOCK Counter
VAR_INPUT
    reset : BOOL;
    enable : BOOL;
END_VAR
VAR_OUTPUT
    count : INT;
END_VAR
    IF reset THEN
        count := 0;
    ELSIF enable THEN
        count := count + 1;
    END_IF;
END_FUNCTION_BLOCK
"#,
            // File 1: program that uses the FB
            r#"
PROGRAM Main
VAR
    cnt : Counter;
    result : INT;
    do_reset : BOOL;
    do_enable : BOOL;
END_VAR
    cnt(reset := do_reset, enable := do_enable);
    result := cnt.count;
END_PROGRAM
"#,
        ],
        // edited: body change in file 0
        r#"
FUNCTION_BLOCK Counter
VAR_INPUT
    reset : BOOL;
    enable : BOOL;
END_VAR
VAR_OUTPUT
    count : INT;
END_VAR
    IF reset THEN
        count := 0;
    ELSIF enable THEN
        count := count + 2;
    END_IF;
END_FUNCTION_BLOCK
"#,
    ),
];

// ---------------------------------------------------------------------------
// Database setup helpers
// ---------------------------------------------------------------------------

/// Create a fresh database, parse sources, and return the DB + file handles.
pub fn setup_db(sources: &[&str]) -> (RootDatabase, Vec<File>) {
    let mut db = RootDatabase::default();
    let mut files = Vec::with_capacity(sources.len());

    for (i, source) in sources.iter().enumerate() {
        let url = Url::parse(&format!("file:///bench{i}.st")).unwrap();

        let file = File::from_string()
            .db(&db)
            .parsers(&ast::RK_PARSER)
            .url(&url)
            .source(source.to_string())
            .call()
            .unwrap();

        db.add_file(file).unwrap();
        files.push(file);
    }

    (db, files)
}

/// Update the source of a file in-place via `set_document`.
///
/// This preserves the Salsa `File` identity so that downstream query
/// caches are properly *invalidated* rather than orphaned — enabling
/// true incremental re-computation.
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

/// Run full diagnostics for a file (triggers the entire HIR pipeline).
pub fn collect_diagnostics(db: &dyn db::WorkspaceDataBase, file: File) -> usize {
    diagnostics_for_file(db, file).len()
}

/// Collect all POU scope IDs from a file's semantic index.
pub fn all_pou_scopes<'db>(db: &'db dyn db::WorkspaceDataBase, file: File) -> Vec<ScopeId<'db>> {
    let sema = semantic_index(db, file);
    let mut scopes = Vec::new();

    for pou in sema.global_pous.iter() {
        scopes.push(pou.get_scope_id(db));
    }

    for program in sema.programs.iter() {
        scopes.push(program.get_scope_id(db));
    }

    scopes
}

/// Run `semantic_index` for a file.
pub fn bench_semantic_index(db: &dyn db::WorkspaceDataBase, file: File) {
    let _ = semantic_index(db, file);
}

/// Run `infer_signature` for all POU scopes in a file.
pub fn bench_infer_signature(db: &dyn db::WorkspaceDataBase, file: File) {
    for scope in all_pou_scopes(db, file) {
        let _ = infer_signature(db, scope);
    }
}

/// Run `infer_body` for all POU scopes in a file.
pub fn bench_infer_body(db: &dyn db::WorkspaceDataBase, file: File) {
    for scope in all_pou_scopes(db, file) {
        let _ = infer_body(db, scope);
    }
}
