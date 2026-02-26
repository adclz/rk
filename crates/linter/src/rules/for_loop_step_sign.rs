use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_ty::body::BodyInferenceResult,
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "for-loop-step-sign";

/// W0108: FOR loop step direction mismatches bounds.
struct ForLoopStepSign;

impl ErrorCode for ForLoopStepSign {
    fn code(&self) -> &'static str {
        "W0108"
    }

    fn description(&self) -> &'static str {
        "FOR loop step sign mismatch"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    for stmt in &body.mismatched_for_step {
        diagnostics.push(
            diag()
                .message("FOR loop step direction mismatches bounds direction".to_string())
                .desc(&ForLoopStepSign)
                .range(stmt.get_span(db))
                .severity(DiagnosticSeverity::INFORMATION)
                .call(),
        );
    }
}
