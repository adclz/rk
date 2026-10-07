// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{
                Expr, ExprKind, FuncCall, PrimaryExpr, RefValue, VariableAccess, VariableAccessKind,
            },
            statement::{Stmt, StmtKind},
        },
        pous::{
            pou::Pou,
            variable::{VariableDecl, VariableKind},
        },
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{
        body::{ParamBinding, ScopeInference},
        expr_store::PathExprWalkStep,
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};
use rustc_hash::FxHashSet;

pub const NAME: &str = "endless-loop";

/// L0126: a WHILE or REPEAT whose body changes nothing its condition reads.
struct EndlessLoop;

impl ErrorCode for EndlessLoop {
    fn code(&self) -> &'static str {
        "L0126"
    }
}

/// What a loop body does that may end the loop.
#[derive(Default)]
struct Effects<'db> {
    /// Variables it writes, or hands out by reference: assigned, a FOR
    /// counter, bound to a VAR_IN_OUT or an output, given to `REF()`, an
    /// instance it calls.
    written: FxHashSet<VariableDecl<'db>>,
    /// It calls something, which may write a member, a global or an in-out.
    calls: bool,
    /// It leaves the loop: EXIT at this loop's level, RETURN, `__RAISE`.
    exits: bool,
    /// It writes what no variable names here: through a reference, a direct
    /// address, a `{wasm}` statement.
    writes_anything: bool,
}

/// A WHILE or a REPEAT, by its condition and its body.
pub fn check_loop<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    scope: ScopeId<'db>,
    condition: &Expr<'db>,
    loop_body: &[Stmt<'db>],
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let Some(read) = condition_reads(db, body, condition) else {
        return;
    };
    // A constant condition is L0103's.
    if read.is_empty() {
        return;
    }
    let mut effects = Effects::default();
    statements(db, body, loop_body, true, &mut effects);
    if effects.exits || effects.writes_anything {
        return;
    }
    let changes = |var: &VariableDecl<'db>| {
        effects.written.contains(var) || (effects.calls && !private_to_the_call(db, scope, *var))
    };
    if read.iter().any(changes) {
        return;
    }

    let mut names: Vec<String> = read
        .iter()
        .map(|var| format!("'{}'", var.get_name_with_case(db).text(db)))
        .collect();
    names.sort();
    let (message, help) = match names.as_slice() {
        [one] => (
            format!("the loop never changes {one}"),
            format!("change {one} in the loop, or leave it with EXIT"),
        ),
        many => (
            format!("the loop changes none of {}", many.join(", ")),
            "change one of them in the loop, or leave it with EXIT".to_string(),
        ),
    };
    let mut d = diag()
        .message(message)
        .desc(&EndlessLoop)
        .range(
            hir::denormalize(
                db,
                condition.get_scope_id(db).file(db),
                &condition.get_span(db),
            )
            .unwrap_or_default(),
        )
        .severity(DiagnosticSeverity::WARNING)
        .call();
    d.with_note("its condition never changes, so once the loop starts it never ends".to_string());
    d.with_help(help);
    diagnostics.push(d);
}

/// The variables a condition reads, or `None` when it reads something that
/// may change on its own: a call, a dereference, a direct address.
fn condition_reads<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    condition: &Expr<'db>,
) -> Option<FxHashSet<VariableDecl<'db>>> {
    fn walk<'db>(
        db: &'db dyn WorkspaceDataBase,
        body: ScopeInference<'db>,
        expr: &Expr<'db>,
        read: &mut FxHashSet<VariableDecl<'db>>,
    ) -> Option<()> {
        match expr.expr(db) {
            ExprKind::AddOperator { left, right, .. }
            | ExprKind::MultOperator { left, right, .. }
            | ExprKind::BooleanOperator { left, right, .. }
            | ExprKind::ComparisonOperator { left, right, .. }
            | ExprKind::PowerOperator { left, right } => {
                walk(db, body, left, read)?;
                walk(db, body, right, read)
            }
            ExprKind::UnaryOperator { expr, .. } => walk(db, body, expr, read),
            ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => {
                walk(db, body, expr, read)
            }
            ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(access)) => {
                let (root, through_reference) = root(db, body, *access)?;
                if through_reference {
                    return None;
                }
                read.insert(root);
                Some(())
            }
            ExprKind::PrimaryExpr(PrimaryExpr::Literal(_) | PrimaryExpr::EnumValue { .. }) => {
                Some(())
            }
            _ => None,
        }
    }
    let mut read = FxHashSet::default();
    walk(db, body, condition, &mut read)?;
    Some(read)
}

/// The variable a place starts from, `buf` for `buf.items[i]`, a member for
/// `THIS.x`, and whether the place goes through a dereference. `None` for a
/// direct address or a place the body bound to no variable.
fn root<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    access: VariableAccess<'db>,
) -> Option<(VariableDecl<'db>, bool)> {
    let VariableAccessKind::Symbolic(begin) = access.kind(db) else {
        return None;
    };
    let steps = begin.expr(db)?.flatten(db);
    let PathExprWalkStep::Field { expr, .. } = steps.first()? else {
        return None;
    };
    let root = body.variable_for_path_expr(*expr)?;
    let through_reference = steps
        .iter()
        .any(|step| matches!(step, PathExprWalkStep::Deref { .. }));
    Some((root, through_reference))
}

