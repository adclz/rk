use auto_lsp::{default::db::BaseDatabase, lsp_types::DiagnosticSeverity};
use ide_diagnostic::{IdeDiagnostic, Related, diag};

use crate::{
    check::errors::{
        analysis_error::{AnalysisError, DiagnosticDescription, ToIdeDiagnostic},
        coerce::TypeMismatch,
    }, hir_def::{expressions::{expression::PathExpr, invocation::Invocation}, pous::{pou::{Pou, PouDecl}, variable::VariableDecl}, scope::FileScopeId}, hir_ty::{inheritance_solver::MethodRef, ty::Ty}, HirNodeInfo
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum MethodError<'db> {
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
    ThisOnIncompatiblePou {
        path: PathExpr<'db>,
        method: Invocation<'db>,
    },
    SuperOnIncompatiblePou {
        path: PathExpr<'db>,
        method: Invocation<'db>,
    },
    SuperBodyOnIncompatiblePou {
        ctx: PouDecl<'db>,
        method: Invocation<'db>,
    },
    // Signatures
    SignatureParametersCountMismatch {
        m1: MethodRef<'db>,
        expected: usize,
        m2: MethodRef<'db>,
        got: usize,
    },
    SignatureParametersTypeMismatch {
        err: TypeMismatch<'db>,
        param: VariableDecl<'db>,
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
                        derived_method.name(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(derived_method.name_span(db))
                    .call();

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

                /*if let Some(caller) = ctx {
                    if let Some(span) = caller.def(db).get_span(db) {
                        let origin = caller.def(db).def_as_ty(db).unwrap();

                        diag.with_related(Related::new(
                            format!(
                                "methods are inherited from '{}' here",
                                origin.name(db),
                            ),
                            caller
                                .def(db)
                                .get_scope_id(db)
                                .file(db),
                            span,
                        ));
                    }
                }*/
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
                        m1.name(db).text(db),
                        expected,
                        got
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .range(m2.name_span(db).clone())
                    .call();

                diag
            }
            Self::SignatureParametersTypeMismatch { err, param } => {
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
            }
        }
    }
}
