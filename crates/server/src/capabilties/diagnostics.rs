// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! Pull-based diagnostics for the IEC 61131-3 LSP server.
//!
//! # How it works
//!
//! The server uses **pull diagnostics** (LSP 3.17+) instead of push (`publishDiagnostics`).
//! The client requests diagnostics and the server responds — no unsolicited notifications.
//!
//! ## Document diagnostics (`textDocument/diagnostic`)
//!
//! Called per-file when the editor needs fresh diagnostics. Each response includes a
//! `result_id` (a fingerprint hash of the diagnostics). On subsequent requests, the client
//! sends back `previous_result_id`. If the fingerprint matches, the server returns an
//! `Unchanged` report — saving bandwidth by not resending identical data.
//!
//! ## Workspace diagnostics (`workspace/diagnostic`)
//!
//! Called to get diagnostics for all files at once. The client sends `previous_result_ids`
//! for every file it has cached results for. The server:
//!
//! 1. Computes diagnostics for all current files
//! 2. Compares each file's fingerprint with the client's cached version
//!    - **Match** → `Unchanged` report (no data sent)
//!    - **Mismatch** → `Full` report with diagnostics
//! 3. Any URI in `previous_result_ids` that's no longer in the file set means the file
//!    was deleted — the server sends an empty `Full` report to clear stale entries
//!    from the client's Problems panel
//!
//! This approach follows the same model as [Ruff's ty_server](https://github.com/astral-sh/ruff).
//!
//! ## Diagnostic refresh
//!
//! After file events (create/change/delete), the server sends `workspace/diagnostic/refresh`
//! to tell the client its cached diagnostics may be stale. The client then re-pulls.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::panic::RefUnwindSafe;

use auto_lsp::anyhow;
use auto_lsp::lsp_types::{
    DocumentDiagnosticParams, DocumentDiagnosticReport, DocumentDiagnosticReportResult,
    FullDocumentDiagnosticReport, RelatedFullDocumentDiagnosticReport,
    RelatedUnchangedDocumentDiagnosticReport, UnchangedDocumentDiagnosticReport, Url,
    WorkspaceDiagnosticParams, WorkspaceDiagnosticReport, WorkspaceDiagnosticReportResult,
    WorkspaceDocumentDiagnosticReport, WorkspaceFullDocumentDiagnosticReport,
    WorkspaceUnchangedDocumentDiagnosticReport,
};
use db::{WorkspaceDataBase, config_file::get_config};
use hir::check::{check_file, diagnostics_for_file};
use hir::hir_ty::calls::CallGraph;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

/// Compute a fingerprint hash for a list of diagnostics.
/// Used as `result_id` so the client can track what it has seen.
fn fingerprint(items: &[auto_lsp::lsp_types::Diagnostic]) -> String {
    let mut hasher = DefaultHasher::new();
    items.len().hash(&mut hasher);
    for item in items {
        item.message.hash(&mut hasher);
        format!("{:?}", item.range).hash(&mut hasher);
        format!("{:?}", item.severity).hash(&mut hasher);
        if let Some(ref code) = item.code {
            format!("{:?}", code).hash(&mut hasher);
        }
    }
    format!("{:x}", hasher.finish())
}

// ── Document diagnostics (single file) ──────────────────────────────────

pub fn diagnostics<Db: WorkspaceDataBase + Clone + RefUnwindSafe>(
    db: &Db,
    params: DocumentDiagnosticParams,
) -> anyhow::Result<DocumentDiagnosticReportResult> {
    let uri = params.text_document.uri;

    let file = match db.get_file(&uri) {
        Some(file) => file,
        None => {
            return Ok(DocumentDiagnosticReportResult::Report(
                DocumentDiagnosticReport::Full(RelatedFullDocumentDiagnosticReport {
                    related_documents: None,
                    full_document_diagnostic_report: FullDocumentDiagnosticReport {
                        result_id: Some(fingerprint(&[])),
                        items: vec![],
                    },
                }),
            ));
        }
    };

    // What the file's bodies reach: the frames of its recursive calls and
    // the recursion lint ask it.
    let graph = CallGraph::of_files(db, [file]);
    let mut all = check_file(db, &graph, file);
    let linter_config = get_config(db).linter.clone().unwrap_or_default();
    let pass = linter::Pass::new(db, &linter_config, &graph);
    linter::lint_file(db, file, &linter_config, &pass, &mut all);

    let items: Vec<_> = all.iter().map(|d| d.to_lsp_diagnostic(db)).collect();
    let new_id = fingerprint(&items);

    // If the client already has this exact result, skip resending.
    if let Some(ref prev_id) = params.previous_result_id
        && *prev_id == new_id
    {
        return Ok(DocumentDiagnosticReportResult::Report(
            DocumentDiagnosticReport::Unchanged(RelatedUnchangedDocumentDiagnosticReport {
                related_documents: None,
                unchanged_document_diagnostic_report: UnchangedDocumentDiagnosticReport {
                    result_id: new_id,
                },
            }),
        ));
    }

    Ok(DocumentDiagnosticReportResult::Report(
        DocumentDiagnosticReport::Full(RelatedFullDocumentDiagnosticReport {
            related_documents: None,
            full_document_diagnostic_report: FullDocumentDiagnosticReport {
                result_id: Some(new_id),
                items,
            },
        }),
    ))
}

