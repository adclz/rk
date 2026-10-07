// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use std::collections::HashMap;

use auto_lsp::lsp_types::{CodeAction, DiagnosticSeverity, TextEdit, WorkspaceEdit};
use db::WorkspaceDataBase;
use hir::{
    CallSite, HasName, HirNodeInfo, hir_def::expressions::expression::ParamAssignKind,
    hir_ty::body::ScopeInference,
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

pub const NAME: &str = "positional-output";

/// L0313: a positional argument in the place of a VAR_OUTPUT. A positional
/// list gives every parameter in declaration order, outputs included, so
/// the variable there receives the output, and nothing at the call site
/// says it is written: `first(y, 4)` reads like two values going in when
/// `first` declares its output first. `o => y` says so.
struct PositionalOutput;

impl ErrorCode for PositionalOutput {
    fn code(&self) -> &'static str {
        "L0313"
    }
}

/// Every call of the body, statement or expression. An argument that is no
/// variable is E0805, not reported here.
pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    body: ScopeInference<'db>,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    for call in body.calls() {
        for param in call.params(db) {
            let ParamAssignKind::NonFormal { value } = param.kind(db) else {
                continue;
            };
            let Some(var) = body.variable_for_param(*param) else {
                continue;
            };
            if !var.is_output(db) || value.variable_access(db).is_none() {
                continue;
            }
            let output = var.name_with_case(db).text(db);
            let callee = CallSite::from_scoped(db, &call.path(db)).to_string(db);
            let written = CallSite::from_scoped(db, &value).to_string(db);
            let file = param.get_scope_id(db).file(db);
            let range = hir::denormalize(db, file, &value.get_span(db)).unwrap_or_default();

            let mut diagnostic = diag()
                .message(format!(
                    "'{written}' receives the output '{output}' of '{callee}'"
                ))
                .desc(&PositionalOutput)
                .range(range)
                .severity(DiagnosticSeverity::HINT)
                .call();
            diagnostic.with_note(
                "a positional list gives every parameter in declaration order, outputs included"
                    .to_string(),
            );
            diagnostic.with_related(Related::new(
                format!("'{output}' is declared here"),
                var.scope_id(db).file(db),
                var.get_name_span(db),
            ));
            let replacement = format!("{output} => {written}");
            diagnostic.with_fix(CodeAction {
                title: format!("write {replacement}"),
                edit: Some(WorkspaceEdit::new(HashMap::from([(
                    file.url(db).clone(),
                    vec![TextEdit {
                        range,
                        new_text: replacement,
                    }],
                )]))),
                is_preferred: Some(true),
                ..Default::default()
            });
            diagnostics.push(diagnostic);
        }
    }
}
