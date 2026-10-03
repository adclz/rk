// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{HasName, HirNodeInfo, hir_ty::body::ScopeInference};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

pub const NAME: &str = "shadowing-variable";

/// L0202: variable name shadows a POU (function, function block, class, etc.)
struct ShadowingVariable;

impl ErrorCode for ShadowingVariable {
    fn code(&self) -> &'static str {
        "L0202"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    for (var, pou) in body.variables_shadowing() {
        let var_name = var.get_name_with_case(db).text(db);
        // Each as it is declared: the two may differ in case.
        let pou_name = pou.get_name_with_case(db).text(db);

        let mut diag = diag()
            .message(format!(
                "variable '{var_name}' has the name of the POU '{pou_name}', in scope here"
            ))
            .severity(DiagnosticSeverity::INFORMATION)
            .desc(&ShadowingVariable)
            .range(
                hir::denormalize(db, var.get_scope_id(db).file(db), &var.get_name_span(db))
                    .unwrap_or_default(),
            )
            .call();

        diag.with_related(Related::new(
            format!("POU '{pou_name}' is declared here"),
            pou.get_scope_id(db).file(db),
            pou.get_name_span(db),
        ));

        diagnostics.push(diag);
    }
}
