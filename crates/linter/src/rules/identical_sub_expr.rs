use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::expression::{
        BeginPathExpr, BooleanOperatorKind, Expr, ExprKind, PrimaryExpr, VariableAccess,
        VariableAccessKind,
    },
    hir_ty::{body::BodyInferenceResult, expr_store::PathExprWalkStep},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "identical-sub-expr";

struct IdenticalSubExpr;

impl ErrorCode for IdenticalSubExpr {
    fn code(&self) -> &'static str {
        "L0311"
    }

    fn description(&self) -> &'static str {
        "identical subexpressions"
    }
}

/// Check a single node for `a AND a`, `a OR a`, `a XOR a`. No recursion.
pub fn check_node<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
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
    if !same_expr(db, body, left, right) {
        return;
    }
    let op = operator.as_str();
    let hint = match operator {
        BooleanOperatorKind::And => "result is always the same as either operand",
        BooleanOperatorKind::Or => "result is always the same as either operand",
        BooleanOperatorKind::Xor => "result is always FALSE",
    };
    diagnostics.push(
        diag()
            .message(format!(
                "identical expressions on both sides of '{op}', {hint}"
            ))
            .desc(&IdenticalSubExpr)
            .range(
                hir::denormalize(db, expr.get_scope_id(db).file(db), &expr.get_span(db))
                    .unwrap_or_default(),
            )
            .severity(DiagnosticSeverity::WARNING)
            .call(),
    );
}

fn same_expr<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    left: &Expr<'db>,
    right: &Expr<'db>,
) -> bool {
    match (left.expr(db), right.expr(db)) {
        (
            ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(la)),
            ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(ra)),
        ) => same_access(db, body, *la, *ra),
        (
            ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr: le }),
            ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr: re }),
        ) => same_expr(db, body, le, re),
        (ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr: le }), _) => {
            same_expr(db, body, le, right)
        }
        (_, ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr: re })) => {
            same_expr(db, body, left, re)
        }
        (
            ExprKind::UnaryOperator {
                expr: le,
                operator: lo,
            },
            ExprKind::UnaryOperator {
                expr: re,
                operator: ro,
            },
        ) => lo == ro && same_expr(db, body, le, re),
        _ => false,
    }
}

/// Whether two accesses name the same place.
///
/// Every step of the path has to agree, not only where it ends: `a.Q` and
/// `b.Q` end at ONE declaration of `Q`, the one their shared block declares,
/// so comparing the destination alone reads two instances as one. The bit
/// selector counts for the same reason — `w.0` and `w.1` are one declaration
/// and two places.
fn same_access<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    left: VariableAccess<'db>,
    right: VariableAccess<'db>,
) -> bool {
    if left.multibits(db) != right.multibits(db) {
        return false;
    }
    match (left.kind(db), right.kind(db)) {
        (VariableAccessKind::Direct(l), VariableAccessKind::Direct(r)) => l == r,
        (VariableAccessKind::Symbolic(l), VariableAccessKind::Symbolic(r)) => {
            same_path(db, body, l, r)
        }
        _ => false,
    }
}

/// Step by step, by what each step resolved to. An INDEX step is never taken
/// as the same place: the subscripts decide it, and `cells[i]` and `cells[j]`
/// would otherwise read alike. Missing one warning is the safe direction for
/// a lint.
fn same_path<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    left: BeginPathExpr<'db>,
    right: BeginPathExpr<'db>,
) -> bool {
    // `THIS`/`SUPER` and a call are not places to compare.
    if left.invocation(db).is_some() || right.invocation(db).is_some() {
        return false;
    }
    let (Some(left), Some(right)) = (left.expr(db), right.expr(db)) else {
        return false;
    };
    let (steps, others) = (left.flatten(db), right.flatten(db));
    if steps.len() != others.len() {
        return false;
    }
    steps.iter().zip(others.iter()).all(|(step, other)| {
        match (step, other) {
            (
                PathExprWalkStep::Field { ident: l, .. },
                PathExprWalkStep::Field { ident: r, .. },
            ) => {
                // Interned, so this is an integer comparison; the resolved
                // type then separates two declarations of one name.
                l.ident == r.ident
                    && body.type_of_path_expr.get(&step.get_expr(db))
                        == body.type_of_path_expr.get(&other.get_expr(db))
            }
            (
                PathExprWalkStep::Deref { count: l, .. },
                PathExprWalkStep::Deref { count: r, .. },
            ) => l == r,
            _ => false,
        }
    })
}
