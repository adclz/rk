use std::collections::HashMap;

use auto_lsp::{
    core::errors::{LexerError, ParseError, ParseErrorAccumulator},
    default::db::file::File,
    lsp_types::{DiagnosticSeverity, WorkspaceEdit},
    tree_sitter::{self, Range},
};
use db::WorkspaceDataBase;
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, action, diag, edit};

use crate::check::errors::ToIdeDiagnostic;

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum SyntaxError {
    MultipleExtends {
        /// Span of the second (duplicate) EXTENDS clause
        location: Range,
        /// Span of the first EXTENDS clause
        first_extend_span: Range,
        /// File containing both clauses
        file: File,
    },
    MultipleImplements {
        /// Span of the second (duplicate) IMPLEMENTS clause
        location: Range,
        /// Span of the first IMPLEMENTS clause
        first_implements_span: Range,
        /// File containing both clauses
        file: File,
    },
    ImplementsBeforeExtends {
        /// Span of the misplaced IMPLEMENTS clause
        implements_span: Range,
        /// Span of the EXTENDS clause
        extends_span: Range,
        file: File,
    },
    ClassVariablesAfterMethod {
        /// Span of the misplaced variables
        var_span: Range,
        /// Span of the first METHOD
        method_span: Range,
        file: File,
    },
    FbVariablesAfterMethod {
        /// Span of the misplaced variables
        var_span: Range,
        /// Span of the first METHOD
        method_span: Range,
        file: File,
    },
    MissingVarType(Range),
    UnexpectedVarInit(Range),
    IncompleteEdgeQualifier(Range),
    UnexpectedThis(Range),
    UnexpectedSuper(Range),
    AssignToFunctionCall(Range),
    /// Comma-separated indices `a[i, j]` used in an access context. The comma
    /// form is valid only in array initializers; element access must chain:
    /// `a[i][j]`.
    CommaIndexAccess(Range),
    EmptyRightHandSide(Range),
    MissingDotInAssignment {
        file: File,
        span: Range,
    },
    MissingEqualInAssignment {
        file: File,
        span: Range,
    },
    MissingDotInForList {
        file: File,
        span: Range,
    },
    MissingEqualInForList {
        file: File,
        span: Range,
    },
    FunctionCallInInitExpression(Range),
    OutputAssignInAssignment {
        file: File,
        span: Range,
    },
    OutputAssignInForList {
        file: File,
        span: Range,
    },
    ProgramNotAllowedInNamespace(Range),
    ConfigNotAllowedInNamespace(Range),
    // tree-sitter
    MissingNode {
        file: File,
        span: Range,
        err: String,
        grammar_name: &'static str,
    },
    VarInOutNotAllowed(Range),
    VarTempNotAllowed(Range),
    VarAccessNotAllowed(Range),
    VarConfigNotAllowed(Range),
    VarLocatedNotAllowed(Range),
    VarExternalNotAllowed(Range),
    VarGlobalNotAllowed(Range),
    VarNotAllowed(Range),
    SingleAfterInterval(Range),
    IntervalAfterPriority(Range),
    SingleAfterPriority(Range),
    MissingPriority(Range),
    ArrayConformandNotSupported(Range),
    AccessSpecNotAllowedInMethodPrototype(Range),
    MethodDeclInBody(Range),
    // todo: use custom lexer to handle syntax errors unhandled by tree-sitter
    SyntaxError {
        span: Range,
        err: String,
    },
}

