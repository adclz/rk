// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{HirNodeInfo, hir_def::expressions::statement::StmtKind, hir_ty::body::ScopeInference};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "for-loop-step-sign";

/// L0110: FOR loop step direction mismatches bounds.
struct ForLoopStepSign;

impl ErrorCode for ForLoopStepSign {
    fn code(&self) -> &'static str {
        "L0110"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    for stmt in body.mismatched_for_step() {
        // The step, where one is written; the bounds where it is left to 1.
        let span = match stmt.stmt(db) {
            StmtKind::For {
                step: Some(step), ..
            } => step.get_span(db),
            StmtKind::For { start, end, .. } => {
                let (from, to) = (start.get_span(db), end.get_span(db));
                auto_lsp::tree_sitter::Range {
                    start_byte: from.start_byte,
                    end_byte: to.end_byte,
                    start_point: from.start_point,
                    end_point: to.end_point,
                }
            }
            _ => stmt.get_span(db),
        };
        diagnostics.push(
            diag()
                .message(
                    "the sign of the step does not match the direction of the bounds".to_string(),
                )
                .desc(&ForLoopStepSign)
                .range(
                    hir::denormalize(db, stmt.get_scope_id(db).file(db), &span).unwrap_or_default(),
                )
                .severity(DiagnosticSeverity::WARNING)
                .call(),
        );
    }
}
