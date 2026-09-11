use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::expression::{Elementary, Expr, ExprKind, PrimaryExpr},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "default-for-step";

/// L0213: FOR loop step of 1 is the default and can be omitted.
struct DefaultForStep;

impl ErrorCode for DefaultForStep {
    fn code(&self) -> &'static str {
        "L0213"
    }

    fn description(&self) -> &'static str {
        "redundant FOR loop step"
    }
}

pub fn check_step<'db>(
    db: &'db dyn WorkspaceDataBase,
    step: &Expr<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    if is_one(db, step) {
        diagnostics.push(
            diag()
                .message("FOR loop step of 1 is the default and can be omitted".to_string())
                .desc(&DefaultForStep)
                .range(
                    hir::denormalize(db, step.get_scope_id(db).file(db), &step.get_span(db))
                        .unwrap_or_default(),
                )
                .severity(DiagnosticSeverity::HINT)
                .call(),
        );
    }
}

fn is_one<'db>(db: &'db dyn WorkspaceDataBase, expr: &Expr<'db>) -> bool {
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
        )) => i.as_u64(db) == Ok(1),
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => is_one(db, expr),
        _ => false,
    }
}
