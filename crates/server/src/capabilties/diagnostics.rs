use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::panic::RefUnwindSafe;

use auto_lsp::anyhow;
use auto_lsp::lsp_types::{
    DocumentDiagnosticParams, DocumentDiagnosticReport, DocumentDiagnosticReportResult,
    FullDocumentDiagnosticReport, RelatedFullDocumentDiagnosticReport, Url,
    WorkspaceDiagnosticParams, WorkspaceDiagnosticReport, WorkspaceDiagnosticReportResult,
    WorkspaceDocumentDiagnosticReport, WorkspaceFullDocumentDiagnosticReport,
};
use db::{WorkspaceDataBase, config_file::get_config};
use hir::check::diagnostics_for_file;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

/// Compute a fingerprint hash for a list of diagnostics.
/// Used as `result_id` so the client can track what it has seen.
fn diagnostics_fingerprint(items: &[auto_lsp::lsp_types::Diagnostic]) -> String {
    let mut hasher = DefaultHasher::new();
    for item in items {
        item.message.hash(&mut hasher);
        format!("{:?}", item.range).hash(&mut hasher);
        format!("{:?}", item.severity).hash(&mut hasher);
    }
    format!("{:x}", hasher.finish())
}

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
                        result_id: Some(diagnostics_fingerprint(&[])),
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
    let result_id = diagnostics_fingerprint(&items);

    Ok(DocumentDiagnosticReportResult::Report(
        DocumentDiagnosticReport::Full(RelatedFullDocumentDiagnosticReport {
            related_documents: None,
            full_document_diagnostic_report: FullDocumentDiagnosticReport {
                result_id: Some(result_id),
                items,
            },
        }),
    ))
}

pub fn workspace_diagnostics<Db: WorkspaceDataBase + Clone + RefUnwindSafe>(
    db: &Db,
    params: WorkspaceDiagnosticParams,
) -> anyhow::Result<WorkspaceDiagnosticReportResult> {
    let config = get_config(db).clone();

    // Build a set of URIs the client previously had results for.
    let mut previous: std::collections::HashMap<Url, String> = params
        .previous_result_ids
        .iter()
        .filter_map(|prev| {
            Url::parse(prev.uri.as_str())
                .ok()
                .map(|url| (url, prev.value.clone()))
        })
        .collect();

    // Compute diagnostics for all current files
    let mut result: Vec<WorkspaceDocumentDiagnosticReport> = db
        .get_files()
        .into_par_iter()
        .map_with(db.clone(), |db, file| {
            let file = *file;

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

            let result_id = diagnostics_fingerprint(&items);

            WorkspaceDocumentDiagnosticReport::Full(WorkspaceFullDocumentDiagnosticReport {
                version: file.version(db).map(|i| i.into()),
                full_document_diagnostic_report: FullDocumentDiagnosticReport {
                    result_id: Some(result_id),
                    items,
                },
                uri: file.url(db).clone(),
            })
        })
        .collect();

    // Remove current URIs from the previous set
    for report in &result {
        if let WorkspaceDocumentDiagnosticReport::Full(full) = report {
            previous.remove(&full.uri);
        }
    }

    // Any URI still in `previous` was reported before but no longer exists.
    // Send empty diagnostics to clear stale entries in the client.
    for (uri, _) in previous {
        result.push(WorkspaceDocumentDiagnosticReport::Full(
            WorkspaceFullDocumentDiagnosticReport {
                version: None,
                full_document_diagnostic_report: FullDocumentDiagnosticReport {
                    result_id: Some(diagnostics_fingerprint(&[])),
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
