// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{HirNodeInfo, hir_ty::body::ScopeInference};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "effectless-statement";

/// L0310: statement has no effect.
struct EffectlessStatement;

impl ErrorCode for EffectlessStatement {
    fn code(&self) -> &'static str {
        "L0310"
    }

    fn description(&self) -> &'static str {
        "effectless statement"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    for stmt in body.effectless_statements() {
        diagnostics.push(
            diag()
                .message("statement has no effect".to_string())
                .desc(&EffectlessStatement)
                .range(
                    hir::denormalize(db, stmt.get_scope_id(db).file(db), &stmt.get_span(db))
                        .unwrap_or_default(),
                )
                .severity(DiagnosticSeverity::HINT)
                .call(),
        );
    }
}
