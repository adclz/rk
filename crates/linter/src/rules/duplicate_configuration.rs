use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use hir::HasName;
use hir::hir_def::{config::ConfigDecl, semantic_index::semantic_index};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};
use rustc_hash::FxHashMap;

pub const NAME: &str = "duplicate-configuration";

/// L0112: same-named CONFIGURATION blocks in one file could be merged.
struct DuplicateConfiguration;

impl ErrorCode for DuplicateConfiguration {
    fn code(&self) -> &'static str {
        "L0112"
    }

    fn description(&self) -> &'static str {
        "duplicate configuration in same file"
    }
}

/// Same-named CONFIGURATION blocks are fragments and merge, which is what lets
/// VAR_GLOBALs live in their own file. Two fragments in the SAME file separate
/// nothing, whatever they hold — merging them loses no ability.
///
/// A lint, not an error: the code is valid and means what it says. Mirrors
/// `duplicate-namespace` (L0109), which says the same about reopening a
/// namespace twice in one file and likewise does not inspect the bodies.
pub fn check<'db>(
    db: &'db dyn WorkspaceDataBase,
    file: auto_lsp::default::db::file::File,
    diagnostics: &mut Vec<IdeDiagnostic>,
) {
    let sema = semantic_index(db, file);

    let mut seen: FxHashMap<_, Vec<ConfigDecl<'db>>> = FxHashMap::default();
    for config in sema.configs.iter() {
        seen.entry(config.get_name_ident(db).caseless(db))
            .or_default()
            .push(*config);
    }

    for decls in seen.values() {
        if decls.len() < 2 {
            continue;
        }
        // Grouped folded, named as the first declaration spelled it.
        let name = decls[0].get_name_ident(db).text(db);

        // Report the reopenings, pointing back at the first.
        for config in &decls[1..] {
            let mut d = diag()
                .message(format!(
                    "CONFIGURATION '{name}' is declared multiple times in this file, consider merging"
                ))
                .desc(&DuplicateConfiguration)
                .range(hir::denormalize(db, file, &config.get_name_span(db)).unwrap_or_default())
                .severity(DiagnosticSeverity::INFORMATION)
                .call();
            d.with_related(ide_diagnostic::Related {
                message: format!("first declaration of '{name}' here"),
                file,
                range: decls[0].get_name_span(db),
            });
            diagnostics.push(d);
        }
    }
}
