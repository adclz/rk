use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{Elementary, Expr, ExprKind, MultOperatorKind, PrimaryExpr},
            statement::{Stmt, StmtKind},
        },
        pous::pou::Pou,
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "division-by-zero";

struct DivisionByZero;

impl ErrorCode for DivisionByZero {
    fn code(&self) -> &'static str {
        "L0305"
    }

    fn description(&self) -> &'static str {
        "division by zero"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let statements = match get_scope(db, scope).kind {
        ScopeKind::Pou(pou) => match pou {
            Pou::Function(f) => f.statements(db),
            Pou::FunctionBlock(fb) => fb.statements(db),
            _ => return,
        },
        ScopeKind::MethodDecl(m) => m.stmts(db),
        ScopeKind::Program(program) => program.statements(db),
        _ => return,
    };

    check_statements(db, statements, diagnostics);
}

fn check_statements<'db>(
    db: &'db dyn WorkspaceDataBase,
    stmts: &[Stmt<'db>],
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    for stmt in stmts {
        match stmt.stmt(db) {
            StmtKind::Assignment { target, .. } => {
                check_expr(db, target, diagnostics);
            }
            StmtKind::If {
                condition,
                then,
                else_if,
                else_,
                ..
            } => {
                check_expr(db, condition, diagnostics);
                if let Some(stmts) = then {
                    check_statements(db, stmts, diagnostics);
                }
                for (cond, stmts) in else_if {
                    check_expr(db, cond, diagnostics);
                    check_statements(db, stmts, diagnostics);
                }
                if let Some(stmts) = else_ {
                    check_statements(db, stmts, diagnostics);
                }
            }
            StmtKind::While {
                condition, body, ..
            } => {
                check_expr(db, condition, diagnostics);
                check_statements(db, body, diagnostics);
            }
            StmtKind::Repeat {
                condition, body, ..
            } => {
                check_expr(db, condition, diagnostics);
                check_statements(db, body, diagnostics);
            }
            StmtKind::Case { cases, else_, .. } => {
                for (_, stmts) in cases {
                    check_statements(db, stmts, diagnostics);
                }
                if let Some(stmts) = else_ {
                    check_statements(db, stmts, diagnostics);
                }
            }
            StmtKind::For { body, .. } => {
                check_statements(db, body, diagnostics);
            }
            _ => {}
        }
    }
}

fn check_expr<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: &Expr<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    match expr.expr(db) {
        ExprKind::MultOperator {
            left,
            operator,
            right,
        } => {
            // Check left side recursively
            check_expr(db, left, diagnostics);
            // Check right side recursively
            check_expr(db, right, diagnostics);

            // Only flag / and MOD
            if matches!(operator, MultOperatorKind::Div | MultOperatorKind::Mod)
                && is_zero_literal(db, right)
            {
                let op = operator.as_str();
                diagnostics.push(
                    diag()
                        .message(format!("division by zero: right-hand side of '{op}' is 0"))
                        .desc(&DivisionByZero)
                        .range(
                            hir::denormalize(
                                db,
                                right.get_scope_id(db).file(db),
                                &right.get_span(db),
                            )
                            .unwrap_or_default(),
                        )
                        .severity(DiagnosticSeverity::WARNING)
                        .call(),
                );
            }
        }
        ExprKind::AddOperator { left, right, .. }
        | ExprKind::BooleanOperator { left, right, .. }
        | ExprKind::ComparisonOperator { left, right, .. }
        | ExprKind::PowerOperator { left, right } => {
            check_expr(db, left, diagnostics);
            check_expr(db, right, diagnostics);
        }
        ExprKind::UnaryOperator { expr, .. } => {
            check_expr(db, expr, diagnostics);
        }
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => {
            check_expr(db, expr, diagnostics);
        }
        _ => {}
    }
}

/// Check if an expression is a literal integer zero (0, in any base or type prefix).
fn is_zero_literal<'db>(db: &'db dyn WorkspaceDataBase, expr: &Expr<'db>) -> bool {
    match expr.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(lit)) => is_zero_elementary(db, lit),
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => is_zero_literal(db, expr),
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
