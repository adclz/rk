use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_ty::body::BodyInferenceResult,
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

pub const NAME: &str = "shadowing-variable";

/// W0102: variable name shadows a POU (function, function block, class, etc.)
struct ShadowingVariable;

impl ErrorCode for ShadowingVariable {
    fn code(&self) -> &'static str {
        "W0102"
    }

    fn description(&self) -> &'static str {
        "name shadowing"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    for (var, pou) in &body.variables_shadowing {
        let var_name = var.get_name_ident(db).text(db);

        let mut diag = diag()
                .message(format!(
                    "variable '{var_name}' shadows POU '{var_name}' available in this scope"
                ))
                .severity(DiagnosticSeverity::INFORMATION)
                .desc(&ShadowingVariable)
                .range(var.get_span(db))
                .call();

        diag.with_related(Related::new(format!("POU {var_name} is declared here"), pou.get_scope_id(db).file(db), pou.get_name_span(db)));

        diagnostics.push(
            diag
        );
    }
}
