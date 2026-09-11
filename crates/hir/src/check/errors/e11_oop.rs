use crate::CallSite;
use crate::HasName;
use crate::HirNodeInfo;
use crate::check::errors::ToIdeDiagnostic;
use crate::hir_def::expressions::expression::VariableAccess;
use crate::hir_def::expressions::spec::Spec;
use crate::hir_def::pous::interface::Interface;
use crate::hir_def::pous::pou::Pou;
use crate::hir_def::pous::variable::VariableDecl;
use crate::hir_def::pous::variable::VariableKind;
use crate::hir_ty::head::inheritance::MethodRef;
use crate::hir_ty::ty::Type;
use auto_lsp::default::db::file::File;
use auto_lsp::lsp_types::DiagnosticSeverity;
use auto_lsp::tree_sitter::Range;
use db::WorkspaceDataBase;
use ide_diagnostic::ErrorCode;
use ide_diagnostic::IdeDiagnostic;
use ide_diagnostic::Related;
use ide_diagnostic::diag;

/// The declaring keyword of a variable section, for messages.
fn section_keyword(kind: VariableKind) -> &'static str {
    match kind {
        VariableKind::Var => "VAR",
        VariableKind::Input => "VAR_INPUT",
        VariableKind::Output => "VAR_OUTPUT",
        VariableKind::InOut => "VAR_IN_OUT",
        VariableKind::External => "VAR_EXTERNAL",
        VariableKind::Global => "VAR_GLOBAL",
        VariableKind::Access => "VAR_ACCESS",
        VariableKind::Temp => "VAR_TEMP",
        VariableKind::Config => "VAR_CONFIG",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum OopError<'db> {
    MultipleExtends {
        /// Span of the second (duplicate) EXTENDS clause
        location: Range,
        /// Span of the first EXTENDS clause
        first_extend_span: Range,
        /// File containing both clauses
        file: File,
    },
    MultipleImplements {
        /// Span of the second (duplicate) IMPLEMENTS clause
        location: Range,
        /// Span of the first IMPLEMENTS clause
        first_implements_span: Range,
        /// File containing both clauses
        file: File,
    },
    ImplementsBeforeExtends {
        /// Span of the misplaced IMPLEMENTS clause
        implements_span: Range,
        /// Span of the EXTENDS clause
        extends_span: Range,
        file: File,
    },
    /// FINAL says the type is complete and closed. The method-level rule was
    /// enforced (E1114) while the type-level one was not, so `FINAL` on a
    /// CLASS or FUNCTION_BLOCK header meant nothing.
    ExtendsFinalPou {
        derived: Pou<'db>,
        base: Pou<'db>,
        extends: CallSite<'db>,
    },
    ThisOnIncompatiblePou {
        call_site: CallSite<'db>,
    },
    SuperOnIncompatiblePou {
        call_site: CallSite<'db>,
    },
    SuperButNoExtends {
        pou: Pou<'db>,
        call_site: CallSite<'db>,
    },
    SuperBodyOnIncompatiblePou {
        call_site: CallSite<'db>,
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
    MissingOverride {
        base_method: MethodRef<'db>,
        derived_method: MethodRef<'db>,
    },
    EmptyOverride {
        base_method: MethodRef<'db>,
    },
    OverrideFinalMethod {
        base_method: MethodRef<'db>,
        derived_method: MethodRef<'db>,
    },
    /// A derived FB/Class declares a variable whose name collides with one
    /// inherited (transitively) from a base. Per IEC 6.6.7.2.9 rule 3, the names
    /// of the variables in the base and the derived function blocks shall be
    /// unique. `derived` is the redeclaration; `base` the inherited declaration.
    InheritedMemberShadowed {
        derived: VariableDecl<'db>,
        base: VariableDecl<'db>,
    },
    MissingAbstractMethod {
        implementer: Pou<'db>,
        base_method: MethodRef<'db>,
    },
    /// IEC 61131-3 6.6.7: a POU declaring an ABSTRACT method shall itself be
    /// ABSTRACT. Unenforced, the method had no body, no derived POU was
    /// obliged to give it one, and calling it returned 0.
    AbstractMethodInConcretePou {
        pou: Pou<'db>,
        method: MethodRef<'db>,
    },
    /// An ABSTRACT type describes what derived POUs must provide; it has no
    /// implementation of its own, so it cannot be instantiated.
    InstantiatedAbstractPou {
        var: VariableDecl<'db>,
        pou: Pou<'db>,
    },
    UnimplementedInterfaceMethod {
        implementer: Pou<'db>,
        method: MethodRef<'db>,
        declared_by: Pou<'db>,
    },
    AccessSpecNotAllowedInMethodPrototype(Range),
    /// An interface type used in a variable that is not a `VAR_INPUT` /
    /// `VAR_IN_OUT` parameter. Interfaces are supported only as (statically
    /// monomorphized) parameters — not stored, output, or returned — so every
    /// call through one resolves to a concrete type at compile time.
    InterfaceOnlyAllowedAsParam {
        var: VariableDecl<'db>,
        interface: Interface<'db>,
    },
    /// An interface `VAR_INPUT` / `VAR_IN_OUT` on a POU with instance state.
    /// An FB or PROGRAM input lives in the instance across calls, which makes
    /// it a STORED interface; parameters specialize per call, so they exist
    /// on FUNCTION and METHOD only. This used to pass `rk check` and ICE in
    /// `rk compile`.
    InterfaceParamOnStatefulPou {
        var: VariableDecl<'db>,
        interface: Interface<'db>,
        pou_kind: &'static str,
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
        base_param: VariableDecl<'db>,
        param: VariableDecl<'db>,
    },
    /// The RETURN is part of the signature too. Unchecked, an INT prototype
    /// implemented as REAL produced a wasm signature the monomorphized call
    /// site disagreed with — invalid wasm at exit 0 — and a same-lane
    /// divergence (INT vs DINT) was silently wrong instead.
    SignatureReturnMismatch {
        expected: Option<Type<'db>>,
        got: Option<Type<'db>>,
        method: MethodRef<'db>,
        base: MethodRef<'db>,
    },
    /// Parameters match by POSITION, and the name at each position is part
    /// of the signature: a caller binding `a := 5` through the prototype
    /// must reach the implementation's `a`.
    SignatureNameMismatch {
        method: MethodRef<'db>,
        base_param: VariableDecl<'db>,
        param: VariableDecl<'db>,
    },
    /// The SECTION is part of the signature too: a `VAR_INPUT` prototype
    /// implemented as `VAR_IN_OUT` is called by value through the interface
    /// and by address in the implementation — executed, the call returned a
    /// wrong value with no diagnostic.
    SignatureSectionMismatch {
        method: MethodRef<'db>,
        base_param: VariableDecl<'db>,
        param: VariableDecl<'db>,
    },
}

impl<'db> ErrorCode for OopError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::MultipleExtends { .. } => "E1101",
            Self::MultipleImplements { .. } => "E1102",
            Self::ImplementsBeforeExtends { .. } => "E1103",
            Self::ExtendsFinalPou { .. } => "E1104",
            Self::ThisOnIncompatiblePou { .. } => "E1105",
            Self::SuperOnIncompatiblePou { .. } => "E1106",
            Self::SuperButNoExtends { .. } => "E1107",
            Self::SuperBodyOnIncompatiblePou { .. } => "E1108",
            Self::SuperBodyInMethod { .. } => "E1109",
            Self::SuperBodyMultiple { .. } => "E1110",
            Self::SuperBodyInLoop { .. } => "E1111",
            Self::MissingOverride { .. } => "E1112",
            Self::EmptyOverride { .. } => "E1113",
            Self::OverrideFinalMethod { .. } => "E1114",
            Self::InheritedMemberShadowed { .. } => "E1115",
            Self::MissingAbstractMethod { .. } => "E1116",
            Self::AbstractMethodInConcretePou { .. } => "E1117",
            Self::InstantiatedAbstractPou { .. } => "E1118",
            Self::UnimplementedInterfaceMethod { .. } => "E1119",
            Self::AccessSpecNotAllowedInMethodPrototype(_) => "E1120",
            Self::InterfaceOnlyAllowedAsParam { .. } => "E1121",
            Self::InterfaceParamOnStatefulPou { .. } => "E1121",
            Self::InterfaceNotAllowedInReturn { .. } => "E1122",
            Self::InterfaceNotAllowedNested { .. } => "E1123",
            Self::InterfaceParamNotAssignable { .. } => "E1124",
            Self::SignatureParametersCountMismatch { .. } => "E1125",
            Self::SignatureTypeMismatch { .. } => "E1126",
            Self::SignatureReturnMismatch { .. } => "E1127",
            Self::SignatureNameMismatch { .. } => "E1128",
            Self::SignatureSectionMismatch { .. } => "E1129",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::MultipleExtends { .. } => "syntax",
            Self::MultipleImplements { .. } => "syntax",
            Self::ImplementsBeforeExtends { .. } => "syntax",
            Self::ExtendsFinalPou { .. } => "inheritance violation",
            Self::ThisOnIncompatiblePou { .. } => "invalid use of SUPER or THIS",
            Self::SuperOnIncompatiblePou { .. } => "invalid use of SUPER or THIS",
            Self::SuperButNoExtends { .. } => "invalid use of SUPER or THIS",
            Self::SuperBodyOnIncompatiblePou { .. } => "invalid use of SUPER or THIS",
            Self::SuperBodyInMethod { .. } => "invalid use of SUPER or THIS",
            Self::SuperBodyMultiple { .. } => "invalid use of SUPER or THIS",
            Self::SuperBodyInLoop { .. } => "invalid use of SUPER or THIS",
            Self::MissingOverride { .. } => "override violation",
            Self::EmptyOverride { .. } => "inheritance violation",
            Self::OverrideFinalMethod { .. } => "override violation",
            Self::InheritedMemberShadowed { .. } => "inheritance violation",
            Self::MissingAbstractMethod { .. } => "inheritance violation",
            Self::AbstractMethodInConcretePou { .. } => "inheritance violation",
            Self::InstantiatedAbstractPou { .. } => "inheritance violation",
            Self::UnimplementedInterfaceMethod { .. } => "inheritance violation",
            Self::AccessSpecNotAllowedInMethodPrototype(_) => "syntax",
            Self::InterfaceOnlyAllowedAsParam { .. } => "interface type not allowed here",
            Self::InterfaceParamOnStatefulPou { .. } => "interface type not allowed here",
            Self::InterfaceNotAllowedInReturn { .. } => "interface type not allowed here",
            Self::InterfaceNotAllowedNested { .. } => "interface type not allowed here",
            Self::InterfaceParamNotAssignable { .. } => "interface parameter is not assignable",
            Self::SignatureParametersCountMismatch { .. } => "method parameter count mismatch",
            Self::SignatureTypeMismatch { .. } => "method parameter type mismatch",
            Self::SignatureReturnMismatch { .. } => "method return type mismatch",
            Self::SignatureNameMismatch { .. } => "method parameter name mismatch",
            Self::SignatureSectionMismatch { .. } => "method parameter section mismatch",
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for OopError<'db> {
    fn to_diagnostic(
        &self,
        db: &'db dyn WorkspaceDataBase,
        file: auto_lsp::default::db::file::File,
    ) -> IdeDiagnostic {
        match self {
            Self::MultipleExtends {
                location,
                first_extend_span,
                file,
            } => {
                let doc = file.document(db);
                let src = doc.as_str();

                let extract = |span: &Range| -> String {
                    src.get(span.start_byte..span.end_byte)
                        .unwrap_or("")
                        .replace("EXTENDS ", "")
                        .trim()
                        .to_string()
                };
                let second = extract(location);
                let first = extract(first_extend_span);

                let mut diag = diag()
                    .message("multiple EXTENDS declarations are not allowed".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, location).unwrap_or_default())
                    .call();

                diag.with_related(Related::new(
                    format!("merge into single clause: 'EXTENDS {first}, {second}'"),
                    *file,
                    *first_extend_span,
                ));
                diag
            }
            Self::MultipleImplements {
                location,
                first_implements_span,
                file,
            } => {
                let doc = file.document(db);
                let src = doc.as_str();

                let extract = |span: &Range| -> String {
                    src.get(span.start_byte..span.end_byte)
                        .unwrap_or("")
                        .replace("IMPLEMENTS ", "")
                        .trim()
                        .to_string()
                };
                let second = extract(location);
                let first = extract(first_implements_span);

                let mut diag = diag()
                    .message("multiple IMPLEMENTS declarations are not allowed".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, location).unwrap_or_default())
                    .call();

                diag.with_related(Related::new(
                    format!("merge into single clause: 'IMPLEMENTS {first}, {second}'"),
                    *file,
                    *first_implements_span,
                ));
                diag
            }
            Self::ImplementsBeforeExtends {
                implements_span,
                extends_span,
                file,
            } => {
                let doc = file.document(db);
                let src = doc.as_str();

                let extract = |span: &Range| -> String {
                    src.get(span.start_byte..span.end_byte)
                        .unwrap_or("")
                        .trim()
                        .to_string()
                };
                let implements_text = extract(implements_span);

                let mut diag = diag()
                    .message("IMPLEMENTS must be declared after EXTENDS".into())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, implements_span).unwrap_or_default())
                    .call();

                diag.with_related(Related::new(
                    format!("move '{implements_text}' here"),
                    *file,
                    *extends_span,
                ));
                diag
            }
            Self::ExtendsFinalPou {
                derived,
                base,
                extends,
            } => {
                let kind = match base {
                    Pou::Class(_) => "CLASS",
                    _ => "FUNCTION_BLOCK",
                };
                let mut diag = diag()
                    .message(format!(
                        "'{}' cannot extend FINAL {kind} '{}'",
                        derived.get_name_ident(db).text(db),
                        base.get_name_ident(db).text(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &extends.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_related(Related::new(
                    format!(
                        "{kind} '{}' is declared FINAL here",
                        base.get_name_ident(db).text(db),
                    ),
                    base.get_scope_id(db).file(db),
                    base.get_name_span(db),
                ));
                diag.with_note(
                    "FINAL declares a type complete: it may be used, but not extended".into(),
                );
                diag
            }
            Self::ThisOnIncompatiblePou { call_site } => diag()
                .message("'THIS' is not valid in this context".to_string())
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
            Self::SuperButNoExtends { call_site, pou } => diag()
                .message(format!(
                    "'SUPER' used but no EXTENDS clause found on '{}'",
                    pou.get_name_ident(db).text(db)
                ))
                .range(crate::denormalize(db, file, &call_site.get_span(db)).unwrap_or_default())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .call(),
            Self::SuperBodyOnIncompatiblePou { call_site } => diag()
                .message("'SUPER()' is not valid in this context".to_string())
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
                    "the first 'SUPER()' is here".to_string(),
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
            Self::InheritedMemberShadowed { derived, base } => {
                let name = derived.get_name_ident(db).text(db);
                let mut diag = diag()
                    .message(format!(
                        "variable '{name}' is already declared in a base POU"
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
                diag.with_note("variable names in a base and derived POU must be unique".into());
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
            Self::AbstractMethodInConcretePou { pou, method } => {
                let kind = match pou {
                    Pou::Class(_) => "CLASS",
                    _ => "FUNCTION_BLOCK",
                };
                let mut diag = diag()
                    .message(format!(
                        "{kind} '{}' declares an ABSTRACT method, so it must be ABSTRACT itself",
                        pou.get_name_ident(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &pou.get_name_span(db)).unwrap_or_default())
                    .call();
                diag.with_related(Related::new(
                    format!(
                        "ABSTRACT method '{}' is declared here",
                        method.get_name_ident(db).text(db),
                    ),
                    method.get_scope_id(db).file(db),
                    method.get_name_span(db),
                ));
                diag.with_note(
                    "an ABSTRACT method has no body, so every POU declaring one is incomplete"
                        .into(),
                );
                diag
            }
            Self::InstantiatedAbstractPou { var, pou } => {
                let kind = match pou {
                    Pou::Class(_) => "CLASS",
                    _ => "FUNCTION_BLOCK",
                };
                let mut diag = diag()
                    .message(format!(
                        "cannot instantiate ABSTRACT {kind} '{}'",
                        pou.get_name_ident(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &var.spec(db).get_span(db))
                            .unwrap_or_default(),
                    )
                    .call();
                diag.with_related(Related::new(
                    format!(
                        "{kind} '{}' is declared ABSTRACT here",
                        pou.get_name_ident(db).text(db),
                    ),
                    pou.get_scope_id(db).file(db),
                    pou.get_name_span(db),
                ));
                diag.with_note("declare a variable of a derived type that implements it".into());
                diag
            }
            Self::UnimplementedInterfaceMethod {
                implementer,
                method,
                declared_by,
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
                        declared_by.get_name_ident(db).text(db),
                    ),
                    method.get_scope_id(db).file(db),
                    method.get_name_span(db),
                ));
                diag
            }
            Self::AccessSpecNotAllowedInMethodPrototype(span) => {
                let mut diag = diag()
                    .message(
                        "access specifiers are not allowed on interface method prototypes".into(),
                    )
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
                    .call();

                diag.with_note("interface methods are implicitly PUBLIC".into());
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
            Self::InterfaceParamOnStatefulPou {
                var,
                interface,
                pou_kind,
            } => {
                let section = match var.kind(db) {
                    VariableKind::InOut => "VAR_IN_OUT",
                    _ => "VAR_INPUT",
                };
                diag()
                    .message(format!(
                        "interface '{}' cannot be a {pou_kind} {section}: the instance would store it across calls; interface parameters exist on FUNCTION and METHOD only",
                        interface.get_name_ident(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &var.get_span(db)).unwrap_or_default())
                    .call()
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
                base_param,
                param,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "parameter '{}' of method '{}' has an incompatible type: expected '{}', got '{}'",
                        param.name(db).text(db),
                        method.get_name_ident(db).text(db),
                        expected.type_name(db),
                        got.type_name(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &param.spec(db).get_span(db))
                            .unwrap_or_default(),
                    )
                    .call();
                diag.with_related(Related::new(
                    format!(
                        "the base method declares '{}' as '{}' here",
                        base_param.name(db).text(db),
                        expected.type_name(db),
                    ),
                    base_param.get_scope_id(db).file(db),
                    base_param.spec(db).get_span(db),
                ));
                diag.with_note("parameter types must match those of the base method".into());
                diag
            }
            Self::SignatureReturnMismatch {
                expected,
                got,
                method,
                base,
            } => {
                let name = |t: &Option<Type<'db>>| match t {
                    Some(t) => t.type_name(db),
                    None => "none".into(),
                };
                let ret_span = |m: &MethodRef<'db>| match m.return_type(db) {
                    Some(spec) => spec.get_span(db),
                    None => m.get_name_span(db),
                };
                let mut diag = diag()
                    .message(format!(
                        "method '{}' has an incompatible return type: expected '{}', got '{}'",
                        method.get_name_ident(db).text(db),
                        name(expected),
                        name(got),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &ret_span(method)).unwrap_or_default())
                    .call();
                diag.with_related(Related::new(
                    format!(
                        "base method '{}' declares its return type here",
                        base.get_name_ident(db).text(db),
                    ),
                    base.get_scope_id(db).file(db),
                    ret_span(base),
                ));
                diag.with_note("the return type must match the base method's".into());
                diag
            }
            Self::SignatureNameMismatch {
                method,
                base_param,
                param,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "parameter '{}' of method '{}' is named '{}' in the base method",
                        param.name(db).text(db),
                        method.get_name_ident(db).text(db),
                        base_param.name(db).text(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &param.get_name_span(db)).unwrap_or_default(),
                    )
                    .call();
                diag.with_related(Related::new(
                    format!(
                        "the base method declares '{}' at this position",
                        base_param.name(db).text(db),
                    ),
                    base_param.get_scope_id(db).file(db),
                    base_param.get_name_span(db),
                ));
                diag
            }
            Self::SignatureSectionMismatch {
                method,
                base_param,
                param,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "parameter '{}' of method '{}' is {} here but {} in the base method",
                        param.name(db).text(db),
                        method.get_name_ident(db).text(db),
                        section_keyword(param.kind(db)),
                        section_keyword(base_param.kind(db)),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &param.get_name_span(db)).unwrap_or_default(),
                    )
                    .call();
                diag.with_related(Related::new(
                    format!(
                        "the base method declares '{}' as {} here",
                        base_param.name(db).text(db),
                        section_keyword(base_param.kind(db)),
                    ),
                    base_param.get_scope_id(db).file(db),
                    base_param.get_name_span(db),
                ));
                diag
            }
        }
    }
}
