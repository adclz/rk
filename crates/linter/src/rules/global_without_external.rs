use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{HasName, HirNodeInfo, hir_ty::body::BodyInferenceResult};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

pub const NAME: &str = "global-without-external";

/// L0410: a config/resource VAR_GLOBAL is accessed directly by name without a
/// matching VAR_EXTERNAL declaration in the POU.
/// This is allowed, but strict IEC 61131-3 wants the global imported via
/// VAR_EXTERNAL.
struct GlobalWithoutExternal;

impl ErrorCode for GlobalWithoutExternal {
    fn code(&self) -> &'static str {
        "L0410"
    }

    fn description(&self) -> &'static str {
        "global accessed without VAR_EXTERNAL"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    for (access, global) in &body.globals_without_external {
        let name = global.get_name_ident(db).text(db);
        let access_file = access.get_scope_id(db).file(db);

        let mut d = diag()
            .message(format!(
                "global '{name}' is accessed without a VAR_EXTERNAL declaration"
            ))
            .desc(&GlobalWithoutExternal)
            .range(hir::denormalize(db, access_file, &access.get_span(db)).unwrap_or_default())
            .severity(DiagnosticSeverity::WARNING)
            .call();

        d.with_related(Related::new(
            format!("global '{name}' is declared here"),
            global.get_scope_id(db).file(db),
            global.get_name_span(db),
        ));

        diagnostics.push(d);
    }
}
