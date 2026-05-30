use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{HirNodeInfo, hir_def::expressions::expression::Expr};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "constant-loop-bounds";

/// L0314: FOR loop start equals end, always exactly one iteration.
struct ConstantLoopBounds;

impl ErrorCode for ConstantLoopBounds {
    fn code(&self) -> &'static str {
        "L0314"
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

    if start_text == end_text {
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
