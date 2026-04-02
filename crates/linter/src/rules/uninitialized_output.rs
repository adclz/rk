use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{VariableAccessKind},
            statement::{Stmt, StmtKind},
        },
        pous::{pou::Pou, variable::{VariableDecl, VariableKind}},
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{body::BodyInferenceResult, ty::Type},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};
use rustc_hash::FxHashSet;

pub const NAME: &str = "uninitialized-output";

/// L0114: a VAR_OUTPUT variable is never assigned in the body.
struct UninitializedOutput;

impl ErrorCode for UninitializedOutput {
    fn code(&self) -> &'static str {
        "L0114"
    }

    fn description(&self) -> &'static str {
        "uninitialized output"
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

    // Collect all variables that are assigned in the body
    let mut assigned = FxHashSet::default();
    collect_assigned_variables(db, body, statements, &mut assigned);

    // Check each VAR_OUTPUT
    let def_map = scope.def_map(db);
    for var in def_map.global_variables.values() {
        if var.kind(db) != VariableKind::Output {
            continue;
        }

        // Skip if the variable has an initializer
        if var.init(db).is_some() {
            continue;
        }

        if assigned.contains(var) {
            continue;
        }

        let name = var.get_name_ident(db).text(db);
        diagnostics.push(
            diag()
                .message(format!("VAR_OUTPUT '{name}' is never assigned in the body"))
                .desc(&UninitializedOutput)
                .range(var.get_span(db))
                .severity(DiagnosticSeverity::WARNING)
                .call(),
        );
    }
}

/// Collect the variable assigned by a single assignment LHS.
/// Called by the unified visitor for each assignment statement.
pub fn collect_assigned<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    var: hir::hir_def::expressions::expression::VariableAccess<'db>,
    assigned: &mut FxHashSet<VariableDecl<'db>>,
) {
    let VariableAccessKind::Symbolic(begin) = var.kind(db) else {
        return;
    };
    let Some(path_expr) = begin.expr(db) else {
        return;
    };
    if let Some(&Type::Variable((var_decl, _))) = body.type_of_path_expr.get(&path_expr) {
        assigned.insert(var_decl);
    }
}

/// Check VAR_OUTPUT variables against the set of assigned variables.
/// Called after the unified visitor has finished collecting.
pub fn check_outputs<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    assigned: &FxHashSet<VariableDecl<'db>>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let def_map = scope.def_map(db);
    for var in def_map.global_variables.values() {
        if var.kind(db) != VariableKind::Output {
            continue;
        }
        if var.init(db).is_some() {
            continue;
        }
        if assigned.contains(var) {
            continue;
        }
        let name = var.get_name_ident(db).text(db);
        diagnostics.push(
            diag()
                .message(format!("VAR_OUTPUT '{name}' is never assigned in the body"))
                .desc(&UninitializedOutput)
                .range(var.get_span(db))
                .severity(DiagnosticSeverity::WARNING)
                .call(),
        );
    }
}

fn collect_assigned_variables<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    stmts: &[Stmt<'db>],
    assigned: &mut FxHashSet<VariableDecl<'db>>,
) {
    for stmt in stmts {
        match stmt.stmt(db) {
            StmtKind::Assignment { var, .. } | StmtKind::AssignmentAttempt { var, .. } => {
                // Resolve the LHS variable
                let VariableAccessKind::Symbolic(begin) = var.kind(db) else {
                    continue;
                };
                let Some(path_expr) = begin.expr(db) else {
                    continue;
                };
                if let Some(&Type::Variable((var_decl, _))) =
                    body.type_of_path_expr.get(&path_expr)
                {
                    assigned.insert(var_decl);
                }
            }
            StmtKind::If {
                then,
                else_if,
                else_,
                ..
            } => {
                if let Some(stmts) = then {
                    collect_assigned_variables(db, body, stmts, assigned);
                }
                for (_, stmts) in else_if {
                    collect_assigned_variables(db, body, stmts, assigned);
                }
                if let Some(stmts) = else_ {
                    collect_assigned_variables(db, body, stmts, assigned);
                }
            }
            StmtKind::Case { cases, else_, .. } => {
                for (_, stmts) in cases {
                    collect_assigned_variables(db, body, stmts, assigned);
                }
                if let Some(stmts) = else_ {
                    collect_assigned_variables(db, body, stmts, assigned);
                }
            }
            StmtKind::For { body: stmts, .. }
            | StmtKind::While { body: stmts, .. }
            | StmtKind::Repeat { body: stmts, .. } => {
                collect_assigned_variables(db, body, stmts, assigned);
            }
            _ => {}
        }
    }
}
