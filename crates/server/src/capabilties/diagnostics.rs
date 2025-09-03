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
                items: diagnostics_for_file(db, file)
                    .iter()
                    .map(|d| d.to_lsp_diagnostic(db))
                    .collect::<Vec<_>>(),
            },
        }),
    ))
}

/// fixme:
/// This code is from [salsa parallel module](https://github.com/salsa-rs/salsa/blob/a0e7a0660c93136f23bf08b4f1604eee3d1f6b11/src/parallel.rs#L25)
/// 
/// Both this struct and the par_iter part are vendored because salsa seems to not be able to downcast
/// the DbView to the actual database type inside the par_iter closure.
/// 
/// It seems to have been fixed in salsa > 0.22, but auto_lsp must also be updated to use the newer version of salsa.

struct DbForkOnClone(Box<dyn salsa::Database>);

impl Clone for DbForkOnClone {
    fn clone(&self) -> Self {
        DbForkOnClone(self.0.fork_db())
    }
}

pub fn workspace_diagnostics(
    db: &impl BaseDatabase,
    _params: WorkspaceDiagnosticParams,
) -> anyhow::Result<WorkspaceDiagnosticReportResult> {
    let result = db
        .get_files()
        .into_par_iter()
        .map_with(DbForkOnClone(db.fork_db()), |db, file| {
            let db= db.0.as_view();
            let file = *file;
            let errors: Vec<auto_lsp::lsp_types::Diagnostic> = diagnostics_for_file(db, file)
                .iter()
                .map(|d| d.to_lsp_diagnostic(db))
                .collect::<Vec<_>>();

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
