// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::expression::{ComparisonOperatorKind, Expr, ExprKind},
    hir_ty::body::ScopeInference,
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

use super::{division_by_zero::is_zero_literal, self_comparison::resolve_variable};

pub const NAME: &str = "float-equality";

/// L0124: two REAL or LREAL values compared with `=` or `<>`.
struct FloatEquality;

impl ErrorCode for FloatEquality {
    fn code(&self) -> &'static str {
        "L0124"
    }
}

pub fn check_node<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    expr: &Expr<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let ExprKind::ComparisonOperator {
        left,
        operator,
        right,
    } = expr.expr(db)
    else {
        return;
    };
    if !matches!(
        operator,
        ComparisonOperatorKind::Eq | ComparisonOperatorKind::Ne
    ) {
        return;
    }
    // Inference records the type only when both sides have one; against an
    // untyped literal, `x = 0.3`, the other side's type is the comparison's.
    let Some(compared_at) = body
        .comparison_operand_type(*expr)
        .into_iter()
        .chain([left, right].map(|side| body.type_of_expr_adjusted(*side).normalize(db)))
        .find(|ty| ty.is_float())
    else {
        return;
    };
    // Zero is exact, `IF d <> 0.0 THEN` guards a division.
    if is_zero_literal(db, left) || is_zero_literal(db, right) {
        return;
    }
    // `x <> x` holds for a NaN only: it is the NaN test.
    if let (Some(l), Some(r)) = (
        resolve_variable(db, body, left),
        resolve_variable(db, body, right),
    ) && l == r
    {
        return;
    }
    let mut d = diag()
        .message(format!(
            "{} values compared with '{}'",
            compared_at.type_name(db),
            operator.as_str()
        ))
        .desc(&FloatEquality)
        .range(
            hir::denormalize(db, expr.get_scope_id(db).file(db), &expr.get_span(db))
                .unwrap_or_default(),
        )
        .severity(DiagnosticSeverity::WARNING)
        .call();
    d.with_note(
        "a REAL holds the nearest binary fraction, so 1.1 + 2.2 = 3.3 is FALSE".to_string(),
    );
    d.with_help(format!(
        "compare with a tolerance if one is acceptable, '{}'",
        with_tolerance(db, left, operator, right)
    ));
    diagnostics.push(d);
}

/// The comparison rewritten against a tolerance, from its own operands:
/// `x = 0.3` is `ABS(x - 0.3) <= tolerance`, `x <> y` is `ABS(x - y) >
/// tolerance`. The tolerance is the values' own, in their unit: no number
/// fits them all, and from 16 on two REALs are further apart than 1.0E-6.
fn with_tolerance<'db>(
    db: &'db dyn WorkspaceDataBase,
    left: &Expr<'db>,
    operator: &ComparisonOperatorKind,
    right: &Expr<'db>,
) -> String {
    let left = left.as_call_site(db).to_string(db);
    // `a = b - c` is `ABS(a - (b - c))`.
    let right = match right.expr(db) {
        ExprKind::AddOperator { .. } | ExprKind::UnaryOperator { .. } => {
            format!("({})", right.as_call_site(db).to_string(db))
        }
        _ => right.as_call_site(db).to_string(db).to_string(),
    };
    let within = match operator {
        ComparisonOperatorKind::Ne => ">",
        _ => "<=",
    };
    format!("ABS({left} - {right}) {within} tolerance")
}
