use std::collections::HashMap;

use auto_lsp::{
    core::{
        errors::{LexerError, ParseError, ParseErrorAccumulator},
        span::Span,
    },
    default::db::file::File,
    lsp_types::{DiagnosticSeverity, WorkspaceEdit},
    tree_sitter::{self, Range},
};
use db::WorkspaceDataBase;
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, action, diag, edit};

use crate::{HirNodeInfo, check::errors::ToIdeDiagnostic};

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum SyntaxError {
    InvalidPouKeyword(Span),
    MultipleExtends {
        /// Span of the second (duplicate) EXTENDS clause
        location: Span,
        /// Span of the first EXTENDS clause
        first_extend_span: Span,
        /// File containing both clauses
        file: File,
    },
    MultipleImplements(Span),
    ImplementsBeforeExtends(Span),
    ClassVariablesAfterMethod(Span),
    FbVariablesAfterMethod(Span),
    MissingVarType(Span),
    UnexpectedVarInit(Span),
    IncompleteEdgeQualifier(Span),
    UnexpectedThis(Span),
    UnexpectedSuper(Span),
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
    OutputAssignInAssignment {
        file: File,
        span: Span,
    },
    OutputAssignInForList {
        file: File,
        span: Span,
    },
    ProgramNotAllowedInNamespace(Span),
    ConfigNotAllowedInNamespace(Span),
    // tree-sitter
    MissingNode {
        file: File,
        span: Span,
        err: String,
        grammar_name: &'static str,
    },
    VarInOutNotAllowed(Span),
    VarTempNotAllowed(Span),
    VarAccessNotAllowed(Span),
    VarConfigNotAllowed(Span),
    VarLocatedNotAllowed(Span),
    VarExternalNotAllowed(Span),
    VarGlobalNotAllowed(Span),
    VarNotAllowed(Span),
    SingleAfterInterval(Span),
    IntervalAfterPriority(Span),
    SingleAfterPriority(Span),
    MissingPriority(Span),
    ArrayConformandNotSupported(Span),
    AccessSpecNotAllowedInMethodPrototype(Span),
    MethodDeclInBody(Span),
    // todo: use custom lexer to handle syntax errors unhandled by tree-sitter
    SyntaxError {
        span: Span,
        err: String,
    },
}

impl ErrorCode for SyntaxError {
    fn code(&self) -> &'static str {
        match self {
            SyntaxError::MultipleExtends { .. } => "E0001",
            SyntaxError::MultipleImplements(_) => "E0002",
            SyntaxError::ImplementsBeforeExtends(_) => "E0003",
            SyntaxError::ClassVariablesAfterMethod(_) => "E0004",
            SyntaxError::FbVariablesAfterMethod(_) => "E0005",
            SyntaxError::MissingVarType(_) => "E0006",
            SyntaxError::UnexpectedVarInit(_) => "E0007",
            SyntaxError::IncompleteEdgeQualifier(_) => "E0008",
            SyntaxError::UnexpectedThis(_) => "E0009",
            SyntaxError::UnexpectedSuper(_) => "E0010",
            SyntaxError::AssignToFunctionCall(_) => "E0011",
            SyntaxError::EmptyRightHandSide(_) => "E0012",
            SyntaxError::MissingDotInAssignment { .. } => "E0013",
            SyntaxError::MissingEqualInAssignment { .. } => "E0014",
            SyntaxError::MissingDotInForList { .. } => "E0015",
            SyntaxError::MissingEqualInForList { .. } => "E0016",
            SyntaxError::FunctionCallInInitExpression(_) => "E0017",
            SyntaxError::InvalidPouKeyword(_) => "E0018",
            SyntaxError::MissingNode { .. } => "E0019",
            SyntaxError::OutputAssignInAssignment { .. } => "E0020",
            SyntaxError::OutputAssignInForList { .. } => "E0021",
            SyntaxError::ProgramNotAllowedInNamespace(_) => "E0022",
            SyntaxError::ConfigNotAllowedInNamespace(_) => "E0023",
            SyntaxError::VarInOutNotAllowed(_) => "E0024",
            SyntaxError::VarTempNotAllowed(_) => "E0025",
            SyntaxError::VarAccessNotAllowed(_) => "E0026",
            SyntaxError::VarConfigNotAllowed(_) => "E0027",
            SyntaxError::VarLocatedNotAllowed(_) => "E0028",
            SyntaxError::VarExternalNotAllowed(_) => "E0029",
            SyntaxError::VarGlobalNotAllowed(_) => "E0030",
            SyntaxError::VarNotAllowed(_) => "E0031",
            SyntaxError::SingleAfterInterval(_) => "E0032",
            SyntaxError::IntervalAfterPriority(_) => "E0033",
            SyntaxError::SingleAfterPriority(_) => "E0034",
            SyntaxError::MissingPriority(_) => "E0035",
            SyntaxError::ArrayConformandNotSupported(_) => "E0036",
            SyntaxError::AccessSpecNotAllowedInMethodPrototype(_) => "E0037",
            SyntaxError::MethodDeclInBody(_) => "E0038",
            SyntaxError::SyntaxError { .. } => "E0050",
        }
    }

    fn description(&self) -> &'static str {
        "syntax"
    }
}

