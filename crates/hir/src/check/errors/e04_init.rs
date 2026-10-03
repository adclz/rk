// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use crate::CallSite;
use crate::HirNodeInfo;
use crate::check::errors::ToIdeDiagnostic;
use crate::hir_def::expressions::expression::Expr;
use crate::hir_def::expressions::expression::ExprKind;
use crate::hir_def::expressions::expression::InitExpr;
use crate::hir_def::expressions::expression::PrimaryExpr;
use crate::hir_def::pous::pou::Pou;
use crate::hir_def::scope::ScopeKind;
use crate::hir_def::semantic_index::get_scope;
use crate::hir_ty::infer::const_eval;
use crate::hir_ty::ty::Type;
use auto_lsp::lsp_types::DiagnosticSeverity;
use auto_lsp::tree_sitter::Range;
use db::WorkspaceDataBase;
use ide_diagnostic::ErrorCode;
use ide_diagnostic::IdeDiagnostic;
use ide_diagnostic::Related;
use ide_diagnostic::diag;

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum InitError<'db> {
    /// A once-per-type initializer (a TYPE default, an FB/CLASS member
    /// default, a static PROGRAM field or config global) referenced something
    /// with no compile-time value. Before this code the leaf was silently
    /// DROPPED: the slot read zero from source the check called clean.
    InitNotConstant {
        value: Expr<'db>,
        /// A FUNCTION's or METHOD's input default, which the caller passes.
        input_default: bool,
    },
    /// A `REF()` that is not one address everywhere, where a constant is
    /// needed: in a FUNCTION's or METHOD's input default, directly or as a
    /// CONSTANT's value, which the caller passes before the callee's own
    /// variables exist; or as a CONSTANT's value, which is one for every
    /// instance and every call. A global's, through subscripts that fold, is.
    ReferenceNotConstant {
        value: Expr<'db>,
        origin: RefOrigin<'db>,
        /// An input default, rather than a CONSTANT's value.
        input_default: bool,
    },
    FunctionCallInInitExpression(Range),
    NoFieldOnElementaryType {
        expr: InitExpr<'db>,
        ty: Type<'db>,
    },
    AssignToConstant {
        access: CallSite<'db>,
        /// The CONSTANT variable written, or the one the place is part of;
        /// `None` for a TYPE's constant.
        constant: Option<crate::hir_def::pous::variable::VariableDecl<'db>>,
    },
    /// An instance's initializer names a member that has no value of its own
    /// for it to give.
    UninitializableMember {
        expr: InitExpr<'db>,
        /// The member, pointed at where it is declared.
        var: crate::hir_def::pous::variable::VariableDecl<'db>,
        kind: UninitializableMember,
    },
    /// A constant handed where it could be changed: to a VAR_IN_OUT, or to
    /// `REF()`. Only a store into it was refused, and it changed through the
    /// parameter or the pointer.
    ConstantHandedOut {
        access: CallSite<'db>,
        route: ConstantRoute,
        /// As in [`Self::AssignToConstant`].
        constant: Option<crate::hir_def::pous::variable::VariableDecl<'db>>,
    },
    /// A FUNCTION_BLOCK or CLASS instance declared CONSTANT: its body and its
    /// methods write its own variables, so it changed whenever it ran.
    ConstantInstance {
        var: crate::hir_def::pous::variable::VariableDecl<'db>,
        /// The block it is an instance of, as named.
        block: String,
        /// An ARRAY of instances.
        many: bool,
    },
    /// A FUNCTION's or METHOD's initializer reads one of its variables that
    /// gets its value after it, itself included. They take their values at
    /// each call, in the order they are declared, so the read saw 0.
    ReadBeforeInitialized {
        /// Where the initializer names it.
        read: CallSite<'db>,
        /// The variable the initializer gives a value.
        var: crate::hir_def::pous::variable::VariableDecl<'db>,
        /// The variable read.
        source: crate::hir_def::pous::variable::VariableDecl<'db>,
        /// The reference it was read through, `p^` with `p := REF(source)`.
        through: Option<crate::hir_def::pous::variable::VariableDecl<'db>>,
    },
    /// A STRUCT or ARRAY initializer on a FUNCTION's or METHOD's input. The
    /// caller passes a default for an omitted argument, and passes constants
    /// only, so this one could never apply: the call either passed the
    /// argument or was refused (E0802). It was accepted and ignored.
    AggregateInputDefault {
        var: crate::hir_def::pous::variable::VariableDecl<'db>,
        init: InitExpr<'db>,
    },
}

