use std::{error::Error, fmt::Display};

use auto_lsp::{
    core::{errors::PositionError, span::Span},
    default::db::BaseDatabase,
    lsp_types::{DiagnosticRelatedInformation, DiagnosticSeverity, DiagnosticTag, Location},
    tree_sitter,
};
use ide_diagnostic::{diag, IdeDiagnostic};

use crate::{
    hir::{
        expressions::{expression::PathExpr, statement::Stmt},
        interned::namespace::NamespacePath,
        namespace::Namespace,
        pous::{
            pou::{Pou, PouDecl},
            variable::Variable,
        },
        scope::FileScopeId,
        using::Using,
    },
    hir_ty::ty::{Ty, TyOrigin},
    to_proto::ToProto,
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
    AssignToFUnctionCall(Span),
    EmptyRightHandSide(Span),
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum NamespaceError<'db> {
    NamespaceNotFound {
        namespace_path: NamespacePath,
        span: Span,
    },
    NamespaceAlreadyInScope {
        using: Using<'db>,
        namespace: Namespace<'db>,
    },
    DuplicateUsing {
        using: Using<'db>,
        other: Using<'db>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum DuplicateError<'db> {
    Variable {
        var1: Variable<'db>,
        var2: Variable<'db>,
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
        scope: FileScopeId,
    },
    UnknownField {
        origin: TyOrigin<'db>,
        expr: PathExpr<'db>,
    },
    UnexpectedIndex {
        origin: TyOrigin<'db>,
        expr: PathExpr<'db>,
    },
    NotAReference {
        origin: TyOrigin<'db>,
        expr: PathExpr<'db>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum StmtError<'db> {
    ContinueOutsideLoop { continue_stmt: Stmt<'db> },
    ExitOutsideLoop { exit_stmt: Stmt<'db> },
    Unreachable { start: Span, end: Span },
    AssignmentToCallable { loc: Span, ty: Ty<'db> },
}

impl<'db> ToIdeDiagnostic<'db> for AnalysisError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            Self::AutoLspError(err) => unreachable!(),
            Self::SyntaxError(err) => err.to_diagnostic(db),
            Self::NamespaceError(err) => err.to_diagnostic(db),
            Self::DuplicateError(err) => err.to_diagnostic(db),
            Self::PathExprError(err) => err.to_diagnostic(db),
            Self::StmtError(err) => err.to_diagnostic(db),
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for SyntaxError {
    fn to_diagnostic(&self, _db: &'db dyn BaseDatabase) -> IdeDiagnostic {
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
            Self::AssignToFUnctionCall(span) => diag()
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
                .call()       
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

            Self::DuplicateUsing { using, other } => diag()
                .message(format!(
                    "duplicate `USING` for namespace '{}'",
                    using.path(db).to_string(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .related_information(vec![DiagnosticRelatedInformation {
                    location: Location {
                        uri: other.scope_id(db).file().url(db).clone(),
                        range: other.get_span(db).into(),
                    },
                    message: format!(
                        "namespace '{}' is already imported here",
                        other.path(db).to_string(db)
                    ),
                }])
                .range(using.get_span(db).clone())
                .call(),    
            Self::NamespaceAlreadyInScope { using, namespace } => diag()
                .message(format!(
                    "'{}' is already in scope",
                    using.path(db).to_string(db)
                ))
                .severity(DiagnosticSeverity::WARNING)
                .tags(vec![DiagnosticTag::UNNECESSARY])
                .related_information(vec![DiagnosticRelatedInformation {
                    location: Location {
                        uri: namespace.scope_id(db).file().url(db).clone(),
                        range: namespace.get_span(db).into(),
                    },
                    message: format!(
                        "namespace '{}' is defined here",
                        using.path(db).to_string(db)
                    ),
                }])
                .range(using.get_span(db).clone())
                .call(),
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for DuplicateError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            Self::Variable { var1, var2 } => diag()
                .message(format!(
                    "variable '{}' is already defined",
                    var1.name(db).text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .range(var1.get_span(db).clone())
                .related_information(vec![DiagnosticRelatedInformation {
                    location: Location {
                        uri: var2.scope_id(db).file().url(db).clone(),
                        range: var2.get_span(db).into(),
                    },
                    message: format!("variable '{}' is defined here", var2.name(db).text(db)),
                }])
                .call(),
            Self::Pou { pou1, pou2 } => diag()
                .message(format!(
                    "POU '{}' is already defined",
                    pou1.name(db).text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .range(pou1.get_span(db).clone())
                .related_information(vec![DiagnosticRelatedInformation {
                    location: Location {
                        uri: pou2.scope_id(db).file().url(db).clone(),
                        range: pou2.get_span(db).into(),
                    },
                    message: format!("POU '{}' is defined here", pou2.name(db).text(db)),
                }])
                .call(),
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for StmtError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            Self::AssignmentToCallable { loc, ty } => {
                if let TyOrigin::FromPou(pou) = ty.origin(db) {
                    if let Pou::FunctionBlock(_) | Pou::Class(_) = pou.pou(db) {
                        return diag()
                            .message(format!(
                            "POU '{}' can not be assigned\nbut you can declare a variable of same type instead",
                                pou.name(db).text(db),
                            ))
                            .severity(DiagnosticSeverity::ERROR)
                            .range(loc.clone())
                            .related_information(vec![DiagnosticRelatedInformation {
                                location: Location {
                                    uri: pou.scope_id(db).file().url(db).clone(),
                                    range: pou.get_name_span(db).unwrap().into(),
                                },
                                message: format!("POU '{}' is declared here", pou.name(db).text(db))}])
                            .call();
                    };
                };
                diag()
                    .message(format!(
                        "'{}' can not be assigned",
                        ty.origin(db).name(db).text(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(loc.clone())
                    .related_information(vec![DiagnosticRelatedInformation {
                        location: Location {
                            uri: ty.origin(db).scope_id(db).file().url(db).clone(),
                            range: ty.origin(db).name_span(db).unwrap().into(),
                        },
                        message: format!(
                            "POU '{}' is declared here",
                            ty.origin(db).name(db).text(db)
                        ),
                    }])
                    .call()
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
                    .range(start.clone())
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
            Self::UnknownField { origin, expr } => diag()
                .message(format!(
                    "field {} not found in type",
                    expr.to_string(db).text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .range(expr.get_span(db).clone())
                .call(),
            Self::UnexpectedIndex { origin, expr } => diag()
                .message("unexpected index expression".to_string())
                .severity(DiagnosticSeverity::ERROR)
                .range(expr.get_span(db).clone())
                .call(),
            Self::NotAReference { origin, expr } => diag()
                .message("type can not be dereferenced".to_string())
                .severity(DiagnosticSeverity::ERROR)
                .range(expr.get_span(db).clone())
                .call(),
        }
    }
}
