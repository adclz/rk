
use std::collections::HashMap;

use auto_lsp::core::errors::{LexerError, ParseError, ParseErrorAccumulator};
use auto_lsp::lsp_types::{DiagnosticRelatedInformation, WorkspaceEdit};
use phf::phf_set;

use crate::diagnostics::diagnostic_builder::{action, diag, edit, OneOf};
use crate::diagnostics::IdeDiagnostic;

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
    file: &auto_lsp::default::db::File,
    errors: &mut Vec<&ParseErrorAccumulator>,
) -> Vec<IdeDiagnostic> {
    errors
        .iter_mut()
        .map(|error| match (*error).into() {
            ParseError::LexerError {
                range,
                error:
                    LexerError::Missing {
                        error: missing_error,
                        grammar_name,
                        ..
                    },
            } => {
                let mut diagnostic = diag()
                    .range(OneOf::U(range))
                    .message(missing_error.to_string())
                    .source("IEC".into())
                    .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                    .related_information(vec![DiagnosticRelatedInformation {
                        location: auto_lsp::lsp_types::Location {
                            uri: file.url(db).clone(),
                            range,
                        },
                        message: format!("help: add missing {grammar_name} here"),
                    }])
                    .call();

                diagnostic.with_fix(
                    action()
                        .title(format!("Insert missing '{grammar_name}'"))
                        .kind(auto_lsp::lsp_types::CodeActionKind::QUICKFIX)
                        .diagnostics(vec![diagnostic.diagnostic.clone()])
                        .is_preferred(true)
                        .edit(WorkspaceEdit::new(HashMap::from([(
                            file.url(db).clone(),
                            vec![edit()
                                .new_text(format!(" {}", grammar_name))
                                .range(OneOf::U(range))
                                .call()],
                        )])))
                        .call(),
                );
                diagnostic
            }
            ParseError::LexerError {
                range,
                error:
                    LexerError::Syntax {
                        error: syntax_error,
                        affected,
                        ..
                    },
            } => {
                if affected.len() == 1 {
                    let mut diagnostic = diag()
                        .range(OneOf::U(range))
                        .message(syntax_error.to_string())
                        .source("IEC".into())
                        .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                        .related_information(vec![DiagnosticRelatedInformation {
                            location: auto_lsp::lsp_types::Location {
                                uri: file.url(db).clone(),
                                range,
                            },
                            message: format!("help: remove '{affected}'"),
                        }])
                        .call();

                    diagnostic.with_fix(
                        action()
                            .title(format!("Remove {affected}"))
                            .kind(auto_lsp::lsp_types::CodeActionKind::QUICKFIX)
                            .diagnostics(vec![diagnostic.diagnostic.clone()])
                            .is_preferred(true)
                            .edit(WorkspaceEdit::new(HashMap::from([(
                                file.url(db).clone(),
                                vec![edit().new_text("".to_string()).range(OneOf::U(range)).call()],
                            )])))
                            .call(),
                    );
                    diagnostic
                } else if KEYWORDS.contains(affected.split_whitespace().next().unwrap_or("")) {
                    diag()
                        .range(OneOf::U(range))
                        .message(format!("{} is a reserved keyword that is not valid in this context", affected.split_whitespace().next().unwrap_or("")))
                        .source("IEC".into())
                        .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                        .related_information(vec![DiagnosticRelatedInformation {
                            location: auto_lsp::lsp_types::Location {
                                uri: file.url(db).clone(),
                                range,
                            },
                            message: format!("help: remove or replace '{}'", affected.split_whitespace().next().unwrap_or("")),
                        }])
                        .call()
                } else {
                    (*error).into()
                }
            }
            _ => (*error).into(),
        })
        .collect()
}


#[cfg(test)]
mod tests {
    use auto_lsp::{core::errors::ParseErrorAccumulator, default::db::{tracked::get_ast, BaseDatabase, FileManager}, lsp_types, texter::core::text::Text};
    use super::*;
    use crate::RootDatabase;

    #[test]
    fn missing_identifier() {
        
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let texter = Text::new(
            r#"
NAMESPACE 
END_NAMESPACE"#
                .into(),
        );

        db.add_file_from_texter(
            ast::RK_PARSER.get("structured_text").unwrap(),
            &url,
            texter,
        )
        .unwrap();

        let file = db.get_file(&url).unwrap();
        let mut diagnostics = get_ast::accumulated::<ParseErrorAccumulator>(&db, file);
        let lexer_errors = add_fixes_to_parse_errors(&db, &file, &mut diagnostics);

        assert_eq!(lexer_errors.len(), 1);
        assert_eq!(lexer_errors[0].diagnostic.message, "Syntax error: Missing 'identifier'");
    }

        #[test]
    fn unexpected_token() {
        
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let texter = Text::new(
            r#"
NAMESPACE ns :
END_NAMESPACE"#
                .into(),
        );

        db.add_file_from_texter(
            ast::RK_PARSER.get("structured_text").unwrap(),
            &url,
            texter,
        )
        .unwrap();

        let file = db.get_file(&url).unwrap();
        let mut diagnostics = get_ast::accumulated::<ParseErrorAccumulator>(&db, file);
        let lexer_errors = add_fixes_to_parse_errors(&db, &file, &mut diagnostics);

        assert_eq!(lexer_errors.len(), 1);
        assert_eq!(lexer_errors[0].diagnostic.message, "Unexpected token(s): ':'");
    }


    #[test]
    fn reserved_keyword() {
        
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let texter = Text::new(
            r#"
NAMESPACE first
    FUNCTION

            FUNCTION

    END_FUNCTION
END_NAMESPACE"#
                .into(),
        );

        db.add_file_from_texter(
            ast::RK_PARSER.get("structured_text").unwrap(),
            &url,
            texter,
        )
        .unwrap();

        let file = db.get_file(&url).unwrap();
        let mut diagnostics = get_ast::accumulated::<ParseErrorAccumulator>(&db, file);
        let lexer_errors = add_fixes_to_parse_errors(&db, &file, &mut diagnostics);

        assert_eq!(lexer_errors.len(), 1);
        assert_eq!(lexer_errors[0].diagnostic.message, "FUNCTION is a reserved keyword that is not valid in this context");
    }

       #[test]
    fn reserved_keyword_in_expression() {
        
        let mut db = RootDatabase::default();
        let url = lsp_types::Url::parse("file:///test.st").unwrap();
        let texter = Text::new(
            r#"
NAMESPACE first
    FUNCTION

            FUNCTION := something

    END_FUNCTION
END_NAMESPACE"#
                .into(),
        );

        db.add_file_from_texter(
            ast::RK_PARSER.get("structured_text").unwrap(),
            &url,
            texter,
        )
        .unwrap();

        let file = db.get_file(&url).unwrap();
        let mut diagnostics = get_ast::accumulated::<ParseErrorAccumulator>(&db, file);
        let lexer_errors = add_fixes_to_parse_errors(&db, &file, &mut diagnostics);

        assert_eq!(lexer_errors.len(), 1);
        assert_eq!(lexer_errors[0].diagnostic.message, "FUNCTION is a reserved keyword that is not valid in this context");
    }

}