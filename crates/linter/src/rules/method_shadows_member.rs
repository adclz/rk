// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{HasName, HirNodeInfo, hir_ty::body::ScopeInference};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

pub const NAME: &str = "method-shadows-member";

/// L0116: a method's local/parameter has the same name as a member of the FB or
/// class it belongs to. This is legal — the method variable shadows the member,
/// and bare-name access inside the method resolves to the local (per IEC
/// and HIR name resolution) — but it is easy to misread, so warn. The shadow set
/// is computed in HIR (`ScopeInference::method_shadowed_members`).
struct MethodShadowsMember;

impl ErrorCode for MethodShadowsMember {
    fn code(&self) -> &'static str {
        "L0116"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    for (mvar, member) in body.method_shadowed_members() {
        let name = mvar.get_name_with_case(db).text(db);
        // Each as it is declared: the two may differ in case.
        let member_name = member.get_name_with_case(db).text(db);

        let mut d = diag()
            .message(format!(
                "method variable '{name}' hides the member '{member_name}' of its block"
            ))
            .desc(&MethodShadowsMember)
            .range(
                hir::denormalize(db, mvar.get_scope_id(db).file(db), &mvar.get_name_span(db))
                    .unwrap_or_default(),
            )
            .severity(DiagnosticSeverity::WARNING)
            .call();

        d.with_related(Related::new(
            format!("member '{member_name}' is declared here"),
            member.get_scope_id(db).file(db),
            member.get_name_span(db),
        ));

        diagnostics.push(d);
    }
}
