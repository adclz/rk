use auto_lsp::default::db::BaseDatabase;
use auto_lsp::lsp_types::{
    DiagnosticRelatedInformation, DiagnosticSeverity, DiagnosticTag, Location,
};
use auto_lsp::{core::span::Span, default::db::file::File};
use salsa::Accumulator;

use crate::check::errors::recovery::pou_recovery;
use crate::check::{diagnostic_builder::diag, DiagnosticAccumulator};
use crate::hir::expressions::expression::PathExpr;
use crate::hir::expressions::spec::Spec;
use crate::hir::interned::identifier::Ident;
use crate::hir::interned::namespace::NamespacePath;
use crate::hir::namespace::Namespace;
use crate::hir::pous::pou::Pou;
use crate::hir::pous::variable::Variable;
use crate::hir::semantic_index::SemanticIndex;
use crate::hir::ty::TyOrigin;
use crate::hir::using::Using;

/// POU has multiple EXTENDS declared
pub fn multiple_extends(db: &dyn salsa::Database, span: Span) {
    let diag = diag()
        .message("EXTENDS can only be defined once".into())
        .severity(DiagnosticSeverity::ERROR)
        .range(span)
        .call();
    DiagnosticAccumulator::accumulate(diag.into(), db);
}

/// POU has multiple IMPLEMENTS declared
pub fn multiple_implements(db: &dyn salsa::Database, span: Span) {
    let diag = diag()
        .message("IMPLEMENTS can only be defined once".into())
        .severity(DiagnosticSeverity::ERROR)
        .range(span)
        .call();
    DiagnosticAccumulator::accumulate(diag.into(), db);
}

/// POU has IMPLEMENTS declared before EXTENDS
pub fn implements_before_extends(db: &dyn salsa::Database, span: Span) {
    let diag = diag()
        .message("IMPLEMENTS can only be defined after EXTENDS".into())
        .severity(DiagnosticSeverity::ERROR)
        .range(span)
        .call();
    DiagnosticAccumulator::accumulate(diag.into(), db);
}

/// Missing Type for variable (VAR x (?) : = 0)
pub fn missing_type_for_variable(db: &dyn salsa::Database, span: Span) {
    let diag = diag()
        .message("variable declaration is missing type".into())
        .severity(DiagnosticSeverity::ERROR)
        .range(span)
        .call();
    DiagnosticAccumulator::accumulate(diag.into(), db);
}

/// Incomplete EDGE qualifer (VAR x : BOOL R_EDG)
pub fn incomplete_edge_qualifier(db: &dyn salsa::Database, span: Span) {
    let diag = diag()
        .message("incomplete EDGE qualifier".into())
        .severity(DiagnosticSeverity::ERROR)
        .range(span)
        .call();
    DiagnosticAccumulator::accumulate(diag.into(), db);
}

/// A variable declared in TEMP or IN_OUT has a default value (x : INT := 0;)
pub fn unauthorized_variable_init(db: &dyn salsa::Database, span: Span) {
    let diag = diag()
        .message("variables declared in TEMP or IN_OUT can not have a default value".into())
        .severity(DiagnosticSeverity::ERROR)
        .range(span)
        .call();
    DiagnosticAccumulator::accumulate(diag.into(), db);
}

/// Invocation in expression context
pub fn invocation_in_expression(db: &dyn salsa::Database, span: Span) {
    let diag = diag()
        .message("invocation in expression context is not allowed".into())
        .severity(DiagnosticSeverity::ERROR)
        .range(span)
        .call();
    DiagnosticAccumulator::accumulate(diag.into(), db);
}

/// Unexpected 'this' in path (n.THIS := 0)
pub fn unexpected_this(db: &dyn salsa::Database, span: Span) {
    let diag = diag()
        .message("unexpected 'THIS' in path".into())
        .severity(DiagnosticSeverity::ERROR)
        .range(span)
        .call();
    DiagnosticAccumulator::accumulate(diag.into(), db);
}

/// Attempt to assign to a function call (fn() := 0)
pub fn assign_to_function_call(db: &dyn salsa::Database, span: Span) {
    let diag = diag()
        .message("cannot assign to a function call".into())
        .severity(DiagnosticSeverity::ERROR)
        .range(span)
        .call();
    DiagnosticAccumulator::accumulate(diag.into(), db);
}

