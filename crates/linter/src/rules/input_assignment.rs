use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{VariableAccess, VariableAccessKind},
            statement::{Stmt, StmtKind},
        },
        pous::{pou::Pou, variable::VariableKind},
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{body::BodyInferenceResult, ty::Type},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

pub const NAME: &str = "input-assignment";

/// L0303: assigning to a VAR_INPUT variable.
struct InputAssignment;

impl ErrorCode for InputAssignment {
    fn code(&self) -> &'static str {
        "L0303"
    }

    fn description(&self) -> &'static str {
        "assignment to input variable"
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
            StmtKind::Assignment { var, .. } | StmtKind::AssignmentAttempt { var, .. } => {
                check_assignment(db, body, *var, diagnostics);
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
    var_access: VariableAccess<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let VariableAccessKind::Symbolic(begin) = var_access.kind(db) else {
        return;
    };

    let Some(path_expr) = begin.expr(db) else {
        return;
    };

    let Some(&Type::Variable((var_decl, _))) = body.type_of_path_expr.get(&path_expr) else {
        return;
    };

    if var_decl.kind(db) != VariableKind::Input {
        return;
    }

    let name = var_decl.get_name_ident(db).text(db);
    let mut diag = diag()
        .message(format!("assignment to VAR_INPUT '{name}'"))
        .desc(&InputAssignment)
        .range(var_access.get_span(db))
        .severity(DiagnosticSeverity::WARNING)
        .call();

    diag.with_related(Related::new(
        format!("'{name}' is declared here"),
        var_decl.get_scope_id(db).file(db),
        var_decl.get_name_span(db),
    ));

    diagnostics.push(diag);
}
