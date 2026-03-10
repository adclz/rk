use std::sync::Arc;
use std::sync::RwLock;

use auto_lsp::core::document::Document;
use auto_lsp::default::db::BaseDatabase;
use auto_lsp::default::db::file::File;
use auto_lsp::salsa::{self, Event, EventKind, Setter};
use db::RootDatabase;
use hir::check::diagnostics_for_file;
use insta::assert_snapshot;
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

/// Format salsa events as a sorted, newline-separated string for snapshotting.
///
/// Keeps all events except `WillCheckCancellation` and `DidSetCancellationFlag`.
/// Events are sorted alphabetically to produce deterministic output regardless
/// of salsa's lazy validation ordering.
///
/// Must be called within `salsa::attach` for readable query names.
fn snapshot_log(log: &[Event]) -> String {
    let mut entries: Vec<_> = log
        .iter()
        .filter(|e| {
            !matches!(
                e.kind,
                EventKind::WillCheckCancellation | EventKind::DidSetCancellationFlag
            )
        })
        .map(|e| format!("{:?}", e.kind))
        .collect();
    entries.sort();
    entries.join("\n")
}

/// Prime all diagnostics, clear the log, then return files sorted by URL.
fn prime_all(db: &RootDatabase, log: &Arc<RwLock<Vec<Event>>>) -> Vec<File> {
    let mut files: Vec<File> = db.get_files().iter().map(|e| *e.value()).collect();
    files.sort_by_key(|f| f.url(db).as_str().to_string());

    for file in &files {
        diagnostics_for_file(db, *file);
    }

    log.write().unwrap().clear();
    files
}

// ---------------------------------------------------------------------------
// Basic: body edit only re-analyzes the changed file
// ---------------------------------------------------------------------------