/// Empty right-hand side in assignment (x := (?))
pub fn empty_right_hand_assignment(db: &dyn salsa::Database, span: Span) {
    let diag = diag()
        .message("empty right-hand side in assignment".into())
        .severity(DiagnosticSeverity::ERROR)
        .range(span)
        .call();
    DiagnosticAccumulator::accumulate(diag.into(), db);
}

/// Invalid POU keyword:
/// NAMESPACE
///     smh <-- ?
/// END_NAMESPACE
pub fn invalid_pou_keyword(db: &dyn salsa::Database, span: Span) {
    let diag = diag()
        .message("expected a POU keyword".into())
        .severity(DiagnosticSeverity::ERROR)
        .range(span)
        .call();
    DiagnosticAccumulator::accumulate(diag.into(), db);
}

/// Namespace not found
pub fn namespace_not_found(db: &dyn BaseDatabase, span: Span, namespace: NamespacePath) {
    let diag = diag()
        .message(format!("namespace '{}' not found", namespace.to_string(db)))
        .severity(DiagnosticSeverity::ERROR)
        .range(span)
        .call();
    DiagnosticAccumulator::accumulate(diag.into(), db);
}

/// Duplicate declaration of USING directive
/// USING ns, ns;
///
/// or
///
/// USING ns;
/// USING ns;
pub fn duplicate_using_declaration(
    db: &dyn BaseDatabase,
    file: File,
    origin: Using,
    related: Using,
) {
    let diag = diag()
        .message(format!(
            "duplicate USING declaration for namespace '{}'",
            origin.path(db).to_string(db)
        ))
        .severity(DiagnosticSeverity::WARNING)
        .tags(vec![DiagnosticTag::UNNECESSARY])
        .related_information(vec![DiagnosticRelatedInformation {
            location: Location {
                uri: file.url(db).clone(),
                range: related.span(db).into(),
            },
            message: format!(
                "namespace '{}' is already imported here",
                origin.path(db).to_string(db)
            ),
        }])
        .range(origin.span(db).clone())
        .call();

    DiagnosticAccumulator::accumulate(diag.into(), db);
}

/// Namespace already in scope
pub fn namespace_already_in_scope(
    db: &dyn BaseDatabase,
    file: File,
    origin: Using,
    imported: Namespace,
) {
    let diag = diag()
        .message(format!(
            "'{}' is already in scope",
            origin.path(db).to_string(db)
        ))
        .severity(DiagnosticSeverity::WARNING)
        .tags(vec![DiagnosticTag::UNNECESSARY])
        .related_information(vec![DiagnosticRelatedInformation {
            location: Location {
                uri: file.url(db).clone(),
                range: imported.span(db).into(),
            },
            message: format!(
                "namespace '{}' is defined here",
                origin.path(db).to_string(db)
            ),
        }])
        .range(origin.span(db).clone())
        .call();

    DiagnosticAccumulator::accumulate(diag.into(), db);
}

/// Mismatch type error
pub fn mismatch_type<'db>(
    db: &'db dyn BaseDatabase,
    sema: &'db SemanticIndex<'db>,
    origin: Span,
    spec: &'db Spec<'db>,
    err: impl ToString,
) {
    let diag = diag()
        .range(origin.clone())
        .message(err.to_string())
        .source("IEC".into())
        .severity(DiagnosticSeverity::ERROR)
        .related_information(vec![DiagnosticRelatedInformation {
            location: Location {
                uri: sema.file.url(db).clone(),
                range: spec.span(db).clone().into(),
            },
            message: format!("because of type: '{:?}' declared here", spec.kind(db)),
        }])
        .call();
    DiagnosticAccumulator::accumulate(diag.into(), db);
}

/// Duplicate variable declaration
pub fn duplicate_variable_declaration(
    db: &dyn BaseDatabase,
    file: File,
    origin: &Variable,
    other: &Variable,
) {
    let message = format!(
        "duplicate variable declaration: '{}'",
        origin.name(db).text(db)
    );
    let diag = diag()
        .range(origin.name_span(db).clone())
        .message(message)
        .source("IEC".into())
        .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
        .related_information(vec![DiagnosticRelatedInformation {
            location: auto_lsp::lsp_types::Location {
                uri: origin.file(db).url(db).clone(),
                range: other.name_span(db).into(),
            },
            message: format!(
                "variable '{}' is previously declared here",
                other.name(db).text(db)
            ),
        }])
        .call();
    DiagnosticAccumulator::accumulate(diag.into(), db);
}

