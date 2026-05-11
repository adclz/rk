use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{HirNodeInfo, hir_ty::body::BodyInferenceResult};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "effectless-statement";

/// L0211: statement has no effect.
struct EffectlessStatement;

impl ErrorCode for EffectlessStatement {
    fn code(&self) -> &'static str {
        "L0212"
    }

    fn description(&self) -> &'static str {
        "effectless statement"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    for stmt in &body.effectless_statements {
        diagnostics.push(
            diag()
                .message("statement has no effect".to_string())
                .desc(&EffectlessStatement)
                .range(stmt.get_span(db))
                .severity(DiagnosticSeverity::HINT)
                .call(),
        );
    }
}
