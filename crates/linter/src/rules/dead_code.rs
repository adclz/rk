// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::{DiagnosticSeverity, DiagnosticTag};
use db::WorkspaceDataBase;
use hir::{HirNodeInfo, hir_ty::body::ScopeInference};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "dead-code";

/// L0101: unreachable statement.
struct DeadCode;

impl ErrorCode for DeadCode {
    fn code(&self) -> &'static str {
        "L0101"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    for stmt in body.dead_code_statements() {
        diagnostics.push(
            diag()
                .message("unreachable statement".to_string())
                .desc(&DeadCode)
                .range(
                    hir::denormalize(db, stmt.get_scope_id(db).file(db), &stmt.get_span(db))
                        .unwrap_or_default(),
                )
                .severity(DiagnosticSeverity::WARNING)
                .tags(vec![DiagnosticTag::UNNECESSARY])
                .call(),
        );
    }
}
