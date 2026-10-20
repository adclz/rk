// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

//! The places a statement or an expression names, in the order a scan
//! reaches them: the subscripts of a place before the place, a call's inputs
//! before its outputs, an assignment's value before its target.

use db::WorkspaceDataBase;
use hir::hir_def::expressions::{
    expression::{
        BeginPathExpr, Expr, ExprKind, FuncCall, ParamAssignKind, PathExpr, PathExprKind,
        PrimaryExpr, RefValue, VariableAccess, VariableAccessKind,
    },
    statement::{CaseKind, Stmt, StmtKind},
};

/// What a place is used for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Use {
    Read,
    Write,
}

/// A place a body names.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Place<'db> {
    /// A variable, a member, an element or an address.
    Access(VariableAccess<'db>),
    /// What `REF()` hands out.
    Referenced(BeginPathExpr<'db>),
}

/// Every place `expr` names. A call's argument is read, an in-out's
/// included, and a formal output (`q => x`) is written.
pub(crate) fn expression<'db>(
    db: &'db dyn WorkspaceDataBase,
    expr: Expr<'db>,
    f: &mut dyn FnMut(Place<'db>, Use),
) {
    match expr.expr(db) {
        ExprKind::AddOperator { left, right, .. }
        | ExprKind::MultOperator { left, right, .. }
        | ExprKind::BooleanOperator { left, right, .. }
        | ExprKind::ComparisonOperator { left, right, .. }
        | ExprKind::PowerOperator { left, right } => {
            expression(db, *left, f);
            expression(db, *right, f);
        }
        ExprKind::UnaryOperator { expr, .. }
        | ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => {
            expression(db, *expr, f)
        }
        ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(access)) => {
            place(db, *access, Use::Read, f)
        }
        ExprKind::PrimaryExpr(PrimaryExpr::FuncCall(call)) => self::call(db, *call, f),
        ExprKind::PrimaryExpr(PrimaryExpr::RefValue { value }) => match value {
            RefValue::Address(begin) => {
                if let Some(path) = begin.expr(db) {
                    subscripts(db, path, f);
                }
                f(Place::Referenced(*begin), Use::Read);
            }
            RefValue::Value(expr) => expression(db, *expr, f),
            RefValue::Null => {}
        },
        ExprKind::PrimaryExpr(PrimaryExpr::Literal(_) | PrimaryExpr::EnumValue { .. })
        | ExprKind::FoldExpr { .. } => {}
    }
}

/// A place used as `how`: its subscripts are read first.
pub(crate) fn place<'db>(
    db: &'db dyn WorkspaceDataBase,
    access: VariableAccess<'db>,
    how: Use,
    f: &mut dyn FnMut(Place<'db>, Use),
) {
    if let VariableAccessKind::Symbolic(begin) = access.kind(db)
        && let Some(path) = begin.expr(db)
    {
        subscripts(db, path, f);
    }
    f(Place::Access(access), how);
}

/// The places a call names: the subscripts of what it calls, `fbs[i]()`,
/// its inputs, then its outputs.
pub(crate) fn call<'db>(
    db: &'db dyn WorkspaceDataBase,
    call: FuncCall<'db>,
    f: &mut dyn FnMut(Place<'db>, Use),
) {
    if let Some(path) = call.path(db).expr(db) {
        subscripts(db, path, f);
    }
    let params = call.params(db);
    for param in params {
        if !matches!(param.kind(db), ParamAssignKind::FormalOutput { .. }) {
            expression(db, param.kind(db).value(), f);
        }
    }
    for param in params {
        if let ParamAssignKind::FormalOutput { value, .. } = param.kind(db) {
            match value.expr(db) {
                ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(access)) => {
                    place(db, *access, Use::Write, f)
                }
                _ => expression(db, value, f),
            }
        }
    }
}

/// The expressions inside a path's brackets, outermost first.
fn subscripts<'db>(
    db: &'db dyn WorkspaceDataBase,
    path: PathExpr<'db>,
    f: &mut dyn FnMut(Place<'db>, Use),
) {
    match path.expr(db) {
        PathExprKind::Field(field) => subscripts(db, field.path, f),
        PathExprKind::Deref(deref) => subscripts(db, deref.path, f),
        PathExprKind::Index(index) => {
            subscripts(db, index.path, f);
            for expr in index.index {
                expression(db, expr, f);
            }
        }
        PathExprKind::VarAccess(_) => {}
    }
}

/// Every place a statement names, nested statements included, in the
/// order they are written.
pub(crate) fn statements<'db>(
    db: &'db dyn WorkspaceDataBase,
    stmts: &[Stmt<'db>],
    f: &mut dyn FnMut(Place<'db>, Use),
) {
    for stmt in stmts {
        statement(db, *stmt, f);
    }
}

fn statement<'db>(
    db: &'db dyn WorkspaceDataBase,
    stmt: Stmt<'db>,
    f: &mut dyn FnMut(Place<'db>, Use),
) {
    match stmt.stmt(db) {
        StmtKind::Assignment { var, target } => {
            expression(db, *target, f);
            place(db, *var, Use::Write, f);
        }
        StmtKind::FuncCall(call) => self::call(db, *call, f),
        StmtKind::EmptyPathExpression(begin) => {
            if let Some(path) = begin.expr(db) {
                subscripts(db, path, f);
            }
        }
        StmtKind::If {
            condition,
            then,
            else_if,
            else_,
        } => {
            expression(db, *condition, f);
            statements(db, then.as_deref().unwrap_or_default(), f);
            for (condition, stmts) in else_if {
                expression(db, *condition, f);
                statements(db, stmts, f);
            }
            statements(db, else_.as_deref().unwrap_or_default(), f);
        }
        StmtKind::Case {
            condition,
            cases,
            else_,
        } => {
            expression(db, *condition, f);
            for (labels, stmts) in cases {
                for label in labels {
                    case_label(db, *label, f);
                }
                statements(db, stmts, f);
            }
            statements(db, else_.as_deref().unwrap_or_default(), f);
        }
        StmtKind::For {
            control_variable,
            start,
            end,
            step,
            body,
        } => {
            expression(db, *start, f);
            expression(db, *end, f);
            if let Some(step) = step {
                expression(db, *step, f);
            }
            place(db, *control_variable, Use::Write, f);
            statements(db, body, f);
        }
        StmtKind::While { condition, body } => {
            expression(db, *condition, f);
            statements(db, body, f);
        }
        StmtKind::Repeat { condition, body } => {
            statements(db, body, f);
            expression(db, *condition, f);
        }
        StmtKind::Raise { message } => expression(db, *message, f),
        StmtKind::Try {
            body,
            catch,
            finally,
        } => {
            statements(db, body, f);
            if let Some(catch) = catch {
                if let Some(variable) = catch.variable {
                    place(db, variable, Use::Write, f);
                }
                statements(db, &catch.body, f);
            }
            statements(db, finally.as_deref().unwrap_or_default(), f);
        }
        StmtKind::Return
        | StmtKind::Exit
        | StmtKind::Continue
        | StmtKind::WasmPragma(_)
        | StmtKind::AllowPragma(_) => {}
    }
}

/// A CASE label is a constant, or a range of two.
pub(crate) fn case_label<'db>(
    db: &'db dyn WorkspaceDataBase,
    label: CaseKind<'db>,
    f: &mut dyn FnMut(Place<'db>, Use),
) {
    match label {
        CaseKind::Expression(expr) => expression(db, expr, f),
        CaseKind::Subrange { lower, upper } => {
            expression(db, lower, f);
            expression(db, upper, f);
        }
    }
}