// ── Workspace diagnostics (all files) ───────────────────────────────────

pub fn workspace_diagnostics<Db: WorkspaceDataBase + Clone + RefUnwindSafe>(
    db: &Db,
    params: WorkspaceDiagnosticParams,
) -> anyhow::Result<WorkspaceDiagnosticReportResult> {
    // As above: absent means the recommended rules, not none of them.
    let linter_config = get_config(db).linter.clone().unwrap_or_default();

    // Index previous result IDs by URI for O(1) lookup.
    let mut previous: std::collections::HashMap<Url, String> = params
        .previous_result_ids
        .iter()
        .filter_map(|prev| {
            Url::parse(prev.uri.as_str())
                .ok()
                .map(|url| (url, prev.value.clone()))
        })
        .collect();

    // Every file is indexed before any is checked, the libraries' too, as
    // `rk check` does: a name is looked up in every file's index, so a check
    // reaching a file no thread had indexed yet indexed it while the other
    // threads reaching it waited. A file a write cancels is checked below,
    // and found cancelled there.
    db.get_files()
        .iter()
        .chain(db.get_library_files())
        .copied()
        .collect::<Vec<_>>()
        .into_par_iter()
        .for_each_with(db.clone(), |db, file| {
            let _ = salsa::Cancelled::catch(|| {
                hir::hir_def::semantic_index::semantic_index(db, file);
            });
        });
    // Every file is checked before the call graph is built and any is
    // linted, as `rk check` does: the graph, and a lint that looks across
    // the workspace, then read what the check computed, instead of
    // computing the other files' inference on one thread.
    let checked: Vec<_> = db
        .get_files()
        .into_par_iter()
        .map_with(db.clone(), |db, file| {
            let file = *file;
            let checked = salsa::Cancelled::catch(|| {
                diagnostics_for_file(db, file);
            })
            .is_ok();
            (file, checked)
        })
        .collect();
    // `None` when a write cancelled it.
    let graph =
        salsa::Cancelled::catch(|| CallGraph::of_files(db, db.get_files().iter().copied())).ok();
    let pass = graph.as_ref().and_then(|graph| {
        salsa::Cancelled::catch(|| linter::Pass::new(db, &linter_config, graph)).ok()
    });
    let computed: Vec<_> = checked
        .into_par_iter()
        .map_with(db.clone(), |db, (file, checked)| {
            let uri = file.url(db).clone();

            let db = &*db;
            let linter_config = &linter_config;
            // `None` when a write cancelled the file's check.
            let items = pass.as_ref().filter(|_| checked).and_then(|pass| {
                salsa::Cancelled::catch(move || {
                    let mut all = check_file(db, pass.graph, file);
                    linter::lint_file(db, file, linter_config, pass, &mut all);
                    all.iter()
                        .map(|d| d.to_lsp_diagnostic(db))
                        .collect::<Vec<_>>()
                })
                .ok()
            });

            let version: Option<i64> = file.version(db).map(|i| i as i64);

            (uri, items, version)
        })
        .collect();

    // Build reports, comparing against client's cached result IDs.
    let mut result: Vec<WorkspaceDocumentDiagnosticReport> = computed
        .into_iter()
        .filter_map(|(uri, items, version)| {
            let previous_id = previous.remove(&uri);
            // A file whose check a write cancelled is no clean file: what the
            // client holds for it stands until the next pull, and a file it
            // holds nothing for waits for that pull.
            let Some(items) = items else {
                return previous_id.map(|result_id| {
                    WorkspaceDocumentDiagnosticReport::Unchanged(
                        WorkspaceUnchangedDocumentDiagnosticReport {
                            version,
                            unchanged_document_diagnostic_report:
                                UnchangedDocumentDiagnosticReport { result_id },
                            uri,
                        },
                    )
                });
            };
            let new_id = fingerprint(&items);

            Some(if previous_id.as_deref() == Some(&new_id) {
                // Client already has this — skip resending.
                WorkspaceDocumentDiagnosticReport::Unchanged(
                    WorkspaceUnchangedDocumentDiagnosticReport {
                        version,
                        unchanged_document_diagnostic_report: UnchangedDocumentDiagnosticReport {
                            result_id: new_id,
                        },
                        uri,
                    },
                )
            } else {
                // Changed or new — send full diagnostics.
                WorkspaceDocumentDiagnosticReport::Full(WorkspaceFullDocumentDiagnosticReport {
                    version,
                    full_document_diagnostic_report: FullDocumentDiagnosticReport {
                        result_id: Some(new_id),
                        items,
                    },
                    uri,
                })
            })
        })
        .collect();

    // Any URI still in `previous` was reported before but no longer exists
    // (file was deleted). Send empty diagnostics to clear the client's cache.
    for (uri, _) in previous {
        result.push(WorkspaceDocumentDiagnosticReport::Full(
            WorkspaceFullDocumentDiagnosticReport {
                version: None,
                full_document_diagnostic_report: FullDocumentDiagnosticReport {
                    result_id: Some(fingerprint(&[])),
                    items: vec![],
                },
                uri,
            },
        ));
    }

    Ok(WorkspaceDiagnosticReportResult::Report(
        WorkspaceDiagnosticReport { items: result },
    ))
}
