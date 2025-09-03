use core::panic;
use std::{collections::HashMap, error::Error, fmt::Display};

use auto_lsp::{
    core::{errors::PositionError, span::Span},
    default::db::{BaseDatabase, file::File},
    lsp_types::{
        DiagnosticRelatedInformation, DiagnosticSeverity, DiagnosticTag, Location, WorkspaceEdit,
    },
    tree_sitter,
};
use ide_diagnostic::{IdeDiagnostic, Related, action, diag, edit};

use crate::{
    def::{
        expressions::{expression::PathExpr, statement::Stmt},
        interned::namespace::NamespacePath,
        namespace::NamespaceDecl,
        pous::{
            pou::{Pou, PouDecl},
            variable::VariableDecl,
        },
        scope::FileScopeId,
        using::Using,
    },
    to_proto::ToProto,
    ty::{
        expr_resolver::ResolvedExpr,
        ty::{Ty, TyDecl},
    },
};

pub trait ToIdeDiagnostic<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic;
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum AnalysisError<'db> {
    // Specific
    AutoLspError(PositionError),
    SyntaxError(SyntaxError),
    NamespaceError(NamespaceError<'db>),
    DuplicateError(DuplicateError<'db>),
    PathExprError(PathExprError<'db>),
    StmtError(StmtError<'db>),
}

impl<'db> From<StmtError<'db>> for AnalysisError<'db> {
    fn from(err: StmtError<'db>) -> Self {
        AnalysisError::StmtError(err)
    }
}

impl<'db> From<(LitCheckError, Ty<'db>, ResolvedExpr<'db>)> for AnalysisError<'db> {
    fn from((err, ty, expr): (LitCheckError, Ty<'db>, ResolvedExpr<'db>)) -> Self {
        AnalysisError::StmtError(StmtError::LitCheckError {
            ty,
            literal: expr,
            err,
        })
    }
}

impl Error for AnalysisError<'_> {}

impl Display for AnalysisError<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}

impl From<PositionError> for AnalysisError<'_> {
    fn from(err: PositionError) -> Self {
        AnalysisError::AutoLspError(err)
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
    FUnctionCallInInitExpression(Span),
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

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum NamespaceError<'db> {
    NamespaceNotFound {
        namespace_path: NamespacePath,
        span: Span,
    },
    NamespaceAlreadyInScope {
        using: Using<'db>,
        namespace: NamespaceDecl<'db>,
    },
    DuplicateUsing {
        using: Using<'db>,
        other: Using<'db>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum DuplicateError<'db> {
    Variable {
        var1: VariableDecl<'db>,
        var2: VariableDecl<'db>,
    },
    Pou {
        pou1: PouDecl<'db>,
        pou2: PouDecl<'db>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum PathExprError<'db> {
    NoItemInScope {
        expr: PathExpr<'db>,
        scope: FileScopeId<'db>,
    },
    UnknownField {
        ty: Ty<'db>,
        expr: PathExpr<'db>,
    },
    UnexpectedIndex {
        ty: Ty<'db>,
        expr: PathExpr<'db>,
    },
    NotAnArray {
        ty: Ty<'db>,
        expr: PathExpr<'db>,
    },
    NotAReference {
        ty: Ty<'db>,
        expr: PathExpr<'db>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum StmtError<'db> {
    ContinueOutsideLoop {
        continue_stmt: Stmt<'db>,
    },
    ExitOutsideLoop {
        exit_stmt: Stmt<'db>,
    },
    Unreachable {
        start: Span,
        end: Span,
    },
    AssignmentToCallable {
        loc: Span,
        ty: Ty<'db>,
    },
    RecursiveType {
        ty: Ty<'db>,
    },
    LitCheckError {
        ty: Ty<'db>,
        literal: ResolvedExpr<'db>,
        err: LitCheckError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LitCheckError {
    TypeMismatch(String),
    InvalidFormat { kind: &'static str, msg: String },
    OutOfRange(String),
}

impl Display for LitCheckError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LitCheckError::OutOfRange(msg) => write!(f, "Out of range: {msg}"),
            LitCheckError::TypeMismatch(err) => write!(f, "Type mismatch: {err}"),
            LitCheckError::InvalidFormat { kind, msg } => {
                write!(f, "Invalid format for {kind}: {msg}")
            }
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for AnalysisError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            Self::AutoLspError(err) => panic!("A position error happened: {}", err),
            Self::SyntaxError(err) => err.to_diagnostic(db),
            Self::NamespaceError(err) => err.to_diagnostic(db),
            Self::DuplicateError(err) => err.to_diagnostic(db),
            Self::PathExprError(err) => err.to_diagnostic(db),
            Self::StmtError(err) => err.to_diagnostic(db),
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
            Self::InvalidPouKeyword(span) => diag()
                .message("invalid POU keyword".into())
                .severity(DiagnosticSeverity::ERROR)
                .range(span.clone())
                .call(),
            Self::FUnctionCallInInitExpression(span) => diag()
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

impl<'db> ToIdeDiagnostic<'db> for NamespaceError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            Self::NamespaceNotFound {
                namespace_path,
                span,
            } => diag()
                .message(format!(
                    "namespace '{}' not found",
                    namespace_path.to_string(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .range(span.clone())
                .call(),

            Self::DuplicateUsing { using, other } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate `USING` for namespace '{}'",
                        using.path(db).to_string(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(using.get_span(db).clone())
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "namespace '{}' is already imported here",
                        other.path(db).to_string(db)
                    ),
                    other.scope_id(db).file(db),
                    other.get_span(db),
                ));

                diag
            }
            Self::NamespaceAlreadyInScope { using, namespace } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' is already in scope",
                        using.path(db).to_string(db)
                    ))
                    .severity(DiagnosticSeverity::WARNING)
                    .tags(vec![DiagnosticTag::UNNECESSARY])
                    .range(using.get_span(db).clone())
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "namespace '{}' is defined here",
                        using.path(db).to_string(db)
                    ),
                    namespace.scope_id(db).file(db),
                    namespace.get_span(db).into(),
                ));

                diag
            }
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for DuplicateError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            Self::Variable { var1, var2 } => {
                let mut diag = diag()
                    .message(format!(
                        "variable '{}' is already defined",
                        var1.name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(var1.get_span(db).clone())
                    .call();

                diag.with_related(Related::new(
                    format!("variable '{}' is defined here", var2.name(db).text(db)),
                    var2.scope_id(db).file(db),
                    var2.get_span(db).into(),
                ));

                diag
            }
            Self::Pou { pou1, pou2 } => {
                let mut diag = diag()
                    .message(format!(
                        "POU '{}' is already defined",
                        pou1.name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(pou1.get_span(db).clone())
                    .call();

                diag.with_related(Related::new(
                    format!("POU '{}' is defined here", pou2.name(db).text(db)),
                    pou2.scope_id(db).file(db),
                    pou2.get_span(db).into(),
                ));

                diag
            }
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for StmtError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            Self::AssignmentToCallable { loc, ty } => {
                if let TyDecl::FromPou(pou) = ty.decl(db) {
                    if let Pou::FunctionBlock(_) | Pou::Class(_) = pou.pou(db) {
                        return {
                            let mut diag = diag()
                            .message(format!(
                            "POU '{}' can not be assigned\nbut you can declare a variable of same type instead",
                                pou.name(db).text(db),
                            ))
                            .severity(DiagnosticSeverity::ERROR)
                            .range(loc.clone())
                            .call();

                            diag.with_related(Related::new(
                                format!("POU '{}' is declared here", pou.name(db).text(db)),
                                pou.scope_id(db).file(db),
                                pou.get_span(db).into(),
                            ));

                            diag
                        };
                    };
                };
                let mut diag = diag()
                    .message(format!(
                        "'{}' can not be assigned",
                        ty.decl(db).name(db).text(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(loc.clone())
                    .call();

                diag.with_related(Related::new(
                    format!("POU '{}' is declared here", ty.decl(db).name(db).text(db)),
                    ty.decl(db).scope_id(db).file(db),
                    ty.decl(db).span(db).into(),
                ));

                diag
            }
            Self::Unreachable { start, end } => {
                let range = Span::from(tree_sitter::Range {
                    start_byte: start.start_byte,
                    end_byte: end.end_byte,
                    start_point: start.start_point,
                    end_point: end.end_point,
                });

                diag()
                    .message("unreachable code".into())
                    .severity(DiagnosticSeverity::WARNING)
                    .tags(vec![DiagnosticTag::UNNECESSARY])
                    .range(range.clone())
                    .call()
            }
            Self::ExitOutsideLoop { exit_stmt } => diag()
                .message("exit statement outside of loop".into())
                .severity(DiagnosticSeverity::ERROR)
                .range(exit_stmt.get_span(db))
                .call(),
            Self::ContinueOutsideLoop { continue_stmt } => diag()
                .message("continue statement outside of loop".into())
                .severity(DiagnosticSeverity::ERROR)
                .range(continue_stmt.get_span(db))
                .call(),
            Self::LitCheckError { ty, literal, err } => match err {
                LitCheckError::TypeMismatch(err) => {
                    let mut diag = diag()
                        .message(err.to_string())
                        .severity(DiagnosticSeverity::ERROR)
                        .range(literal.get_span(db).clone())
                        .call();

                    get_decl_and_def_for_ty(db, *ty, &mut diag);

                    diag
                }
                LitCheckError::InvalidFormat { kind, msg } => diag()
                    .message(format!("invalid format for literal '{}': {}", kind, msg))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(literal.get_span(db).clone())
                    .call(),
                LitCheckError::OutOfRange(err) => diag()
                    .message(err.to_string())
                    .severity(DiagnosticSeverity::ERROR)
                    .range(literal.get_span(db).clone())
                    .call(),
            },
            Self::RecursiveType { ty } => diag()
                .message(format!(
                    "recursive type detected for '{}'",
                    ty.decl(db).name(db).text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .range(ty.decl(db).name_span(db).clone())
                .call(),
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for PathExprError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            Self::NoItemInScope { expr, scope } => diag()
                .message(format!(
                    "no item '{}' in scope",
                    expr.to_string(db).text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .range(expr.get_span(db).clone())
                .call(),
            Self::UnknownField { ty: origin, expr } => diag()
                .message(format!(
                    "field {} not found in type",
                    expr.to_string(db).text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .range(expr.get_span(db).clone())
                .call(),
            Self::UnexpectedIndex { ty: origin, expr } => diag()
                .message("unexpected index expression".to_string())
                .severity(DiagnosticSeverity::ERROR)
                .range(expr.get_span(db).clone())
                .call(),
            Self::NotAReference { ty: origin, expr } => diag()
                .message("type can not be dereferenced".to_string())
                .severity(DiagnosticSeverity::ERROR)
                .range(expr.get_span(db).clone())
                .call(),
            Self::NotAnArray { ty: origin, expr } => diag()
                .message("type is not an array".to_string())
                .severity(DiagnosticSeverity::ERROR)
                .range(expr.get_span(db).clone())
                .call(),
        }
    }
}

pub fn get_decl_and_def_for_ty(db: &dyn BaseDatabase, ty: Ty<'_>, diag: &mut IdeDiagnostic) {
    diag.with_related(Related::new(
        format!("'{}' is declared here", ty.decl(db).name(db).text(db),),
        ty.decl(db).scope_id(db).file(db),
        ty.decl(db).name_span(db),
    ));

    if let Some(span) = ty.def(db).get_span(db) {
        diag.with_related(Related::new(
            format!("type defined here",),
            ty.def(db).get_scope_id(db).unwrap().file(db),
            span.into(),
        ));
    }
}
