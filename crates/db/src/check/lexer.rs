use std::collections::HashMap;

use auto_lsp::core::errors::{LexerError, ParseError, ParseErrorAccumulator};
use auto_lsp::lsp_types::{DiagnosticRelatedInformation, WorkspaceEdit};
use phf::phf_set;

use crate::check::diagnostic_builder::{action, diag, edit};
use crate::check::errors::syntax_errors::{missing_node, unexpected_char, unexpected_keyword};
use crate::check::IdeDiagnostic;

static KEYWORDS: phf::Set<&'static str> = phf_set! {
"PROGRAM", "END_PROGRAM",
    "CONFIGURATION", "END_CONFIGURATION",
    "RESOURCE", "END_RESOURCE",
    "NAMESPACE", "END_NAMESPACE",
    "USING",
    "CLASS", "END_CLASS",
    "INTERFACE", "END_INTERFACE",
    "FUNCTION", "END_FUNCTION",
    "FUNCTION_BLOCK", "END_FUNCTION_BLOCK",
    "TYPE", "END_TYPE",
    "VAR", "END_VAR",
    "VAR_INPUT",
    "VAR_OUTPUT",
    "VAR_IN_OUT",
    "VAR_TEMP",
    "VAR_EXTERNAL",
    "VAR_GLOBAL",
    "IF", "THEN", "ELSE", "END_IF",
    "CASE", "OF", "END_CASE",
    "FOR", "TO", "BY", "END_FOR",
    "REPEAT", "UNTIL", "END_REPEAT",
    "WHILE", "END_WHILE",
    "DO",
    "EXIT", "RETURN",
    // Types
    "BOOL", "BYTE", "WORD", "DWORD", "LWORD",
    "SINT", "INT", "DINT", "LINT",
    "USINT", "UINT", "UDINT", "ULINT",
    "REAL", "LREAL",
    "CHAR", "WCHAR",
    "STRING", "WSTRING",
    "DATE", "TIME", "DT", "TOD", "LDATE", "LTIME", "LDT", "LTOD"
};

pub fn add_fixes_to_parse_errors(
    db: &dyn auto_lsp::default::db::BaseDatabase,
    file: &auto_lsp::default::db::file::File,
    errors: &mut Vec<&ParseErrorAccumulator>,
) -> Vec<IdeDiagnostic> {
    errors
        .iter_mut()
        .map(|error| match (*error).into() {
            ParseError::LexerError {
                span,
                error:
                    LexerError::Missing {
                        error: missing_error,
                        grammar_name,
                        ..
                    },
            } => missing_node(db, *file, span, &missing_error, grammar_name),
            ParseError::LexerError {
                span,
                error:
                    LexerError::Syntax {
                        error: syntax_error,
                        affected,
                        ..
                    },
            } => {
                if affected.len() == 1 {
                    unexpected_char(db, *file, span, &affected, &syntax_error)
                } else if KEYWORDS.contains(affected.split_whitespace().next().unwrap_or("")) {
                    unexpected_keyword(db, *file, span, &affected)
                } else {
                    (*file, *error).into()
                }
            }
            _ => (*file, *error).into(),
        })
        .collect()
}