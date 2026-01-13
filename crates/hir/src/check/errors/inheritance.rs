use auto_lsp::{default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use db::WorkspaceDataBase;
use ide_diagnostic::{IdeDiagnostic, Related, diag};

use crate::{
    HasName, HirNodeInfo,
    check::errors::analysis_error::{AnalysisError, ToIdeDiagnostic},
    hir_def::{
        expressions::{expression::PathExpr, invocation::Invocation},
        interned::namespace::SpanNamespaceAccess,
        pous::pou::Pou,
    },
    hir_ty::{inheritance_solver::MethodRef, ty::Type},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum MethodError<'db> {
    UnresolvedPou {
        access: SpanNamespaceAccess<'db>,
    },
    OverrideFinalMethod {
        base_method: MethodRef<'db>,
        derived_method: MethodRef<'db>,
    },
    MissingOverride {
        base_method: MethodRef<'db>,
        derived_method: MethodRef<'db>,
    },
    MissingAbstractMethod {
        implementer: Pou<'db>,
        base_method: MethodRef<'db>,
    },
    EmptyOverride {
        base_method: MethodRef<'db>,
    },
    AbstractClassHasNoAbstractMethods {
        class: Pou<'db>,
    },
    UnimplementedInterfaceMethod {
        implementer: Pou<'db>,
        method: MethodRef<'db>,
    },
    // Invocations
    UnresolvedThisMethod {
        ctx: Pou<'db>,
        path: PathExpr<'db>,
        method: Invocation<'db>,
    },
    UnresolvedSuperMethod {
        ctx: Option<Pou<'db>>,
        path: PathExpr<'db>,
        method: Invocation<'db>,
    },
    // Signatures
    SignatureParametersCountMismatch {
        m1: MethodRef<'db>,
        expected: usize,
        m2: MethodRef<'db>,
        got: usize,
    },
    SignatureTypeMismatch {
        expected: Type<'db>,
        got: Type<'db>,
        method: MethodRef<'db>,
    },
}

impl<'db> From<MethodError<'db>> for AnalysisError<'db> {
    fn from(err: MethodError<'db>) -> Self {
        AnalysisError::MethodError(err)
    }
}

impl<'db> ToIdeDiagnostic<'db> for MethodError<'db> {
    fn to_diagnostic(&self, db: &'db dyn WorkspaceDataBase) -> IdeDiagnostic {
        match self {
            Self::UnresolvedPou { access } => diag()
                .message(format!(
                    "no item '{}' found in the current scope",
                    access.to_string(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .range(access.get_span(db).clone())
                .call(),

            Self::MissingOverride {
                base_method,
                derived_method,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "missing OVERRIDE keyword for method '{}'",
                        derived_method.get_name_ident(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(derived_method.get_name_span(db))
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "base method '{}' is declared here",
                        base_method.get_name_ident(db).text(db),
                    ),
                    base_method.get_scope_id(db).file(db),
                    base_method.get_name_span(db),
                ));
                diag.with_note("OVERRIDE keyword must be used even if the base method is not marked as ABSTRACT".into());
                diag
            }
            Self::OverrideFinalMethod {
                base_method,
                derived_method,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "cannot override FINAL method '{}'",
                        base_method.get_name_ident(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(derived_method.get_name_span(db))
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "FINAL method '{}' is declared here",
                        base_method.get_name_ident(db).text(db),
                    ),
                    base_method.get_scope_id(db).file(db),
                    base_method.get_name_span(db),
                ));
                diag.with_note("methods marked as FINAL cannot be overridden".into());

                diag
            }
            Self::MissingAbstractMethod {
                implementer,
                base_method,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "missing implementation for ABSTRACT method '{}'",
                        base_method.get_name_ident(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(implementer.get_name_span(db))
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "ABSTRACT method '{}' is declared here",
                        base_method.get_name_ident(db).text(db),
                    ),
                    base_method.get_scope_id(db).file(db),
                    base_method.get_name_span(db),
                ));
                diag.with_note("ABSTRACT methods must be implemented by derived POUs".into());
                diag
            }
            Self::EmptyOverride { base_method } => {
                let mut diag = diag()
                    .message(format!(
                        "invalid usage of OVERRIDE for method '{}'",
                        base_method.get_name_ident(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(base_method.get_name_span(db))
                    .call();

                diag.with_note("OVERRIDE is only valid when the method is inherited".into());

                diag
            }
            Self::AbstractClassHasNoAbstractMethods { class } => {
                let mut diag = diag()
                    .message(format!(
                        "ABSTRACT class '{}' has no abstract methods",
                        class.get_name_ident(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(class.get_name_span(db))
                    .call();

                diag.with_note("abstract classes must have at least one abstract method".into());

                diag
            }
            Self::UnimplementedInterfaceMethod {
                implementer,
                method,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "missing implementation for interface method '{}'",
                        method.get_name_ident(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(implementer.get_name_span(db))
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "method '{}' is declared by interface '{}' here",
                        method.get_name_ident(db).text(db),
                        implementer.get_name_ident(db).text(db),
                    ),
                    method.get_scope_id(db).file(db),
                    method.get_name_span(db),
                ));
                diag
            }
            Self::UnresolvedThisMethod { ctx, path, method } => diag()
                .message(format!(
                    "no method '{}' in declared methods of '{}'",
                    path.ident(db).ident.text(db),
                    ctx.get_name_ident(db).text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .range(method.get_span(db).clone())
                .call(),
            Self::UnresolvedSuperMethod { ctx, path, method } => {
                let mut diag = diag()
                    .message(format!(
                        "no method '{}' in inherited methods",
                        path.ident(db).ident.text(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(method.get_span(db).clone())
                    .call();

                if let Some(caller) = ctx {
                    diag.with_related(Related::new(
                        format!(
                            "methods are inherited from '{}' here",
                            caller.get_name_ident(db).text(db)
                        ),
                        caller.get_scope_id(db).file(db),
                        caller.get_name_span(db),
                    ));
                }
                diag
            }
            Self::SignatureParametersCountMismatch {
                m1,
                expected,
                m2,
                got,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "invalid number of parameters for method '{}': expected {}, got {}",
                        m1.get_name_ident(db).text(db),
                        expected,
                        got
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(m2.get_name_span(db).clone())
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "base method '{}' is declared here",
                        m1.get_name_ident(db).text(db)
                    ),
                    m1.get_scope_id(db).file(db),
                    m1.get_name_span(db),
                ));
                diag
            }
            Self::SignatureTypeMismatch {
                expected,
                got,
                method,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "method '{}' has incompatible parameter types: expected '{}', got '{}'",
                        method.get_name_ident(db).text(db),
                        expected.full_type_name(db),
                        got.full_type_name(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(method.get_name_span(db).clone())
                    .call();

                diag.with_note("parameter types must match those of the base method".into());

                diag
            }
        }
    }
}
