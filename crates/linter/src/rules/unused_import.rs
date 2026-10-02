// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use auto_lsp::lsp_types::{DiagnosticSeverity, DiagnosticTag};
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{scope::ScopeId, using::Using},
    hir_ty::head::signature::infer_signature,
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};
use rustc_hash::FxHashSet;

pub const NAME: &str = "unused-import";

/// L0301: USING directive is never used.
struct UnusedImport;

impl ErrorCode for UnusedImport {
    fn code(&self) -> &'static str {
        "L0301"
    }

    fn description(&self) -> &'static str {
        "unused import"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    all_scopes: &[ScopeId<'db>],
    body_scopes: &[ScopeId<'db>],
    file_usings: &[Using<'db>],
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    if file_usings.is_empty() {
        return;
    }

    // Collect all USINGs marked as used across all signature and body inferences in the file
    let mut all_used: FxHashSet<Using<'db>> = FxHashSet::default();
    for scope in all_scopes {
        let sig = infer_signature(db, *scope);
        all_used.extend(&sig.usings_used);
    }
    for scope in body_scopes {
        all_used.extend(scope.inference(db).usings_used());
    }

    // Report unused USINGs
    for using in file_usings {
        if all_used.contains(using) {
            continue;
        }

        // As written, for the message.
        let path = using.path(db);
        let name = path
            .path_with_case
            .fragments(db)
            .iter()
            .map(|f| f.text(db).as_str().to_owned())
            .collect::<Vec<_>>()
            .join(".");

        diagnostics.push(
            diag()
                .message(format!("unused import '{name}'"))
                .severity(DiagnosticSeverity::HINT)
                .tags(vec![DiagnosticTag::UNNECESSARY])
                .desc(&UnusedImport)
                .range(
                    hir::denormalize(db, using.get_scope_id(db).file(db), &using.get_span(db))
                        .unwrap_or_default(),
                )
                .call(),
        );
    }
}
