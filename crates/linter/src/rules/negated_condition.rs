use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::{
        expression::{Expr, ExprKind, UnaryOperatorKind},
        statement::Stmt,
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

pub const NAME: &str = "negated-condition";

/// L0104: IF NOT ... THEN ... ELSE can be simplified by swapping branches.
struct NegatedCondition;

impl ErrorCode for NegatedCondition {
    fn code(&self) -> &'static str {
        "L0104"
    }

    fn description(&self) -> &'static str {
        "negated condition"
    }
}

/// Check if an IF statement has a negated condition with an ELSE branch.
/// Called by the unified visitor.
pub fn check_if<'db>(
    db: &'db dyn WorkspaceDataBase,
    condition: &Expr<'db>,
    then: &Option<Vec<Stmt<'db>>>,
    else_: &Option<Vec<Stmt<'db>>>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    // Only applies when there's an ELSE branch
    let Some(else_stmts) = else_ else {
        return;
    };
    if else_stmts.is_empty() {
        return;
    }

    // Check if the condition is NOT <expr>
    if !is_negated(db, condition) {
        return;
    }

    let Some(then_stmts) = then else {
        return;
    };
    if then_stmts.is_empty() {
        return;
    }
    let file = condition.get_scope_id(db).file(db);

    let mut d = diag()
        .message("remove NOT and swap the THEN and ELSE bodies".to_string())
        .desc(&NegatedCondition)
        .range(condition.get_span(db))
        .severity(DiagnosticSeverity::INFORMATION)
        .call();

    d.with_related(Related::new(
        "swap this".into(),
        file,
        then_stmts.first().unwrap().get_span(db),
    ));
    d.with_related(Related::new(
        "with this".into(),
        file,
        else_stmts.first().unwrap().get_span(db),
    ));

    diagnostics.push(d);
}

fn is_negated<'db>(db: &'db dyn WorkspaceDataBase, expr: &Expr<'db>) -> bool {
    match expr.expr(db) {
        ExprKind::UnaryOperator {
            operator: UnaryOperatorKind::Not,
            ..
        } => true,
        _ => false,
    }
}
