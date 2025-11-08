use auto_lsp::{default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use ide_diagnostic::{IdeDiagnostic, Related, diag};

use crate::{
    HirNodeInfo,
    check::errors::analysis_error::{AnalysisError, ToIdeDiagnostic},
    hir_def::{
        expressions::spec::StructElement,
        interned::identifier::SpanIdent,
        pous::{class::MethodDecl, interface::MethodPrototype, pou::PouDecl, variable::VariableDecl},
    },
    hir_ty::inheritance_solver::InheritedMethod,
};

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub enum DuplicateError<'db> {
    Pou {
        pou1: PouDecl<'db>,
        pou2: PouDecl<'db>,
    },
    Variable {
        var1: VariableDecl<'db>,
        var2: VariableDecl<'db>,
    },
    StructField {
        field1: StructElement<'db>,
        field2: StructElement<'db>,
    },
    EnumVariant {
        variant1: SpanIdent<'db>,
        variant2: SpanIdent<'db>,
    },
    MethodDecl {
        method1: MethodDecl<'db>,
        method2: MethodDecl<'db>,
    },
    MethodProt {
        method1: MethodPrototype<'db>,
        method2: MethodPrototype<'db>,
    },
    InheritedMethod {
        method1: InheritedMethod<'db>,
        method2: InheritedMethod<'db>,
    },
}

impl<'db> From<DuplicateError<'db>> for AnalysisError<'db> {
    fn from(err: DuplicateError<'db>) -> Self {
        AnalysisError::DuplicateError(err)
    }
}

impl<'db> ToIdeDiagnostic<'db> for DuplicateError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            Self::Pou { pou1, pou2 } => {
                let mut diag = diag()
                    .message(format!("duplicate POU '{}'", pou1.name(db).text(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(pou1.name_span(db))
                    .call();

                diag.with_related(Related::new(
                    format!("POU '{}' is already defined here", pou2.name(db).text(db)),
                    pou2.get_scope_id(db).file(db),
                    pou2.name_span(db),
                ));

                diag
            }
            Self::Variable { var1, var2 } => {
                let mut diag = diag()
                    .message(format!("duplicate variable '{}'", var1.name(db).text(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(var1.get_name_span(db).unwrap())
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "variable '{}' is already defined here",
                        var2.name(db).text(db)
                    ),
                    var2.get_scope_id(db).file(db),
                    var2.get_name_span(db).unwrap(),
                ));

                diag
            }
            Self::EnumVariant { variant1, variant2 } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate enum variant '{}'",
                        variant1.ident.text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(variant1.get_span(db))
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "enum variant '{}' is already defined here",
                        variant2.ident.text(db)
                    ),
                    variant2.get_scope_id(db).file(db),
                    variant2.get_span(db),
                ));

                diag
            }
            Self::StructField { field1, field2 } => {
                let mut diag = diag()
                    .message(format!("duplicate field '{}'", field1.name(db).text(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(field1.get_name_span(db).unwrap())
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "field '{}' is already defined here",
                        field2.name(db).text(db)
                    ),
                    field2.get_scope_id(db).file(db),
                    field2.get_name_span(db).unwrap(),
                ));

                diag
            }
            Self::MethodDecl { method1, method2 } => {
                let mut diag = diag()
                    .message(format!("duplicate method '{}'", method1.name(db).text(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(method1.get_name_span(db).unwrap())
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "method '{}' is already defined here",
                        method2.name(db).text(db)
                    ),
                    method2.get_scope_id(db).file(db),
                    method2.get_name_span(db).unwrap(),
                ));

                diag
            }
            Self::MethodProt { method1, method2 } => {
                let mut diag = diag()
                    .message(format!("duplicate method '{}'", method1.name(db).text(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(method1.get_name_span(db).unwrap())
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "method '{}' is already defined here",
                        method2.name(db).text(db)
                    ),
                    method2.get_scope_id(db).file(db),
                    method2.get_name_span(db).unwrap(),
                ));

                diag
            }
            Self::InheritedMethod { method1, method2 } => {
                let mut diag = diag()
                    .message(format!(
                        "duplicate method '{}'",
                        method1.method.name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(method1.method.name_span(db))
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "method '{}' is already defined here",
                        method2.method.name(db).text(db)
                    ),
                    method2.method.get_scope_id(db).file(db),
                    method2.method.name_span(db),
                ));

                diag.with_note(format!(
                    "this error happens because both interfaces '{}' and '{}' define a method '{}'",
                    method1.source.name(db).text(db),
                    method2.source.name(db).text(db),
                    method1.method.name(db).text(db)
                ));

                diag
            }
        }
    }
}
