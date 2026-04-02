//! Single-pass statement visitor that dispatches to all statement-walking lint rules.
//!
//! Instead of each lint walking the entire statement tree independently,
//! this module walks once and calls into the relevant checkers at each node.

use db::{WorkspaceDataBase, config_file::LinterConfig};
use hir::{
    HirNodeInfo,
    hir_def::{
        expressions::statement::{Stmt, StmtKind},
        pous::pou::Pou,
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::body::BodyInferenceResult,
};
use ide_diagnostic::IdeDiagnostic;

use super::{constant_condition, input_assignment, missing_input_param, negated_condition, self_assignment, uninitialized_output, unnecessary_else};

/// Run all statement-walking lints in a single pass over the statement tree.
pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    config: &LinterConfig,
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

    let ctx = VisitorCtx {
        input_assignment: config.is_enabled(input_assignment::NAME),
        self_assignment: config.is_enabled(self_assignment::NAME),
        constant_condition: config.is_enabled(constant_condition::NAME),
        unnecessary_else: config.is_enabled(unnecessary_else::NAME),
        uninitialized_output: config.is_enabled(uninitialized_output::NAME),
        negated_condition: config.is_enabled(negated_condition::NAME),
        missing_input_param: config.is_enabled(missing_input_param::NAME),
    };

    // Nothing enabled - skip walk entirely
    if !ctx.any_enabled() {
        return;
    }

    let mut assigned_vars = if ctx.uninitialized_output {
        Some(rustc_hash::FxHashSet::default())
    } else {
        None
    };

    visit_statements(db, body, &ctx, statements, diagnostics, &mut assigned_vars);

    // Post-walk: check uninitialized outputs
    if let Some(assigned) = assigned_vars {
        uninitialized_output::check_outputs(db, scope, &assigned, diagnostics);
    }
}

struct VisitorCtx {
    input_assignment: bool,
    self_assignment: bool,
    constant_condition: bool,
    unnecessary_else: bool,
    uninitialized_output: bool,
    negated_condition: bool,
    missing_input_param: bool,
}

impl VisitorCtx {
    fn any_enabled(&self) -> bool {
        self.input_assignment
            || self.self_assignment
            || self.constant_condition
            || self.unnecessary_else
            || self.uninitialized_output
            || self.negated_condition
            || self.missing_input_param
    }
}

fn visit_statements<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    ctx: &VisitorCtx,
    stmts: &[Stmt<'db>],
    diagnostics: &mut Vec<IdeDiagnostic>,
    assigned_vars: &mut Option<rustc_hash::FxHashSet<hir::hir_def::pous::variable::VariableDecl<'db>>>,
) {
    for stmt in stmts {
        match stmt.stmt(db) {
            StmtKind::Assignment { var, target } => {
                if ctx.input_assignment {
                    input_assignment::check_assignment(db, body, *var, diagnostics);
                }
                if ctx.self_assignment {
                    self_assignment::check_assignment(db, body, *stmt, *var, *target, diagnostics);
                }
                if ctx.uninitialized_output {
                    if let Some(assigned) = assigned_vars.as_mut() {
                        uninitialized_output::collect_assigned(db, body, *var, assigned);
                    }
                }
            }
            StmtKind::AssignmentAttempt { var, .. } => {
                if ctx.input_assignment {
                    input_assignment::check_assignment(db, body, *var, diagnostics);
                }
                if ctx.uninitialized_output {
                    if let Some(assigned) = assigned_vars.as_mut() {
                        uninitialized_output::collect_assigned(db, body, *var, assigned);
                    }
                }
            }
            StmtKind::If {
                condition,
                then,
                else_if,
                else_,
                ..
            } => {
                if ctx.constant_condition {
                    constant_condition::check_condition(db, condition, "IF", diagnostics);
                }
                if let Some(stmts) = then {
                    visit_statements(db, body, ctx, stmts, diagnostics, assigned_vars);
                }
                for (cond, stmts) in else_if {
                    if ctx.constant_condition {
                        constant_condition::check_condition(db, cond, "ELSIF", diagnostics);
                    }
                    visit_statements(db, body, ctx, stmts, diagnostics, assigned_vars);
                }
                if let Some(stmts) = else_ {
                    visit_statements(db, body, ctx, stmts, diagnostics, assigned_vars);
                }
                if ctx.unnecessary_else {
                    unnecessary_else::check_if(db, *stmt, then, else_if, else_, diagnostics);
                }
                if ctx.negated_condition && else_if.is_empty() {
                    negated_condition::check_if(db, condition, then, else_, diagnostics);
                }
            }
            StmtKind::While {
                condition, body: loop_body, ..
            } => {
                if ctx.constant_condition {
                    constant_condition::check_condition(db, condition, "WHILE", diagnostics);
                }
                visit_statements(db, body, ctx, loop_body, diagnostics, assigned_vars);
            }
            StmtKind::Repeat {
                condition, body: loop_body, ..
            } => {
                if ctx.constant_condition {
                    constant_condition::check_condition(db, condition, "UNTIL", diagnostics);
                }
                visit_statements(db, body, ctx, loop_body, diagnostics, assigned_vars);
            }
            StmtKind::Case { cases, else_, .. } => {
                for (_, stmts) in cases {
                    visit_statements(db, body, ctx, stmts, diagnostics, assigned_vars);
                }
                if let Some(stmts) = else_ {
                    visit_statements(db, body, ctx, stmts, diagnostics, assigned_vars);
                }
            }
            StmtKind::FuncCall(call) => {
                if ctx.missing_input_param {
                    missing_input_param::check_func_call(db, body, *stmt, *call, diagnostics);
                }
            }
            StmtKind::For { body: loop_body, .. } => {
                visit_statements(db, body, ctx, loop_body, diagnostics, assigned_vars);
            }
            _ => {}
        }
    }
}
