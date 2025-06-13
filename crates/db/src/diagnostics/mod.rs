use auto_lsp::{core::errors::ParseErrorAccumulator, default::db::{tracked::get_ast, BaseDatabase, File}, lsp_types::Diagnostic};

use crate::diagnostics::lints::LintAccumulator;

pub mod lints;


#[salsa::tracked(no_eq, returns(ref))]
pub fn get_diagnostics(db: &dyn BaseDatabase, file: File) -> Vec<Diagnostic> {
    get_ast::accumulated::<auto_lsp::core::errors::ParseErrorAccumulator>(db, file);
    lints::query_lints(db, file);


    let parse_errors = get_ast::accumulated::<ParseErrorAccumulator>(db, file);
    let lints = get_lints::accumulated::<LintAccumulator>(db, file);

    let mut all_diagnostics = Vec::new();
    all_diagnostics.extend(parse_errors.into_iter().map(|d| d.into()));
    all_diagnostics.extend(lints.into_iter().map(|d| d.into()));

    all_diagnostics
}

#[salsa::tracked(no_eq, returns(ref))]
pub fn get_lints(db: &dyn BaseDatabase, file: File) {
    lints::query_lints(db, file);
}