/// Whether nothing a call makes can reach `var`: a FUNCTION's or METHOD's
/// own VAR, VAR_TEMP or VAR_INPUT, a VAR_TEMP anywhere. A member, a global, a
/// PROGRAM's or a block's VAR, an in-out are within reach of a callee.
fn private_to_the_call<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    var: VariableDecl<'db>,
) -> bool {
    if var.get_scope_id(db) != scope {
        return false;
    }
    match var.kind(db) {
        VariableKind::Temp => true,
        VariableKind::Var | VariableKind::Input => matches!(
            get_scope(db, scope).kind,
            ScopeKind::Pou(Pou::Function(_)) | ScopeKind::MethodDecl(_)
        ),
        _ => false,
    }
}

/// `at_loop_level`: an EXIT here leaves the loop being checked, not one
/// nested in it.
fn statements<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    stmts: &[Stmt<'db>],
    at_loop_level: bool,
    effects: &mut Effects<'db>,
) {
    for stmt in stmts {
        match stmt.stmt(db) {
            StmtKind::Assignment { var, target } => {
                write(db, body, *var, effects);
                expression(db, body, target, effects);
            }
            StmtKind::FuncCall(call) => call_effects(db, body, *call, effects),
            StmtKind::If {
                condition,
                then,
                else_if,
                else_,
            } => {
                expression(db, body, condition, effects);
                statements(
                    db,
                    body,
                    then.as_deref().unwrap_or_default(),
                    at_loop_level,
                    effects,
                );
                for (condition, stmts) in else_if {
                    expression(db, body, condition, effects);
                    statements(db, body, stmts, at_loop_level, effects);
                }
                statements(
                    db,
                    body,
                    else_.as_deref().unwrap_or_default(),
                    at_loop_level,
                    effects,
                );
            }
            StmtKind::Case {
                condition,
                cases,
                else_,
            } => {
                expression(db, body, condition, effects);
                for (_, stmts) in cases {
                    statements(db, body, stmts, at_loop_level, effects);
                }
                statements(
                    db,
                    body,
                    else_.as_deref().unwrap_or_default(),
                    at_loop_level,
                    effects,
                );
            }
            StmtKind::For {
                control_variable,
                start,
                end,
                step,
                body: stmts,
            } => {
                write(db, body, *control_variable, effects);
                expression(db, body, start, effects);
                expression(db, body, end, effects);
                if let Some(step) = step {
                    expression(db, body, step, effects);
                }
                statements(db, body, stmts, false, effects);
            }
            StmtKind::While {
                condition,
                body: stmts,
            }
            | StmtKind::Repeat {
                condition,
                body: stmts,
            } => {
                expression(db, body, condition, effects);
                statements(db, body, stmts, false, effects);
            }
            StmtKind::Exit if at_loop_level => effects.exits = true,
            StmtKind::Return | StmtKind::Raise { .. } => effects.exits = true,
            StmtKind::WasmPragma(_) => effects.writes_anything = true,
            _ => {}
        }
    }
}

/// A place written: its root variable, or anything when it goes through a
/// reference or is a direct address.
fn write<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    access: VariableAccess<'db>,
    effects: &mut Effects<'db>,
) {
    match root(db, body, access) {
        Some((root, false)) => {
            effects.written.insert(root);
        }
        _ => effects.writes_anything = true,
    }
}

/// The calls and the `REF()`s of an expression.
fn expression<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    expr: &Expr<'db>,
    effects: &mut Effects<'db>,
) {
    match expr.expr(db) {
        ExprKind::AddOperator { left, right, .. }
        | ExprKind::MultOperator { left, right, .. }
        | ExprKind::BooleanOperator { left, right, .. }
        | ExprKind::ComparisonOperator { left, right, .. }
        | ExprKind::PowerOperator { left, right } => {
            expression(db, body, left, effects);
            expression(db, body, right, effects);
        }
        ExprKind::UnaryOperator { expr, .. }
        | ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => {
            expression(db, body, expr, effects)
        }
        ExprKind::PrimaryExpr(PrimaryExpr::FuncCall(call)) => {
            call_effects(db, body, *call, effects)
        }
        ExprKind::PrimaryExpr(PrimaryExpr::RefValue {
            value: RefValue::Address(begin),
        }) => match begin
            .expr(db)
            .and_then(|path| path.flatten(db).first().copied())
            .and_then(|step| body.variable_for_path_expr(step.get_expr(db)))
        {
            Some(root) => {
                effects.written.insert(root);
            }
            None => effects.writes_anything = true,
        },
        ExprKind::FoldExpr { .. } => effects.calls = true,
        _ => {}
    }
}

/// A call: it may write what its callee reaches, the instance it runs on,
/// and what it binds by reference.
fn call_effects<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    call: FuncCall<'db>,
    effects: &mut Effects<'db>,
) {
    effects.calls = true;
    // `inst()`, `inst.m()`: the instance's members change.
    if let Some(root) = call
        .path(db)
        .expr(db)
        .and_then(|path| path.flatten(db).first().copied())
        .and_then(|step| body.variable_for_path_expr(step.get_expr(db)))
    {
        effects.written.insert(root);
    }
    let Some(resolved) = body.resolved_call(call) else {
        // A call inference could not resolve: anything may happen.
        effects.writes_anything = true;
        return;
    };
    for (param, binding) in &resolved.params {
        match binding {
            ParamBinding::Values(values) => {
                for value in values {
                    if param.is_in_out(db)
                        && let ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(access)) =
                            value.expr(db)
                    {
                        write(db, body, *access, effects);
                    }
                    expression(db, body, value, effects);
                }
            }
            ParamBinding::Output { variable, .. } => write(db, body, *variable, effects),
            ParamBinding::Default(_) | ParamBinding::Omitted => {}
        }
    }
}
