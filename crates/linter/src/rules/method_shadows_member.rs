use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{HasName, HirNodeInfo, hir_ty::body::BodyInferenceResult};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

pub const NAME: &str = "method-shadows-member";

/// L0318: a method's local/parameter has the same name as a member of the FB or
/// class it belongs to. This is legal — the method variable shadows the member,
/// and bare-name access inside the method resolves to the local (per IEC
/// and HIR name resolution) — but it is easy to misread, so warn. The shadow set
/// is computed in HIR (`BodyInferenceResult::method_shadowed_members`).
struct MethodShadowsMember;

impl ErrorCode for MethodShadowsMember {
    fn code(&self) -> &'static str {
        "L0318"
    }

    fn description(&self) -> &'static str {
        "method variable shadows an owner member"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    for (mvar, member) in &body.method_shadowed_members {
        let name = mvar.get_name_ident(db).text(db);

        let mut d = diag()
            .message(format!(
                "method variable '{name}' shadows the member '{name}' of its FB/class"
            ))
            .desc(&MethodShadowsMember)
            .range(
                hir::denormalize(db, mvar.get_scope_id(db).file(db), &mvar.get_name_span(db))
                    .unwrap_or_default(),
            )
            .severity(DiagnosticSeverity::WARNING)
            .call();

        d.with_related(Related::new(
            format!("member '{name}' is declared here"),
            member.get_scope_id(db).file(db),
            member.get_name_span(db),
        ));

        diagnostics.push(d);
    }
}