impl ErrorCode for SyntaxError {
    fn code(&self) -> &'static str {
        match self {
            SyntaxError::MultipleExtends { .. } => "E0001",
            SyntaxError::MultipleImplements { .. } => "E0002",
            SyntaxError::ImplementsBeforeExtends { .. } => "E0003",
            SyntaxError::ClassVariablesAfterMethod { .. } => "E0004",
            SyntaxError::FbVariablesAfterMethod { .. } => "E0005",
            SyntaxError::MissingVarType(_) => "E0006",
            SyntaxError::UnexpectedVarInit(_) => "E0007",
            SyntaxError::IncompleteEdgeQualifier(_) => "E0008",
            SyntaxError::UnexpectedThis(_) => "E0009",
            SyntaxError::UnexpectedSuper(_) => "E0010",
            SyntaxError::AssignToFunctionCall(_) => "E0011",
            SyntaxError::CommaIndexAccess(_) => "E0018",
            SyntaxError::EmptyRightHandSide(_) => "E0012",
            SyntaxError::MissingDotInAssignment { .. } => "E0013",
            SyntaxError::MissingEqualInAssignment { .. } => "E0014",
            SyntaxError::MissingDotInForList { .. } => "E0015",
            SyntaxError::MissingEqualInForList { .. } => "E0016",
            SyntaxError::FunctionCallInInitExpression(_) => "E0017",
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
            ParseError::LexerError { error, .. } => match error {
                LexerError::Missing {
                    range,
                    error,
                    grammar_name,
                } => SyntaxError::MissingNode {
                    file,
                    span: *range,
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
                    SyntaxError::SyntaxError { span: *range, err }
                }
            },
            _ => unreachable!("Only lexer errors should be present here"),
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for SyntaxError {
    fn to_diagnostic(
        &self,
        db: &'db dyn WorkspaceDataBase,
        file: auto_lsp::default::db::file::File,
    ) -> IdeDiagnostic {
        match self {
            Self::MultipleExtends {
                location,
                first_extend_span,
                file,
            } => {
                let doc = file.document(db);
                let src = doc.as_str();

                let extract = |span: &Range| -> String {
                    src.get(span.start_byte..span.end_byte)
                        .unwrap_or("")
                        .replace("EXTENDS ", "")
                        .trim()
                        .to_string()
                };
                let second = extract(location);
                let first = extract(first_extend_span);

                let mut diag = diag()
                    .message("multiple EXTENDS declarations are not allowed".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, location).unwrap_or_default())
                    .call();

                diag.with_related(Related::new(
                    format!("merge into single clause: 'EXTENDS {first}, {second}'"),
                    *file,
                    *first_extend_span,
                ));
                diag
            }
            Self::MultipleImplements {
                location,
                first_implements_span,
                file,
            } => {
                let doc = file.document(db);
                let src = doc.as_str();

                let extract = |span: &Range| -> String {
                    src.get(span.start_byte..span.end_byte)
                        .unwrap_or("")
                        .replace("IMPLEMENTS ", "")
                        .trim()
                        .to_string()
                };
                let second = extract(location);
                let first = extract(first_implements_span);

                let mut diag = diag()
                    .message("multiple IMPLEMENTS declarations are not allowed".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, location).unwrap_or_default())
                    .call();

                diag.with_related(Related::new(
                    format!("merge into single clause: 'IMPLEMENTS {first}, {second}'"),
                    *file,
                    *first_implements_span,
                ));
                diag
            }
            Self::ImplementsBeforeExtends {
                implements_span,
                extends_span,
                file,
            } => {
                let doc = file.document(db);
                let src = doc.as_str();

                let extract = |span: &Range| -> String {
                    src.get(span.start_byte..span.end_byte)
                        .unwrap_or("")
                        .trim()
                        .to_string()
                };
                let implements_text = extract(implements_span);

                let mut diag = diag()
                    .message("IMPLEMENTS must be declared after EXTENDS".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, implements_span).unwrap_or_default())
                    .call();

                diag.with_related(Related::new(
                    format!("move '{implements_text}' here"),
                    *file,
                    *extends_span,
                ));
                diag
            }
            Self::ClassVariablesAfterMethod {
                file,
                var_span,
                method_span,
            } => {
                // We do not want to highlight the first method, just the start point
                let method_span_start = Range {
                    start_byte: method_span.start_byte,
                    end_byte: method_span.start_byte,
                    start_point: method_span.start_point,
                    end_point: method_span.start_point,
                };

                let mut diag = diag()
                    .message("CLASS variable declarations must appear before methods".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, var_span).unwrap_or_default())
                    .call();

                diag.with_related(Related::new(
                    "move variables before methods here".into(),
                    *file,
                    method_span_start,
                ));
                diag
            }
            Self::FbVariablesAfterMethod {
                file,
                var_span,
                method_span,
            } => {
                // We do not want to highlight the first method, just the start point
                let method_span_start = Range {
                    start_byte: method_span.start_byte,
                    end_byte: method_span.start_byte,
                    start_point: method_span.start_point,
                    end_point: method_span.start_point,
                };

                let mut diag = diag()
                    .message("FB variable declarations must appear before methods".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, var_span).unwrap_or_default())
                    .call();

