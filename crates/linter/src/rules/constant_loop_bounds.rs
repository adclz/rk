// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{HirNodeInfo, hir_def::expressions::expression::Expr};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "constant-loop-bounds";

/// L0111: FOR loop start equals end, always exactly one iteration.
struct ConstantLoopBounds;

impl ErrorCode for ConstantLoopBounds {
    fn code(&self) -> &'static str {
        "L0111"
    }

    fn description(&self) -> &'static str {
        "constant FOR loop bounds"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    start: &Expr<'db>,
    end: &Expr<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let start_text = start.as_call_site(db).to_string(db);
    let end_text = end.as_call_site(db).to_string(db);

    // With case out of the way: `n` and `N` are one bound, `16#ff` and
    // `16#FF` one value.
    if start_text.to_lowercase() == end_text.to_lowercase() {
        diagnostics.push(
            diag()
                .message(format!(
                    "FOR loop bounds are equal (both {start_text}), loop body executes exactly once"
                ))
                .desc(&ConstantLoopBounds)
                .range(
                    hir::denormalize(db, start.get_scope_id(db).file(db), &start.get_span(db))
                        .unwrap_or_default(),
                )
                .severity(DiagnosticSeverity::WARNING)
                .call(),
        );
    }
}
