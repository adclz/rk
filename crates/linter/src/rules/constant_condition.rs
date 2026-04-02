use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{Elementary, Expr, ExprKind, PrimaryExpr},
            statement::{Stmt, StmtKind},
        },
        pous::pou::Pou,
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "constant-condition";

/// L0112: condition is always true or always false.
struct ConstantCondition;

impl ErrorCode for ConstantCondition {
    fn code(&self) -> &'static str {
        "L0112"
    }

    fn description(&self) -> &'static str {
        "constant condition"
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
            StmtKind::If {
                condition,
                then,
                else_if,
                else_,
                ..
            } => {
                check_condition(db, condition, "IF", diagnostics);
                if let Some(stmts) = then {
                    check_statements(db, stmts, diagnostics);
                }
                for (cond, stmts) in else_if {
                    check_condition(db, cond, "ELSIF", diagnostics);
                    check_statements(db, stmts, diagnostics);
                }
                if let Some(stmts) = else_ {
                    check_statements(db, stmts, diagnostics);
                }
            }
            StmtKind::While {
                condition, body, ..
            } => {
                check_condition(db, condition, "WHILE", diagnostics);
                check_statements(db, body, diagnostics);
            }
            StmtKind::Repeat {
                condition, body, ..
            } => {
                check_condition(db, condition, "UNTIL", diagnostics);
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

pub fn check_condition<'db>(
    db: &'db dyn WorkspaceDataBase,
    condition: &Expr<'db>,
    keyword: &str,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let value_str = match is_boolean_literal(db, condition) {
        Some(value_str) => value_str,
        _ => return,
    };

    diagnostics.push(
        diag()
            .message(format!("{keyword} condition is always {value_str}",))
            .desc(&ConstantCondition)
            .range(condition.get_span(db))
            .severity(DiagnosticSeverity::WARNING)
            .call(),
    );
}

/// Returns `Some(true)` for TRUE, `Some(false)` for FALSE, `None` for non-boolean-literal.
fn is_boolean_literal<'db>(db: &'db dyn WorkspaceDataBase, expr: &Expr<'db>) -> Option<String> {
    match expr.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(Elementary::Bool(ident))) => {
            Some(ident.text(db).to_owned().to_uppercase().to_string())
        }
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => {
            is_boolean_literal(db, expr)
        }
        _ => None,
    }
}
