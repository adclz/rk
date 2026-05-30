use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{ExprKind, PrimaryExpr, VariableAccess, VariableAccessKind},
            statement::{Stmt, StmtKind},
        },
        pous::pou::Pou,
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{body::BodyInferenceResult, ty::Type},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "self-assignment";

/// L0309: variable is assigned to itself.
struct SelfAssignment;

impl ErrorCode for SelfAssignment {
    fn code(&self) -> &'static str {
        "L0309"
    }

    fn description(&self) -> &'static str {
        "self-assignment"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    body: &BodyInferenceResult<'db>,
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

    check_statements(db, body, statements, diagnostics);
}

fn check_statements<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    stmts: &[Stmt<'db>],
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    for stmt in stmts {
        match stmt.stmt(db) {
            StmtKind::Assignment { var, target } => {
                check_assignment(db, body, *stmt, *var, *target, diagnostics);
            }
            StmtKind::If {
                then,
                else_if,
                else_,
                ..
            } => {
                if let Some(stmts) = then {
                    check_statements(db, body, stmts, diagnostics);
                }
                for (_, stmts) in else_if {
                    check_statements(db, body, stmts, diagnostics);
                }
                if let Some(stmts) = else_ {
                    check_statements(db, body, stmts, diagnostics);
                }
            }
            StmtKind::Case { cases, else_, .. } => {
                for (_, stmts) in cases {
                    check_statements(db, body, stmts, diagnostics);
                }
                if let Some(stmts) = else_ {
                    check_statements(db, body, stmts, diagnostics);
                }
            }
            StmtKind::For { body: stmts, .. }
            | StmtKind::While { body: stmts, .. }
            | StmtKind::Repeat { body: stmts, .. } => {
                check_statements(db, body, stmts, diagnostics);
            }
            _ => {}
        }
    }
}

pub fn check_assignment<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    stmt: Stmt<'db>,
    lhs: VariableAccess<'db>,
    rhs: hir::hir_def::expressions::expression::Expr<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    // RHS must be a simple variable access (not a function call, literal, etc.)
    let ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(rhs_var)) = rhs.expr(db) else {
        return;
    };

    // Both sides must be symbolic (not direct %I/%Q addresses)
    let VariableAccessKind::Symbolic(lhs_begin) = lhs.kind(db) else {
        return;
    };
    let VariableAccessKind::Symbolic(rhs_begin) = rhs_var.kind(db) else {
        return;
    };

    // Both must have a path expression (not an invocation like THIS/SUPER)
    let Some(lhs_path) = lhs_begin.expr(db) else {
        return;
    };
    let Some(rhs_path) = rhs_begin.expr(db) else {
        return;
    };

    // Compare resolved types — both must resolve to the same variable
    let Some(&Type::Variable((lhs_var, _))) = body.type_of_path_expr.get(&lhs_path) else {
        return;
    };
    let Some(&Type::Variable((rhs_var, _))) = body.type_of_path_expr.get(&rhs_path) else {
        return;
    };

    if lhs_var != rhs_var {
        return;
    }

    let name = lhs_var.name(db).text(db);
    diagnostics.push(
        diag()
            .message(format!("variable '{name}' is assigned to itself"))
            .desc(&SelfAssignment)
            .range(
                hir::denormalize(db, stmt.get_scope_id(db).file(db), &stmt.get_span(db))
                    .unwrap_or_default(),
            )
            .severity(DiagnosticSeverity::WARNING)
            .call(),
    );
}
