use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{
        expressions::statement::{Stmt, StmtKind},
        pous::pou::Pou,
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "unnecessary-else";

/// W0113: ELSE branch is unnecessary because the IF/ELSIF body always exits.
struct UnnecessaryElse;

impl ErrorCode for UnnecessaryElse {
    fn code(&self) -> &'static str {
        "W0113"
    }

    fn description(&self) -> &'static str {
        "unnecessary ELSE"
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
                then,
                else_if,
                else_,
                ..
            } => {
                // Check nested statements in all branches first
                if let Some(stmts) = then {
                    check_statements(db, stmts, diagnostics);
                }
                for (_, stmts) in else_if {
                    check_statements(db, stmts, diagnostics);
                }
                if let Some(stmts) = else_ {
                    check_statements(db, stmts, diagnostics);
                }

                // Now check if the else is unnecessary:
                // All preceding branches (then + all elsif) must end with an exit
                let Some(else_stmts) = else_ else {
                    continue;
                };
                if else_stmts.is_empty() {
                    continue;
                }

                let then_exits = then.as_ref().is_some_and(|stmts| ends_with_exit(db, stmts));
                if !then_exits {
                    continue;
                }

                let all_elsif_exit = else_if.iter().all(|(_, stmts)| ends_with_exit(db, stmts));
                if !all_elsif_exit {
                    continue;
                }

                for else_ in else_stmts {
                    diagnostics.push(
                    diag()
                        .message(
                            "unnecessary ELSE branch: all preceding branches end with RETURN, EXIT, or CONTINUE"
                                .to_string(),
                        )
                        .desc(&UnnecessaryElse)
                        .range(else_.get_span(db))
                        .severity(DiagnosticSeverity::INFORMATION)
                        .call(),
                );
                }
            }
            StmtKind::Case { cases, else_, .. } => {
                for (_, stmts) in cases {
                    check_statements(db, stmts, diagnostics);
                }
                if let Some(stmts) = else_ {
                    check_statements(db, stmts, diagnostics);
                }
            }
            StmtKind::For { body, .. }
            | StmtKind::While { body, .. }
            | StmtKind::Repeat { body, .. } => {
                check_statements(db, body, diagnostics);
            }
            _ => {}
        }
    }
}

/// Returns true if the statement list ends with an unconditional exit
/// (RETURN, EXIT, or CONTINUE).
fn ends_with_exit<'db>(db: &'db dyn WorkspaceDataBase, stmts: &[Stmt<'db>]) -> bool {
    stmts.last().is_some_and(|last| {
        matches!(
            last.stmt(db),
            StmtKind::Return | StmtKind::Exit | StmtKind::Continue
        )
    })
}
