use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::HirNodeInfo;
use hir::hir_def::{namespace::NamespaceDecl, semantic_index::semantic_index};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};
use rustc_hash::FxHashMap;

pub const NAME: &str = "duplicate-namespace";

/// L0204: duplicate namespace declarations in the same file could be merged.
struct DuplicateNamespace;

impl ErrorCode for DuplicateNamespace {
    fn code(&self) -> &'static str {
        "L0204"
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
    let mut seen: FxHashMap<
        hir::hir_def::interned::namespace::NamespacePath,
        Vec<NamespaceDecl<'db>>,
    > = FxHashMap::default();

    for ns in sema.namespaces.iter() {
        seen.entry(ns.path(db).caseless(db)).or_default().push(*ns);
    }

    for decls in seen.values() {
        if decls.len() < 2 {
            continue;
        }

        // Grouped by the FOLDED path, since a namespace reopened in another
        // case is the same namespace — but named by the spelling the first
        // declaration used, which is what the reader wrote.
        let path_str = decls[0].path(db).to_string(db);

        // Emit on every duplicate (skip the first)
        for ns in &decls[1..] {
            let mut d = diag()
                .message(format!(
                    "NAMESPACE '{path_str}' is declared multiple times in this file, consider merging"
                ))
                .desc(&DuplicateNamespace)
                .range(hir::denormalize(db, ns.get_scope_id(db).file(db), &ns.name_span(db)).unwrap_or_default())
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
