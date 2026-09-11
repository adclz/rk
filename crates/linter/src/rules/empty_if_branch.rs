use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::{expression::Expr, statement::Stmt},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "empty-if-branch";

/// L0306: IF, ELSIF, or ELSE branch with no statements.
struct EmptyIfBranch;

impl ErrorCode for EmptyIfBranch {
    fn code(&self) -> &'static str {
        "L0306"
    }

    fn description(&self) -> &'static str {
        "empty IF branch"
    }
}

pub fn check_if<'db>(
    db: &'db dyn WorkspaceDataBase,
    stmt: Stmt<'db>,
    condition: &Expr<'db>,
    then: &Option<Vec<Stmt<'db>>>,
    else_if: &[(Expr<'db>, Vec<Stmt<'db>>)],
    else_: &Option<Vec<Stmt<'db>>>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    // Check empty THEN
    let then_empty = match then {
        None => true,
        Some(v) => v.is_empty(),
    };
    if then_empty {
        diagnostics.push(
            diag()
                .message("IF branch has no statements".to_string())
                .desc(&EmptyIfBranch)
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

    // Check empty ELSIF branches
    for (cond, stmts) in else_if {
        if stmts.is_empty() {
            diagnostics.push(
                diag()
                    .message("ELSIF branch has no statements".to_string())
                    .desc(&EmptyIfBranch)
                    .range(
                        hir::denormalize(db, cond.get_scope_id(db).file(db), &cond.get_span(db))
                            .unwrap_or_default(),
                    )
                    .severity(DiagnosticSeverity::HINT)
                    .call(),
            );
        }
    }

    // Check empty ELSE
    if matches!(else_, Some(v) if v.is_empty()) {
        diagnostics.push(
            diag()
                .message("ELSE branch has no statements".to_string())
                .desc(&EmptyIfBranch)
                .range(
                    hir::denormalize(db, stmt.get_scope_id(db).file(db), &stmt.get_span(db))
                        .unwrap_or_default(),
                )
                .severity(DiagnosticSeverity::HINT)
                .call(),
        );
    }
}
