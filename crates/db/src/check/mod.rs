use std::{ops::Deref, sync::Arc};

use auto_lsp::{
    core::errors::ParseErrorAccumulator,
    default::db::{file::File, tracked::get_ast, BaseDatabase},
};

use crate::check::{duplicates::duplicate_declarations, lexer::add_fixes_to_parse_errors};

pub mod diagnostic_builder;
pub mod duplicates;
pub mod lexer;
pub mod literals;
pub mod signature;
pub mod errors;

#[derive(Clone)]
pub struct IdeDiagnostic {
    pub diagnostic: auto_lsp::lsp_types::Diagnostic,
    pub fixes: Vec<auto_lsp::lsp_types::CodeAction>,
}

impl IdeDiagnostic {
    pub fn new(diagnostic: auto_lsp::lsp_types::Diagnostic, file: File) -> Self {
        Self {
            diagnostic,
            fixes: vec![],
        }
    }

    pub fn with_fix(&mut self, fix: auto_lsp::lsp_types::CodeAction) {
        self.fixes.push(fix);
    }
}

impl From<IdeDiagnostic> for auto_lsp::lsp_types::Diagnostic {
    fn from(d: IdeDiagnostic) -> Self {
        d.diagnostic
    }
}

impl From<&IdeDiagnostic> for auto_lsp::lsp_types::Diagnostic {
    fn from(d: &IdeDiagnostic) -> Self {
        d.diagnostic.clone()
    }
}

impl From<(File, auto_lsp::lsp_types::Diagnostic)> for IdeDiagnostic {
    fn from((f, d): (File, auto_lsp::lsp_types::Diagnostic)) -> Self {
        IdeDiagnostic::new(d, f)
    }
}

impl From<(File, &ParseErrorAccumulator)> for IdeDiagnostic {
    fn from((f, e): (File, &ParseErrorAccumulator)) -> Self {
        IdeDiagnostic::new(e.0.clone().into(), f)
    }
}

#[salsa::accumulator]
pub struct DiagnosticAccumulator(pub IdeDiagnostic);

impl From<(File, auto_lsp::lsp_types::Diagnostic)> for DiagnosticAccumulator {
    fn from((f, d): (File, auto_lsp::lsp_types::Diagnostic)) -> Self {
        DiagnosticAccumulator((f, d).into())
    }
}

impl From<IdeDiagnostic> for DiagnosticAccumulator {
    fn from(d: IdeDiagnostic) -> Self {
        DiagnosticAccumulator(d)
    }
}

impl From<&DiagnosticAccumulator> for IdeDiagnostic {
    fn from(error: &DiagnosticAccumulator) -> Self {
        IdeDiagnostic {
            diagnostic: error.0.diagnostic.clone(),
            fixes: error.0.fixes.clone(),
        }
    }
}

impl From<&DiagnosticAccumulator> for auto_lsp::lsp_types::Diagnostic {
    fn from(error: &DiagnosticAccumulator) -> Self {
        error.0.diagnostic.clone()
    }
}

pub struct DiagnosticResults(Arc<Vec<IdeDiagnostic>>);

impl PartialEq for DiagnosticResults {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Deref for DiagnosticResults {
    type Target = Vec<IdeDiagnostic>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub fn cached_diagnostics(db: &dyn BaseDatabase, file: File) -> DiagnosticResults {
    let lexer_errors = add_fixes_to_parse_errors(
        db,
        &file,
        &mut get_ast::accumulated::<ParseErrorAccumulator>(db, file),
    );

    let uncached_diags = duplicate_declarations::accumulated::<DiagnosticAccumulator>(db, file);

    let mut all_diagnostics = vec![];
    all_diagnostics.extend(lexer_errors.into_iter());
    all_diagnostics.extend(uncached_diags.into_iter().map(|d| d.into()));

    DiagnosticResults(Arc::new(all_diagnostics))
}
