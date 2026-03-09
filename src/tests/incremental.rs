use std::sync::Arc;
use std::sync::RwLock;

use auto_lsp::core::document::Document;
use auto_lsp::default::db::BaseDatabase;
use auto_lsp::default::db::file::File;
use auto_lsp::salsa::{self, Event, EventKind, Setter};
use db::RootDatabase;
use hir::check::diagnostics_for_file;
use rstest::rstest;

use crate::tests::utils::{add_sources, with_log_db};

/// Edit a file's source in-place, preserving salsa File identity.
fn edit_file(db: &mut RootDatabase, file: File, new_source: &str) {
    let parsers = file.parsers(db);
    let tree = parsers
        .parser
        .write()
        .parse(new_source.as_bytes(), None)
        .expect("tree-sitter parse failed");
    let document = Document::new(new_source.to_string(), tree, None);
    file.set_document(db).to(Arc::new(document));
}

/// Collect all `WillExecute` query names from the event log.
/// Must be called within `salsa::attach` for readable query names.
fn executed_queries(log: &[Event]) -> Vec<String> {
    log.iter()
        .filter_map(|e| {
            if let EventKind::WillExecute { database_key } = &e.kind {
                Some(format!("{:?}", database_key))
            } else {
                None
            }
        })
        .collect()
}

/// Count how many times a query matching `substr` was executed.
fn count_executions(queries: &[String], substr: &str) -> usize {
    queries.iter().filter(|q| q.contains(substr)).count()
}

// ---------------------------------------------------------------------------
// Basic: body edit only re-analyzes the changed file
// ---------------------------------------------------------------------------

#[rstest]
fn body_edit_only_reruns_semantic_index_for_changed_file(
    with_log_db: (RootDatabase, Arc<RwLock<Vec<Event>>>),
) {
    let (mut db, log) = with_log_db;

    let source0 = r#"
        FUNCTION_BLOCK Counter
        VAR
            count : INT;
        END_VAR
            count := count + 1;
        END_FUNCTION_BLOCK
    "#;

    let source1 = r#"
        FUNCTION main : INT
        VAR
            c : Counter;
        END_VAR
            c();
            main := 0;
        END_FUNCTION
    "#;

    add_sources(&mut db, &[source0, source1]);

    // Prime all caches by running diagnostics on both files
    let files: Vec<File> = db.get_files().iter().map(|e| *e.value()).collect();
    for file in &files {
        diagnostics_for_file(&db, *file);
    }

    // Clear the log
    log.write().unwrap().clear();

    // Edit only file1's body (change return value)
    let file1 = files
        .iter()
        .find(|f| f.url(&db).as_str().contains("test1"))
        .unwrap();

    edit_file(
        &mut db,
        *file1,
        r#"
        FUNCTION main : INT
        VAR
            c : Counter;
        END_VAR
            c();
            main := 42;
        END_FUNCTION
    "#,
    );

    // Re-run diagnostics for BOTH files
    for file in &files {
        diagnostics_for_file(&db, *file);
    }

    salsa::attach(&db, || {
        let events = log.read().unwrap();
        let queries = executed_queries(&events);

        // semantic_index should only re-run once (for the edited file),
        // not twice (which would mean the untouched file was also re-analyzed)
        let sema_count = count_executions(&queries, "semantic_index");
        assert_eq!(
            sema_count, 1,
            "semantic_index should re-run for exactly 1 file (the edited one), \
             but ran {} times. Queries: {:?}",
            sema_count,
            queries
                .iter()
                .filter(|q| q.contains("semantic_index"))
                .collect::<Vec<_>>()
        );
    });
}

#[rstest]
fn body_edit_does_not_reinfer_signature_of_other_file(
    with_log_db: (RootDatabase, Arc<RwLock<Vec<Event>>>),
) {
    let (mut db, log) = with_log_db;

    let source0 = r#"
        FUNCTION_BLOCK Counter
        VAR
            count : INT;
        END_VAR
            count := count + 1;
        END_FUNCTION_BLOCK
    "#;

    let source1 = r#"
        FUNCTION main : INT
        VAR
            c : Counter;
        END_VAR
            c();
            main := 0;
        END_FUNCTION
    "#;

    add_sources(&mut db, &[source0, source1]);

    // Prime all caches
    let files: Vec<File> = db.get_files().iter().map(|e| *e.value()).collect();
    for file in &files {
        diagnostics_for_file(&db, *file);
    }

    // Clear the log
    log.write().unwrap().clear();

    // Edit only file1's body
    let file1 = files
        .iter()
        .find(|f| f.url(&db).as_str().contains("test1"))
        .unwrap();

    edit_file(
        &mut db,
        *file1,
        r#"
        FUNCTION main : INT
        VAR
            c : Counter;
        END_VAR
            c();
            main := 42;
        END_FUNCTION
    "#,
    );

    // Re-run diagnostics for BOTH files
    for file in &files {
        diagnostics_for_file(&db, *file);
    }

    salsa::attach(&db, || {
        let events = log.read().unwrap();
        let queries = executed_queries(&events);

        // infer_signature should NOT re-run for Counter (untouched file)
        let sig_executions: Vec<_> = queries
            .iter()
            .filter(|q| q.contains("infer_signature"))
            .collect();

        assert!(
            !sig_executions.iter().any(|q| q.contains("Counter")),
            "infer_signature should NOT re-run for Counter (untouched), got: {:?}",
            sig_executions
        );
    });
}

