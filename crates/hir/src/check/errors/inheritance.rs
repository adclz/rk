use auto_lsp::{default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use ide_diagnostic::{IdeDiagnostic, Related, diag};

use crate::{
    check::errors::{
        analysis_error::{AnalysisError, DiagnosticDescription, ToIdeDiagnostic},
    }, hir_def::{
        expressions::{expression::PathExpr, invocation::Invocation}, interned::namespace::SpanNamespaceAccess, pous::{
            pou::PouDecl,
            variable::VariableDecl,
        }
    }, hir_ty::inheritance_solver::MethodRef, HirNodeInfo
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
        implementer: PouDecl<'db>,
        base_method: MethodRef<'db>,
    },
    EmptyOverride {
        base_method: MethodRef<'db>,
    },
    AbstractClassHasNoAbstractMethods {
        class: PouDecl<'db>,
    },
    UnimplementedInterfaceMethod {
        implementer: PouDecl<'db>,
        method: MethodRef<'db>,
    },
    // Invocations
    UnresolvedThisMethod {
        ctx: PouDecl<'db>,
        path: PathExpr<'db>,
        method: Invocation<'db>,
    },
    UnresolvedSuperMethod {
        ctx: Option<PouDecl<'db>>,
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
    /*SignatureParametersTypeMismatch {
        err: Type<'db>,
        param: VariableDecl<'db>,
    },*/
}

impl<'db> From<MethodError<'db>> for AnalysisError<'db> {
    fn from(err: MethodError<'db>) -> Self {
        AnalysisError::MethodError(err)
    }
}

impl<'db> ToIdeDiagnostic<'db> for MethodError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
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
                        derived_method.name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(derived_method.name_span(db))
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "base method '{}' is declared here",
                        base_method.name(db).text(db),
                    ),
                    base_method.get_scope_id(db).file(db),
                    base_method.name_span(db),
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
                        base_method.name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(derived_method.name_span(db))
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "FINAL method '{}' is declared here",
                        base_method.name(db).text(db),
                    ),
                    base_method.get_scope_id(db).file(db),
                    base_method.name_span(db),
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
                        base_method.name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(implementer.name_span(db))
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "ABSTRACT method '{}' is declared here",
                        base_method.name(db).text(db),
                    ),
                    base_method.get_scope_id(db).file(db),
                    base_method.name_span(db),
                ));
                diag.with_note("ABSTRACT methods must be implemented by derived POUs".into());
                diag
            }
            Self::EmptyOverride { base_method } => {
                let mut diag = diag()
                    .message(format!(
                        "invalid usage of OVERRIDE for method '{}'",
                        base_method.name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(base_method.name_span(db))
                    .call();

                diag.with_note("OVERRIDE is only valid when the method is inherited".into());

                diag
            }
            Self::AbstractClassHasNoAbstractMethods { class } => {
                let mut diag = diag()
                    .message(format!(
                        "ABSTRACT class '{}' has no abstract methods",
                        class.name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(class.name_span(db))
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
                        method.name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(implementer.name_span(db))
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "method '{}' is declared by interface '{}' here",
                        method.name(db).text(db),
                        implementer.name(db).text(db),
                    ),
                    method.get_scope_id(db).file(db),
                    method.name_span(db),
                ));
                diag
            }
            Self::UnresolvedThisMethod { ctx, path, method } => diag()
                .message(format!(
                    "no method '{}' in declared methods of '{}'",
                    path.ident(db).ident.text(db),
                    ctx.name(db).text(db)
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
                            caller.name(db).text(db)
                        ),
                        caller.get_scope_id(db).file(db),
                        caller.name_span(db),
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
                        m1.name(db).text(db),
                        expected,
                        got
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(m2.name_span(db).clone())
                    .call();

                diag.with_related(Related::new(
                    format!("base method '{}' is declared here", m1.name(db).text(db)),
                    m1.get_scope_id(db).file(db),
                    m1.name_span(db),
                ));
                diag
            }
            /*Self::SignatureParametersTypeMismatch { err, param } => {
                let mut diag = diag()
                    .message(format!(
                        "invalid parameter in method signature: {}",
                        err.description(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(param.get_name_span(db).unwrap())
                    .call();

                err.related(db, &mut diag);
                err.note(db, &mut diag);

                diag
            }*/
        }
    }
}
