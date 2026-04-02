use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{HirNodeInfo, hir_ty::body::BodyInferenceResult};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "unused-return-type";

/// L0104: function call discards a return value.
struct UnusedReturnType;

impl ErrorCode for UnusedReturnType {
    fn code(&self) -> &'static str {
        "L0104"
    }

    fn description(&self) -> &'static str {
        "unused return value"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    for (stmt, typ) in &body.unused_return_types {
        let mut diag = diag()
            .message(format!("unused return value of '{}'", typ.type_name(db)))
            .desc(&UnusedReturnType)
            .range(stmt.get_span(db))
            .severity(DiagnosticSeverity::INFORMATION)
            .call();

        typ.with_location(db, &mut diag);

        diagnostics.push(diag);
    }
}
