// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    CallSite, HasName, HirNodeInfo,
    hir_def::{
        expressions::expression::{
            Expr, ExprKind, PathExpr, PathExprKind, PrimaryExpr, VariableAccess, VariableAccessKind,
        },
        interned::identifier::Ident,
        pous::variable::{VariableDecl, VariableKind},
        scope::ScopeId,
    },
    hir_ty::{
        body::{ParamBinding, ScopeInference},
        expr_store::PathExprWalkStep,
        index_graphs::external_var_lookup,
        infer::const_eval::spec_value,
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};
use rustc_hash::FxHashSet;

use super::{double_writer::callee_scope, self_comparison::resolve_variable};

pub const NAME: &str = "in-out-alias";

/// L0125: one call binds the same storage to two by-reference parameters.
struct InOutAlias;

impl ErrorCode for InOutAlias {
    fn code(&self) -> &'static str {
        "L0125"
    }
}

/// One step of a place, `buf.items[3]^`: a variable or a block's member, a
/// STRUCT's field, a subscript, a dereference.
#[derive(PartialEq)]
enum Step<'db> {
    Variable(VariableDecl<'db>),
    Field(Ident),
    Index(Vec<Subscript<'db>>),
    Deref,
}

/// A subscript, as far as two of them can be told equal before the call.
#[derive(PartialEq)]
enum Subscript<'db> {
    Constant(i128),
    Variable(VariableDecl<'db>),
    Other,
}

/// A by-reference argument: the parameter, the argument as written (a
/// VAR_IN_OUT's value or an output's `=> dest`), and the storage it names.
struct Bound<'db> {
    param: VariableDecl<'db>,
    written: CallSite<'db>,
    place: Vec<Step<'db>>,
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    // In source order: the map is not.
    let mut calls: Vec<_> = body.resolved_calls().collect();
    calls.sort_by_key(|(call, _)| call.path(db).get_span(db).start_byte);
    for (call, resolved) in calls {
        // The globals the callee reaches itself, by VAR_EXTERNAL or by name,
        // in its body or in what it calls; worked out once a global is bound.
        let mut reached: Option<FxHashSet<VariableDecl<'db>>> = None;
        // What reaches the callee by reference: a VAR_IN_OUT, and an output,
        // which a FUNCTION writes in place and an FB copies out at the end.
        let mut bound: Vec<Bound<'db>> = Vec::new();
        for (param, binding) in &resolved.params {
            let (written, path) = match binding {
                ParamBinding::Values(values) if param.is_in_out(db) => {
                    let [value] = values.as_slice() else {
                        continue;
                    };
                    let Some(path) = value_path(db, value) else {
                        continue;
                    };
                    (value.as_call_site(db), path)
                }
                ParamBinding::Output { variable, .. } => {
                    let Some(path) = access_path(db, *variable) else {
                        continue;
                    };
                    (variable.as_call_site(db), path)
                }
                _ => continue,
            };
            let place = place(db, body, path);
            if place.is_empty() {
                continue;
            }
            // Reported once, against the first binding it meets.
            if let Some(first) = bound.iter().find(|b| overlap(&b.place, &place)) {
                diagnostics.push(report(db, first, *param, written));
            } else if let Some(Step::Variable(root)) = place.first()
                && let Some(global) = global_of(db, *root)
                && reached
                    .get_or_insert_with(|| {
                        let mut reached = FxHashSet::default();
                        if let Some(callee) = callee_scope(db, resolved.callable) {
                            globals_reached(db, callee, &mut FxHashSet::default(), &mut reached);
                        }
                        reached
                    })
                    .contains(&global)
            {
                diagnostics.push(report_global(db, call, *param, written));
            }
            bound.push(Bound {
                param: *param,
                written,
                place,
            });
        }
    }
}

fn report<'db>(
    db: &'db dyn WorkspaceDataBase,
    first: &Bound<'db>,
    param: VariableDecl<'db>,
    written: CallSite<'db>,
) -> IdeDiagnostic {
    let text = written.to_string(db);
    let first_text = first.written.to_string(db);
    let first_param = first.param.get_name_with_case(db).text(db);
    let mut d = diag()
        .message(format!(
            "'{text}' is passed to '{}' and to '{first_param}'",
            param.get_name_with_case(db).text(db)
        ))
        .desc(&InOutAlias)
        .range(
            hir::denormalize(db, written.scope.file(db), &written.get_span(db)).unwrap_or_default(),
        )
        .severity(DiagnosticSeverity::WARNING)
        .call();
    d.with_related(Related::new(
        format!("'{first_text}' is passed to '{first_param}' here"),
        first.written.scope.file(db),
        first.written.get_span(db),
    ));
    d.with_note(
        "both parameters reach the same storage, so a write through one changes what the other reads"
            .to_string(),
    );
    d.with_help("pass two different variables, or copy one first".to_string());
    d
}

