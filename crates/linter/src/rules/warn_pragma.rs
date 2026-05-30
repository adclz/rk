use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::pous::pragma::WarnPragmaLevel,
    hir_ty::{body::BodyInferenceResult, ty::Type},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

pub const NAME: &str = "warn-pragma";

struct InfoPragma;

impl ErrorCode for InfoPragma {
    fn code(&self) -> &'static str {
        "L0001"
    }

    fn description(&self) -> &'static str {
        "call site info notice"
    }
}

struct WarnPragma;

impl ErrorCode for WarnPragma {
    fn code(&self) -> &'static str {
        "L0002"
    }

    fn description(&self) -> &'static str {
        "call site warning notice"
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

        let Some((pragma_span, pragma)) = callable.warn_pragma(db) else {
            continue;
        };

        let mut diag = match pragma.level {
            WarnPragmaLevel::Warn => diag()
                .message(pragma.message.to_string())
                .desc(&WarnPragma)
                .range(
                    hir::denormalize(
                        db,
                        path_expr.get_scope_id(db).file(db),
                        &path_expr.get_span(db),
                    )
                    .unwrap_or_default(),
                )
                .severity(DiagnosticSeverity::WARNING)
                .call(),
            WarnPragmaLevel::Info => diag()
                .message(pragma.message.to_string())
                .desc(&InfoPragma)
                .range(
                    hir::denormalize(
                        db,
                        path_expr.get_scope_id(db).file(db),
                        &path_expr.get_span(db),
                    )
                    .unwrap_or_default(),
                )
                .severity(DiagnosticSeverity::INFORMATION)
                .call(),
        };

        diag.with_related(Related::new(
            "pragma declared here".into(),
            pragma_span.get_scope_id(db).file(db),
            pragma_span.get_span(db),
        ));

        diagnostics.push(diag);
    }
}
