use std::collections::HashMap;

use auto_lsp::{
    core::{
        errors::{LexerError, ParseError, ParseErrorAccumulator},
        span::Span,
    },
    default::db::{BaseDatabase, file::File},
    lsp_types::{DiagnosticSeverity, WorkspaceEdit},
    tree_sitter::{self, Range},
};
use ide_diagnostic::{IdeDiagnostic, Related, action, diag, edit};

use crate::check::errors::analysis_error::{AnalysisError, ToIdeDiagnostic};

impl From<SyntaxError> for AnalysisError<'_> {
    fn from(err: SyntaxError) -> Self {
        AnalysisError::SyntaxError(err)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum SyntaxError {
    InvalidPouKeyword(Span),
    MultipleExtends(Span),
    MultipleImplements(Span),
    ImplementsBeforeExtends(Span),
    MissingVarType(Span),
    UnexpectedVarInit(Span),
    IncompleteEdgeQualifier(Span),
    InvocationInExpression(Span),
    UnexpectedThis(Span),
    AssignToFunctionCall(Span),
    EmptyRightHandSide(Span),
    MissingDotInAssignment {
        file: File,
        span: Span,
    },
    MissingEqualInAssignment {
        file: File,
        span: Span,
    },
    MissingDotInForList {
        file: File,
        span: Span,
    },
    MissingEqualInForList {
        file: File,
        span: Span,
    },
    FunctionCallInInitExpression(Span),
    // tree-sitter
    MissingNode {
        file: File,
        span: Span,
        err: String,
        grammar_name: &'static str,
    },
    // todo: use custom lexer to handle syntax errors unhandled by tree-sitter
    SyntaxError {
        span: Span,
        err: String,
    },
}

impl<'db> From<(File, &ParseErrorAccumulator)> for AnalysisError<'db> {
    fn from((file, err): (File, &ParseErrorAccumulator)) -> Self {
        match &err.0 {
            ParseError::LexerError { span, error } => match error {
                LexerError::Missing {
                    range,
                    error,
                    grammar_name,
                } => AnalysisError::SyntaxError(SyntaxError::MissingNode {
                    file,
                    span: range.into(),
                    err: error.to_owned(),
                    grammar_name,
                }),
                LexerError::Syntax {
                    range,
                    error,
                    affected,
                } => AnalysisError::SyntaxError(SyntaxError::SyntaxError {
                    span: range.into(),
                    err: error.to_owned(),
                }),
            },
            _ => unreachable!("Only lexer errors should be present here"),
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for SyntaxError {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            Self::MultipleExtends(span) => diag()
                .message("multiple extends declarations".into())
                .severity(DiagnosticSeverity::ERROR)
                .range(span.clone())
                .call(),
            Self::MultipleImplements(span) => diag()
                .message("multiple implements declarations".into())
                .severity(DiagnosticSeverity::ERROR)
                .range(span.clone())
                .call(),
            Self::ImplementsBeforeExtends(span) => diag()
                .message("implements must be declared after extends".into())
                .severity(DiagnosticSeverity::ERROR)
                .range(span.clone())
                .call(),
            Self::MissingVarType(span) => diag()
                .message("variable type is missing".into())
                .severity(DiagnosticSeverity::ERROR)
                .range(span.clone())
                .call(),
            Self::IncompleteEdgeQualifier(span) => diag()
                .message("incomplete edge qualifier".into())
                .severity(DiagnosticSeverity::ERROR)
                .range(span.clone())
                .call(),
            Self::UnexpectedVarInit(span) => diag()
                .message("unexpected variable initialization".into())
                .severity(DiagnosticSeverity::ERROR)
                .range(span.clone())
                .call(),
            Self::InvocationInExpression(span) => diag()
                .message("invocation in expression is not allowed".into())
                .severity(DiagnosticSeverity::ERROR)
                .range(span.clone())
                .call(),
            Self::UnexpectedThis(span) => diag()
                .message("'this' is not valid in this context".into())
                .severity(DiagnosticSeverity::ERROR)
                .range(span.clone())
                .call(),
            Self::AssignToFunctionCall(span) => diag()
                .message("assignment to function call is not allowed".into())
                .severity(DiagnosticSeverity::ERROR)
                .range(span.clone())
                .call(),
            Self::EmptyRightHandSide(span) => diag()
                .message("right-hand side of assignment cannot be empty".into())
                .severity(DiagnosticSeverity::ERROR)
                .range(span.clone())
                .call(),
            Self::MissingDotInAssignment { file, span } => {
                let mut diag = diag()
                    .message("'=' is not a valid assignment sign".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .range(span.clone())
                    .call();

                // only replace '=' with ':='
                let range = Range {
                    start_byte: span.start_byte,
                    end_byte: span.start_byte + 1,
                    start_point: span.start_point,
                    end_point: tree_sitter::Point {
                        row: span.start_point.row,
                        column: span.start_point.column + 1,
                    },
                };

                diag.with_fix(
                    action()
                        .title("replace '=' with ':='".into())
                        .kind(auto_lsp::lsp_types::CodeActionKind::QUICKFIX)
                        .diagnostics(vec![diag.inner()])
                        .is_preferred(true)
                        .edit(WorkspaceEdit::new(HashMap::from([(
                            file.url(db).clone(),
                            vec![edit().new_text(":=".to_string()).range(range.into()).call()],
                        )])))
                        .call(),
                );

                diag
            }
            Self::MissingEqualInAssignment { file, span } => {
                let mut diag = diag()
                    .message("':' is not a valid assignment sign".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .range(span.clone())
                    .call();

                // only replace '=' with ':='
                let range = Range {
                    start_byte: span.start_byte,
                    end_byte: span.start_byte + 1,
                    start_point: span.start_point,
                    end_point: tree_sitter::Point {
                        row: span.start_point.row,
                        column: span.start_point.column + 1,
                    },
                };

                diag.with_fix(
                    action()
                        .title("replace ':' with ':='".into())
                        .kind(auto_lsp::lsp_types::CodeActionKind::QUICKFIX)
                        .diagnostics(vec![diag.inner()])
                        .is_preferred(true)
                        .edit(WorkspaceEdit::new(HashMap::from([(
                            file.url(db).clone(),
                            vec![edit().new_text(":=".to_string()).range(range.into()).call()],
                        )])))
                        .call(),
                );

                diag
            }
            Self::MissingDotInForList { file, span } => {
                let mut diag = diag()
                    .message("'=' is not a valid assignment sign".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .range(span.clone())
                    .call();

                // only replace '=' with ':='
                let range = Range {
                    start_byte: span.start_byte,
                    end_byte: span.start_byte + 1,
                    start_point: span.start_point,
                    end_point: tree_sitter::Point {
                        row: span.start_point.row,
                        column: span.start_point.column + 1,
                    },
                };

                diag.with_fix(
                    action()
                        .title("replace '=' with ':='".into())
                        .kind(auto_lsp::lsp_types::CodeActionKind::QUICKFIX)
                        .diagnostics(vec![diag.inner()])
                        .is_preferred(true)
                        .edit(WorkspaceEdit::new(HashMap::from([(
                            file.url(db).clone(),
                            vec![edit().new_text(":=".to_string()).range(range.into()).call()],
                        )])))
                        .call(),
                );

                diag
            }
            Self::MissingEqualInForList { file, span } => {
                let mut diag = diag()
                    .message("':' is not a valid assignment sign".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .range(span.clone())
                    .call();

                // only replace '=' with ':='
                let range = Range {
                    start_byte: span.start_byte,
                    end_byte: span.start_byte + 1,
                    start_point: span.start_point,
                    end_point: tree_sitter::Point {
                        row: span.start_point.row,
                        column: span.start_point.column + 1,
                    },
                };

                diag.with_fix(
                    action()
                        .title("replace ':' with ':='".into())
                        .kind(auto_lsp::lsp_types::CodeActionKind::QUICKFIX)
                        .diagnostics(vec![diag.inner()])
                        .is_preferred(true)
                        .edit(WorkspaceEdit::new(HashMap::from([(
                            file.url(db).clone(),
                            vec![edit().new_text(":=".to_string()).range(range.into()).call()],
                        )])))
                        .call(),
                );

                diag
            }
            Self::InvalidPouKeyword(span) => diag()
                .message("invalid POU keyword".into())
                .severity(DiagnosticSeverity::ERROR)
                .range(span.clone())
                .call(),
            Self::FunctionCallInInitExpression(span) => diag()
                .message("function call in initialization expression is not allowed".into())
                .severity(DiagnosticSeverity::ERROR)
                .range(span.clone())
                .call(),
            Self::MissingNode {
                file,
                span,
                err,
                grammar_name,
            } => {
                let mut diagnostic = diag()
                    .range(span.clone())
                    .message(err.to_string())
                    .source("IEC".into())
                    .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                    .call();

                diagnostic.with_related(Related::new(
                    format!("add missing {grammar_name} here"),
                    *file,
                    span.clone(),
                ));

                // If the grammar name is an identifier, suggest inserting it.
                if grammar_name.len() == 1 {
                    diagnostic.with_fix(
                        action()
                            .title(format!("insert missing '{grammar_name}'"))
                            .kind(auto_lsp::lsp_types::CodeActionKind::QUICKFIX)
                            .diagnostics(vec![diagnostic.inner()])
                            .is_preferred(true)
                            .edit(WorkspaceEdit::new(HashMap::from([(
                                file.url(db).clone(),
                                vec![
                                    edit()
                                        .new_text(format!(" {grammar_name}"))
                                        .range(span.clone())
                                        .call(),
                                ],
                            )])))
                            .call(),
                    );
                };
                diagnostic
            }
            Self::SyntaxError { span, err } => diag()
                .message(err.to_string())
                .severity(DiagnosticSeverity::ERROR)
                .range(span.clone())
                .call(),
        }
    }
}
