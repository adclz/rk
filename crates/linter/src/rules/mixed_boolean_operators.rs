// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::expression::{Expr, ExprKind},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "mixed-boolean-operators";

/// L0314: AND, XOR and OR mixed in one expression without parentheses.
struct MixedBooleanOperators;

impl ErrorCode for MixedBooleanOperators {
    fn code(&self) -> &'static str {
        "L0314"
    }
}

/// `a AND b OR c`: the OR's operand `a AND b` is another operator's,
/// written bare. Reported on each such operand, so `a AND b OR c AND d`
/// gets two.
pub fn check_node<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: &Expr<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let ExprKind::BooleanOperator {
        left,
        operator,
        right,
    } = expr.expr(db)
    else {
        return;
    };
    for operand in [left, right] {
        let ExprKind::BooleanOperator {
            operator: inner, ..
        } = operand.expr(db)
        else {
            continue;
        };
        if inner == operator {
            continue;
        }
        let text = operand.as_call_site(db).to_string(db);
        let mut d = diag()
            .message(format!(
                "'{text}' is an operand of {operator} without parentheses"
            ))
            .desc(&MixedBooleanOperators)
            .range(
                hir::denormalize(db, operand.get_scope_id(db).file(db), &operand.get_span(db))
                    .unwrap_or_default(),
            )
            .severity(DiagnosticSeverity::HINT)
            .call();
        d.with_note("AND is applied first, then XOR, then OR".to_string());
        d.with_help(format!("write '({text})'"));
        diagnostics.push(d);
    }
}
