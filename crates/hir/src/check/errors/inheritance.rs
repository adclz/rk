use auto_lsp::{default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use ide_diagnostic::{IdeDiagnostic, Related, diag};

use crate::{
    HirNodeInfo,
    check::errors::{
        analysis_error::{AnalysisError, DiagnosticDescription, ToIdeDiagnostic},
        coerce::TypeMismatch,
        utils::get_decl_for_ty,
    },
    hir_def::expressions::{expression::PathExpr, invocation::Invocation},
    hir_ty::ty::Ty,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum MethodError<'db> {
    OverrideFinalMethod {
        base_method: Ty<'db>,
        derived_method: Ty<'db>,
    },
    MissingOverride {
        base_method: Ty<'db>,
        derived_method: Ty<'db>,
    },
    MissingAbstractMethod {
        implementer: Ty<'db>,
        base_method: Ty<'db>,
    },
    EmptyOverride {
        base_method: Ty<'db>,
    },
    AbstractClassHasNoAbstractMethods {
        class: Ty<'db>,
    },
    UnimplementedInterfaceMethod {
        implementer: Ty<'db>,
        method: Ty<'db>,
    },
    // Invocations
    UnresolvedThisMethod {
        ctx: Ty<'db>,
        path: PathExpr<'db>,
        method: Invocation<'db>,
    },
    UnresolvedSuperMethod {
        ctx: Option<Ty<'db>>,
        path: PathExpr<'db>,
        method: Invocation<'db>,
    },
    ThisOnIncompatiblePou {
        path: PathExpr<'db>,
        method: Invocation<'db>,
    },
    SuperOnIncompatiblePou {
        path: PathExpr<'db>,
        method: Invocation<'db>,
    },
    SuperBodyOnIncompatiblePou {
        ctx: Ty<'db>,
        method: Invocation<'db>,
    },
    // Signatures
    SignatureParametersCountMismatch {
        m1: Ty<'db>,
        expected: usize,
        m2: Ty<'db>,
        got: usize,
    },
    SignatureParametersTypeMismatch {
        err: TypeMismatch<'db>,
        param: Ty<'db>,
    },
}

impl<'db> From<MethodError<'db>> for AnalysisError<'db> {
    fn from(err: MethodError<'db>) -> Self {
        AnalysisError::MethodError(err)
    }
}

impl<'db> ToIdeDiagnostic<'db> for MethodError<'db> {
    fn to_diagnostic(&self, db: &'db dyn BaseDatabase) -> IdeDiagnostic {
        match self {
            Self::MissingOverride {
                base_method,
                derived_method,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "missing OVERRIDE keyword for method '{}'",
                        derived_method.decl(db).name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(derived_method.decl(db).name_span(db))
                    .call();

                get_decl_for_ty(db, *base_method, &mut diag);
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
                        base_method.decl(db).name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(derived_method.decl(db).name_span(db))
                    .call();

                get_decl_for_ty(db, *base_method, &mut diag);
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
                        base_method.decl(db).name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(implementer.decl(db).name_span(db))
                    .call();

                get_decl_for_ty(db, *base_method, &mut diag);
                diag.with_note("ABSTRACT methods must be implemented by derived POUs".into());

                diag
            }
            Self::EmptyOverride { base_method } => {
                let mut diag = diag()
                    .message(format!(
                        "invalid usage of OVERRIDE for method '{}'",
                        base_method.decl(db).name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(base_method.decl(db).name_span(db))
                    .call();

                diag.with_note("OVERRIDE is only valid when the method is inherited".into());

                diag
            }
            Self::AbstractClassHasNoAbstractMethods { class } => {
                let mut diag = diag()
                    .message(format!(
                        "ABSTRACT class '{}' has no abstract methods",
                        class.decl(db).name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(class.decl(db).name_span(db))
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
                        method.decl(db).name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(implementer.decl(db).name_span(db))
                    .call();

                get_decl_for_ty(db, *method, &mut diag);

                diag
            }
            Self::UnresolvedThisMethod { ctx, path, method } => diag()
                .message(format!(
                    "no method '{}' in declared methods of '{}'",
                    path.ident(db).ident.text(db),
                    ctx.decl(db).name(db).text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .range(method.get_span(db).clone())
                .call(),
            Self::UnresolvedSuperMethod { ctx, path, method } => {
                let mut diag = diag()
                    .message(format!(
                        "no method '{}' in inherited methods of '{}'",
                        path.ident(db).ident.text(db),
                        ctx.map(|c| c.decl(db).name(db).text(db).to_string())
                            .unwrap_or_else(|| "<unknown>".into())
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(method.get_span(db).clone())
                    .call();

                if let Some(caller) = ctx {
                    if let Some(span) = caller.def(db).get_span(db) {
                        let origin = caller.def(db).def_as_ty(db).unwrap();

                        diag.with_related(Related::new(
                            format!(
                                "methods are inherited from '{}' here",
                                origin.decl(db).name(db).text(db),
                            ),
                            caller
                                .def(db)
                                .get_scope_id(db)
                                .expect("A ty definition with a span always has a scope id")
                                .file(db),
                            span,
                        ));
                    }
                }
                diag
            }
            Self::ThisOnIncompatiblePou { path, method } => diag()
                .message("THIS can only be used in in FUNCTION_BLOCK or CLASS POUs".into())
                .severity(DiagnosticSeverity::ERROR)
                .range(method.get_span(db).clone())
                .call(),
            Self::SuperOnIncompatiblePou { path, method } => diag()
                .message("SUPER can only be used in FUNCTION_BLOCK or CLASS POUs".into())
                .severity(DiagnosticSeverity::ERROR)
                .range(method.get_span(db).clone())
                .call(),
            Self::SuperBodyOnIncompatiblePou { ctx, method } => {
                let mut diag = diag()
                    .message("SUPER() can only be called in FUNCTION_BLOCK POUs".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .range(method.get_span(db).clone())
                    .call();

                get_decl_for_ty(db, *ctx, &mut diag);

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
                        "invalid number of parameters for inherited method '{}': expected {}, got {}",
                        m1.decl(db).name(db).text(db),
                        expected,
                        got
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(m2.decl(db).name_span(db).clone())
                    .call();

                get_decl_for_ty(db, *m1, &mut diag);

                diag
            }
            Self::SignatureParametersTypeMismatch { err, param } => {
                let mut diag = diag()
                    .message(format!(
                        "invalid parameter in method signature: {}",
                        err.description(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(param.decl(db).name_span(db).clone())
                    .call();

                err.related(db, &mut diag);
                err.note(db, &mut diag);

                diag
            }
        }
    }
}
