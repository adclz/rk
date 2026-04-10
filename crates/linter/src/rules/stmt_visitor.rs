//! Single-pass statement visitor that dispatches to all statement-walking lint rules.
//!
//! Instead of each lint walking the entire statement tree independently,
//! this module walks once and calls into the relevant checkers at each node.

use db::{WorkspaceDataBase, config_file::LinterConfig};
use hir::{
    hir_def::{
        expressions::{
            expression::{Expr, VariableAccess},
            statement::{Stmt, StmtKind},
        },
        pous::{pou::Pou, variable::VariableDecl},
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::body::BodyInferenceResult,
};
use ide_diagnostic::IdeDiagnostic;

use super::{
    bool_comparison, collapsible_if, constant_condition, constant_loop_bounds, default_for_step,
    duplicate_case, empty_case_branch, empty_if_branch, empty_loop_body, external_mutation,
    for_zero_step, identical_sub_expr, identity_operation, input_assignment, loop_var_modified,
    missing_input_param, missing_return, negated_comparison, negated_condition, redundant_not,
    run_lint, self_assignment, self_comparison, sub_self, uninitialized_output, unnecessary_else,
    unnecessary_parens, yoda_condition,
};

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
        collapsible_if: config.is_enabled(collapsible_if::NAME),
        empty_if_branch: config.is_enabled(empty_if_branch::NAME),
        empty_loop_body: config.is_enabled(empty_loop_body::NAME),
        external_mutation: config.is_enabled(external_mutation::NAME),
        constant_condition: config.is_enabled(constant_condition::NAME),
        constant_loop_bounds: config.is_enabled(constant_loop_bounds::NAME),
        unnecessary_else: config.is_enabled(unnecessary_else::NAME),
        uninitialized_output: config.is_enabled(uninitialized_output::NAME),
        negated_condition: config.is_enabled(negated_condition::NAME),
        missing_input_param: config.is_enabled(missing_input_param::NAME),
        missing_return: config.is_enabled(missing_return::NAME),
        bool_comparison: config.is_enabled(bool_comparison::NAME),
        self_comparison: config.is_enabled(self_comparison::NAME),
        identical_sub_expr: config.is_enabled(identical_sub_expr::NAME),
        identity_operation: config.is_enabled(identity_operation::NAME),
        redundant_not: config.is_enabled(redundant_not::NAME),
        sub_self: config.is_enabled(sub_self::NAME),
        negated_comparison: config.is_enabled(negated_comparison::NAME),
        duplicate_case: config.is_enabled(duplicate_case::NAME),
        empty_case_branch: config.is_enabled(empty_case_branch::NAME),
        default_for_step: config.is_enabled(default_for_step::NAME),
        for_zero_step: config.is_enabled(for_zero_step::NAME),
        loop_var_modified: config.is_enabled(loop_var_modified::NAME),
        unnecessary_parens: config.is_enabled(unnecessary_parens::NAME),
        yoda_condition: config.is_enabled(yoda_condition::NAME),
    };

    if !ctx.any_enabled() {
        return;
    }

    let mut assigned_vars = if ctx.uninitialized_output {
        Some(rustc_hash::FxHashSet::default())
    } else {
        None
    };

    let mut return_assigned = false;

    let mut active_loop_vars: Vec<(VariableDecl<'db>, VariableAccess<'db>)> = Vec::new();
    visit_statements(
        db,
        body,
        &ctx,
        scope,
        statements,
        diagnostics,
        &mut assigned_vars,
        &mut active_loop_vars,
        &mut return_assigned,
    );

    // Post-walk: missing return
    if ctx.missing_return {
        run_lint(missing_return::NAME, diagnostics, |d| {
            missing_return::check_result(db, scope, return_assigned, d)
        });
    }

    if let Some(assigned) = assigned_vars {
        run_lint(uninitialized_output::NAME, diagnostics, |d| {
            uninitialized_output::check_outputs(db, scope, &assigned, d)
        });
    }
}

struct VisitorCtx {
    input_assignment: bool,
    self_assignment: bool,
    collapsible_if: bool,
    empty_if_branch: bool,
    empty_loop_body: bool,
    external_mutation: bool,
    constant_condition: bool,
    constant_loop_bounds: bool,
    unnecessary_else: bool,
    uninitialized_output: bool,
    negated_condition: bool,
    missing_input_param: bool,
    missing_return: bool,
    bool_comparison: bool,
    self_comparison: bool,
    identical_sub_expr: bool,
    redundant_not: bool,
    identity_operation: bool,
    sub_self: bool,
    negated_comparison: bool,
    duplicate_case: bool,
    empty_case_branch: bool,
    default_for_step: bool,
    for_zero_step: bool,
    loop_var_modified: bool,
    unnecessary_parens: bool,
    yoda_condition: bool,
}

