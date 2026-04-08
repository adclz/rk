use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::{
    HirNodeInfo,
    hir_def::{namespace::NamespaceDecl, semantic_index::semantic_index},
};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};
use rustc_hash::FxHashMap;

pub const NAME: &str = "duplicate-namespace";

/// L0109: duplicate namespace declarations in the same file could be merged.
struct DuplicateNamespace;

impl ErrorCode for DuplicateNamespace {
    fn code(&self) -> &'static str {
        "L0109"
    }

    fn description(&self) -> &'static str {
        "duplicate namespace in same file"
    }
}

pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: auto_lsp::default::db::file::File,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let sema = semantic_index(db, file);

    // Group namespaces by their path
    let mut seen: FxHashMap<hir::hir_def::interned::namespace::NamespacePath, Vec<NamespaceDecl<'db>>> =
        FxHashMap::default();

    for ns in sema.namespaces.iter() {
        seen.entry(*ns.path(db)).or_default().push(*ns);
    }

    for (path, decls) in &seen {
        if decls.len() < 2 {
            continue;
        }

        let path_str = path.to_string(db);

        // Emit on every duplicate (skip the first)
        for ns in &decls[1..] {
            let mut d = diag()
                .message(format!(
                    "NAMESPACE '{path_str}' is declared multiple times in this file, consider merging"
                ))
                .desc(&DuplicateNamespace)
                .range(ns.name_span(db))
                .severity(DiagnosticSeverity::INFORMATION)
                .call();
            d.with_related(ide_diagnostic::Related {
                message: format!("first declaration of '{path_str}' here"),
                file,
                range: decls[0].name_span(db),
            });
            diagnostics.push(d);
        }
    }
}
