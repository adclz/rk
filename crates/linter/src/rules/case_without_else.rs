use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{HirNodeInfo, hir_ty::body::BodyInferenceResult};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "case-without-else";

/// L0106: CASE statement without ELSE branch.
struct CaseWithoutElse;

impl ErrorCode for CaseWithoutElse {
    fn code(&self) -> &'static str {
        "L0106"
    }

    fn description(&self) -> &'static str {
        "CASE without ELSE"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    for stmt in &body.case_without_else {
        diagnostics.push(
            diag()
                .message("CASE statement has no ELSE branch".to_string())
                .desc(&CaseWithoutElse)
                .range(stmt.get_span(db))
                .severity(DiagnosticSeverity::WARNING)
                .call(),
        );
    }
}
