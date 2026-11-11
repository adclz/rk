// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HasName, HirNodeInfo,
    hir_def::{
        expressions::{
            expression::{
                BeginPathExpr, ExprKind, PrimaryExpr, VariableAccess, VariableAccessKind,
            },
            statement::Stmt,
        },
        pous::{
            pou::Pou,
            variable::{VariableDecl, VariableKind},
        },
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{body::ScopeInference, expr_store::PathExprWalkStep},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};
use rustc_hash::FxHashSet;

use super::accesses::{self, Place, Use};

pub const NAME: &str = "uninitialized-output";

/// L0203: one or more VAR_OUTPUT variables are never assigned in the body.
///
/// All uninitialized outputs for a given POU body are collapsed into a single
/// diagnostic so the user gets one summary instead of N pointers. Leaving an
/// output unassigned is permitted for
/// FUNCTION_BLOCK / PROGRAM (compiler zero-inits the instance field) but
/// surfacing it as an INFO-level lint helps catch unintended omissions.
struct UninitializedOutput;

impl ErrorCode for UninitializedOutput {
    fn code(&self) -> &'static str {
        "L0203"
    }
}

/// The variables `statements` may write: assigned whole or in part, a counter,
/// bound to a call's output or in-out, or handed out by `REF()`.
pub fn written<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    statements: &[Stmt<'db>],
) -> FxHashSet<VariableDecl<'db>> {
    let symbolic = |access: VariableAccess<'db>| match access.kind(db) {
        VariableAccessKind::Symbolic(begin) => Some(begin),
        _ => None,
    };
    let mut written = FxHashSet::default();
    accesses::statements(db, statements, &mut |place, how| {
        let begin = match (place, how) {
            (Place::Access(access), Use::Write) => symbolic(access),
            (Place::Referenced(begin), _) => Some(begin),
            (Place::Argument(param), _)
                if body
                    .variable_for_param(param)
                    .is_some_and(|parameter| parameter.is_in_out(db)) =>
            {
                match param.kind(db).value().expr(db) {
                    ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(access)) => symbolic(*access),
                    _ => None,
                }
            }
            _ => None,
        };
        if let Some(variable) = begin.and_then(|begin| root_variable(db, body, begin)) {
            written.insert(variable);
        }
    });
    written
}

/// The variable a path starts from. Past a `^`, what it writes is what the
/// reference points at, and the reference is only read.
fn root_variable<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    begin: BeginPathExpr<'db>,
) -> Option<VariableDecl<'db>> {
    let path = begin.expr(db)?;
    let steps = path.flatten(db);
    if steps
        .iter()
        .any(|step| matches!(step, PathExprWalkStep::Deref { .. }))
    {
        return None;
    }
    body.variable_for_path_expr(steps.first()?.get_expr(db))
}

/// Check VAR_OUTPUT variables against the set of assigned variables.
/// Called after the unified visitor has finished collecting.
pub fn check_outputs<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope: ScopeId<'db>,
    assigned: &FxHashSet<VariableDecl<'db>>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    // An extern FUNCTION's outputs are fields of the import's result, which
    // the host writes; its body is empty by contract (E1502 refuses
    // statements).
    if let ScopeKind::Pou(Pou::Function(f)) = get_scope(db, scope).kind {
        use hir::HasPragmas;
        if f.extern_pragma(db).is_some() {
            return;
        }
    }
    let def_map = scope.def_map(db);
    // In the order written: this map has none, and its order changes with the
    // interned ids, so an edit moved the caret and reordered the message.
    let mut missing: Vec<_> = def_map
        .global_variables
        .values()
        .filter(|var| var.kind(db) == VariableKind::Output)
        .filter(|var| var.init(db).is_none())
        .filter(|var| !assigned.contains(*var))
        .copied()
        .collect();
    missing.sort_by_key(|var| var.get_name_span(db).start_byte);

    if missing.is_empty() {
        return;
    }

    let names: Vec<_> = missing
        .iter()
        .map(|v| format!("'{}'", v.name_with_case(db).text(db)))
        .collect();

    // Anchor the diagnostic on the first uninitialized output's name span.
    // The related-info entries cover the rest of the declarations.
    let anchor = missing[0].get_name_span(db);

    let mut d = diag()
        .message(format!(
            "{} VAR_OUTPUT {} never assigned in the body: {}",
            missing.len(),
            if missing.len() > 1 { "are" } else { "is" },
            names.join(", "),
        ))
        .desc(&UninitializedOutput)
        .range(
            hir::denormalize(db, missing[0].get_scope_id(db).file(db), &anchor).unwrap_or_default(),
        )
        .severity(DiagnosticSeverity::INFORMATION)
        .call();

    for var in &missing {
        d.with_related(Related::new(
            format!("'{}' is declared here", var.name_with_case(db).text(db)),
            var.scope_id(db).file(db),
            var.get_name_span(db),
        ));
    }

    diagnostics.push(d);
}
