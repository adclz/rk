use crate::check::errors::ToIdeDiagnostic;
use auto_lsp::core::errors::LexerError;
use auto_lsp::core::errors::ParseError;
use auto_lsp::core::errors::ParseErrorAccumulator;
use auto_lsp::default::db::file::File;
use auto_lsp::lsp_types::DiagnosticSeverity;
use auto_lsp::lsp_types::WorkspaceEdit;
use auto_lsp::tree_sitter;
use auto_lsp::tree_sitter::Range;
use db::WorkspaceDataBase;
use ide_diagnostic::ErrorCode;
use ide_diagnostic::IdeDiagnostic;
use ide_diagnostic::Related;
use ide_diagnostic::action;
use ide_diagnostic::diag;
use ide_diagnostic::edit;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum SyntaxError {
    // todo: use custom lexer to handle syntax errors unhandled by tree-sitter
    SyntaxError {
        span: Range,
        err: String,
    },
    // tree-sitter
    MissingNode {
        file: File,
        span: Range,
        err: String,
        grammar_name: &'static str,
    },
    MissingVarType(Range),
    UnexpectedVarInit(Range),
    EmptyRightHandSide(Range),
    MissingDotInAssignment {
        file: File,
        span: Range,
    },
    MissingEqualInAssignment {
        file: File,
        span: Range,
    },
    OutputAssignInAssignment {
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
    OutputAssignInForList {
        file: File,
        span: Range,
    },
    AssignInCondition {
        file: File,
        span: Range,
        /// The `:=` and the space around it, which is all the fix replaces.
        sign: Range,
    },
    AssignToFunctionCall(Range),
    IncompleteEdgeQualifier(Range),
    UnexpectedThis(Range),
    UnexpectedSuper(Range),
    VarNotAllowed(Range),
    VarInOutNotAllowed(Range),
    VarTempNotAllowed(Range),
    VarExternalNotAllowed(Range),
    VarGlobalNotAllowed(Range),
    VarAccessNotAllowed(Range),
    VarConfigNotAllowed(Range),
    ProgramNotAllowedInNamespace(Range),
    ConfigNotAllowedInNamespace(Range),
    FbVariablesAfterMethod {
        /// Span of the misplaced variables
        var_span: Range,
        /// Span of the first METHOD
        method_span: Range,
        file: File,
    },
    ClassVariablesAfterMethod {
        /// Span of the misplaced variables
        var_span: Range,
        /// Span of the first METHOD
        method_span: Range,
        file: File,
    },
    MethodDeclInBody(Range),
}

impl ErrorCode for SyntaxError {
    fn code(&self) -> &'static str {
        match self {
            Self::SyntaxError { .. } => "E0001",
            Self::MissingNode { .. } => "E0002",
            Self::MissingVarType(_) => "E0003",
            Self::UnexpectedVarInit(_) => "E0004",
            Self::EmptyRightHandSide(_) => "E0005",
            Self::MissingDotInAssignment { .. } => "E0006",
            Self::MissingEqualInAssignment { .. } => "E0007",
            Self::OutputAssignInAssignment { .. } => "E0008",
            Self::MissingDotInForList { .. } => "E0009",
            Self::MissingEqualInForList { .. } => "E0010",
            Self::OutputAssignInForList { .. } => "E0011",
            Self::AssignInCondition { .. } => "E0012",
            Self::AssignToFunctionCall(_) => "E0013",
            Self::IncompleteEdgeQualifier(_) => "E0014",
            Self::UnexpectedThis(_) => "E0015",
            Self::UnexpectedSuper(_) => "E0016",
            Self::VarNotAllowed(_) => "E0017",
            Self::VarInOutNotAllowed(_) => "E0018",
            Self::VarTempNotAllowed(_) => "E0019",
            Self::VarExternalNotAllowed(_) => "E0020",
            Self::VarGlobalNotAllowed(_) => "E0021",
            Self::VarAccessNotAllowed(_) => "E0022",
            Self::VarConfigNotAllowed(_) => "E0023",
            Self::ProgramNotAllowedInNamespace(_) => "E0024",
            Self::ConfigNotAllowedInNamespace(_) => "E0025",
            Self::FbVariablesAfterMethod { .. } => "E0026",
            Self::ClassVariablesAfterMethod { .. } => "E0027",
            Self::MethodDeclInBody(_) => "E0028",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::SyntaxError { .. } => "syntax",
            Self::MissingNode { .. } => "syntax",
            Self::MissingVarType(_) => "syntax",
            Self::UnexpectedVarInit(_) => "syntax",
            Self::EmptyRightHandSide(_) => "syntax",
            Self::MissingDotInAssignment { .. } => "syntax",
            Self::MissingEqualInAssignment { .. } => "syntax",
            Self::OutputAssignInAssignment { .. } => "syntax",
            Self::MissingDotInForList { .. } => "syntax",
            Self::MissingEqualInForList { .. } => "syntax",
            Self::OutputAssignInForList { .. } => "syntax",
            Self::AssignInCondition { .. } => "syntax",
            Self::AssignToFunctionCall(_) => "syntax",
            Self::IncompleteEdgeQualifier(_) => "syntax",
            Self::UnexpectedThis(_) => "syntax",
            Self::UnexpectedSuper(_) => "syntax",
            Self::VarNotAllowed(_) => "syntax",
            Self::VarInOutNotAllowed(_) => "syntax",
            Self::VarTempNotAllowed(_) => "syntax",
            Self::VarExternalNotAllowed(_) => "syntax",
            Self::VarGlobalNotAllowed(_) => "syntax",
            Self::VarAccessNotAllowed(_) => "syntax",
            Self::VarConfigNotAllowed(_) => "syntax",
            Self::ProgramNotAllowedInNamespace(_) => "syntax",
            Self::ConfigNotAllowedInNamespace(_) => "syntax",
            Self::FbVariablesAfterMethod { .. } => "syntax",
            Self::ClassVariablesAfterMethod { .. } => "syntax",
            Self::MethodDeclInBody(_) => "syntax",
        }
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
            // The tree parsed, but the typed AST generated from the grammar
            // has no place for a node in it: the grammar and the generated
            // code disagree, which is the compiler's fault, not the source's.
            // It was an `unreachable!`, so `REF_TO TIME` crashed `rk check`
            // (auto-lsp-codegen flattened nested supertypes one level only).
            ParseError::AstError { span, error } => SyntaxError::SyntaxError {
                span: *span,
                err: format!(
                    "the parser has no place for this construct ({error}); this is a compiler bug"
                ),
            },
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
            Self::SyntaxError { span, err } => diag()
                .message(err.to_string())
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
            Self::MissingVarType(span) => diag()
                .message("variable type is missing".into())
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
            Self::AssignInCondition { file, span, sign } => {
                let mut diag = diag()
                    .message("':=' assigns, a condition compares with '='".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
                    .call();

                let assign = crate::denormalize(db, file, sign).unwrap_or_default();

                diag.with_fix(
                    action()
                        .title("replace ':=' with '='".into())
                        .kind(auto_lsp::lsp_types::CodeActionKind::QUICKFIX)
                        .diagnostics(vec![diag.inner()])
                        .is_preferred(true)
                        .edit(WorkspaceEdit::new(HashMap::from([(
                            file.url(db).clone(),
                            vec![edit().new_text(" = ".to_string()).range(assign).call()],
                        )])))
                        .call(),
                );

                diag
            }
            Self::AssignToFunctionCall(span) => diag()
                .message("assignment to function call is not allowed".into())
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

                diag.with_note("VAR_GLOBAL can only be used inside CONFIGURATION".into());
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
            Self::MethodDeclInBody(span) => diag()
                .message("method declarations are not allowed inside a body".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
        }
    }
}
