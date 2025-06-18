use std::{collections::HashMap, ops::Deref, sync::Arc};

use auto_lsp::{
    core::errors::ParseErrorAccumulator,
    default::db::{tracked::get_ast, BaseDatabase, File},
    lsp_types::{
        self, CodeAction, CodeActionKind, DiagnosticRelatedInformation, DiagnosticSeverity,
        DiagnosticTag, NumberOrString, WorkspaceEdit,
    },
    tree_sitter,
};

use crate::diagnostics::{
    duplicates::duplicate_declarations, lexer::add_fixes_to_parse_errors, lints::get_duplicates_by_query,
};

pub mod duplicates;
pub mod lexer;
pub mod lints;
pub mod diagnostic_builder;

#[derive(Debug, Clone)]
pub struct IdeDiagnostic {
    pub diagnostic: auto_lsp::lsp_types::Diagnostic,
    pub fixes: Vec<auto_lsp::lsp_types::CodeAction>,
}

impl IdeDiagnostic {
    pub fn new(diagnostic: auto_lsp::lsp_types::Diagnostic) -> Self {
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

impl From<auto_lsp::lsp_types::Diagnostic> for IdeDiagnostic {
    fn from(d: auto_lsp::lsp_types::Diagnostic) -> Self {
        IdeDiagnostic::new(d)
    }
}

impl From<&ParseErrorAccumulator> for IdeDiagnostic {
    fn from(e: &ParseErrorAccumulator) -> Self {
        IdeDiagnostic::new(e.0.clone().into())
    }
}

#[salsa::accumulator]
pub struct DiagnosticAccumulator(pub IdeDiagnostic);

impl From<auto_lsp::lsp_types::Diagnostic> for DiagnosticAccumulator {
    fn from(d: auto_lsp::lsp_types::Diagnostic) -> Self {
        DiagnosticAccumulator(d.into())
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
    let mut parse_errors = get_ast::accumulated::<ParseErrorAccumulator>(db, file);
    let lexer_errors = add_fixes_to_parse_errors(db, &file, &mut parse_errors);
    let lints = get_duplicates_by_query::accumulated::<DiagnosticAccumulator>(db, file);

    let mut uncached_diags = vec![];
    duplicate_declarations(db, file, &mut uncached_diags);

    let mut all_diagnostics = vec![];
    all_diagnostics.extend(lexer_errors.into_iter().map(|d| d.into()));
    all_diagnostics.extend(lints.into_iter().map(|d| d.into()));
    all_diagnostics.extend(uncached_diags.into_iter().map(|d| d.into()));

    DiagnosticResults(Arc::new(all_diagnostics))
}
