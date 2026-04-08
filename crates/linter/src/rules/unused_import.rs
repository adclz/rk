use auto_lsp::lsp_types::{DiagnosticSeverity, DiagnosticTag};
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{scope::ScopeId, using::Using},
    hir_ty::{body::infer_body, head::signature::infer_signature},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};
use rustc_hash::FxHashSet;

pub const NAME: &str = "unused-import";

/// L0201: USING directive is never used.
struct UnusedImport;

impl ErrorCode for UnusedImport {
    fn code(&self) -> &'static str {
        "L0201"
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
        let body = infer_body(db, *scope);
        all_used.extend(&body.usings_used);
    }

    // Report unused USINGs
    for using in file_usings {
        if all_used.contains(using) {
            continue;
        }

        let path = using.path(db);
        let name = path
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
                .range(using.get_span(db))
                .call(),
        );
    }
}