#[rstest]
fn body_edit_only_reruns_semantic_index_for_changed_file(
    with_log_db: (RootDatabase, Arc<RwLock<Vec<Event>>>),
) {
    let (mut db, log) = with_log_db;

    add_sources(
        &mut db,
        &[
            r#"
        FUNCTION_BLOCK Counter
        VAR
            count : INT;
        END_VAR
            count := count + 1;
        END_FUNCTION_BLOCK
    "#,
            r#"
        FUNCTION main : INT
        VAR
            c : Counter;
        END_VAR
            c();
            main := 0;
        END_FUNCTION
    "#,
        ],
    );

    let files = prime_all(&db, &log);

    let file1 = *files
        .iter()
        .find(|f| f.url(&db).as_str().contains("test1"))
        .unwrap();

    edit_file(
        &mut db,
        file1,
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

    for file in &files {
        diagnostics_for_file(&db, *file);
    }

    salsa::attach(&db, || {
        let events = log.read().unwrap();
        assert_snapshot!(snapshot_log(&events), @r#"
        DidDiscard { key: Expr(Id(2003)) }
        DidDiscard { key: Stmt(Id(2802)) }
        DidInternValue { key: Ident(Id(806)), revision: R2 }
        DidInternValue { key: Integer(Id(2402)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(800)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(801)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(802)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(803)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(804)), revision: R2 }
        DidValidateInternedValue { key: Integer(Id(2400)), revision: R2 }
        DidValidateMemoizedValue { database_key: Integer::as_i16_(Id(2400)) }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::flatten_(Id(1400)) }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::flatten_(Id(1401)) }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::flatten_(Id(1402)) }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::flatten_(Id(1403)) }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::to_namespace_access_(Id(1403)) }
        DidValidateMemoizedValue { database_key: ScopeId < 'db >::def_map_(Id(402)) }
        DidValidateMemoizedValue { database_key: ScopeId < 'db >::def_map_(Id(405)) }
        DidValidateMemoizedValue { database_key: ScopeId < 'db >::inheritors_(Id(402)) }
        DidValidateMemoizedValue { database_key: ScopeId < 'db >::inheritors_(Id(405)) }
        DidValidateMemoizedValue { database_key: file_global_pous(Id(0)) }
        DidValidateMemoizedValue { database_key: get_ast(Id(0)) }
        DidValidateMemoizedValue { database_key: get_scope(Id(401)) }
        DidValidateMemoizedValue { database_key: get_scope(Id(402)) }
        DidValidateMemoizedValue { database_key: infer_body(Id(401)) }
        DidValidateMemoizedValue { database_key: infer_body(Id(402)) }
        DidValidateMemoizedValue { database_key: infer_body(Id(404)) }
        DidValidateMemoizedValue { database_key: infer_initialization(Id(401)) }
        DidValidateMemoizedValue { database_key: infer_initialization(Id(402)) }
        DidValidateMemoizedValue { database_key: infer_initialization(Id(404)) }
        DidValidateMemoizedValue { database_key: infer_initialization(Id(405)) }
        DidValidateMemoizedValue { database_key: infer_signature(Id(401)) }
        DidValidateMemoizedValue { database_key: infer_signature(Id(402)) }
        DidValidateMemoizedValue { database_key: infer_signature(Id(404)) }
        DidValidateMemoizedValue { database_key: inherited_methods(Id(2c00)) }
        DidValidateMemoizedValue { database_key: inherited_methods(Id(3400)) }
        DidValidateMemoizedValue { database_key: semantic_index(Id(0)) }
        WillDiscardStaleOutput { execute_key: semantic_index(Id(1)), output_key: Expr(Id(2003)) }
        WillDiscardStaleOutput { execute_key: semantic_index(Id(1)), output_key: Stmt(Id(2802)) }
        WillExecute { database_key: Integer::as_i16_(Id(2402)) }
        WillExecute { database_key: file_global_pous(Id(1)) }
        WillExecute { database_key: get_ast(Id(1)) }
        WillExecute { database_key: get_scope(Id(404)) }
        WillExecute { database_key: get_scope(Id(405)) }
        WillExecute { database_key: infer_body(Id(405)) }
        WillExecute { database_key: infer_signature(Id(405)) }
        WillExecute { database_key: semantic_index(Id(1)) }
        "#);
    });
}

#[rstest]
fn body_edit_does_not_reinfer_signature_of_other_file(
    with_log_db: (RootDatabase, Arc<RwLock<Vec<Event>>>),
) {
    let (mut db, log) = with_log_db;

    add_sources(
        &mut db,
        &[
            r#"
        FUNCTION_BLOCK Counter
        VAR
            count : INT;
        END_VAR
            count := count + 1;
        END_FUNCTION_BLOCK
    "#,
            r#"
        FUNCTION main : INT
        VAR
            c : Counter;
        END_VAR
            c();
            main := 0;
        END_FUNCTION
    "#,
        ],
    );

    let files = prime_all(&db, &log);

    let file1 = *files
        .iter()
        .find(|f| f.url(&db).as_str().contains("test1"))
        .unwrap();

    edit_file(
        &mut db,
        file1,
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

    for file in &files {
        diagnostics_for_file(&db, *file);
    }

    salsa::attach(&db, || {
        let events = log.read().unwrap();
        assert_snapshot!(snapshot_log(&events), @r#"
        DidDiscard { key: Expr(Id(2003)) }
        DidDiscard { key: Stmt(Id(2802)) }
        DidInternValue { key: Ident(Id(806)), revision: R2 }
        DidInternValue { key: Integer(Id(2402)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(800)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(801)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(802)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(803)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(804)), revision: R2 }
        DidValidateInternedValue { key: Integer(Id(2400)), revision: R2 }
        DidValidateMemoizedValue { database_key: Integer::as_i16_(Id(2400)) }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::flatten_(Id(1400)) }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::flatten_(Id(1401)) }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::flatten_(Id(1402)) }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::flatten_(Id(1403)) }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::to_namespace_access_(Id(1403)) }
        DidValidateMemoizedValue { database_key: ScopeId < 'db >::def_map_(Id(402)) }
        DidValidateMemoizedValue { database_key: ScopeId < 'db >::def_map_(Id(405)) }
        DidValidateMemoizedValue { database_key: ScopeId < 'db >::inheritors_(Id(402)) }
        DidValidateMemoizedValue { database_key: ScopeId < 'db >::inheritors_(Id(405)) }
        DidValidateMemoizedValue { database_key: file_global_pous(Id(0)) }
        DidValidateMemoizedValue { database_key: get_ast(Id(0)) }
        DidValidateMemoizedValue { database_key: get_scope(Id(401)) }
        DidValidateMemoizedValue { database_key: get_scope(Id(402)) }
        DidValidateMemoizedValue { database_key: infer_body(Id(401)) }
        DidValidateMemoizedValue { database_key: infer_body(Id(402)) }
        DidValidateMemoizedValue { database_key: infer_body(Id(404)) }
        DidValidateMemoizedValue { database_key: infer_initialization(Id(401)) }
        DidValidateMemoizedValue { database_key: infer_initialization(Id(402)) }
        DidValidateMemoizedValue { database_key: infer_initialization(Id(404)) }
        DidValidateMemoizedValue { database_key: infer_initialization(Id(405)) }
        DidValidateMemoizedValue { database_key: infer_signature(Id(401)) }
        DidValidateMemoizedValue { database_key: infer_signature(Id(402)) }
        DidValidateMemoizedValue { database_key: infer_signature(Id(404)) }
        DidValidateMemoizedValue { database_key: inherited_methods(Id(2c00)) }
        DidValidateMemoizedValue { database_key: inherited_methods(Id(3400)) }
        DidValidateMemoizedValue { database_key: semantic_index(Id(0)) }
        WillDiscardStaleOutput { execute_key: semantic_index(Id(1)), output_key: Expr(Id(2003)) }
        WillDiscardStaleOutput { execute_key: semantic_index(Id(1)), output_key: Stmt(Id(2802)) }
        WillExecute { database_key: Integer::as_i16_(Id(2402)) }
        WillExecute { database_key: file_global_pous(Id(1)) }
        WillExecute { database_key: get_ast(Id(1)) }
        WillExecute { database_key: get_scope(Id(404)) }
        WillExecute { database_key: get_scope(Id(405)) }
        WillExecute { database_key: infer_body(Id(405)) }
        WillExecute { database_key: infer_signature(Id(405)) }
        WillExecute { database_key: semantic_index(Id(1)) }
        "#);
    });
}

// ---------------------------------------------------------------------------
// Cross-file: editing an unrelated POU does not invalidate the dependent
// ---------------------------------------------------------------------------

/// file0 has Counter (used by file1) and Helper (independent).
/// Editing Helper's body should NOT cause file1's queries to re-run.
#[rstest]
fn editing_unrelated_pou_does_not_invalidate_cross_file_dependent(
    with_log_db: (RootDatabase, Arc<RwLock<Vec<Event>>>),
) {
    let (mut db, log) = with_log_db;

    add_sources(
        &mut db,
        &[
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
            helper := x + 1;
        END_FUNCTION
    "#,
            r#"
        FUNCTION main : INT
        VAR
            c : Counter;
        END_VAR
            c();
            main := c.count;
        END_FUNCTION
    "#,
        ],
    );

    let files = prime_all(&db, &log);

    // Edit helper's body in file0 — Counter is untouched
    let file0 = *files
        .iter()
        .find(|f| f.url(&db).as_str().contains("test0"))
        .unwrap();

    edit_file(
        &mut db,
        file0,
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
        assert_snapshot!(snapshot_log(&events), @r#"
        DidDiscard { key: Expr(Id(2004)) }
        DidDiscard { key: Expr(Id(2005)) }
        DidDiscard { key: Stmt(Id(2801)) }
        DidInternValue { key: Ident(Id(807)), revision: R2 }
        DidInternValue { key: Integer(Id(2401)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(800)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(800)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(801)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(802)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(802)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(803)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(804)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(805)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(806)), revision: R2 }
        DidValidateInternedValue { key: Integer(Id(2400)), revision: R2 }
        DidValidateMemoizedValue { database_key: Integer::as_i16_(Id(2400)) }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::flatten_(Id(1400)) }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::flatten_(Id(1401)) }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::flatten_(Id(1402)) }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::flatten_(Id(1403)) }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::flatten_(Id(1404)) }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::flatten_(Id(1405)) }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::flatten_(Id(1407)) }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::to_namespace_access_(Id(1402)) }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::to_namespace_access_(Id(1405)) }
        DidValidateMemoizedValue { database_key: ScopeId < 'db >::def_map_(Id(402)) }
        DidValidateMemoizedValue { database_key: ScopeId < 'db >::def_map_(Id(403)) }
        DidValidateMemoizedValue { database_key: ScopeId < 'db >::def_map_(Id(406)) }
        DidValidateMemoizedValue { database_key: ScopeId < 'db >::inheritors_(Id(402)) }
        DidValidateMemoizedValue { database_key: ScopeId < 'db >::inheritors_(Id(403)) }
        DidValidateMemoizedValue { database_key: ScopeId < 'db >::inheritors_(Id(406)) }
        DidValidateMemoizedValue { database_key: file_global_pous(Id(1)) }
        DidValidateMemoizedValue { database_key: get_ast(Id(1)) }
        DidValidateMemoizedValue { database_key: get_scope(Id(405)) }
        DidValidateMemoizedValue { database_key: get_scope(Id(406)) }
        DidValidateMemoizedValue { database_key: infer_body(Id(401)) }
        DidValidateMemoizedValue { database_key: infer_body(Id(405)) }
        DidValidateMemoizedValue { database_key: infer_body(Id(406)) }
        DidValidateMemoizedValue { database_key: infer_initialization(Id(401)) }
        DidValidateMemoizedValue { database_key: infer_initialization(Id(402)) }
        DidValidateMemoizedValue { database_key: infer_initialization(Id(403)) }
        DidValidateMemoizedValue { database_key: infer_initialization(Id(405)) }
        DidValidateMemoizedValue { database_key: infer_initialization(Id(406)) }
        DidValidateMemoizedValue { database_key: infer_signature(Id(401)) }
        DidValidateMemoizedValue { database_key: infer_signature(Id(402)) }
        DidValidateMemoizedValue { database_key: infer_signature(Id(403)) }
        DidValidateMemoizedValue { database_key: infer_signature(Id(405)) }
        DidValidateMemoizedValue { database_key: infer_signature(Id(406)) }
        DidValidateMemoizedValue { database_key: inherited_methods(Id(2c00)) }
        DidValidateMemoizedValue { database_key: inherited_methods(Id(3000)) }
        DidValidateMemoizedValue { database_key: inherited_methods(Id(3001)) }
        DidValidateMemoizedValue { database_key: semantic_index(Id(1)) }
        WillDiscardStaleOutput { execute_key: semantic_index(Id(0)), output_key: Expr(Id(2004)) }
        WillDiscardStaleOutput { execute_key: semantic_index(Id(0)), output_key: Expr(Id(2005)) }
        WillDiscardStaleOutput { execute_key: semantic_index(Id(0)), output_key: Stmt(Id(2801)) }
        WillExecute { database_key: Integer::as_i16_(Id(2401)) }
        WillExecute { database_key: file_global_pous(Id(0)) }
        WillExecute { database_key: get_ast(Id(0)) }
        WillExecute { database_key: get_scope(Id(401)) }
        WillExecute { database_key: get_scope(Id(402)) }
        WillExecute { database_key: get_scope(Id(403)) }
        WillExecute { database_key: infer_body(Id(402)) }
        WillExecute { database_key: infer_body(Id(403)) }
        WillExecute { database_key: semantic_index(Id(0)) }
        "#);
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

    add_sources(
        &mut db,
        &[
            r#"
        FUNCTION_BLOCK Counter
        VAR
            count : INT;
        END_VAR
            count := count + 1;
        END_FUNCTION_BLOCK
    "#,
            r#"
        FUNCTION main : INT
        VAR
            c : Counter;
        END_VAR
            c();
            main := c.count;
        END_FUNCTION
    "#,
        ],
    );

    let files = prime_all(&db, &log);

    let file0 = *files
        .iter()
        .find(|f| f.url(&db).as_str().contains("test0"))
        .unwrap();

    edit_file(
        &mut db,
        file0,
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
        assert_snapshot!(snapshot_log(&events), @r#"
        DidDiscard { key: Expr(Id(2001)) }
        DidDiscard { key: Expr(Id(2002)) }
        DidDiscard { key: Stmt(Id(2800)) }
        DidInternValue { key: Ident(Id(805)), revision: R2 }
        DidInternValue { key: Integer(Id(2401)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(800)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(800)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(802)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(802)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(803)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(804)), revision: R2 }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::flatten_(Id(1400)) }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::flatten_(Id(1401)) }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::flatten_(Id(1402)) }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::flatten_(Id(1403)) }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::flatten_(Id(1405)) }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::to_namespace_access_(Id(1403)) }
        DidValidateMemoizedValue { database_key: ScopeId < 'db >::def_map_(Id(402)) }
        DidValidateMemoizedValue { database_key: ScopeId < 'db >::def_map_(Id(405)) }
        DidValidateMemoizedValue { database_key: ScopeId < 'db >::inheritors_(Id(402)) }
        DidValidateMemoizedValue { database_key: ScopeId < 'db >::inheritors_(Id(405)) }
        DidValidateMemoizedValue { database_key: file_global_pous(Id(1)) }
        DidValidateMemoizedValue { database_key: get_ast(Id(1)) }
        DidValidateMemoizedValue { database_key: get_scope(Id(404)) }
        DidValidateMemoizedValue { database_key: get_scope(Id(405)) }
        DidValidateMemoizedValue { database_key: infer_body(Id(401)) }
        DidValidateMemoizedValue { database_key: infer_body(Id(404)) }
        DidValidateMemoizedValue { database_key: infer_body(Id(405)) }
        DidValidateMemoizedValue { database_key: infer_initialization(Id(401)) }
        DidValidateMemoizedValue { database_key: infer_initialization(Id(402)) }
        DidValidateMemoizedValue { database_key: infer_initialization(Id(404)) }
        DidValidateMemoizedValue { database_key: infer_initialization(Id(405)) }
        DidValidateMemoizedValue { database_key: infer_signature(Id(401)) }
        DidValidateMemoizedValue { database_key: infer_signature(Id(402)) }
        DidValidateMemoizedValue { database_key: infer_signature(Id(404)) }
        DidValidateMemoizedValue { database_key: infer_signature(Id(405)) }
        DidValidateMemoizedValue { database_key: inherited_methods(Id(2c00)) }
        DidValidateMemoizedValue { database_key: inherited_methods(Id(3400)) }
        DidValidateMemoizedValue { database_key: semantic_index(Id(1)) }
        WillDiscardStaleOutput { execute_key: semantic_index(Id(0)), output_key: Expr(Id(2001)) }
        WillDiscardStaleOutput { execute_key: semantic_index(Id(0)), output_key: Expr(Id(2002)) }
        WillDiscardStaleOutput { execute_key: semantic_index(Id(0)), output_key: Stmt(Id(2800)) }
        WillExecute { database_key: Integer::as_i16_(Id(2401)) }
        WillExecute { database_key: file_global_pous(Id(0)) }
        WillExecute { database_key: get_ast(Id(0)) }
        WillExecute { database_key: get_scope(Id(401)) }
        WillExecute { database_key: get_scope(Id(402)) }
        WillExecute { database_key: infer_body(Id(402)) }
        WillExecute { database_key: semantic_index(Id(0)) }
        "#);
    });
}

// ---------------------------------------------------------------------------
// Cross-file: signature change DOES invalidate the dependent's body
// ---------------------------------------------------------------------------

/// file0 has Counter with `count: INT`. file1 uses `c.count`.
/// Changing Counter's variable type from INT to REAL should trigger
/// re-inference of main's body (because the type of `c.count` changed).
///
/// Note: `infer_signature(main)` should NOT re-run — it only resolves
/// `c : Counter`, and Counter's identity (name) hasn't changed.
/// The body, however, accesses `c.count` which reads Counter's variable
/// type through tracked field dependencies, so it SHOULD re-run.
#[rstest]
fn signature_change_does_reinfer_dependent_body(
    with_log_db: (RootDatabase, Arc<RwLock<Vec<Event>>>),
) {
    let (mut db, log) = with_log_db;

    add_sources(
        &mut db,
        &[
            r#"
        FUNCTION_BLOCK Counter
        VAR
            count : INT;
        END_VAR
            count := count + 1;
        END_FUNCTION_BLOCK
    "#,
            r#"
        FUNCTION main : INT
        VAR
            c : Counter;
        END_VAR
            c();
            main := c.count;
        END_FUNCTION
    "#,
        ],
    );

    let files = prime_all(&db, &log);

    let file0 = *files
        .iter()
        .find(|f| f.url(&db).as_str().contains("test0"))
        .unwrap();

    edit_file(
        &mut db,
        file0,
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
        assert_snapshot!(snapshot_log(&events), @r#"
        DidDiscard { key: BeginPathExpr(Id(1800)) }
        DidDiscard { key: BeginPathExpr(Id(1801)) }
        DidDiscard { key: Expr(Id(2000)) }
        DidDiscard { key: Expr(Id(2001)) }
        DidDiscard { key: Expr(Id(2002)) }
        DidDiscard { key: PathExpr < 'db >::flatten_(Id(1400)) }
        DidDiscard { key: PathExpr < 'db >::flatten_(Id(1401)) }
        DidDiscard { key: Spec(Id(c00)) }
        DidDiscard { key: Stmt(Id(2800)) }
        DidDiscard { key: VariableAccess(Id(1c00)) }
        DidDiscard { key: VariableAccess(Id(1c01)) }
        DidInternValue { key: Ident(Id(805)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(800)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(800)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(802)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(802)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(803)), revision: R2 }
        DidValidateInternedValue { key: Ident(Id(804)), revision: R2 }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::flatten_(Id(1402)) }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::flatten_(Id(1403)) }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::flatten_(Id(1405)) }
        DidValidateMemoizedValue { database_key: PathExpr < 'db >::to_namespace_access_(Id(1403)) }
        DidValidateMemoizedValue { database_key: ScopeId < 'db >::def_map_(Id(402)) }
        DidValidateMemoizedValue { database_key: ScopeId < 'db >::def_map_(Id(405)) }
        DidValidateMemoizedValue { database_key: ScopeId < 'db >::inheritors_(Id(402)) }
        DidValidateMemoizedValue { database_key: ScopeId < 'db >::inheritors_(Id(405)) }
        DidValidateMemoizedValue { database_key: file_global_pous(Id(1)) }
        DidValidateMemoizedValue { database_key: get_ast(Id(1)) }
        DidValidateMemoizedValue { database_key: get_scope(Id(404)) }
        DidValidateMemoizedValue { database_key: get_scope(Id(405)) }
        DidValidateMemoizedValue { database_key: infer_body(Id(401)) }
        DidValidateMemoizedValue { database_key: infer_body(Id(404)) }
        DidValidateMemoizedValue { database_key: infer_initialization(Id(401)) }
        DidValidateMemoizedValue { database_key: infer_initialization(Id(404)) }
        DidValidateMemoizedValue { database_key: infer_initialization(Id(405)) }
        DidValidateMemoizedValue { database_key: infer_signature(Id(401)) }
        DidValidateMemoizedValue { database_key: infer_signature(Id(404)) }
        DidValidateMemoizedValue { database_key: infer_signature(Id(405)) }
        DidValidateMemoizedValue { database_key: inherited_methods(Id(2c00)) }
        DidValidateMemoizedValue { database_key: inherited_methods(Id(3400)) }
        DidValidateMemoizedValue { database_key: semantic_index(Id(1)) }
        WillDiscardStaleOutput { execute_key: semantic_index(Id(0)), output_key: BeginPathExpr(Id(1800)) }
        WillDiscardStaleOutput { execute_key: semantic_index(Id(0)), output_key: BeginPathExpr(Id(1801)) }
        WillDiscardStaleOutput { execute_key: semantic_index(Id(0)), output_key: Expr(Id(2000)) }
        WillDiscardStaleOutput { execute_key: semantic_index(Id(0)), output_key: Expr(Id(2001)) }
        WillDiscardStaleOutput { execute_key: semantic_index(Id(0)), output_key: Expr(Id(2002)) }
        WillDiscardStaleOutput { execute_key: semantic_index(Id(0)), output_key: Spec(Id(c00)) }
        WillDiscardStaleOutput { execute_key: semantic_index(Id(0)), output_key: Stmt(Id(2800)) }
        WillDiscardStaleOutput { execute_key: semantic_index(Id(0)), output_key: VariableAccess(Id(1c00)) }
        WillDiscardStaleOutput { execute_key: semantic_index(Id(0)), output_key: VariableAccess(Id(1c01)) }
        WillExecute { database_key: Ident::as_f32_(Id(805)) }
        WillExecute { database_key: PathExpr < 'db >::flatten_(Id(1400g1)) }
        WillExecute { database_key: PathExpr < 'db >::flatten_(Id(1401g1)) }
        WillExecute { database_key: file_global_pous(Id(0)) }
        WillExecute { database_key: get_ast(Id(0)) }
        WillExecute { database_key: get_scope(Id(401)) }
        WillExecute { database_key: get_scope(Id(402)) }
        WillExecute { database_key: infer_body(Id(402)) }
        WillExecute { database_key: infer_body(Id(405)) }
        WillExecute { database_key: infer_initialization(Id(402)) }
        WillExecute { database_key: infer_signature(Id(402)) }
        WillExecute { database_key: semantic_index(Id(0)) }
        "#);
    });
}
