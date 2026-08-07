use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::{expression::Expr, statement::Stmt},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "empty-loop-body";

/// L0213: FOR, WHILE, or REPEAT loop with no statements in the body.
struct EmptyLoopBody;

impl ErrorCode for EmptyLoopBody {
    fn code(&self) -> &'static str {
        "L0213"
    }

    fn description(&self) -> &'static str {
        "empty loop body"
    }
}

pub fn check_for<'db>(
    db: &'db dyn WorkspaceDataBase,
    stmt: Stmt<'db>,
    body: &[Stmt<'db>],
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    if body.is_empty() {
        diagnostics.push(
            diag()
                .message("FOR loop has no statements".to_string())
                .desc(&EmptyLoopBody)
                .range(
                    hir::denormalize(db, stmt.get_scope_id(db).file(db), &stmt.get_span(db))
                        .unwrap_or_default(),
                )
                .severity(DiagnosticSeverity::HINT)
                .call(),
        );
    }
}

pub fn check_while<'db>(
    db: &'db dyn WorkspaceDataBase,
    condition: &Expr<'db>,
    body: &[Stmt<'db>],
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    if body.is_empty() {
        diagnostics.push(
            diag()
                .message("WHILE loop has no statements".to_string())
                .desc(&EmptyLoopBody)
                .range(
                    hir::denormalize(
                        db,
                        condition.get_scope_id(db).file(db),
                        &condition.get_span(db),
                    )
                    .unwrap_or_default(),
                )
                .severity(DiagnosticSeverity::HINT)
                .call(),
        );
    }
}

pub fn check_repeat<'db>(
    db: &'db dyn WorkspaceDataBase,
    condition: &Expr<'db>,
    body: &[Stmt<'db>],
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    if body.is_empty() {
        diagnostics.push(
            diag()
                .message("REPEAT loop has no statements".to_string())
                .desc(&EmptyLoopBody)
                .range(
                    hir::denormalize(
                        db,
                        condition.get_scope_id(db).file(db),
                        &condition.get_span(db),
                    )
                    .unwrap_or_default(),
                )
                .severity(DiagnosticSeverity::HINT)
                .call(),
        );
    }
}
