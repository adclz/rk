use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{HirNodeInfo, hir_def::expressions::expression::Expr};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

use super::for_bound_at_type_limit::fold_const_int;

pub const NAME: &str = "nonconstant-for-step";

/// L0320: BY step is not a compile-time literal — the loop direction is
/// already fixed (ascending), whatever the runtime value's sign.
struct NonconstantForStep;

impl ErrorCode for NonconstantForStep {
    fn code(&self) -> &'static str {
        "L0320"
    }

    fn description(&self) -> &'static str {
        "non-constant FOR step"
    }
}

/// A step that is not a literal cannot prove its sign, and codegen decided
/// the loop's direction at compile time (a non-constant step compares
/// ascending) — so a negative runtime value will not run the loop
/// backwards. The lint states that deviation exactly where someone would
/// rely on it.
pub fn check_step<'db>(
    db: &'db dyn WorkspaceDataBase,
    step: &Expr<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    if fold_const_int(db, step).is_some() {
        return;
    }
    diagnostics.push(
        diag()
            .message(
                "the BY step is not a compile-time literal: the loop's direction is \
                 decided at compile time (ascending), so a negative value at runtime \
                 will not run the loop backwards"
                    .to_string(),
            )
            .desc(&NonconstantForStep)
            .range(
                hir::denormalize(db, step.get_scope_id(db).file(db), &step.get_span(db))
                    .unwrap_or_default(),
            )
            .severity(DiagnosticSeverity::WARNING)
            .call(),
    );
}
