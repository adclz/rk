use auto_lsp::lsp_types::{DiagnosticSeverity, DiagnosticTag};
use db::WorkspaceDataBase;
use hir::{HirNodeInfo, hir_ty::body::BodyInferenceResult};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "dead-code";

/// L0301: unreachable statement.
struct DeadCode;

impl ErrorCode for DeadCode {
    fn code(&self) -> &'static str {
        "L0301"
    }

    fn description(&self) -> &'static str {
        "unreachable code"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    for stmt in &body.dead_code_statements {
        diagnostics.push(
            diag()
                .message("unreachable statement".to_string())
                .desc(&DeadCode)
                .range(stmt.get_span(db))
                .severity(DiagnosticSeverity::WARNING)
                .tags(vec![DiagnosticTag::UNNECESSARY])
                .call(),
        );
    }
}