// ---------------------------------------------------------------------------
// Cross-file: editing an unrelated POU does not invalidate the dependent
// ---------------------------------------------------------------------------

/// file0 has Counter (used by file1) and Helper (independent).
/// Editing Helper's body should NOT cause Counter's signature or
/// file1's body inference to re-run.
#[rstest]
fn editing_unrelated_pou_does_not_invalidate_cross_file_dependent(
    with_log_db: (RootDatabase, Arc<RwLock<Vec<Event>>>),
) {
    let (mut db, log) = with_log_db;

    let source0 = r#"
        FUNCTION_BLOCK Counter
        VAR
            count : INT;
        END_VAR
            count := count + 1;
        END_FUNCTION_BLOCK

        FUNCTION helper : INT
        VAR
            x : INT;
        END_VAR
            helper := x + 1;
        END_FUNCTION
    "#;

    let source1 = r#"
        FUNCTION main : INT
        VAR
            c : Counter;
        END_VAR
            c();
            main := c.count;
        END_FUNCTION
    "#;

    add_sources(&mut db, &[source0, source1]);

    let files: Vec<File> = db.get_files().iter().map(|e| *e.value()).collect();
    for file in &files {
        diagnostics_for_file(&db, *file);
    }

    log.write().unwrap().clear();

    // Edit helper's body in file0 — Counter is untouched
    let file0 = files
        .iter()
        .find(|f| f.url(&db).as_str().contains("test0"))
        .unwrap();

    edit_file(
        &mut db,
        *file0,
        r#"
        FUNCTION_BLOCK Counter
        VAR
            count : INT;
        END_VAR
            count := count + 1;
        END_FUNCTION_BLOCK

        FUNCTION helper : INT
        VAR
            x : INT;
        END_VAR
            helper := x + 99;
        END_FUNCTION
    "#,
    );

    for file in &files {
        diagnostics_for_file(&db, *file);
    }

    salsa::attach(&db, || {
        let events = log.read().unwrap();
        let queries = executed_queries(&events);

        // semantic_index must re-run for file0 (edited), but should NOT for file1
        let sema_count = count_executions(&queries, "semantic_index");
        assert_eq!(
            sema_count, 1,
            "semantic_index should re-run only for file0, got {} executions: {:?}",
            sema_count,
            queries
                .iter()
                .filter(|q| q.contains("semantic_index"))
                .collect::<Vec<_>>()
        );

        // Counter's signature should NOT be re-inferred (it didn't change)
        let sig_executions: Vec<_> = queries
            .iter()
            .filter(|q| q.contains("infer_signature"))
            .collect();

        assert!(
            !sig_executions.iter().any(|q| q.contains("Counter")),
            "Counter's signature should be cached, got: {:?}",
            sig_executions
        );

        // main's body should NOT be re-inferred (its dependency Counter didn't change)
        let body_executions: Vec<_> = queries
            .iter()
            .filter(|q| q.contains("infer_body"))
            .collect();

        assert!(
            !body_executions.iter().any(|q| q.contains("main")),
            "main's body should be cached (Counter unchanged), got: {:?}",
            body_executions
        );
    });
}

// ---------------------------------------------------------------------------
// Cross-file: body-only edit in dependency does not re-infer dependent's body
// ---------------------------------------------------------------------------

