use std::panic::RefUnwindSafe;

use auto_lsp::anyhow;
use auto_lsp::lsp_types::{
    DocumentDiagnosticParams, DocumentDiagnosticReport, DocumentDiagnosticReportResult,
    FullDocumentDiagnosticReport, RelatedFullDocumentDiagnosticReport, WorkspaceDiagnosticParams,
    WorkspaceDiagnosticReport, WorkspaceDiagnosticReportResult, WorkspaceDocumentDiagnosticReport,
    WorkspaceFullDocumentDiagnosticReport,
};
use db::{WorkspaceDataBase, config_file::get_config};
use hir::check::diagnostics_for_file;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

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
                        result_id: None,
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

    Ok(DocumentDiagnosticReportResult::Report(
        DocumentDiagnosticReport::Full(RelatedFullDocumentDiagnosticReport {
            related_documents: None,
            full_document_diagnostic_report: FullDocumentDiagnosticReport {
                result_id: None,
                items: all
                    .iter()
                    .map(|d| d.to_lsp_diagnostic(db))
                    .collect::<Vec<_>>(),
            },
        }),
    ))
}

pub fn workspace_diagnostics<Db: WorkspaceDataBase + Clone + RefUnwindSafe>(
    db: &Db,
    _params: WorkspaceDiagnosticParams,
) -> anyhow::Result<WorkspaceDiagnosticReportResult> {
    let config = get_config(db).clone();

    let result: Vec<WorkspaceDocumentDiagnosticReport> = db
        .get_files()
        .into_par_iter()
        .map_with(db.clone(), |db, file| {
            let file = *file;

            let errors = salsa::Cancelled::catch(|| {
                let mut all = diagnostics_for_file(db, file).as_ref().clone();
                if let Some(ref linter_config) = config.linter {
                    linter::lint_file(db, file, linter_config, &mut all);
                }
                all.iter()
                    .map(|d| d.to_lsp_diagnostic(db))
                    .collect::<Vec<_>>()
            })
            // ignore salsa errors
            .unwrap_or_default();

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