/// Unexpected index expression (x := [0])
pub fn unexpected_index_expression(db: &dyn BaseDatabase, file: File, span: &Span) {
    let diag = diag()
        .message("unexpected index expression".to_string())
        .severity(DiagnosticSeverity::ERROR)
        .range(span.clone())
        .call();
    DiagnosticAccumulator::accumulate(diag.into(), db);
}

/// field not found
pub fn unknown_field(db: &dyn BaseDatabase, file: File, field: &Ident, span: &Span) {
    let diag = diag()
        .message(format!("field {} not found in type", field.text(db)))
        .severity(DiagnosticSeverity::ERROR)
        .range(span.clone())
        .call();
    DiagnosticAccumulator::accumulate(diag.into(), db);
}

/// no item found in scope
pub fn no_item_in_scope(db: &dyn BaseDatabase, file: File, field: &Ident, span: &Span) {
    let mut diag = diag()
        .message(format!("no item '{}' in scope", field.text(db)))
        .severity(DiagnosticSeverity::ERROR)
        .range(span.clone())
        .call();

    let recovery = pou_recovery(db, file, field.text(db));
    if !recovery.is_empty() {
        let suggestions = recovery
            .into_iter()
            .map(|name| format!("  - {name}"))
            .collect::<Vec<_>>()
            .join("\n");

        diag.diagnostic.related_information = Some(vec![DiagnosticRelatedInformation {
            location: Location {
                uri: file.url(db).clone(),
                range: span.clone().into(),
            },
            message: format!("did you mean:\n{suggestions}"),
        }]);
    }
    DiagnosticAccumulator::accumulate(diag.into(), db);
}

/// no item found in scope
pub fn type_has_no_field(db: &dyn BaseDatabase, file: File, option: Option<Spec>, span: &Span) {
    let diag = diag()
        .message(format!(
            "'{}' is a primitive type and therefore doesn't have fields",
            option.map(|s| s.to_string(db)).unwrap_or("".to_string())
        ))
        .severity(DiagnosticSeverity::ERROR)
        .range(span.clone())
        .call();
    DiagnosticAccumulator::accumulate(diag.into(), db);
}

pub fn type_can_not_be_dereferenced(db: &dyn BaseDatabase, file: File, span: &Span) {
    let diag = diag()
        .message("type can not be dereferenced".to_string())
        .severity(DiagnosticSeverity::ERROR)
        .range(span.clone())
        .call();
    DiagnosticAccumulator::accumulate(diag.into(), db);
}

pub fn assign_direct_pou_to_a_variable(
    db: &dyn BaseDatabase,
    file: File,
    expr: PathExpr<'_>,
    origin: TyOrigin,
) {
    if let TyOrigin::FromPou(pou) = origin {
        match pou.pou(db) {
            Pou::FunctionBlock(_) | Pou::Class(_) => {
                let diag = diag()
                .message(format!(
                    "POU '{}' can not be assigned\nbut you can declare a variable of same type instead",
                    pou.name(db).text(db),
                ))
                .severity(DiagnosticSeverity::ERROR)
                .range(expr.span(db).clone())
                .related_information(vec![DiagnosticRelatedInformation {
                    location: Location {
                        uri: pou.scope_id(db).file().url(db).clone(),
                        range: pou.name_span(db).into(),
                    },
                    message: format!("POU '{}' is declared here", pou.name(db).text(db)),
                }])
                .call();
                DiagnosticAccumulator::accumulate(diag.into(), db);
            }
            _ => {
                let diag = diag()
                    .message(format!(
                        "POU '{}' can not be assigned",
                        pou.name(db).text(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(expr.span(db).clone())
                    .related_information(vec![DiagnosticRelatedInformation {
                        location: Location {
                            uri: pou.scope_id(db).file().url(db).clone(),
                            range: pou.name_span(db).into(),
                        },
                        message: format!("POU '{}' is declared here", pou.name(db).text(db)),
                    }])
                    .call();
                DiagnosticAccumulator::accumulate(diag.into(), db);
            }
        }
    }
}