impl VisitorCtx {
    fn any_enabled(&self) -> bool {
        self.input_assignment
            || self.self_assignment
            || self.collapsible_if
            || self.empty_if_branch
            || self.empty_loop_body
            || self.external_mutation
            || self.constant_condition
            || self.constant_loop_bounds
            || self.unnecessary_else
            || self.uninitialized_output
            || self.negated_condition
            || self.missing_input_param
            || self.missing_return
            || self.bool_comparison
            || self.self_comparison
            || self.identical_sub_expr
            || self.redundant_not
            || self.identity_operation
            || self.sub_self
            || self.negated_comparison
            || self.duplicate_case
            || self.empty_case_branch
            || self.default_for_step
            || self.for_zero_step
            || self.loop_var_modified
            || self.unnecessary_parens
            || self.yoda_condition
    }

    fn any_expr_lint(&self) -> bool {
        self.bool_comparison
            || self.self_comparison
            || self.identical_sub_expr
            || self.redundant_not
            || self.identity_operation
            || self.sub_self
            || self.negated_comparison
            || self.unnecessary_parens
            || self.yoda_condition
    }
}

/// Recursively walk an expression tree and run all expression-level lints at each node.
/// Each lint only checks the current node - the recursion is handled here.
fn check_expr_lints<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    ctx: &VisitorCtx,
    expr: &Expr<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    use hir::hir_def::expressions::expression::{ExprKind, PrimaryExpr};

    if !ctx.any_expr_lint() {
        return;
    }

    // Run all lints on the current node
    if ctx.bool_comparison {
        run_lint(bool_comparison::NAME, diagnostics, |d| {
            bool_comparison::check_node(db, expr, d)
        });
    }
    if ctx.self_comparison {
        run_lint(self_comparison::NAME, diagnostics, |d| {
            self_comparison::check_node(db, body, expr, d)
        });
    }
    if ctx.identical_sub_expr {
        run_lint(identical_sub_expr::NAME, diagnostics, |d| {
            identical_sub_expr::check_node(db, body, expr, d)
        });
    }
    if ctx.redundant_not {
        run_lint(redundant_not::NAME, diagnostics, |d| {
            redundant_not::check_node(db, expr, d)
        });
    }
    if ctx.identity_operation {
        run_lint(identity_operation::NAME, diagnostics, |d| {
            identity_operation::check_node(db, expr, d)
        });
    }
    if ctx.sub_self {
        run_lint(sub_self::NAME, diagnostics, |d| {
            sub_self::check_node(db, body, expr, d)
        });
    }
    if ctx.negated_comparison {
        run_lint(negated_comparison::NAME, diagnostics, |d| {
            negated_comparison::check_node(db, expr, d)
        });
    }
    if ctx.unnecessary_parens {
        run_lint(unnecessary_parens::NAME, diagnostics, |d| {
            unnecessary_parens::check_node(db, expr, d)
        });
    }
    if ctx.yoda_condition {
        run_lint(yoda_condition::NAME, diagnostics, |d| {
            yoda_condition::check_node(db, expr, d)
        });
    }

    // Recurse into sub-expressions (single place for all lints)
    match expr.expr(db) {
        ExprKind::AddOperator { left, right, .. }
        | ExprKind::MultOperator { left, right, .. }
        | ExprKind::BooleanOperator { left, right, .. }
        | ExprKind::ComparisonOperator { left, right, .. }
        | ExprKind::PowerOperator { left, right } => {
            check_expr_lints(db, body, ctx, left, diagnostics);
            check_expr_lints(db, body, ctx, right, diagnostics);
        }
        ExprKind::UnaryOperator { expr, .. } => {
            check_expr_lints(db, body, ctx, expr, diagnostics);
        }
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => {
            check_expr_lints(db, body, ctx, expr, diagnostics);
        }
        _ => {}
    }
}

