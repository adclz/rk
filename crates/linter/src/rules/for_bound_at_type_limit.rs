use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::{
        expression::{Expr, ExprKind, PrimaryExpr, UnaryOperatorKind, VariableAccess},
        spec::ElementarySpec,
    },
    hir_ty::{body::BodyInferenceResult, ty::Type},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "for-bound-at-type-limit";

/// L0319: FOR bound at the control type's wrap limit — the loop never
/// terminates.
struct ForBoundAtTypeLimit;

impl ErrorCode for ForBoundAtTypeLimit {
    fn code(&self) -> &'static str {
        "L0319"
    }

    fn description(&self) -> &'static str {
        "FOR loop never terminates"
    }
}

/// The wrap footgun: with the end bound at the control type's own limit,
/// the exit check can never be true — the counter wraps at the type width
/// first (codegen wraps sub-width arithmetic deliberately — so
/// `FOR i : USINT := 0 TO 255` runs forever. Ascending checks the type's
/// max; a constant-negative step checks its min.
pub fn check_for<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    control_variable: &VariableAccess<'db>,
    end: &Expr<'db>,
    step: Option<&Expr<'db>>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let Some(end_val) = fold_const_int(db, end) else {
        return;
    };
    // A SUBRANGE counter wraps at its BASE type's width, so the limit comes
    // from the peeled elementary type — `TO 100` on `INT (0..100)` is fine.
    let ctrl = body
        .type_of_variable_access_with_adjustments(db, *control_variable)
        .peel_subrange(db);
    let Type::Elementary(spec) = ctrl else {
        return;
    };
    let Some((min, max)) = integer_type_limits(spec) else {
        return;
    };
    let descending = step.and_then(|s| fold_const_int(db, s)).is_some_and(|s| s < 0);
    let limit = if descending { min } else { max };
    if end_val != limit {
        return;
    }
    diagnostics.push(
        diag()
            .message(format!(
                "this FOR loop never terminates: the bound {end_val} is {}'s own limit, \
                 so the counter wraps before the exit check can fail",
                ctrl.type_name(db)
            ))
            .desc(&ForBoundAtTypeLimit)
            .range(
                hir::denormalize(db, end.get_scope_id(db).file(db), &end.get_span(db))
                    .unwrap_or_default(),
            )
            .severity(DiagnosticSeverity::WARNING)
            .call(),
    );
}

/// A FOR bound/step as a compile-time integer: every typed and untyped
/// literal form (`255`, `USINT#255`, `16#FF`) via [`Expr::as_const_int`],
/// plus a unary sign and parentheses. Named constants are not folded — a
/// bound behind one is treated as non-constant, exactly as MIR treats it.
pub(crate) fn fold_const_int(db: &dyn WorkspaceDataBase, expr: &Expr<'_>) -> Option<i64> {
    match expr.expr(db) {
        ExprKind::UnaryOperator {
            expr: inner,
            operator,
        } => match operator {
            UnaryOperatorKind::Minus => fold_const_int(db, inner).and_then(i64::checked_neg),
            UnaryOperatorKind::Plus => fold_const_int(db, inner),
            _ => None,
        },
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr: inner }) => {
            fold_const_int(db, inner)
        }
        _ => expr.as_const_int(db),
    }
}

/// The `(min, max)` an integer elementary type can hold — a FOR counter's
/// wrap limits. `None` for non-integers, and for ULINT, whose maximum
/// exceeds `i64` and so can never equal a folded bound here.
fn integer_type_limits(spec: ElementarySpec) -> Option<(i64, i64)> {
    Some(match spec {
        ElementarySpec::SInt => (i8::MIN as i64, i8::MAX as i64),
        ElementarySpec::Int => (i16::MIN as i64, i16::MAX as i64),
        ElementarySpec::DInt => (i32::MIN as i64, i32::MAX as i64),
        ElementarySpec::LInt => (i64::MIN, i64::MAX),
        ElementarySpec::USInt => (0, u8::MAX as i64),
        ElementarySpec::UInt => (0, u16::MAX as i64),
        ElementarySpec::UDInt => (0, u32::MAX as i64),
        _ => return None,
    })
}