/// What keeps a `REF()` from being one address everywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum RefOrigin<'db> {
    /// It names a variable that is not a global, or a subscript does.
    Variable(crate::hir_def::pous::variable::VariableDecl<'db>),
    /// A subscript calls something.
    Call,
    /// It dereferences a reference, known only when the program runs.
    Deref,
    /// A name that did not resolve, reported where it stands.
    Unknown,
}

/// Where a constant was handed out to be changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum ConstantRoute {
    InOut,
    Reference,
}

/// A member an instance's initializer names but cannot set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update)]
pub enum UninitializableMember {
    /// Each call binds it to its argument. The value was written into the
    /// pointer the binding lives in, and `__init` failed to validate.
    InOut,
    /// Each call makes it afresh. The value was silently dropped.
    Temp,
    /// It names a VAR_GLOBAL. The value was silently dropped.
    External,
    /// Its reads fold to its declared value, while `__init` wrote this one.
    Constant,
}

impl<'db> ErrorCode for InitError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::InitNotConstant { .. } | Self::ReferenceNotConstant { .. } => "E0401",
            Self::FunctionCallInInitExpression(_) => "E0402",
            Self::NoFieldOnElementaryType { .. } => "E0403",
            Self::AssignToConstant { .. } => "E0404",
            Self::ConstantHandedOut { .. } | Self::ConstantInstance { .. } => "E0404",
            Self::UninitializableMember { .. } => "E0405",
            Self::ReadBeforeInitialized { .. } => "E0406",
            Self::AggregateInputDefault { .. } => "E0407",
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for InitError<'db> {
    fn to_diagnostic(
        &self,
        db: &'db dyn WorkspaceDataBase,
        file: auto_lsp::default::db::file::File,
    ) -> IdeDiagnostic {
        match self {
            Self::InitNotConstant {
                value,
                input_default,
            } => {
                let mut d = diag()
                    .message(if *input_default {
                        "this initial value must be a constant: the caller passes it".to_string()
                    } else {
                        "this initial value must be a constant: it is fixed before the program runs"
                            .to_string()
                    })
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &value.get_span(db)).unwrap_or_default())
                    .call();
                // Say WHY, naming the part that is not constant (`a` in
                // `a * 2`), especially when it IS a constant, just not one
                // this scope can fold.
                let part = const_eval::non_constant_part(db, *value).unwrap_or(*value);
                if let ExprKind::PrimaryExpr(PrimaryExpr::FuncCall(_)) = part.expr(db) {
                    d.with_note("a call is not a constant".to_string());
                }
                if let ExprKind::PrimaryExpr(PrimaryExpr::VariableAccess(va)) = part.expr(db) {
                    use crate::Qualifier;
                    match const_eval::spec_name_binding(db, *va) {
                        // The callee's own variable: nothing the caller can
                        // read, before the call binds or creates it.
                        Some(decl)
                            if *input_default
                                && decl.get_scope_id(db) == value.get_scope_id(db)
                                && decl.storage_class(db)
                                    != crate::hir_def::pous::variable::StorageClass::Global
                                && !decl.qualifier(db).contains(Qualifier::CONSTANT) =>
                        {
                            let name = decl.name_with_case(db);
                            d.with_note(if decl.is_input(db) {
                                format!(
                                    "'{}' is another input of this call: it has no value before the call binds it",
                                    name.text(db)
                                )
                            } else {
                                format!(
                                    "'{}' belongs to the call, not yet started when the caller passes the default",
                                    name.text(db)
                                )
                            });
                        }
                        Some(decl) if decl.qualifier(db).contains(Qualifier::CONSTANT) => {
                            // Three reasons a CONSTANT still refuses, told apart
                            // so the advice is not a catch-all.
                            if decl.init(db).is_none() {
                                d.with_note(format!(
                                    "'{}' is CONSTANT but declares no initial value, \
                                     so there is nothing to fold",
                                    decl.name_with_case(db).text(db)
                                ));
                            } else {
                                d.with_note(format!(
                                    "'{}' is CONSTANT, but its own value does not fold \
                                     (a reference cycle, or a non-constant initializer)",
                                    decl.name_with_case(db).text(db)
                                ));
                            }
                        }
                        Some(decl) => {
                            d.with_note(format!(
                                "'{}' is not CONSTANT",
                                decl.name_with_case(db).text(db)
                            ));
                            d.with_help(
                                "declare it CONSTANT if its value never changes".to_string(),
                            );
                        }
                        None => {
                            if let Some(ident) = const_eval::bare_access_name(db, *va)
                                && let Some(global) =
                                    crate::hir_ty::index_graphs::external_var_lookup(db, ident)
                            {
                                if !global.qualifier(db).contains(Qualifier::CONSTANT) {
                                    d.with_note(format!(
                                        "'{}' is not CONSTANT",
                                        global.name_with_case(db).text(db)
                                    ));
                                    d.with_help(
                                        "declare it CONSTANT if its value never changes"
                                            .to_string(),
                                    );
                                    return d;
                                }
                                let in_type = matches!(
                                    get_scope(db, value.get_scope_id(db)).kind,
                                    ScopeKind::Pou(Pou::DataType(_))
                                );
                                if in_type {
                                    d.with_note(format!(
                                        "'{}' IS a CONSTANT, but a TYPE declaration cannot \
                                         see it: a TYPE default folds only literals, \
                                         arithmetic, and constants in its own scope",
                                        global.name_with_case(db).text(db)
                                    ));
                                } else {
                                    d.with_note(format!(
                                        "'{}' IS a CONSTANT: declare it in this POU as \
                                         `VAR_EXTERNAL CONSTANT` and the reference folds",
                                        global.name_with_case(db).text(db)
                                    ));
                                }
                            }
                        }
                    }
                }
                d
            }
            Self::ReferenceNotConstant {
                value,
                origin,
                input_default,
            } => {
                let mut d = diag()
                    .message(if *input_default {
                        "this initial value must be a constant: the caller passes it".to_string()
                    } else {
                        "a CONSTANT is one value for every instance and every call: \
                         a REF() in it names a global"
                            .to_string()
                    })
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &value.get_span(db)).unwrap_or_default())
                    .call();
                let note = match origin {
                    RefOrigin::Variable(var) => {
                        use crate::hir_def::pous::variable::StorageClass;
                        let name = var.name_with_case(db).text(db).to_string();
                        match var.storage_class(db) {
                            StorageClass::Global => format!("'{name}' is not CONSTANT"),
                            StorageClass::InstanceMember => {
                                format!("'{name}' is a member: each instance has its own")
                            }
                            StorageClass::Local if *input_default && var.is_input(db) => format!(
                                "'{name}' is another input of this call: it has no value before the call binds it"
                            ),
                            StorageClass::Local if *input_default => format!(
                                "'{name}' belongs to the call, not yet started when the caller passes the default"
                            ),
                            StorageClass::Local => {
                                format!("'{name}' belongs to the call: each call has its own")
                            }
                        }
                    }
                    RefOrigin::Call => "a call is not a constant".to_string(),
                    RefOrigin::Deref => {
                        "a dereference reads what a reference holds when the program runs"
                            .to_string()
                    }
                    RefOrigin::Unknown => {
                        "a REF() in it names a global, through constant subscripts".to_string()
                    }
                };
                d.with_note(note);
                d
            }
            Self::FunctionCallInInitExpression(span) => diag()
                .message("the repeat count is a call, not a constant".into())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, span).unwrap_or_default())
                .call(),
            Self::NoFieldOnElementaryType { expr, ty } => ide_diagnostic::diag()
                .message(format!(
                    "'{}' is an elementary type and takes no '()' initializer",
                    ty.type_name(db)
                ))
                .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                .call(),
            Self::AssignToConstant { access, constant } => {
                let mut d = diag()
                    .message(format!(
                        "cannot write to {}",
                        constant_place(db, *access, *constant)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &access.get_span(db)).unwrap_or_default())
                    .call();
                declared_constant(db, *constant, &mut d);
                d.with_note("a CONSTANT keeps the value it is declared with".to_string());
                d.with_help("copy it into a variable to change the copy".to_string());
                d
            }
            Self::ConstantHandedOut {
                access,
                route,
                constant,
            } => {
                let place = constant_place(db, *access, *constant);
                let (message, note, help) = match route {
                    ConstantRoute::InOut => (
                        format!("cannot pass {place} to a VAR_IN_OUT"),
                        "a VAR_IN_OUT could change it",
                        "pass it to a VAR_INPUT, or copy it into a variable and pass that",
                    ),
                    ConstantRoute::Reference => (
                        format!("cannot take a reference to {place}"),
                        "a reference could change it",
                        "copy it into a variable and take the reference of that",
                    ),
                };
                let mut d = diag()
                    .message(message)
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &access.get_span(db)).unwrap_or_default())
                    .call();
                declared_constant(db, *constant, &mut d);
                d.with_note(note.to_string());
                d.with_help(help.to_string());
                d
            }
            Self::ConstantInstance { var, block, many } => {
                use crate::HasName;
                let name = var.get_name_with_case(db).text(db);
                let message = if *many {
                    format!("array '{name}' of '{block}' instances cannot be CONSTANT")
                } else {
                    format!("instance '{name}' of '{block}' cannot be CONSTANT")
                };
                let mut d = diag()
                    .message(message)
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &var.get_name_span(db)).unwrap_or_default())
                    .call();
                d.with_note("an instance changes when it runs".to_string());
                d.with_help("declare it in a VAR section without CONSTANT".to_string());
                d
            }
            Self::UninitializableMember { expr, var, kind } => {
                use crate::HasName;
                let name = var.get_name_with_case(db).text(db);
                let (what, note, help) = match kind {
                    UninitializableMember::InOut => (
                        "a VAR_IN_OUT",
                        "a VAR_IN_OUT is bound to its argument at each call",
                        format!("pass the variable in the call, as '{name} := x'"),
                    ),
                    UninitializableMember::Temp => (
                        "a VAR_TEMP",
                        "a VAR_TEMP is made afresh at each call",
                        "give the value in its declaration".to_string(),
                    ),
                    UninitializableMember::External => (
                        "a VAR_EXTERNAL",
                        "a VAR_EXTERNAL names a VAR_GLOBAL",
                        "give the value in the VAR_GLOBAL's declaration".to_string(),
                    ),
                    UninitializableMember::Constant => (
                        "a CONSTANT",
                        "a CONSTANT has the value of its declaration",
                        "declare it without CONSTANT to let each instance start at its own value"
                            .to_string(),
                    ),
                };
                let mut d = diag()
                    .message(format!("an initializer cannot set '{name}', {what}"))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();
                d.with_related(Related::new(
                    format!("'{name}' is declared here"),
                    var.get_scope_id(db).file(db),
                    var.get_name_span(db),
                ));
                d.with_note(note.to_string());
                d.with_help(help);
                d
            }
            Self::AggregateInputDefault { var, init } => {
                use crate::HasName;
                let name = var.get_name_with_case(db).text(db);
                let mut d = diag()
                    .message(format!("the default of '{name}' is an aggregate"))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &init.get_span(db)).unwrap_or_default())
                    .call();
                d.with_related(Related::new(
                    format!("'{name}' is declared here"),
                    var.get_scope_id(db).file(db),
                    var.get_name_span(db),
                ));
                d.with_note(
                    "a default is passed by the caller for an omitted argument, so it has to be a constant"
                        .to_string(),
                );
                d.with_help(format!(
                    "set '{name}' in the body, or pass it in every call"
                ));
                d
            }
            Self::ReadBeforeInitialized {
                read,
                var,
                source,
                through,
            } => {
                use crate::HasName;
                let name = var.get_name_with_case(db).text(db);
                let read_name = source.get_name_with_case(db).text(db);
                let via = through
                    .map(|reference| {
                        format!(", through '{}'", reference.get_name_with_case(db).text(db))
                    })
                    .unwrap_or_default();
                let (message, note, help) = if var == source && through.is_none() {
                    (
                        format!("the initial value of '{name}' reads '{name}' itself"),
                        "an initial value cannot read the variable it initializes",
                        None,
                    )
                } else {
                    (
                        format!(
                            "the initial value of '{name}' reads '{read_name}', declared after it{via}"
                        ),
                        "variables get their initial values in the order they are declared",
                        Some(format!("declare '{read_name}' before '{name}'")),
                    )
                };
                let mut d = diag()
                    .message(message)
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &read.get_span(db)).unwrap_or_default())
                    .call();
                if var != source {
                    d.with_related(Related::new(
                        format!("'{read_name}' is declared here"),
                        source.get_scope_id(db).file(db),
                        source.get_name_span(db),
                    ));
                }
                d.with_advice(Some(note), help);
                d
            }
        }
    }
}

/// The place a write would change: `constant 'k'`, or `'p.x' in constant 'p'`
/// when it is a field or an element of one.
fn constant_place<'db>(
    db: &'db dyn WorkspaceDataBase,
    access: CallSite<'db>,
    constant: Option<crate::hir_def::pous::variable::VariableDecl<'db>>,
) -> String {
    let text = access.to_string(db);
    match constant {
        Some(var) if !text.eq_ignore_ascii_case(var.name_with_case(db).text(db)) => {
            format!("'{text}' in constant '{}'", var.name_with_case(db).text(db))
        }
        _ => format!("constant '{text}'"),
    }
}

/// Where the constant is declared CONSTANT, when it is a variable.
fn declared_constant<'db>(
    db: &'db dyn WorkspaceDataBase,
    constant: Option<crate::hir_def::pous::variable::VariableDecl<'db>>,
    diag: &mut IdeDiagnostic,
) {
    use crate::HasName;
    if let Some(var) = constant {
        diag.with_related(Related::new(
            format!(
                "'{}' is declared CONSTANT here",
                var.name_with_case(db).text(db)
            ),
            var.get_scope_id(db).file(db),
            var.get_name_span(db),
        ));
    }
}
