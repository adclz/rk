// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{HirNodeInfo, hir_ty::body::ScopeInference};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "unused-return-type";

/// L0302: function call discards a return value.
struct UnusedReturnType;

impl ErrorCode for UnusedReturnType {
    fn code(&self) -> &'static str {
        "L0302"
    }

    fn description(&self) -> &'static str {
        "unused return value"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    for (stmt, typ) in body.unused_return_types() {
        let mut diag = diag()
            .message(format!("unused return value of '{}'", typ.type_name(db)))
            .desc(&UnusedReturnType)
            .range(
                hir::denormalize(db, stmt.get_scope_id(db).file(db), &stmt.get_span(db))
                    .unwrap_or_default(),
            )
            .severity(DiagnosticSeverity::HINT)
            .call();

        typ.with_location(db, &mut diag);

        diagnostics.push(diag);
    }
}
