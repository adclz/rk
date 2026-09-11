use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{HirNodeInfo, hir_def::expressions::statement::CaseKind};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "empty-case-branch";

/// L0307: CASE branch with no statements.
struct EmptyCaseBranch;

impl ErrorCode for EmptyCaseBranch {
    fn code(&self) -> &'static str {
        "L0307"
    }

    fn description(&self) -> &'static str {
        "empty CASE branch"
    }
}

/// Check a CASE statement's branches for empty bodies.
pub fn check_case<'db>(
    db: &'db dyn WorkspaceDataBase,
    cases: &[(
        Vec<CaseKind<'db>>,
        Vec<hir::hir_def::expressions::statement::Stmt<'db>>,
    )],
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    for (selectors, stmts) in cases {
        if stmts.is_empty() {
            // Use the span of the first selector for the diagnostic
            if let Some(first) = selectors.first() {
                let (span, file) = match first {
                    CaseKind::Expression(expr) => {
                        (expr.get_span(db), expr.get_scope_id(db).file(db))
                    }
                    CaseKind::Subrange { lower, .. } => {
                        (lower.get_span(db), lower.get_scope_id(db).file(db))
                    }
                };
                diagnostics.push(
                    diag()
                        .message("CASE branch has no statements".to_string())
                        .desc(&EmptyCaseBranch)
                        .range(hir::denormalize(db, file, &span).unwrap_or_default())
                        .severity(DiagnosticSeverity::HINT)
                        .call(),
                );
            }
        }
    }
}
