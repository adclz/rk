// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use crate::check::errors::ToIdeDiagnostic;
use crate::check::errors::first_word;
use auto_lsp::core::errors::LexerError;
use auto_lsp::core::errors::ParseError;
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
    /// `values : INT...` on a PROGRAM's or a FUNCTION_BLOCK's input: an
    /// instance has no argument count to be specialized for.
    VariadicNotAllowed {
        span: Range,
        holder: &'static str,
    },
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
            Self::VariadicNotAllowed { .. } => "E0029",
        }
    }
}

impl SyntaxError {
    pub fn from_parse_error(db: &dyn WorkspaceDataBase, file: File, err: &ParseError) -> Self {
        match err {
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
                            format!("unexpected token(s): '{}'", text.trim())
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
                err: format!("compiler bug: the parser has no place for this construct ({error})"),
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
                .message("the variable has no type".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::UnexpectedVarInit(span) => diag()
                .message("an initial value is not allowed here".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::EmptyRightHandSide(span) => diag()
                .message("the assignment has no right-hand side".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::MissingDotInAssignment { file, span } => {
                let mut diag = diag()
                    .message("'=' is not a valid assignment sign".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &first_bytes(span, 1)).unwrap_or_default())
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
                    .range(crate::denormalize(db, file, &first_bytes(span, 1)).unwrap_or_default())
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
                    .range(crate::denormalize(db, file, &first_bytes(span, 2)).unwrap_or_default())
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
                    .message("a condition compares with '=', not ':='".into())
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
                .message("a function call cannot be assigned".into())
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
                .message("'THIS' cannot follow a dot".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::UnexpectedSuper(span) => diag()
                .message("'SUPER' cannot follow a dot".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::VarNotAllowed(span) => section_not_allowed(
                self,
                db,
                file,
                span,
                "VAR",
                "VAR declares the variables of a FUNCTION, FUNCTION_BLOCK, PROGRAM, CLASS or METHOD",
            ),
            Self::VarInOutNotAllowed(span) => section_not_allowed(
                self,
                db,
                file,
                span,
                "VAR_IN_OUT",
                "VAR_IN_OUT is a parameter of a FUNCTION, FUNCTION_BLOCK, PROGRAM or METHOD",
            ),
            Self::VarTempNotAllowed(span) => section_not_allowed(
                self,
                db,
                file,
                span,
                "VAR_TEMP",
                "VAR_TEMP declares the per-call variables of a FUNCTION, FUNCTION_BLOCK, PROGRAM or METHOD",
            ),
            Self::VariadicNotAllowed { span, holder } => {
                let mut diag = diag()
                    .message(format!("a {holder} takes no variadic parameter"))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
                    .call();
                diag.with_note(format!(
                    "the inputs of a {holder} are members of one instance"
                ));
                diag
            }
            Self::VarExternalNotAllowed(span) => section_not_allowed(
                self,
                db,
                file,
                span,
                "VAR_EXTERNAL",
                "VAR_EXTERNAL names a global from a FUNCTION, FUNCTION_BLOCK, PROGRAM, CLASS or METHOD",
            ),
            Self::VarGlobalNotAllowed(span) => section_not_allowed(
                self,
                db,
                file,
                span,
                "VAR_GLOBAL",
                "VAR_GLOBAL goes in a CONFIGURATION",
            ),
            Self::VarAccessNotAllowed(span) => section_not_allowed(
                self,
                db,
                file,
                span,
                "VAR_ACCESS",
                "VAR_ACCESS goes in a PROGRAM or a CONFIGURATION",
            ),
            Self::VarConfigNotAllowed(span) => section_not_allowed(
                self,
                db,
                file,
                span,
                "VAR_CONFIG",
                "VAR_CONFIG goes in a CONFIGURATION",
            ),
            Self::ProgramNotAllowedInNamespace(span) => diag()
                .message("a PROGRAM cannot be declared in a NAMESPACE".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(
                    crate::denormalize(db, file, &first_word(db, file, span)).unwrap_or_default(),
                )
                .call(),
            Self::ConfigNotAllowedInNamespace(span) => diag()
                .message("a CONFIGURATION cannot be declared in a NAMESPACE".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(
                    crate::denormalize(db, file, &first_word(db, file, span)).unwrap_or_default(),
                )
                .call(),
            Self::FbVariablesAfterMethod {
                file,
                var_span,
                method_span,
            } => {
                // The method's keyword, not its whole body.
                let method_span_start = first_word(db, *file, method_span);

                let mut diag = diag()
                    .message(
                        "the variable sections of a FUNCTION_BLOCK come before its methods".into(),
                    )
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &first_word(db, *file, var_span))
                            .unwrap_or_default(),
                    )
                    .call();

                diag.with_related(Related::new(
                    "the first method is declared here".into(),
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
                // The method's keyword, not its whole body.
                let method_span_start = first_word(db, *file, method_span);

                let mut diag = diag()
                    .message("the variable sections of a CLASS come before its methods".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &first_word(db, *file, var_span))
                            .unwrap_or_default(),
                    )
                    .call();

                diag.with_related(Related::new(
                    "the first method is declared here".into(),
                    *file,
                    method_span_start,
                ));
                diag
            }
            Self::MethodDeclInBody(span) => diag()
                .message("a METHOD cannot be declared in a body".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(
                    crate::denormalize(db, file, &first_word(db, file, span)).unwrap_or_default(),
                )
                .call(),
        }
    }
}

/// A variable section in a POU that takes none. The report underlines the
/// keyword, and its note says where the section goes.
fn section_not_allowed(
    error: &SyntaxError,
    db: &dyn WorkspaceDataBase,
    file: File,
    span: &Range,
    keyword: &str,
    place: &str,
) -> IdeDiagnostic {
    let mut diag = diag()
        .message(format!("{keyword} is not allowed here"))
        .severity(DiagnosticSeverity::ERROR)
        .desc(error)
        .range(crate::denormalize(db, file, &first_word(db, file, span)).unwrap_or_default())
        .call();
    diag.with_note(place.to_string());
    diag
}

/// The first `len` bytes of `span`: an assignment sign, without what follows.
fn first_bytes(span: &Range, len: usize) -> Range {
    Range {
        start_byte: span.start_byte,
        end_byte: span.start_byte + len,
        start_point: span.start_point,
        end_point: tree_sitter::Point {
            row: span.start_point.row,
            column: span.start_point.column + len,
        },
    }
}
