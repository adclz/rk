// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{FuncCall, PathExpr, PathExprKind, VariableAccessKind},
            statement::{Stmt, StmtKind},
        },
        pous::variable::{VariableDecl, VariableKind},
    },
    hir_ty::{
        body::ScopeInference,
        ty::{CallableType, Type},
    },
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};
use rustc_hash::FxHashSet;

pub const NAME: &str = "missing-input-param";

/// L0208: a FUNCTION_BLOCK or PROGRAM call does not pass every declared
/// VAR_INPUT. This is *not* a hard error — the FB/PROGRAM
/// instance retains the previous value (or compiler-initialised default).
/// We surface it as a lint so the user is notified that not all inputs
/// were wired. FUNCTION/METHOD callsites are covered by `E0802` instead,
/// so this lint deliberately skips them to avoid overlap.
struct MissingInputParam;

impl ErrorCode for MissingInputParam {
    fn code(&self) -> &'static str {
        "L0208"
    }
}

/// The inputs a body writes through an instance, `t.IN := TRUE`: the
/// variables the instance's path names, and the input.
pub type WrittenInputs<'db> = FxHashSet<(Vec<VariableDecl<'db>>, VariableDecl<'db>)>;

/// Every input `stmts` write through an instance, in any branch: before the
/// call or after it, the value is the instance's at its next call.
pub fn written_inputs<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    stmts: &[Stmt<'db>],
) -> WrittenInputs<'db> {
    let mut written = FxHashSet::default();
    collect_written_inputs(db, body, stmts, &mut written);
    written
}

fn collect_written_inputs<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    stmts: &[Stmt<'db>],
    written: &mut WrittenInputs<'db>,
) {
    for stmt in stmts {
        match stmt.stmt(db) {
            StmtKind::Assignment { var, .. } => {
                if let VariableAccessKind::Symbolic(begin) = var.kind(db)
                    && let Some(path) = begin.expr(db)
                    && let PathExprKind::Field(field) = path.expr(db)
                    && let Some(member) = body.variable_for_path_expr(path)
                    && member.kind(db) == VariableKind::Input
                {
                    written.insert((instance_path(db, body, field.path), member));
                }
            }
            StmtKind::If {
                then,
                else_if,
                else_,
                ..
            } => {
                collect_written_inputs(db, body, then.as_deref().unwrap_or_default(), written);
                for (_, stmts) in else_if {
                    collect_written_inputs(db, body, stmts, written);
                }
                collect_written_inputs(db, body, else_.as_deref().unwrap_or_default(), written);
            }
            StmtKind::Case { cases, else_, .. } => {
                for (_, stmts) in cases {
                    collect_written_inputs(db, body, stmts, written);
                }
                collect_written_inputs(db, body, else_.as_deref().unwrap_or_default(), written);
            }
            StmtKind::For { body: stmts, .. }
            | StmtKind::While { body: stmts, .. }
            | StmtKind::Repeat { body: stmts, .. } => {
                collect_written_inputs(db, body, stmts, written);
            }
            _ => {}
        }
    }
}

/// The variables `path` names, step by step: `cells[i].t` is `[cells, t]`.
/// A call and a write name the same instance when these match.
fn instance_path<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    path: PathExpr<'db>,
) -> Vec<VariableDecl<'db>> {
    path.flatten(db)
        .iter()
        .filter_map(|step| body.variable_for_path_expr(step.get_expr(db)))
        .collect()
}

pub fn check_func_call<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    stmt: Stmt<'db>,
    func_call: FuncCall<'db>,
    written: &WrittenInputs<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let typ = body.type_of_begin_path_expr_adjusted(func_call.path(db));
    let callable = match typ {
        Type::CallableType(ct) => ct,
        _ => match typ.as_callable(db) {
            Some(ct) => ct,
            None => return,
        },
    };

    // FUNCTION and METHOD calls are covered by the hard-error E0802 path.
    // Linting them would duplicate that diagnostic.
    if !matches!(callable, CallableType::FunctionBlock(_)) {
        return;
    }

    // The flattened EXTENDS view: inherited VAR_INPUTs are wirable (and thus
    // lintable) at a derived FB's call site, exactly as the resolver binds them.
    let formals = hir::hir_ty::resolver::func_call::call_site_params(db, callable);
    let has_variadic = formals.values().any(|v| v.variadic(db));
    if has_variadic {
        return;
    }

    let matched_vars: rustc_hash::FxHashSet<_> = func_call
        .params(db)
        .iter()
        .filter_map(|param| body.variable_for_param(*param))
        .collect();

    // `t.IN := TRUE; t();` passes IN as `t(IN := TRUE)` does.
    let instance = func_call
        .path(db)
        .expr(db)
        .map(|path| instance_path(db, body, path))
        .unwrap_or_default();
    let missing: Vec<_> = formals
        .values()
        .filter(|var| var.kind(db) == VariableKind::Input && !matched_vars.contains(var))
        .filter(|var| !written.contains(&(instance.clone(), **var)))
        .copied()
        .collect();

    if missing.is_empty() {
        return;
    }

    let callable_name = callable.get_name_with_case(db).text(db);
    let names: Vec<_> = missing
        .iter()
        .map(|v| format!("'{}'", v.name_with_case(db).text(db)))
        .collect();

    let mut d = diag()
        .message(format!(
            "call to '{}' is missing {} input parameter{}: {}",
            callable_name,
            missing.len(),
            if missing.len() > 1 { "s" } else { "" },
            names.join(", "),
        ))
        .desc(&MissingInputParam)
        .range(
            hir::denormalize(db, stmt.get_scope_id(db).file(db), &stmt.get_span(db))
                .unwrap_or_default(),
        )
        .severity(DiagnosticSeverity::INFORMATION)
        .call();

    let file = callable.get_scope_id(db).file(db);
    for var in &missing {
        d.with_related(Related::new(
            format!("'{}' is declared here", var.name_with_case(db).text(db)),
            file,
            var.get_span(db),
        ));
    }

    diagnostics.push(d);
}
