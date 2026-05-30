use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::expression::{Elementary, Expr, ExprKind, PrimaryExpr},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "for-zero-step";

/// L0307: FOR loop with a step of 0 will loop forever.
struct ForZeroStep;

impl ErrorCode for ForZeroStep {
    fn code(&self) -> &'static str {
        "L0307"
    }

    fn description(&self) -> &'static str {
        "FOR loop with zero step"
    }
}

/// Check if a FOR step expression is a literal zero.
pub fn check_step<'db>(
    db: &'db dyn WorkspaceDataBase,
    step: &Expr<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    if is_zero(db, step) {
        diagnostics.push(
            diag()
                .message("FOR loop step is 0, loop will never terminate".to_string())
                .desc(&ForZeroStep)
                .range(
                    hir::denormalize(db, step.get_scope_id(db).file(db), &step.get_span(db))
                        .unwrap_or_default(),
                )
                .severity(DiagnosticSeverity::WARNING)
                .call(),
        );
    }
}

fn is_zero<'db>(db: &'db dyn WorkspaceDataBase, expr: &Expr<'db>) -> bool {
    match expr.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(
            Elementary::InferInteger(i)
            | Elementary::SInt(i)
            | Elementary::Int(i)
            | Elementary::DInt(i)
            | Elementary::LInt(i)
            | Elementary::USInt(i)
            | Elementary::UInt(i)
            | Elementary::UDInt(i)
            | Elementary::ULInt(i)
            | Elementary::Byte(i)
            | Elementary::Word(i)
            | Elementary::DWord(i)
            | Elementary::LWord(i),
        )) => i.as_u64(db) == Ok(0),
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => is_zero(db, expr),
        _ => false,
    }
}