/// file0 has Counter. file1's `main` uses Counter.
/// Editing Counter's BODY (not signature) should not cause main's
/// signature or body to be re-inferred.
#[rstest]
fn dependency_body_edit_does_not_reinfer_dependent(
    with_log_db: (RootDatabase, Arc<RwLock<Vec<Event>>>),
) {
    let (mut db, log) = with_log_db;

    let source0 = r#"
        FUNCTION_BLOCK Counter
        VAR
            count : INT;
        END_VAR
            count := count + 1;
        END_FUNCTION_BLOCK
    "#;

    let source1 = r#"
        FUNCTION main : INT
        VAR
            c : Counter;
        END_VAR
            c();
            main := c.count;
        END_FUNCTION
    "#;

    add_sources(&mut db, &[source0, source1]);

    let files: Vec<File> = db.get_files().iter().map(|e| *e.value()).collect();
    for file in &files {
        diagnostics_for_file(&db, *file);
    }

    log.write().unwrap().clear();

    // Edit Counter's body only (signature unchanged)
    let file0 = files
        .iter()
        .find(|f| f.url(&db).as_str().contains("test0"))
        .unwrap();

    edit_file(
        &mut db,
        *file0,
        r#"
        FUNCTION_BLOCK Counter
        VAR
            count : INT;
        END_VAR
            count := count + 5;
        END_FUNCTION_BLOCK
    "#,
    );

    for file in &files {
        diagnostics_for_file(&db, *file);
    }

    salsa::attach(&db, || {
        let events = log.read().unwrap();
        let queries = executed_queries(&events);

        // Counter's signature should NOT re-run (only body changed)
        let sig_executions: Vec<_> = queries
            .iter()
            .filter(|q| q.contains("infer_signature"))
            .collect();

        assert!(
            !sig_executions.iter().any(|q| q.contains("Counter")),
            "Counter's signature should be cached (only body changed), got: {:?}",
            sig_executions
        );

        // main should NOT have its body re-inferred
        let body_executions: Vec<_> = queries
            .iter()
            .filter(|q| q.contains("infer_body"))
            .collect();

        assert!(
            !body_executions.iter().any(|q| q.contains("main")),
            "main's body should be cached (Counter signature unchanged), got: {:?}",
            body_executions
        );
    });
}

// ---------------------------------------------------------------------------
// Cross-file: signature change DOES invalidate the dependent
// ---------------------------------------------------------------------------

/// file0 has Counter with `count: INT`. file1 uses `c.count`.
/// Changing Counter's variable type from INT to REAL should trigger
/// re-inference of main's body (because the type of `c.count` changed).
#[rstest]
fn signature_change_does_reinfer_dependent(
    with_log_db: (RootDatabase, Arc<RwLock<Vec<Event>>>),
) {
    let (mut db, log) = with_log_db;

    let source0 = r#"
        FUNCTION_BLOCK Counter
        VAR
            count : INT;
        END_VAR
            count := count + 1;
        END_FUNCTION_BLOCK
    "#;

    let source1 = r#"
        FUNCTION main : INT
        VAR
            c : Counter;
        END_VAR
            c();
            main := c.count;
        END_FUNCTION
    "#;

    add_sources(&mut db, &[source0, source1]);

    let files: Vec<File> = db.get_files().iter().map(|e| *e.value()).collect();
    for file in &files {
        diagnostics_for_file(&db, *file);
    }

    log.write().unwrap().clear();

    // Change Counter's signature: count: INT → count: REAL
    let file0 = files
        .iter()
        .find(|f| f.url(&db).as_str().contains("test0"))
        .unwrap();

    edit_file(
        &mut db,
        *file0,
        r#"
        FUNCTION_BLOCK Counter
        VAR
            count : REAL;
        END_VAR
            count := count + 1.0;
        END_FUNCTION_BLOCK
    "#,
    );

    for file in &files {
        diagnostics_for_file(&db, *file);
    }

    salsa::attach(&db, || {
        let events = log.read().unwrap();
        let queries = executed_queries(&events);

        // Both Counter's and main's signatures should re-run:
        // Counter because its variables changed, main because it depends on Counter's type
        let sig_count = count_executions(&queries, "infer_signature");
        assert!(
            sig_count >= 2,
            "infer_signature should re-run for at least 2 scopes (Counter changed, \
             main depends on it), but ran {} times: {:?}",
            sig_count,
            queries
                .iter()
                .filter(|q| q.contains("infer_signature"))
                .collect::<Vec<_>>()
        );

        // main's body SHOULD be re-inferred (type of c.count changed from INT to REAL)
        let body_count = count_executions(&queries, "infer_body");
        assert!(
            body_count >= 1,
            "infer_body should re-run for at least main (Counter signature changed), \
             but ran {} times: {:?}",
            body_count,
            queries
                .iter()
                .filter(|q| q.contains("infer_body"))
                .collect::<Vec<_>>()
        );
    });
}