fn visit_statements<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: &BodyInferenceResult<'db>,
    ctx: &VisitorCtx,
    scope: ScopeId<'db>,
    stmts: &[Stmt<'db>],
    diagnostics: &mut Vec<IdeDiagnostic>,
    assigned_vars: &mut Option<
        rustc_hash::FxHashSet<hir::hir_def::pous::variable::VariableDecl<'db>>,
    >,
    active_loop_vars: &mut Vec<(VariableDecl<'db>, VariableAccess<'db>)>,
    return_assigned: &mut bool,
) {
    for stmt in stmts {
        match stmt.stmt(db) {
            StmtKind::Assignment { var, target } => {
                check_expr_lints(db, body, ctx, target, diagnostics);
                if ctx.input_assignment {
                    run_lint(input_assignment::NAME, diagnostics, |d| {
                        input_assignment::check_assignment(db, body, *var, d)
                    });
                }
                if ctx.external_mutation {
                    run_lint(external_mutation::NAME, diagnostics, |d| {
                        external_mutation::check_assignment(db, body, *var, d)
                    });
                }
                if ctx.self_assignment {
                    run_lint(self_assignment::NAME, diagnostics, |d| {
                        self_assignment::check_assignment(db, body, *stmt, *var, *target, d)
                    });
                }
                if ctx.loop_var_modified {
                    run_lint(loop_var_modified::NAME, diagnostics, |d| {
                        loop_var_modified::check_assignment(db, body, *var, active_loop_vars, d)
                    });
                }
                if ctx.missing_return
                    && !*return_assigned
                    && missing_return::check_assignment(db, body, *var, scope)
                {
                    *return_assigned = true;
                }
                if ctx.uninitialized_output
                    && let Some(assigned) = assigned_vars.as_mut()
                {
                    uninitialized_output::collect_assigned(db, body, *var, assigned);
                }
            }
            StmtKind::AssignmentAttempt { var, target } => {
                check_expr_lints(db, body, ctx, target, diagnostics);
                if ctx.input_assignment {
                    run_lint(input_assignment::NAME, diagnostics, |d| {
                        input_assignment::check_assignment(db, body, *var, d)
                    });
                }
                if ctx.external_mutation {
                    run_lint(external_mutation::NAME, diagnostics, |d| {
                        external_mutation::check_assignment(db, body, *var, d)
                    });
                }
                if ctx.loop_var_modified {
                    run_lint(loop_var_modified::NAME, diagnostics, |d| {
                        loop_var_modified::check_assignment(db, body, *var, active_loop_vars, d)
                    });
                }
                if ctx.missing_return
                    && !*return_assigned
                    && missing_return::check_assignment(db, body, *var, scope)
                {
                    *return_assigned = true;
                }
                if ctx.uninitialized_output
                    && let Some(assigned) = assigned_vars.as_mut()
                {
                    uninitialized_output::collect_assigned(db, body, *var, assigned);
                }
            }
            StmtKind::If {
                condition,
                then,
                else_if,
                else_,
            } => {
                check_expr_lints(db, body, ctx, condition, diagnostics);
                if ctx.constant_condition {
                    run_lint(constant_condition::NAME, diagnostics, |d| {
                        constant_condition::check_condition(db, condition, "IF", d)
                    });
                }
                if let Some(stmts) = then {
                    visit_statements(
                        db,
                        body,
                        ctx,
                        scope,
                        stmts,
                        diagnostics,
                        assigned_vars,
                        active_loop_vars,
                        return_assigned,
                    );
                }
                for (cond, stmts) in else_if {
                    check_expr_lints(db, body, ctx, cond, diagnostics);
                    if ctx.constant_condition {
                        run_lint(constant_condition::NAME, diagnostics, |d| {
                            constant_condition::check_condition(db, cond, "ELSIF", d)
                        });
                    }
                    visit_statements(
                        db,
                        body,
                        ctx,
                        scope,
                        stmts,
                        diagnostics,
                        assigned_vars,
                        active_loop_vars,
                        return_assigned,
                    );
                }
                if let Some(stmts) = else_ {
                    visit_statements(
                        db,
                        body,
                        ctx,
                        scope,
                        stmts,
                        diagnostics,
                        assigned_vars,
                        active_loop_vars,
                        return_assigned,
                    );
                }
                if ctx.unnecessary_else {
                    run_lint(unnecessary_else::NAME, diagnostics, |d| {
                        unnecessary_else::check_if(db, *stmt, then, else_if, else_, d)
                    });
                }
                if ctx.negated_condition && else_if.is_empty() {
                    run_lint(negated_condition::NAME, diagnostics, |d| {
                        negated_condition::check_if(db, condition, then, else_, d)
                    });
                }
                if ctx.collapsible_if {
                    run_lint(collapsible_if::NAME, diagnostics, |d| {
                        collapsible_if::check_if(db, *stmt, condition, then, else_if, else_, d)
                    });
                }
                if ctx.empty_if_branch {
                    run_lint(empty_if_branch::NAME, diagnostics, |d| {
                        empty_if_branch::check_if(db, *stmt, condition, then, else_if, else_, d)
                    });
                }
            }
            StmtKind::While {
                condition,
                body: loop_body,
            } => {
                check_expr_lints(db, body, ctx, condition, diagnostics);
                if ctx.constant_condition {
                    run_lint(constant_condition::NAME, diagnostics, |d| {
                        constant_condition::check_condition(db, condition, "WHILE", d)
                    });
                }
                if ctx.empty_loop_body {
                    run_lint(empty_loop_body::NAME, diagnostics, |d| {
                        empty_loop_body::check_while(db, condition, loop_body, d)
                    });
                }
                visit_statements(
                    db,
                    body,
                    ctx,
                    scope,
                    loop_body,
                    diagnostics,
                    assigned_vars,
                    active_loop_vars,
                    return_assigned,
                );
            }
            StmtKind::For {
                body: loop_body,
                start,
                end,
                step,
                control_variable,
            } => {
                let pushed = if ctx.loop_var_modified {
                    if let Some(decl) =
                        loop_var_modified::resolve_control_var(db, body, *control_variable)
                    {
                        active_loop_vars.push((decl, *control_variable));
                        true
                    } else {
                        false
                    }
                } else {
                    false
                };
                visit_statements(
                    db,
                    body,
                    ctx,
                    scope,
                    loop_body,
                    diagnostics,
                    assigned_vars,
                    active_loop_vars,
                    return_assigned,
                );
                if pushed {
                    active_loop_vars.pop();
                }
                check_expr_lints(db, body, ctx, start, diagnostics);
                check_expr_lints(db, body, ctx, end, diagnostics);
                if ctx.constant_loop_bounds {
                    run_lint(constant_loop_bounds::NAME, diagnostics, |d| {
                        constant_loop_bounds::check(db, start, end, d);
                    });
                }
                if ctx.empty_loop_body {
                    run_lint(empty_loop_body::NAME, diagnostics, |d| {
                        empty_loop_body::check_for(db, *stmt, loop_body, d)
                    });
                }
                if let Some(step) = step {
                    check_expr_lints(db, body, ctx, step, diagnostics);
                    if ctx.for_zero_step {
                        run_lint(for_zero_step::NAME, diagnostics, |d| {
                            for_zero_step::check_step(db, step, d)
                        });
                    }
                    if ctx.default_for_step {
                        run_lint(default_for_step::NAME, diagnostics, |d| {
                            default_for_step::check_step(db, step, d)
                        });
                    }
                }
            }
            StmtKind::Repeat {
                condition,
                body: loop_body,
            } => {
                check_expr_lints(db, body, ctx, condition, diagnostics);
                if ctx.constant_condition {
                    run_lint(constant_condition::NAME, diagnostics, |d| {
                        constant_condition::check_condition(db, condition, "UNTIL", d)
                    });
                }
                if ctx.empty_loop_body {
                    run_lint(empty_loop_body::NAME, diagnostics, |d| {
                        empty_loop_body::check_repeat(db, condition, loop_body, d)
                    });
                }
                visit_statements(
                    db,
                    body,
                    ctx,
                    scope,
                    loop_body,
                    diagnostics,
                    assigned_vars,
                    active_loop_vars,
                    return_assigned,
                );
            }
            StmtKind::Case {
                condition,
                cases,
                else_,
            } => {
                check_expr_lints(db, body, ctx, condition, diagnostics);
                if ctx.duplicate_case {
                    run_lint(duplicate_case::NAME, diagnostics, |d| {
                        duplicate_case::check_case(db, cases, d)
                    });
                }
                if ctx.empty_case_branch {
                    run_lint(empty_case_branch::NAME, diagnostics, |d| {
                        empty_case_branch::check_case(db, cases, d)
                    });
                }
                for (_, stmts) in cases {
                    visit_statements(
                        db,
                        body,
                        ctx,
                        scope,
                        stmts,
                        diagnostics,
                        assigned_vars,
                        active_loop_vars,
                        return_assigned,
                    );
                }
                if let Some(stmts) = else_ {
                    visit_statements(
                        db,
                        body,
                        ctx,
                        scope,
                        stmts,
                        diagnostics,
                        assigned_vars,
                        active_loop_vars,
                        return_assigned,
                    );
                }
            }
            StmtKind::FuncCall(call) => {
                if ctx.missing_input_param {
                    run_lint(missing_input_param::NAME, diagnostics, |d| {
                        missing_input_param::check_func_call(db, body, *stmt, *call, d)
                    });
                }
            }
            _ => {}
        }
    }
}
