// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::expressions::expression::{Expr, ExprKind, PrimaryExpr, VariableAccess},
    hir_ty::{
        body::{ParamBinding, ScopeInference},
        infer::normalize::{declared_capacity_of, string_capacity},
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "string-truncation";

/// L0127: a STRING variable into a STRING declared shorter.
struct StringTruncation;

impl ErrorCode for StringTruncation {
    fn code(&self) -> &'static str {
        "L0127"
    }
}

/// `short := long`.
pub fn check_assignment<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    target: VariableAccess<'db>,
    value: &Expr<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let Some(capacity) = declared_capacity_of(db, body.type_of_variable_access(target)) else {
        return;
    };
    let Some(length) = source_capacity(db, body, value) else {
        return;
    };
    if length <= capacity {
        return;
    }
    let target = target.as_call_site(db).to_string(db);
    diagnostics.push(report(
        db,
        value,
        format!("a STRING[{length}] is assigned to a STRING[{capacity}]"),
        capacity,
        format!("declare '{target}' as STRING[{length}]"),
    ));
}

/// `f(name := long)` into a shorter input, `f(o => short)` from a longer
/// output.
pub fn check_calls<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    // In source order: the map is not.
    let mut calls: Vec<_> = body.resolved_calls().collect();
    calls.sort_by_key(|(call, _)| call.path(db).get_span(db).start_byte);
    for (_, resolved) in calls {
        for (param, binding) in &resolved.params {
            let name = param.get_name_with_case(db).text(db);
            match binding {
                ParamBinding::Values(values) if param.is_input(db) => {
                    let Some(capacity) = string_capacity(db, param.spec(db)) else {
                        continue;
                    };
                    for value in values {
                        let Some(length) = source_capacity(db, body, value) else {
                            continue;
                        };
                        if length > capacity {
                            diagnostics.push(report(
                                db,
                                value,
                                format!(
                                    "a STRING[{length}] is passed to the STRING[{capacity}] '{name}'"
                                ),
                                capacity,
                                format!("declare '{name}' as STRING[{length}]"),
                            ));
                        }
                    }
                }
                ParamBinding::Output { variable, .. } => {
                    let Some(length) = string_capacity(db, param.spec(db)) else {
                        continue;
                    };
                    let Some(capacity) =
                        declared_capacity_of(db, body.type_of_variable_access(*variable))
                    else {
                        continue;
                    };
                    if length > capacity {
                        let target = variable.as_call_site(db).to_string(db);
                        diagnostics.push(report(
                            db,
                            variable,
                            format!(
                                "the STRING[{length}] '{name}' is bound to a STRING[{capacity}]"
                            ),
                            capacity,
                            format!("declare '{target}' as STRING[{length}]"),
                        ));
                    }
                }
                _ => {}
            }
        }
    }
}

/// The declared capacity of a STRING written as a variable or a field. A
/// call's result and a literal are not read: E0314 measures a literal, and
/// a result is as long as its type, STRING[255] for CONCAT.
fn source_capacity<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    value: &Expr<'db>,
) -> Option<u32> {
    match value.expr(db) {
        ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(_)) => {
            declared_capacity_of(db, body.type_of_expr(*value))
        }
        ExprKind::PrimaryExpr(PrimaryExpr::ParenthesizedExpr { expr }) => {
            source_capacity(db, body, expr)
        }
        _ => None,
    }
}

fn report<'db>(
    db: &'db dyn WorkspaceDataBase,
    at: &impl HirNodeInfo<'db>,
    message: String,
    capacity: u32,
    help: String,
) -> IdeDiagnostic {
    let mut d = diag()
        .message(message)
        .desc(&StringTruncation)
        .range(
            hir::denormalize(db, at.get_scope_id(db).file(db), &at.get_span(db))
                .unwrap_or_default(),
        )
        .severity(DiagnosticSeverity::WARNING)
        .call();
    d.with_note(format!("a text longer than {capacity} bytes is cut"));
    d.with_help(help);
    d
}
