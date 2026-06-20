use std::io::Write;

use auto_lsp::lsp_types::Url;
use db::RootDatabase;
use ide_diagnostic::IdeDiagnostic;

use crate::reports::sources;

#[allow(clippy::too_many_arguments)]
pub fn report_diagnostics(
    db: &RootDatabase,
    workspace_path: &std::path::Path,
    config: ariadne::Config,
    url: &Url,
    content: &str,
    diagnostics: &[IdeDiagnostic],
    caches: &Vec<(&str, &str)>,
    total_errors: &mut i32,
    total_warnings: &mut i32,
    out: &mut dyn Write,
) {
    let url_str = url.as_str();
    let rel_path = url
        .to_file_path()
        .ok()
        .and_then(|abs_path| {
            abs_path
                .strip_prefix(workspace_path)
                .ok()
                .map(|p| p.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| url_str.to_string());

    for diagnostic in diagnostics {
        match &diagnostic.diagnostic.severity {
            Some(auto_lsp::lsp_types::DiagnosticSeverity::ERROR) => *total_errors += 1,
            Some(auto_lsp::lsp_types::DiagnosticSeverity::WARNING) => *total_warnings += 1,
            _ => {}
        }

        let report = diagnostic.create_report(db, url, content, Some(config), true);
        let mut buffer = vec![];

        report
            .write(sources(caches.clone()), &mut buffer)
            .expect("failed to write report");

        let output = String::from_utf8_lossy(&buffer);
        let shortened = output.replace(url_str, &rel_path);
        // Render to the caller's sink — stderr for CLI commands, or an in-memory
        // buffer for the debugger (which forwards the text over the debugger transport,
        // since stdout is the debugger transport there).
        let _ = write!(out, "{}", shortened);
    }
}