impl SyntaxError {
    pub fn from_parse_error(
        db: &dyn WorkspaceDataBase,
        file: File,
        err: &ParseErrorAccumulator,
    ) -> Self {
        match &err.0 {
            ParseError::LexerError { span, error } => match error {
                LexerError::Missing {
                    range,
                    error,
                    grammar_name,
                } => SyntaxError::MissingNode {
                    file,
                    span: range.into(),
                    err: error.to_owned(),
                    grammar_name,
                },
                LexerError::Syntax {
                    range,
                    error,
                    affected,
                } => {
                    // When auto-lsp can't extract named children from an ERROR node
                    // (e.g. token-level nodes like unsigned_int), the error text is empty.
                    // Fall back to reading the source text at the error range.
                    let err = if affected.is_empty() {
                        let doc = file.document(db).as_bytes();
                        let start = range.start_byte;
                        let end = range.end_byte;
                        if end <= doc.len() {
                            let text = String::from_utf8_lossy(&doc[start..end]);
                            format!("Unexpected token(s): '{}'", text.trim())
                        } else {
                            error.to_owned()
                        }
                    } else {
                        error.to_owned()
                    };
                    SyntaxError::SyntaxError {
                        span: range.into(),
                        err,
                    }
                }
            },
            _ => unreachable!("Only lexer errors should be present here"),
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for SyntaxError {
    fn to_diagnostic(&self, db: &'db dyn WorkspaceDataBase) -> IdeDiagnostic {
        match self {
            Self::MultipleExtends {
                location,
                first_extend_span,
                file,
            } => {
                let doc = file.document(db);
                let src = doc.as_str();

                let extract = |span: &Span| -> String {
                    src.get(span.start_byte..span.end_byte)
                        .unwrap_or("")
                        .replace("EXTENDS ", "")
                        .trim()
                        .to_string()
                };
                let second = extract(location);
                let first = extract(first_extend_span);

                let mut diag = diag()
                    .message("multiple extends declarations".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(*location)
                    .call();

                diag.with_related(Related::new(
                    format!("merge {second} with {first}: EXTENDS {first}, {second}"),
                    *file,
                    *first_extend_span,
                ));
                diag
            }
            Self::MultipleImplements(span) => diag()
                .message("multiple implements declarations".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(*span)
                .call(),
            Self::ImplementsBeforeExtends(span) => diag()
                .message("implements must be declared after extends".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(*span)
                .call(),
            Self::ClassVariablesAfterMethod(span) => diag()
                .message("class variable declarations must appear before methods".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(*span)
                .call(),
            Self::FbVariablesAfterMethod(span) => diag()
                .message("FB variable declarations must appear before methods".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(*span)
                .call(),
            Self::MissingVarType(span) => diag()
                .message("variable type is missing".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(*span)
                .call(),
            Self::IncompleteEdgeQualifier(span) => diag()
                .message("incomplete edge qualifier".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(*span)
                .call(),
            Self::UnexpectedVarInit(span) => diag()
                .message("unexpected variable initialization".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(*span)
                .call(),
            Self::UnexpectedThis(span) => diag()
                .message("'THIS' is not valid in this context".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(*span)
                .call(),
            Self::UnexpectedSuper(span) => diag()
                .message("'SUPER' is not valid in this context".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(*span)
                .call(),
            Self::AssignToFunctionCall(span) => diag()
                .message("assignment to function call is not allowed".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(*span)
                .call(),
            Self::EmptyRightHandSide(span) => diag()
                .message("right-hand side of assignment cannot be empty".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(*span)
                .call(),
            Self::MissingDotInAssignment { file, span } => {
                let mut diag = diag()
                    .message("'=' is not a valid assignment sign".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(*span)
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
                    .desc(self)
                    .range(*span)
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
                    .desc(self)
                    .range(*span)
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
                    .desc(self)
                    .range(*span)
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
            Self::OutputAssignInAssignment { file, span } => {
                let mut diag = diag()
                    .message("'=>' is not a valid assignment sign".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(*span)
                    .call();

                let range = Range {
                    start_byte: span.start_byte,
                    end_byte: span.start_byte + 2,
                    start_point: span.start_point,
                    end_point: tree_sitter::Point {
                        row: span.start_point.row,
                        column: span.start_point.column + 2,
                    },
                };

                diag.with_fix(
                    action()
                        .title("replace '=>' with ':='".into())
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
            Self::OutputAssignInForList { file, span } => {
                let mut diag = diag()
                    .message("'=>' is not a valid assignment sign".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(*span)
                    .call();

                let range = Range {
                    start_byte: span.start_byte,
                    end_byte: span.start_byte + 2,
                    start_point: span.start_point,
                    end_point: tree_sitter::Point {
                        row: span.start_point.row,
                        column: span.start_point.column + 2,
                    },
                };

                diag.with_fix(
                    action()
                        .title("replace '=>' with ':='".into())
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
                .desc(self)
                .range(*span)
                .call(),

            Self::FunctionCallInInitExpression(span) => diag()
                .message("function call in initialization expression is not allowed".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(*span)
                .call(),
            Self::MissingNode {
                file,
                span,
                err,
                grammar_name,
            } => {
                let mut diagnostic = diag()
                    .range(*span)
                    .message(err.to_string())
                    .source("IEC".into())
                    .desc(self)
                    .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                    .call();

                diagnostic.with_related(Related::new(
                    format!("add missing {grammar_name} here"),
                    *file,
                    *span,
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
                                        .range(*span)
                                        .call(),
                                ],
                            )])))
                            .call(),
                    );
                };
                diagnostic
            }
            Self::ProgramNotAllowedInNamespace(span) => diag()
                .message("programs are not allowed in namespaces".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(*span)
                .call(),
            Self::ConfigNotAllowedInNamespace(span) => diag()
                .message("configs are not allowed in namespaces".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(*span)
                .call(),
            Self::VarInOutNotAllowed(span) => {
                let mut diag = diag()
                    .message("VAR_IN_OUT is not allowed in this context".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(*span)
                    .call();

                diag.with_note(
                    "VAR_IN_OUT can only be used inside FUNCTION, FUNCTION_BLOCK".into(),
                );
                diag
            }
            Self::VarTempNotAllowed(span) => {
                let mut diag = diag()
                    .message("VAR_TEMP is not allowed in this context".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(*span)
                    .call();

                diag.with_note("VAR_TEMP can only be used inside FUNCTION, FUNCTION_BLOCK".into());
                diag
            }
            Self::VarAccessNotAllowed(span) => {
                let mut diag = diag()
                    .message("VAR_ACCESS is not allowed in this context".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(*span)
                    .call();

                diag.with_note("VAR_ACCESS can only be used inside PROGRAM".into());
                diag
            }
            Self::VarConfigNotAllowed(span) => {
                let mut diag = diag()
                    .message("VAR_CONFIG is not allowed in this context".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(*span)
                    .call();

                diag.with_note("VAR_CONFIG can only be used inside CONFIGURATION".into());
                diag
            }
            Self::VarLocatedNotAllowed(span) => {
                let mut diag = diag()
                    .message("VAR_LOCATED is not allowed in this context".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(*span)
                    .call();

                diag.with_note("VAR_LOCATED can only be used inside PROGRAM".into());
                diag
            }
            Self::VarExternalNotAllowed(span) => {
                let mut diag = diag()
                    .message("VAR_EXTERNAL is not allowed in this context".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(*span)
                    .call();

                diag.with_note(
                    "VAR_EXTERNAL can only be used inside PROGRAM, FUNCTION_BLOCK, FUNCTION".into(),
                );
                diag
            }
            Self::VarGlobalNotAllowed(span) => {
                let mut diag = diag()
                    .message("VAR_GLOBAL is not allowed in this context".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(*span)
                    .call();

                diag.with_note("VAR_GLOBAL can only be used inside PROGRAM, CONFIGURATION".into());
                diag
            }
            Self::VarNotAllowed(span) => {
                let mut diag = diag()
                    .message("VAR is not allowed in this context".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(*span)
                    .call();

                diag.with_note(
                    "VAR can only be used inside FUNCTION, FUNCTION_BLOCK, PROGRAM".into(),
                );
                diag
            }
            Self::SingleAfterInterval(span) => diag()
                .message("SINGLE cannot be declared after INTERVAL".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(*span)
                .call(),
            Self::IntervalAfterPriority(span) => diag()
                .message("INTERVAL cannot be declared after PRIORITY".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(*span)
                .call(),
            Self::SingleAfterPriority(span) => diag()
                .message("SINGLE cannot be declared after PRIORITY".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(*span)
                .call(),
            Self::MissingPriority(span) => diag()
                .message("PRIORITY is required in TASK configuration".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(*span)
                .call(),
            Self::ArrayConformandNotSupported(span) => diag()
                .message("array conformands (ARRAY[*]) are not supported".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(*span)
                .call(),
            Self::AccessSpecNotAllowedInMethodPrototype(span) => {
                let mut diag = diag()
                    .message(
                        "access specifiers are not allowed on interface method prototypes".into(),
                    )
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(*span)
                    .call();

                diag.with_note("interface methods are implicitly PUBLIC".into());
                diag
            }
            Self::MethodDeclInBody(span) => diag()
                .message("method declarations are not allowed inside a body".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(*span)
                .call(),
            Self::SyntaxError { span, err } => diag()
                .message(err.to_string())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(*span)
                .call(),
        }
    }
}
