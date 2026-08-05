use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::expressions::expression::{
        AddOperatorKind, Elementary, Expr, ExprKind, MultOperatorKind, PrimaryExpr,
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "identity-operation";

/// L0312: operation with identity value has no effect.
struct IdentityOperation;

impl ErrorCode for IdentityOperation {
    fn code(&self) -> &'static str {
        "L0312"
    }

    fn description(&self) -> &'static str {
        "identity operation"
    }
}

pub fn check_node<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: &Expr<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    match expr.expr(db) {
        ExprKind::MultOperator {
            left,
            operator,
            right,
        } => match operator {
            MultOperatorKind::Mul => {
                if is_one(db, right) {
                    emit(db, expr, "* 1", diagnostics);
                } else if is_one(db, left) {
                    emit(db, expr, "1 *", diagnostics);
                }
            }
            MultOperatorKind::Div if is_one(db, right) => {
                emit(db, expr, "/ 1", diagnostics);
            }
            _ => {}
        },
        ExprKind::AddOperator {
            left,
            operator,
            right,
        } => match operator {
            AddOperatorKind::Plus => {
                if is_zero(db, right) {
                    emit(db, expr, "+ 0", diagnostics);
                } else if is_zero(db, left) {
                    emit(db, expr, "0 +", diagnostics);
                }
            }
            AddOperatorKind::Minus => {
                if is_zero(db, right) {
                    emit(db, expr, "- 0", diagnostics);
                }
            }
        },
        _ => {}
    }
}

fn emit<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: &Expr<'db>,
    op_desc: &str,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    diagnostics.push(
        diag()
            .message(format!(
                "'{op_desc}' has no effect, the result is always the same as the other operand"
            ))
            .desc(&IdentityOperation)
            .range(
                hir::denormalize(db, expr.get_scope_id(db).file(db), &expr.get_span(db))
                    .unwrap_or_default(),
            )
            .severity(DiagnosticSeverity::WARNING)
            .call(),
    );
}

fn is_one<'db>(db: &'db dyn WorkspaceDataBase, expr: &Expr<'db>) -> bool {
    match expr.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(lit)) => is_one_elementary(db, lit),
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => is_one(db, expr),
        _ => false,
    }
}

fn is_one_elementary(db: &dyn WorkspaceDataBase, lit: &Elementary) -> bool {
    match lit {
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
        | Elementary::LWord(i) => i.as_u64(db) == Ok(1),
        Elementary::Real(ident) | Elementary::LReal(ident) | Elementary::InferFloat(ident) => {
            ident.text(db).trim().parse::<f64>() == Ok(1.0)
        }
        _ => false,
    }
}

fn is_zero<'db>(db: &'db dyn WorkspaceDataBase, expr: &Expr<'db>) -> bool {
    match expr.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(lit)) => is_zero_elementary(db, lit),
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => is_zero(db, expr),
        _ => false,
    }
}

fn is_zero_elementary(db: &dyn WorkspaceDataBase, lit: &Elementary) -> bool {
    match lit {
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
        | Elementary::LWord(i) => i.as_u64(db) == Ok(0),
        Elementary::Real(ident) | Elementary::LReal(ident) | Elementary::InferFloat(ident) => {
            ident.text(db).trim().parse::<f64>() == Ok(0.0)
        }
        _ => false,
    }
}
