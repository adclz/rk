use auto_lsp::anyhow;
use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::{
    DocumentDiagnosticParams, DocumentDiagnosticReport,  WorkspaceFullDocumentDiagnosticReport, DocumentDiagnosticReportResult, FullDocumentDiagnosticReport, RelatedFullDocumentDiagnosticReport, WorkspaceDiagnosticParams, WorkspaceDiagnosticReport, WorkspaceDiagnosticReportResult, WorkspaceDocumentDiagnosticReport
};
use db::diagnostics::{cached_diagnostics};

pub fn diagnostics(
    db: &impl BaseDatabase,
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
                items: cached_diagnostics(db, file).iter().map(|d| d.diagnostic.clone()).collect(),
            },
        }),
    ))
}



pub fn workspace_diagnostics(
    db: &impl BaseDatabase,
    _params: WorkspaceDiagnosticParams,
) -> anyhow::Result<WorkspaceDiagnosticReportResult> {

    let result: Vec<WorkspaceDocumentDiagnosticReport> = db
        .get_files()
        .iter()
        .map(|file| {   
            let file = *file;
            let errors: Vec<auto_lsp::lsp_types::Diagnostic> = cached_diagnostics(db, file)
                .iter()
                .map(|d| d.into())
                .collect();

            WorkspaceDocumentDiagnosticReport::Full(WorkspaceFullDocumentDiagnosticReport {
                version: None,
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
