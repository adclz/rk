use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

use crate::{
    CallSite, HasName, HirNodeInfo,
    check::errors::ToIdeDiagnostic,
    hir_def::{
        expressions::{
            expression::{PathExpr, VariableAccess},
            invocation::Invocation,
            spec::Spec,
        },
        pous::{
            interface::Interface,
            pou::Pou,
            variable::{VariableDecl, VariableKind},
        },
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
    /// An interface type used in a variable that is not a `VAR_INPUT` /
    /// `VAR_IN_OUT` parameter. Interfaces are supported only as (statically
    /// monomorphized) parameters — not stored, output, or returned — so every
    /// call through one resolves to a concrete type at compile time.
    InterfaceOnlyAllowedAsParam {
        var: VariableDecl<'db>,
        interface: Interface<'db>,
    },
    /// An interface used as a function/method return type. Interfaces are only
    /// allowed as VAR_INPUT / VAR_IN_OUT parameters (Design 1) — a return would
    /// flow the concrete type callee→caller, which can't be monomorphized.
    InterfaceNotAllowedInReturn {
        interface: Interface<'db>,
        spec: Spec<'db>,
    },
    /// An interface nested inside an aggregate type — an array element, a
    /// reference target, or a struct field (e.g. `ARRAY OF ITF1`, `REF_TO ITF1`,
    /// `STRUCT f : ITF1`). Unlike a direct interface (which is allowed as a
    /// param), a nested interface has NO valid placement: it is stored /
    /// heterogeneous state that can't be monomorphized.
    InterfaceNotAllowedNested {
        interface: Interface<'db>,
        spec: Spec<'db>,
    },
    /// Assignment to an interface parameter. An interface `VAR_IN_OUT` param is a
    /// fixed binding to the concrete type the caller supplied; reassigning it
    /// would break monomorphization (the body is specialized to one concrete
    /// type) and write a wrong-typed value into the caller's concrete slot.
    InterfaceParamNotAssignable {
        var: VariableDecl<'db>,
        access: VariableAccess<'db>,
    },
    /// `SUPER()` (base function-block body call) used inside a METHOD. Per IEC
    /// 6.6.7.2.9 rule 5, `SUPER()` may only appear in the function block BODY,
    /// not in a method of a function block.
    SuperBodyInMethod {
        call_site: CallSite<'db>,
    },
    /// More than one `SUPER()` in a function block body. Per IEC 6.6.7.2.9 rule 2,
    /// the call of `SUPER()` shall occur once. `call_site` is the offending
    /// (second) call; `first` is the first `SUPER()`, shown as related info.
    SuperBodyMultiple {
        call_site: CallSite<'db>,
        first: CallSite<'db>,
    },
    /// `SUPER()` nested inside a loop (`FOR`/`WHILE`/`REPEAT`). Per IEC 6.6.7.2.9
    /// rule 2, the call of `SUPER()` shall not be in a loop.
    SuperBodyInLoop {
        call_site: CallSite<'db>,
    },
    /// A derived FB/Class declares a variable whose name collides with one
    /// inherited (transitively) from a base. Per IEC 6.6.7.2.9 rule 3, the names
    /// of the variables in the base and the derived function blocks shall be
    /// unique. `derived` is the redeclaration; `base` the inherited declaration.
    InheritedMemberShadowed {
        derived: VariableDecl<'db>,
        base: VariableDecl<'db>,
    },
}

impl<'db> ErrorCode for InheritanceError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::SuperBodyOnIncompatiblePou { .. } => "E0501",
            Self::SuperOnIncompatiblePou { .. } => "E0502",
            Self::ThisOnIncompatiblePou { .. } => "E0503",
            Self::OverrideFinalMethod { .. } => "E0504",
            Self::MissingOverride { .. } => "E0505",
            Self::MissingAbstractMethod { .. } => "E0506",
            Self::EmptyOverride { .. } => "E0507",
            Self::AbstractClassHasNoAbstractMethods { .. } => "E0508",
            Self::UnimplementedInterfaceMethod { .. } => "E0509",
            Self::UnresolvedThisMethod { .. } => "E0510",
            Self::UnresolvedSuperMethod { .. } => "E0511",
            Self::SignatureParametersCountMismatch { .. } => "E0512",
            Self::SignatureTypeMismatch { .. } => "E0512",
            Self::SuperButNoExtends { .. } => "E0513",
            Self::InterfaceOnlyAllowedAsParam { .. } => "E0514",
            Self::InterfaceNotAllowedInReturn { .. } => "E0515",
            Self::InterfaceNotAllowedNested { .. } => "E0516",
            Self::InterfaceParamNotAssignable { .. } => "E0517",
            Self::SuperBodyInMethod { .. } => "E0518",
            Self::SuperBodyMultiple { .. } => "E0519",
            Self::SuperBodyInLoop { .. } => "E0520",
            Self::InheritedMemberShadowed { .. } => "E0521",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::SuperBodyOnIncompatiblePou { .. }
            | Self::SuperOnIncompatiblePou { .. }
            | Self::ThisOnIncompatiblePou { .. }
            | Self::SuperButNoExtends { .. }
            | Self::SuperBodyInMethod { .. }
            | Self::SuperBodyMultiple { .. }
            | Self::SuperBodyInLoop { .. } => "invalid use of SUPER or THIS",
            Self::OverrideFinalMethod { .. } | Self::MissingOverride { .. } => "override violation",
            Self::MissingAbstractMethod { .. }
            | Self::EmptyOverride { .. }
            | Self::AbstractClassHasNoAbstractMethods { .. }
            | Self::UnimplementedInterfaceMethod { .. }
            | Self::InheritedMemberShadowed { .. } => "inheritance violation",
            Self::UnresolvedThisMethod { .. } | Self::UnresolvedSuperMethod { .. } => {
                "unresolved method in inheritance context"
            }
            Self::SignatureParametersCountMismatch { .. } | Self::SignatureTypeMismatch { .. } => {
                "method signature mismatch"
            }
            Self::InterfaceOnlyAllowedAsParam { .. }
            | Self::InterfaceNotAllowedInReturn { .. }
            | Self::InterfaceNotAllowedNested { .. } => "interface type not allowed here",
            Self::InterfaceParamNotAssignable { .. } => "interface parameter is not assignable",
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for InheritanceError<'db> {
    fn to_diagnostic(
        &self,
        db: &'db dyn WorkspaceDataBase,
        file: auto_lsp::default::db::file::File,
    ) -> IdeDiagnostic {
        match self {
            Self::SuperBodyOnIncompatiblePou { call_site } => diag()
                .message("'SUPER()' is not valid in this context".to_string())
                .range(crate::denormalize(db, file, &call_site.get_span(db)).unwrap_or_default())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .call(),
            Self::SuperOnIncompatiblePou { call_site } => diag()
                .message("'SUPER' is not valid in this context".to_string())
                .range(crate::denormalize(db, file, &call_site.get_span(db)).unwrap_or_default())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .call(),
            Self::ThisOnIncompatiblePou { call_site } => diag()
                .message("'THIS' is not valid in this context".to_string())
                .range(crate::denormalize(db, file, &call_site.get_span(db)).unwrap_or_default())
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
                    .range(
                        crate::denormalize(db, file, &derived_method.get_name_span(db))
                            .unwrap_or_default(),
                    )
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "base method '{}' is declared here",
                        base_method.get_name_ident(db).text(db),
                    ),
                    base_method.get_scope_id(db).file(db),
                    base_method.get_name_span(db),
                ));
                diag.with_note("OVERRIDE is required when redefining a method with the same signature from a base class or function block".into());
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
                    .range(
                        crate::denormalize(db, file, &derived_method.get_name_span(db))
                            .unwrap_or_default(),
                    )
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
                    .range(
                        crate::denormalize(db, file, &implementer.get_name_span(db))
                            .unwrap_or_default(),
                    )
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
                    .range(
                        crate::denormalize(db, file, &base_method.get_name_span(db))
                            .unwrap_or_default(),
                    )
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
                    .range(
                        crate::denormalize(db, file, &class.get_name_span(db)).unwrap_or_default(),
                    )
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
                    .range(
                        crate::denormalize(db, file, &implementer.get_name_span(db))
                            .unwrap_or_default(),
                    )
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
                .range(crate::denormalize(db, file, &method.get_span(db)).unwrap_or_default())
                .call(),
            Self::UnresolvedSuperMethod { ctx, path, method } => {
                let mut diag = diag()
                    .message(format!(
                        "no method '{}' in inherited methods",
                        path.ident(db).ident.text(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &method.get_span(db)).unwrap_or_default())
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
            Self::InterfaceOnlyAllowedAsParam { var, interface } => {
                let section = match var.kind(db) {
                    VariableKind::Var => "VAR",
                    VariableKind::Output => "VAR_OUTPUT",
                    VariableKind::External => "VAR_EXTERNAL",
                    VariableKind::Global => "VAR_GLOBAL",
                    VariableKind::Access => "VAR_ACCESS",
                    VariableKind::Temp => "VAR_TEMP",
                    VariableKind::Config => "VAR_CONFIG",
                    VariableKind::Input => "VAR_INPUT",
                    VariableKind::InOut => "VAR_IN_OUT",
                };
                let mut diag = diag()
                    .message(format!(
                        "interface type '{}' is not allowed in {section}",
                        interface.get_name_ident(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &var.get_name_span(db)).unwrap_or_default())
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "interface '{}' is defined here",
                        interface.get_name_ident(db).text(db)
                    ),
                    interface.get_scope_id(db).file(db),
                    interface.get_name_span(db),
                ));
                diag.with_note(
                    "interfaces are supported only as VAR_INPUT or VAR_IN_OUT parameters".into(),
                );
                diag
            }
            Self::InterfaceNotAllowedInReturn { interface, spec } => {
                let mut diag = diag()
                    .message(format!(
                        "interface type '{}' is not allowed as a return type",
                        interface.get_name_ident(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &spec.get_span(db)).unwrap_or_default())
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "interface '{}' is defined here",
                        interface.get_name_ident(db).text(db)
                    ),
                    interface.get_scope_id(db).file(db),
                    interface.get_name_span(db),
                ));
                diag.with_note(
                    "interfaces are supported only as VAR_INPUT or VAR_IN_OUT parameters".into(),
                );
                diag
            }
            Self::InterfaceNotAllowedNested { interface, spec } => {
                let mut diag = diag()
                    .message(format!(
                        "interface type '{}' cannot be nested inside another type (array, reference, or struct)",
                        interface.get_name_ident(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &spec.get_span(db)).unwrap_or_default())
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "interface '{}' is defined here",
                        interface.get_name_ident(db).text(db)
                    ),
                    interface.get_scope_id(db).file(db),
                    interface.get_name_span(db),
                ));
                diag.with_note(
                    "an interface may only appear directly as a VAR_INPUT or VAR_IN_OUT parameter"
                        .into(),
                );
                diag
            }
            Self::InterfaceParamNotAssignable { var, access } => {
                let name = var.get_name_ident(db).text(db);
                let mut diag = diag()
                    .message(format!("cannot assign to interface parameter '{name}'"))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &access.get_span(db)).unwrap_or_default())
                    .call();

                diag.with_related(Related::new(
                    format!("interface parameter '{name}' is declared here"),
                    var.get_scope_id(db).file(db),
                    var.get_name_span(db),
                ));
                diag.with_note("an interface parameter is a fixed binding to the concrete type passed by the caller; it can be used (methods called, passed on) but not reassigned".into());
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
                    .range(crate::denormalize(db, file, &m2.get_name_span(db)).unwrap_or_default())
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
                    .range(
                        crate::denormalize(db, file, &method.get_name_span(db)).unwrap_or_default(),
                    )
                    .call();

                diag.with_note("parameter types must match those of the base method".into());

                diag
            }
            Self::SuperButNoExtends { call_site, pou } => diag()
                .message(format!(
                    "'SUPER' used but no EXTENDS clause found on '{}'",
                    pou.get_name_ident(db).text(db)
                ))
                .range(crate::denormalize(db, file, &call_site.get_span(db)).unwrap_or_default())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .call(),
            Self::SuperBodyInMethod { call_site } => {
                let mut diag = diag()
                    .message(
                        "'SUPER()' cannot be called in a method of a function block".to_string(),
                    )
                    .range(
                        crate::denormalize(db, file, &call_site.get_span(db)).unwrap_or_default(),
                    )
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .call();
                diag.with_note(
                    "SUPER() is only valid in the function block body, not in a method".into(),
                );
                diag
            }
            Self::SuperBodyMultiple { call_site, first } => {
                let mut diag = diag()
                    .message(
                        "'SUPER()' may only be called once in a function block body".to_string(),
                    )
                    .range(
                        crate::denormalize(db, file, &call_site.get_span(db)).unwrap_or_default(),
                    )
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .call();
                diag.with_related(Related::new(
                    "'SUPER()' is already called here".to_string(),
                    file,
                    first.get_span(db),
                ));
                diag
            }
            Self::SuperBodyInLoop { call_site } => diag()
                .message("'SUPER()' cannot be called inside a loop".to_string())
                .range(crate::denormalize(db, file, &call_site.get_span(db)).unwrap_or_default())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .call(),
            Self::InheritedMemberShadowed { derived, base } => {
                let name = derived.get_name_ident(db).text(db);
                let mut diag = diag()
                    .message(format!(
                        "variable '{name}' is already declared in a base function block"
                    ))
                    .range(
                        crate::denormalize(db, file, &derived.get_name_span(db))
                            .unwrap_or_default(),
                    )
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .call();
                diag.with_related(Related::new(
                    format!("inherited variable '{name}' is declared here"),
                    base.get_scope_id(db).file(db),
                    base.get_name_span(db),
                ));
                diag.with_note(
                    "variable names in a base and derived function block must be unique".into(),
                );
                diag
            }
        }
    }
}
