// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{
        expressions::expression::{PathExpr, VariableAccess, VariableAccessKind},
        pous::variable::VariableKind,
    },
    hir_ty::{body::ScopeInference, expr_store::PathExprWalkStep, ty::Type},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

pub const NAME: &str = "external-mutation";

/// L0117: a variable of a function block or class instance is modified from outside.
/// Instances should own their own data.
struct ExternalMutation;

impl ErrorCode for ExternalMutation {
    fn code(&self) -> &'static str {
        "L0117"
    }
}

/// Called by the stmt_visitor for each place written: `fb.x := 42`, and
/// `fb.x[0] := 1` or `fb.s.a := 1`, which write a part of `fb.x` or `fb.s`.
pub fn check_assignment<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    var: VariableAccess<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let VariableAccessKind::Symbolic(begin) = var.kind(db) else {
        return;
    };
    // THIS.field.x is internal OOP access, not external mutation
    if begin.invocation(db).is_some() {
        return;
    }
    let Some(path) = begin.expr(db) else {
        return;
    };
    if !enters_an_instance(db, body, path) {
        return;
    }
    let place = var.as_call_site(db).to_string(db);
    diagnostics.push(
        diag()
            .message(format!("'{place}' is written from outside its instance"))
            .desc(&ExternalMutation)
            .range(
                hir::denormalize(db, var.get_scope_id(db).file(db), &var.get_span(db))
                    .unwrap_or_default(),
            )
            .severity(DiagnosticSeverity::WARNING)
            .call(),
    );
}

/// Whether `path` names a member of an instance on its way, other than an
/// input. An input is the caller's to set: `fb.x := 42; fb();` passes it the
/// way `fb(x := 42)` does. A VAR_IN_OUT the block keeps as a member is an
/// input written back, and the caller's to set the same way. Past a `^`,
/// what is written is what the reference points at, so only the steps after
/// the last one count.
fn enters_an_instance<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    path: PathExpr<'db>,
) -> bool {
    let steps = path.flatten(db);
    let after_deref = steps
        .iter()
        .rposition(|step| matches!(step, PathExprWalkStep::Deref { .. }))
        .map_or(0, |deref| deref + 1);
    (after_deref.max(1)..steps.len()).any(|i| {
        let PathExprWalkStep::Field { expr, .. } = steps[i] else {
            return false;
        };
        let receiver = body
            .type_of_path_expr_adjusted(steps[i - 1].get_expr(db))
            .normalize(db);
        if !matches!(receiver, Type::FunctionBlock(_) | Type::Class(_)) {
            return false;
        }
        match body.type_of_path_expr(expr) {
            Type::Variable((member, _)) => {
                member.kind(db) != VariableKind::Input
                    && !hir::hir_ty::layout::in_out_is_member(db, member)
            }
            _ => false,
        }
    })
}