                diag.with_related(Related::new(
                    "move variables before methods here".into(),
                    *file,
                    method_span_start,
                ));
                diag
            }
            Self::MissingVarType(span) => diag()
                .message("variable type is missing".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::IncompleteEdgeQualifier(span) => diag()
                .message("incomplete edge qualifier".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::UnexpectedVarInit(span) => diag()
                .message("unexpected variable initialization".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::UnexpectedThis(span) => diag()
                .message("'THIS' is not valid in this context".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::UnexpectedSuper(span) => diag()
                .message("'SUPER' is not valid in this context".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::AssignToFunctionCall(span) => diag()
                .message("assignment to function call is not allowed".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::CommaIndexAccess(span) => diag()
                .message(
                    "comma-separated indices are not allowed in array access; \
                     use chained subscripts, e.g. `a[i][j]`"
                        .into(),
                )
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::EmptyRightHandSide(span) => diag()
                .message("right-hand side of assignment cannot be empty".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::MissingDotInAssignment { file, span } => {
                let mut diag = diag()
                    .message("'=' is not a valid assignment sign".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
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
                            vec![
                                edit()
                                    .new_text(":=".to_string())
                                    .range(crate::denormalize(db, file, &range).unwrap_or_default())
                                    .call(),
                            ],
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
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
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
                            vec![
                                edit()
                                    .new_text(":=".to_string())
                                    .range(crate::denormalize(db, file, &range).unwrap_or_default())
                                    .call(),
                            ],
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
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
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
                            vec![
                                edit()
                                    .new_text(":=".to_string())
                                    .range(crate::denormalize(db, file, &range).unwrap_or_default())
                                    .call(),
                            ],
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
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
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
                            vec![
                                edit()
                                    .new_text(":=".to_string())
                                    .range(crate::denormalize(db, file, &range).unwrap_or_default())
                                    .call(),
                            ],
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
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
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
                            vec![
                                edit()
                                    .new_text(":=".to_string())
                                    .range(crate::denormalize(db, file, &range).unwrap_or_default())
                                    .call(),
                            ],
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
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
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
                            vec![
                                edit()
                                    .new_text(":=".to_string())
                                    .range(crate::denormalize(db, file, &range).unwrap_or_default())
                                    .call(),
                            ],
                        )])))
                        .call(),
                );

                diag
            }
            Self::FunctionCallInInitExpression(span) => diag()
                .message("function call in initialization expression is not allowed".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::MissingNode {
                file,
                span,
                err,
                grammar_name,
            } => {
                let mut diagnostic = diag()
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
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
                                        .range(
                                            crate::denormalize(db, file, span).unwrap_or_default(),
                                        )
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
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::ConfigNotAllowedInNamespace(span) => diag()
                .message("configs are not allowed in namespaces".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::VarInOutNotAllowed(span) => {
                let mut diag = diag()
                    .message("VAR_IN_OUT is not allowed in this context".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
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
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
                    .call();

                diag.with_note("VAR_TEMP can only be used inside FUNCTION, FUNCTION_BLOCK".into());
                diag
            }
            Self::VarAccessNotAllowed(span) => {
                let mut diag = diag()
                    .message("VAR_ACCESS is not allowed in this context".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
                    .call();

                diag.with_note("VAR_ACCESS can only be used inside PROGRAM".into());
                diag
            }
            Self::VarConfigNotAllowed(span) => {
                let mut diag = diag()
                    .message("VAR_CONFIG is not allowed in this context".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
                    .call();

                diag.with_note("VAR_CONFIG can only be used inside CONFIGURATION".into());
                diag
            }
            Self::VarLocatedNotAllowed(span) => {
                let mut diag = diag()
                    .message("VAR_LOCATED is not allowed in this context".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
                    .call();

                diag.with_note("VAR_LOCATED can only be used inside PROGRAM".into());
                diag
            }
            Self::VarExternalNotAllowed(span) => {
                let mut diag = diag()
                    .message("VAR_EXTERNAL is not allowed in this context".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
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
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
                    .call();

                diag.with_note("VAR_GLOBAL can only be used inside PROGRAM, CONFIGURATION".into());
                diag
            }
            Self::VarNotAllowed(span) => {
                let mut diag = diag()
                    .message("VAR is not allowed in this context".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
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
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::IntervalAfterPriority(span) => diag()
                .message("INTERVAL cannot be declared after PRIORITY".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::SingleAfterPriority(span) => diag()
                .message("SINGLE cannot be declared after PRIORITY".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::MissingPriority(span) => diag()
                .message("PRIORITY is required in TASK configuration".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::ArrayConformandNotSupported(span) => diag()
                .message("array conformands (ARRAY[*]) are not supported".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::AccessSpecNotAllowedInMethodPrototype(span) => {
                let mut diag = diag()
                    .message(
                        "access specifiers are not allowed on interface method prototypes".into(),
                    )
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
                    .call();

                diag.with_note("interface methods are implicitly PUBLIC".into());
                diag
            }
            Self::MethodDeclInBody(span) => diag()
                .message("method declarations are not allowed inside a body".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::SyntaxError { span, err } => diag()
                .message(err.to_string())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
        }
    }
}
