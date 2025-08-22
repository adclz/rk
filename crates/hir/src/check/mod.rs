use auto_lsp::{
    core::errors::ParseErrorAccumulator,
    default::db::{BaseDatabase, file::File, tracked::get_ast},
};
use ide_diagnostic::IdeDiagnostic;

use crate::check::lexer::add_fixes_to_parse_errors;

pub mod duplicates;
pub mod errors;
pub mod hir;
pub mod lexer;
pub mod literals;

pub fn cached_diagnostics(db: &dyn BaseDatabase, file: File) -> Vec<IdeDiagnostic> {
    let lexer_errors = add_fixes_to_parse_errors(
        db,
        &file,
        &mut get_ast::accumulated::<ParseErrorAccumulator>(db, file),
    );

    //let uncached_diags = duplicate_declarations::accumulated::<DiagnosticAccumulator>(db, file);

    let mut all_diagnostics = vec![];
    all_diagnostics.extend(lexer_errors);
    //all_diagnostics.extend(uncached_diags.into_iter().map(|d| d.into()));

    all_diagnostics
}
