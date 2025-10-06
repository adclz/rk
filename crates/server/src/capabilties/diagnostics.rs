use std::panic::RefUnwindSafe;

use auto_lsp::anyhow;
use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::{
    DocumentDiagnosticParams, DocumentDiagnosticReport, DocumentDiagnosticReportResult,
    FullDocumentDiagnosticReport, RelatedFullDocumentDiagnosticReport, WorkspaceDiagnosticParams,
    WorkspaceDiagnosticReport, WorkspaceDiagnosticReportResult, WorkspaceDocumentDiagnosticReport,
    WorkspaceFullDocumentDiagnosticReport,
};
use hir::check::diagnostics_for_file;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

pub fn diagnostics<Db: BaseDatabase + Clone + RefUnwindSafe>(
    db: &Db,
    params: DocumentDiagnosticParams,
) -> anyhow::Result<DocumentDiagnosticReportResult> {
    let uri = params.text_document.uri;

    let file = db
        .get_file(&uri)
        .ok_or_else(|| anyhow::format_err!("File not found in workspace"))?;

    Ok(DocumentDiagnosticReportResult::Report(
        DocumentDiagnosticReport::Full(RelatedFullDocumentDiagnosticReport {
            related_documents: None,
            full_document_diagnostic_report: FullDocumentDiagnosticReport {
                result_id: None,
                items: diagnostics_for_file(db, file)
                    .iter()
                    .map(|d| d.to_lsp_diagnostic(db))
                    .collect::<Vec<_>>(),
            },
        }),
    ))
}

pub fn workspace_diagnostics<Db: BaseDatabase + Clone + RefUnwindSafe>(
    db: &Db,
    _params: WorkspaceDiagnosticParams,
) -> anyhow::Result<WorkspaceDiagnosticReportResult> {
    let result = db
        .get_files()
        .into_par_iter()
        .map_with(db.clone(), |db, file| {
            let file = *file;

            let errors = match salsa::Cancelled::catch(|| {
                diagnostics_for_file(db, file)
                    .iter()
                    .map(|d| d.to_lsp_diagnostic(db))
                    .collect::<Vec<_>>()
            })
            // ignore salsa errors
            .ok()
            {
                Some(errors) => errors,
                None => vec![],
            };

            WorkspaceDocumentDiagnosticReport::Full(WorkspaceFullDocumentDiagnosticReport {
                version: file.version(db).map(|i| i.into()),
                full_document_diagnostic_report: FullDocumentDiagnosticReport {
                    result_id: None,
                    items: errors,
                },
                uri: file.url(db).clone(),
            })
        })
        .collect();

    Ok(WorkspaceDiagnosticReportResult::Report(
        WorkspaceDiagnosticReport { items: result },
    ))
}
