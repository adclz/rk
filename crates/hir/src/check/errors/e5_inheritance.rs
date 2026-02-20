use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

use crate::{
    CallSite, HasName, HirNodeInfo,
    check::errors::analysis_error::{AnalysisError, ToIdeDiagnostic},
    hir_def::{
        expressions::{expression::PathExpr, invocation::Invocation},
        pous::pou::Pou,
    },
    hir_ty::{head::inheritance::MethodRef, ty::Type},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum InheritanceError<'db> {
    SuperBodyOnIncompatiblePou {
        call_site: CallSite<'db>,
    },
    SuperOnIncompatiblePou {
        call_site: CallSite<'db>,
    },
    ThisOnIncompatiblePou {
        call_site: CallSite<'db>,
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
    SuperButNoExtends {
        pou: Pou<'db>,
        call_site: CallSite<'db>,
    },
}

impl<'db> From<InheritanceError<'db>> for AnalysisError<'db> {
    fn from(err: InheritanceError<'db>) -> Self {
        AnalysisError::Inheritance(err)
    }
}

impl<'db> ErrorCode for InheritanceError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::SuperBodyOnIncompatiblePou { .. } => "E0501",
            Self::SuperOnIncompatiblePou { .. } => "E0502",
            Self::ThisOnIncompatiblePou { .. } => "E0503",
            Self::OverrideFinalMethod { .. } => "E0502",
            Self::MissingOverride { .. } => "E0503",
            Self::MissingAbstractMethod { .. } => "E0504",
            Self::EmptyOverride { .. } => "E0505",
            Self::AbstractClassHasNoAbstractMethods { .. } => "E0506",
            Self::UnimplementedInterfaceMethod { .. } => "E0507",
            Self::UnresolvedThisMethod { .. } => "E0508",
            Self::UnresolvedSuperMethod { .. } => "E0509",
            Self::SignatureParametersCountMismatch { .. } => "E0510",
            Self::SignatureTypeMismatch { .. } => "E0511",
            Self::SuperButNoExtends { .. } => "E0512",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::SuperBodyOnIncompatiblePou { .. }
            | Self::SuperOnIncompatiblePou { .. }
            | Self::ThisOnIncompatiblePou { .. }
            | Self::SuperButNoExtends { .. } => "invalid use of SUPER or THIS",
            Self::OverrideFinalMethod { .. } | Self::MissingOverride { .. } => "override violation",
            Self::MissingAbstractMethod { .. }
            | Self::EmptyOverride { .. }
            | Self::AbstractClassHasNoAbstractMethods { .. }
            | Self::UnimplementedInterfaceMethod { .. } => "inheritance violation",
            Self::UnresolvedThisMethod { .. } | Self::UnresolvedSuperMethod { .. } => {
                "unresolved method in inheritance context"
            }
            Self::SignatureParametersCountMismatch { .. } | Self::SignatureTypeMismatch { .. } => {
                "method signature mismatch"
            }
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for InheritanceError<'db> {
    fn to_diagnostic(&self, db: &'db dyn WorkspaceDataBase) -> IdeDiagnostic {
        match self {
            Self::SuperBodyOnIncompatiblePou { call_site } => diag()
                .message("'SUPER()' is not valid in this context".to_string())
                .range(call_site.get_span(db))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .call(),
            Self::SuperOnIncompatiblePou { call_site } => diag()
                .message("'SUPER' is not valid in this context".to_string())
                .range(call_site.get_span(db))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .call(),
            Self::ThisOnIncompatiblePou { call_site } => diag()
                .message("'THIS' is not valid in this context".to_string())
                .range(call_site.get_span(db))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
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
                    .desc(self)
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
                    .desc(self)
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
                    .desc(self)
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
                    .desc(self)
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
                    .desc(self)
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
                    .desc(self)
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
                .desc(self)
                .range(method.get_span(db))
                .call(),
            Self::UnresolvedSuperMethod { ctx, path, method } => {
                let mut diag = diag()
                    .message(format!(
                        "no method '{}' in inherited methods",
                        path.ident(db).ident.text(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(method.get_span(db))
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
                    .desc(self)
                    .range(m2.get_name_span(db))
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
                        expected.type_name(db),
                        got.type_name(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(method.get_name_span(db))
                    .call();

                diag.with_note("parameter types must match those of the base method".into());

                diag
            }
            Self::SuperButNoExtends { call_site, pou } => diag()
                .message(format!(
                    "'SUPER' used but no EXTENDS clause found on '{}'",
                    pou.get_name_ident(db).text(db)
                ))
                .range(call_site.get_span(db))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .call(),
        }
    }
}
