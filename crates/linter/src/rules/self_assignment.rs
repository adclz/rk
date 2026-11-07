// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

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
    hir_ty::{body::ScopeInference, ty::Type},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "self-assignment";

/// L0104: variable is assigned to itself.
struct SelfAssignment;

impl ErrorCode for SelfAssignment {
    fn code(&self) -> &'static str {
        "L0104"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    body: ScopeInference<'db>,
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
    body: ScopeInference<'db>,
    stmts: &[Stmt<'db>],
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    for stmt in stmts {
        match stmt.stmt(db) {
            StmtKind::Assignment { vars, target, .. } => {
                check_chain(db, body, *stmt, vars, *target, diagnostics);
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
            StmtKind::Try {
                body: stmts,
                catch,
                finally,
            } => {
                check_statements(db, body, stmts, diagnostics);
                if let Some(catch) = catch {
                    check_statements(db, body, &catch.body, diagnostics);
                }
                if let Some(finally) = finally {
                    check_statements(db, body, finally, diagnostics);
                }
            }
            _ => {}
        }
    }
}

/// Each link of `a := b := e`: the last place with the value, and each other
/// place with the place after it, whose value it takes.
pub fn check_chain<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    stmt: Stmt<'db>,
    vars: &[VariableAccess<'db>],
    rhs: hir::hir_def::expressions::expression::Expr<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let Some((&last, links)) = vars.split_last() else {
        return;
    };
    check_assignment(db, body, stmt, last, rhs, diagnostics);
    let mut source = last;
    for &var in links.iter().rev() {
        if let Some(name) = same_variable(db, body, var, source) {
            // `b := b` within `a := b := b := 1`: from the place to the one it
            // takes the value of.
            let (from, to) = (var.get_span(db), source.get_span(db));
            let span = auto_lsp::tree_sitter::Range {
                start_byte: from.start_byte,
                end_byte: to.end_byte,
                start_point: from.start_point,
                end_point: to.end_point,
            };
            diagnostics.push(report(
                &name,
                hir::denormalize(db, stmt.get_scope_id(db).file(db), &span),
            ));
        }
        source = var;
    }
}

pub fn check_assignment<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    stmt: Stmt<'db>,
    lhs: VariableAccess<'db>,
    rhs: hir::hir_def::expressions::expression::Expr<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    // RHS must be a simple variable access (not a function call, literal, etc.)
    let ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(rhs_var)) = rhs.expr(db) else {
        return;
    };
    if let Some(name) = same_variable(db, body, lhs, *rhs_var) {
        diagnostics.push(report(
            &name,
            hir::denormalize(db, stmt.get_scope_id(db).file(db), &stmt.get_span(db)),
        ));
    }
}

/// The name of the variable both accesses resolve to, when they do.
fn same_variable<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    lhs: VariableAccess<'db>,
    rhs_var: VariableAccess<'db>,
) -> Option<String> {
    // Both sides must be symbolic (not direct %I/%Q addresses)
    let VariableAccessKind::Symbolic(lhs_begin) = lhs.kind(db) else {
        return None;
    };
    let VariableAccessKind::Symbolic(rhs_begin) = rhs_var.kind(db) else {
        return None;
    };

    // Both must have a path expression (not an invocation like THIS/SUPER)
    let lhs_path = lhs_begin.expr(db)?;
    let rhs_path = rhs_begin.expr(db)?;

    // Compare resolved types — both must resolve to the same variable
    let Type::Variable((lhs_var, _)) = body.type_of_path_expr(lhs_path) else {
        return None;
    };
    let Type::Variable((rhs_var, _)) = body.type_of_path_expr(rhs_path) else {
        return None;
    };

    if lhs_var != rhs_var {
        return None;
    }
    Some(lhs_var.name_with_case(db).text(db).to_string())
}

fn report(name: &str, range: Option<auto_lsp::lsp_types::Range>) -> IdeDiagnostic {
    diag()
        .message(format!("variable '{name}' is assigned to itself"))
        .desc(&SelfAssignment)
        .range(range.unwrap_or_default())
        .severity(DiagnosticSeverity::WARNING)
        .call()
}
