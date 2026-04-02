use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo, hir_def::pous::warn_pragma::WarnPragmaLevel, hir_ty::{body::BodyInferenceResult, ty::Type}
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

pub const NAME: &str = "warn-pragma";

struct WarnPragmaLint;

impl ErrorCode for WarnPragmaLint {
    fn code(&self) -> &'static str {
        "L0117"
    }

    fn description(&self) -> &'static str {
        "call site notice"
    }
}

/// Check all resolved call sites for {warn}/{info} pragmas on the target callable.
pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    for (path_expr, typ) in &body.type_of_path_expr {
        let callable = match typ {
            Type::CallableType(ct) => *ct,
            _ => continue,
        };

        let Some(pragma) = callable.warn_pragma(db) else {
            continue;
        };

        let severity = match pragma.level {
            WarnPragmaLevel::Warn => DiagnosticSeverity::WARNING,
            WarnPragmaLevel::Info => DiagnosticSeverity::INFORMATION,
        };

        let mut diag = diag()
            .message(pragma.message.to_string())
            .desc(&WarnPragmaLint)
            .range(path_expr.get_span(db))
            .severity(severity)
            .call();

        diag.with_related(Related::new(
            "notice emitted here".into(),
            callable.get_scope_id(db).file(db),
            callable.get_name_span(db),
        ));

        diagnostics.push(diag);
    }
}
