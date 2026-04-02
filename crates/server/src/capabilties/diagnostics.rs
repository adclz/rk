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
use hir::check::diagnostics_for_file;
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

    let mut all = diagnostics_for_file(db, file).as_ref().clone();
    if let Some(ref linter_config) = get_config(db).linter {
        linter::lint_file(db, file, linter_config, &mut all);
    }

    let items: Vec<_> = all.iter().map(|d| d.to_lsp_diagnostic(db)).collect();
    let new_id = fingerprint(&items);

    // If the client already has this exact result, skip resending.
    if let Some(ref prev_id) = params.previous_result_id
        && *prev_id == new_id {
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
    let config = get_config(db).clone();

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

    // Compute diagnostics for all current files.
    let computed: Vec<_> = db
        .get_files()
        .into_par_iter()
        .map_with(db.clone(), |db, file| {
            let file = *file;
            let uri = file.url(db).clone();

            let items = salsa::Cancelled::catch(|| {
                let mut all = diagnostics_for_file(db, file).as_ref().clone();
                if let Some(ref linter_config) = config.linter {
                    linter::lint_file(db, file, linter_config, &mut all);
                }
                all.iter()
                    .map(|d| d.to_lsp_diagnostic(db))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

            let new_id = fingerprint(&items);
            let version: Option<i64> = file.version(db).map(|i| i as i64);

            (uri, items, new_id, version)
        })
        .collect();

    // Build reports, comparing against client's cached result IDs.
    let mut result: Vec<WorkspaceDocumentDiagnosticReport> = computed
        .into_iter()
        .map(|(uri, items, new_id, version)| {
            let previous_id = previous.remove(&uri);

            if previous_id.as_deref() == Some(&new_id) {
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
            }
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
