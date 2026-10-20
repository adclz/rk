// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::expression::{
        ComparisonOperatorKind, Elementary, Expr, ExprKind, PrimaryExpr,
    },
    hir_ty::{
        body::ScopeInference,
        infer::const_eval::{non_constant_part, resolve_constant_ref},
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "time-equality";

/// L0131: a time compared with `=` or `<>` against a constant.
struct TimeEquality;

impl ErrorCode for TimeEquality {
    fn code(&self) -> &'static str {
        "L0131"
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
    // A DATE counts whole days and holds a value all day long: it is not
    // reported. Against an untyped literal the other side's type is the
    // comparison's.
    let Some(compared_at) = body
        .comparison_operand_type(*expr)
        .into_iter()
        .chain([left, right].map(|side| body.type_of_expr_adjusted(*side).normalize(db)))
        .find(|ty| ty.is_time() || ty.is_tod() || ty.is_dt())
    else {
        return;
    };
    // Against a threshold: two variables compared are a change test, `pt <>
    // last_pt`, and two constants are L0103's.
    let constant = |side: &Expr<'db>| non_constant_part(db, *side).is_none();
    if constant(left) == constant(right) {
        return;
    }
    // A duration is zero exactly, as a timer's ET is before it starts.
    if is_zero_duration(db, left) || is_zero_duration(db, right) {
        return;
    }
    let mut d = diag()
        .message(format!(
            "{} values compared with '{}'",
            compared_at.type_name(db),
            operator.as_str()
        ))
        .desc(&TimeEquality)
        .range(
            hir::denormalize(db, expr.get_scope_id(db).file(db), &expr.get_span(db))
                .unwrap_or_default(),
        )
        .severity(DiagnosticSeverity::WARNING)
        .call();
    d.with_note(
        "a time read at each scan moves by a whole scan, and can step past the value without ever equalling it"
            .to_string(),
    );
    let (instead, either) = match operator {
        ComparisonOperatorKind::Ne => ("<", "'<' or '>'"),
        _ => (">=", "'>=' or '<='"),
    };
    // The time that moves first, `et >= T#5S` for `T#5S = et`.
    let (moving, threshold) = match constant(left) {
        true => (right, left),
        false => (left, right),
    };
    d.with_help(format!(
        "compare with {either}, '{} {instead} {}'",
        moving.as_call_site(db).to_string(db),
        threshold.as_call_site(db).to_string(db)
    ));
    diagnostics.push(d);
}

/// A TIME or LTIME literal of zero, written as one or held by a CONSTANT.
fn is_zero_duration<'db>(db: &'db dyn WorkspaceDataBase, expr: &Expr<'db>) -> bool {
    let expr = resolve_constant_ref(db, *expr).unwrap_or(*expr);
    match expr.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => {
            is_zero_duration(db, expr)
        }
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(Elementary::Time(text))) => {
            text.as_time(db).is_ok_and(|time| time.is_zero())
        }
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(Elementary::LTime(text))) => {
            text.as_ltime(db).is_ok_and(|time| time.is_zero())
        }
        _ => false,
    }
}