fn report_global<'db>(
    db: &'db dyn WorkspaceDataBase,
    call: hir::hir_def::expressions::expression::FuncCall<'db>,
    param: VariableDecl<'db>,
    written: CallSite<'db>,
) -> IdeDiagnostic {
    let text = written.to_string(db);
    let callee = call.path(db).to_string(db);
    let mut d = diag()
        .message(format!(
            "'{text}' passed to '{}' is a global '{callee}' also reaches",
            param.get_name_with_case(db).text(db)
        ))
        .desc(&InOutAlias)
        .range(
            hir::denormalize(db, written.scope.file(db), &written.get_span(db)).unwrap_or_default(),
        )
        .severity(DiagnosticSeverity::WARNING)
        .call();
    d.with_note(
        "the parameter and the global are the same storage, so a write through one changes what the other reads"
            .to_string(),
    );
    d.with_help(format!(
        "pass a copy, or let '{callee}' reach it one way only"
    ));
    d
}

/// The global a variable names: a VAR_EXTERNAL's, or the VAR_GLOBAL itself
/// when it is named directly.
fn global_of<'db>(
    db: &'db dyn WorkspaceDataBase,
    var: VariableDecl<'db>,
) -> Option<VariableDecl<'db>> {
    match var.kind(db) {
        VariableKind::External => external_var_lookup(db, var.get_name_ident(db)),
        VariableKind::Global => Some(var),
        _ => None,
    }
}

/// Every global `scope` uses, and the scopes it calls use.
fn globals_reached<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    visited: &mut FxHashSet<ScopeId<'db>>,
    reached: &mut FxHashSet<VariableDecl<'db>>,
) {
    if !visited.insert(scope) {
        return;
    }
    let body = scope.inference(db);
    reached.extend(body.variables_used().filter_map(|var| global_of(db, var)));
    for (_, resolved) in body.resolved_calls() {
        if let Some(callee) = callee_scope(db, resolved.callable) {
            globals_reached(db, callee, visited, reached);
        }
    }
}

/// The path of a VAR_IN_OUT argument, which E0806 holds to a variable.
fn value_path<'db>(db: &'db dyn WorkspaceDataBase, value: &Expr<'db>) -> Option<PathExpr<'db>> {
    match value.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(access)) => access_path(db, *access),
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => value_path(db, expr),
        _ => None,
    }
}

fn access_path<'db>(
    db: &'db dyn WorkspaceDataBase,
    access: VariableAccess<'db>,
) -> Option<PathExpr<'db>> {
    match access.kind(db) {
        VariableAccessKind::Symbolic(begin) => begin.expr(db),
        _ => None,
    }
}

/// The steps of a place, or nothing when a step is not a variable the body
/// bound: such a place is never reported.
fn place<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    path: PathExpr<'db>,
) -> Vec<Step<'db>> {
    let mut steps = Vec::new();
    for step in path.flatten(db) {
        steps.push(match step {
            PathExprWalkStep::Field { expr, ident } => match body.variable_for_path_expr(*expr) {
                Some(var) => Step::Variable(var),
                // A STRUCT's field declares no variable: its name, under
                // the parent the steps before named.
                None if !steps.is_empty() => Step::Field(ident.ident(db)),
                None => return Vec::new(),
            },
            PathExprWalkStep::Index { expr } => match expr.expr(db) {
                PathExprKind::Index(index) => Step::Index(
                    index
                        .index
                        .iter()
                        .map(|sub| subscript(db, body, sub))
                        .collect(),
                ),
                _ => return Vec::new(),
            },
            PathExprWalkStep::Deref { .. } => Step::Deref,
        });
    }
    steps
}

fn subscript<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    sub: &Expr<'db>,
) -> Subscript<'db> {
    if let Some(value) = spec_value(db, *sub) {
        return Subscript::Constant(value);
    }
    match resolve_variable(db, body, sub) {
        Some(var) => Subscript::Variable(var),
        None => Subscript::Other,
    }
}

/// Whether two places share storage: one is the other, or a part of it.
/// Subscripts are equal when they are the same constant or the same
/// variable, both read before the call; any other pair may differ, and is
/// not reported: `swap(a := v[i], b := v[j])` is how a sort swaps.
fn overlap(a: &[Step<'_>], b: &[Step<'_>]) -> bool {
    a.iter().zip(b).all(|pair| match pair {
        (Step::Variable(x), Step::Variable(y)) => x == y,
        (Step::Field(x), Step::Field(y)) => x == y,
        (Step::Index(x), Step::Index(y)) => {
            x.len() == y.len()
                && x.iter().zip(y).all(|subs| match subs {
                    (Subscript::Constant(p), Subscript::Constant(q)) => p == q,
                    (Subscript::Variable(p), Subscript::Variable(q)) => p == q,
                    _ => false,
                })
        }
        // The same reference, dereferenced twice: the same target.
        (Step::Deref, Step::Deref) => true,
        _ => false,
    })
}
