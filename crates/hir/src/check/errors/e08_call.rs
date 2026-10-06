// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

use crate::CallSite;
use crate::HasName;
use crate::HirNodeInfo;
use crate::check::errors::ToIdeDiagnostic;
use crate::hir_def::expressions::expression::Expr;
use crate::hir_def::expressions::expression::FuncCall;
use crate::hir_def::interned::identifier::Ident;
use crate::hir_def::interned::identifier::SpanIdent;
use crate::hir_def::pous::function::Function;
use crate::hir_def::pous::variable::VariableDecl;
use crate::hir_ty::ty::CallableType;
use crate::hir_ty::ty::Type;
use crate::query_string::method::fuzzy_callable_type_parameters;
use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use ide_diagnostic::ErrorCode;
use ide_diagnostic::IdeDiagnostic;
use ide_diagnostic::Related;
use ide_diagnostic::diag;

/// Why a call fits several overloads alike (E0809).
#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum Ambiguity<'db> {
    /// They take the same parameters and differ by return type, and the
    /// call's site expects none of them.
    Return,
    /// NULL, which is a reference of any type.
    Null,
    /// An instance implementing each interface they take.
    Implementer { instance: Type<'db> },
    /// An argument widens to each of them.
    Widening,
    /// No argument the call passes tells them apart: they differ in inputs
    /// it leaves to their defaults.
    Defaults,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub enum CallError<'db> {
    IncorrectNumberOfParameters {
        /// How many same-name FUNCTION overloads exist. Above one, naming a
        /// single arity misstates the situation: the true claim is that NO
        /// overload takes this count.
        overloads: usize,
        expected: usize,
        actual: usize,
        func_call: FuncCall<'db>,
        callable: CallableType<'db>,
    },
    /// One or more required call-site parameters (VAR_INPUT on FUNCTION/METHOD
    /// without a scalar default, or VAR_IN_OUT on any callable) were not
    /// supplied. All missing params for a single call site are collapsed into
    /// one diagnostic.
    MissingRequiredParameter {
        func: CallableType<'db>,
        vars: Vec<VariableDecl<'db>>,
        func_call: FuncCall<'db>,
    },
    UnknownInputParameter {
        func: CallableType<'db>,
        param: SpanIdent<'db>,
    },
    UnknownOutputParameter {
        func: CallableType<'db>,
        param: SpanIdent<'db>,
    },
    OutputParameterUsedAsInput {
        func: CallableType<'db>,
        var: VariableDecl<'db>,
        expr: Expr<'db>,
        param: usize,
    },
    /// A VAR_IN_OUT argument is not an l-value (a variable, field, or array
    /// element). VAR_IN_OUT binds the callee to the caller's storage by
    /// reference, so a literal, arithmetic expression, or call result has no
    /// address to bind.
    /// An `ARRAY[*]` VAR_INPUT bound to a value: the parameter is connected
    /// to a variable, or a row of one, whose bounds it takes. A VAR_IN_OUT
    /// one is E0806.
    ConformandRequiresVariable {
        func: CallableType<'db>,
        var: VariableDecl<'db>,
        expr: Expr<'db>,
    },
    InOutParameterRequiresLValue {
        func: CallableType<'db>,
        var: VariableDecl<'db>,
        expr: Expr<'db>,
    },
    /// A VAR_IN_OUT parameter bound with output syntax (`v => x`). VAR_IN_OUT
    /// is bound by reference at call entry with `:=`; `=>` is an output
    /// copy-back binding and would leave the reference unbound.
    InOutParameterBoundWithArrow {
        func: CallableType<'db>,
        var: VariableDecl<'db>,
        param: SpanIdent<'db>,
    },
    CallNonCallableType {
        typ: Type<'db>,
        func_call: FuncCall<'db>,
    },
    /// Several FUNCTION overloads are equally viable for the given argument
    /// types — the compiler won't guess. The caller must disambiguate with an
    /// explicit cast. `candidates` are the conflicting overloads.
    AmbiguousOverload {
        func_call: FuncCall<'db>,
        /// As the overloads write it.
        name: Ident,
        candidates: Vec<Function<'db>>,
        /// Why the call cannot tell them apart, which decides what would.
        why: Ambiguity<'db>,
        /// The names the overloads give the first positional argument that
        /// tells them apart, as written, when they differ: naming it picks one.
        by_name: Vec<Ident>,
    },
    /// No overload of the set accepts the call's argument types. The first
    /// overload used to stand in and report ITS parameter mismatch, so the
    /// message named a type nobody wrote: "expected 'CHAR', got 'DATE'" for
    /// a date assertion.
    NoMatchingOverload {
        func_call: FuncCall<'db>,
        /// As the overloads write it.
        name: Ident,
        arg_types: Vec<Type<'db>>,
        candidates: Vec<Function<'db>>,
    },
    /// Only elementary types can be variadic.
    NonVariadicTypeForVariable {
        var: VariableDecl<'db>,
        typ: Type<'db>,
    },
    /// More than one variadic variable declared.
    MultipleVariadicVariables {
        first: VariableDecl<'db>,
        second: VariableDecl<'db>,
    },
    /// A call bound nothing to a variadic parameter. A fold over an empty pack
    /// has no value, so an empty pack has no lowering — the callee is refused
    /// here rather than left to fail in MIR.
    EmptyVariadicCall {
        func: CallableType<'db>,
        var: VariableDecl<'db>,
        func_call: FuncCall<'db>,
    },
    /// A fold names something other than the POU's variadic parameter: a
    /// variable that is not variadic, a member, a name that is nothing.
    NonVariadicFoldParameter {
        call_site: CallSite<'db>,
        /// As written.
        name: Ident,
        /// The POU's variadic parameter, when it has one.
        pack: Option<VariableDecl<'db>>,
    },
    /// A variadic parameter read, written or passed as a whole: only a
    /// fold consumes it.
    VariadicOutsideFold {
        expr: crate::hir_def::expressions::expression::PathExpr<'db>,
        var: VariableDecl<'db>,
    },
    /// Variadic parameter must be the only VAR_INPUT parameter.
    VariadicMixedWithOtherInputs {
        variadic_var: VariableDecl<'db>,
        other_var: VariableDecl<'db>,
    },
}

impl<'db> ErrorCode for CallError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::IncorrectNumberOfParameters { .. } => "E0801",
            Self::MissingRequiredParameter { .. } => "E0802",
            Self::UnknownInputParameter { .. } => "E0803",
            Self::UnknownOutputParameter { .. } => "E0804",
            Self::OutputParameterUsedAsInput { .. } => "E0805",
            Self::InOutParameterRequiresLValue { .. } => "E0806",
            Self::ConformandRequiresVariable { .. } => "E0817",
            Self::InOutParameterBoundWithArrow { .. } => "E0807",
            Self::CallNonCallableType { .. } => "E0808",
            Self::AmbiguousOverload { .. } => "E0809",
            Self::NoMatchingOverload { .. } => "E0810",
            Self::NonVariadicTypeForVariable { .. } => "E0811",
            Self::MultipleVariadicVariables { .. } => "E0812",
            Self::EmptyVariadicCall { .. } => "E0813",
            Self::NonVariadicFoldParameter { .. } => "E0814",
            Self::VariadicMixedWithOtherInputs { .. } => "E0815",
            Self::VariadicOutsideFold { .. } => "E0816",
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for CallError<'db> {
    fn to_diagnostic(
        &self,
        db: &'db dyn WorkspaceDataBase,
        file: auto_lsp::default::db::file::File,
    ) -> IdeDiagnostic {
        match self {
            Self::VariadicMixedWithOtherInputs {
                variadic_var,
                other_var,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' is declared beside the variadic parameter '{}'",
                        other_var.name_with_case(db).text(db),
                        variadic_var.name_with_case(db).text(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &other_var.get_name_span(db))
                            .unwrap_or_default(),
                    )
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "variadic parameter '{}' is declared here",
                        variadic_var.name_with_case(db).text(db)
                    ),
                    variadic_var.scope_id(db).file(db),
                    variadic_var.get_name_span(db),
                ));
                diag.with_note("a variadic parameter takes every argument of the call".into());
                diag
            }
            Self::IncorrectNumberOfParameters {
                overloads,
                expected,
                actual,
                func_call,
                callable,
            } => diag()
                .message(if *overloads > 1 {
                    format!(
                        "no overload of '{}' takes {} parameter{}",
                        callable.get_name_with_case(db).text(db),
                        actual,
                        match actual {
                            1 => "",
                            _ => "s",
                        },
                    )
                } else {
                    format!(
                        "'{}' expects {} parameter{}, but got {}",
                        callable.get_name_with_case(db).text(db),
                        expected,
                        match expected {
                            1 => "",
                            _ => "s",
                        },
                        actual
                    )
                })
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(
                    crate::denormalize(db, file, &func_call.path(db).get_span(db))
                        .unwrap_or_default(),
                )
                .call(),
            Self::MissingRequiredParameter {
                func,
                vars,
                func_call,
            } => {
                let names: Vec<String> = vars
                    .iter()
                    .map(|v| format!("'{}'", v.name_with_case(db).text(db)))
                    .collect();
                let names_joined = names.join(", ");

                let mut diag = diag()
                    .message(format!(
                        "call to '{}' is missing {} required parameter{}: {}",
                        func.get_name_with_case(db).text(db),
                        vars.len(),
                        if vars.len() > 1 { "s" } else { "" },
                        names_joined,
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &func_call.path(db).get_span(db))
                            .unwrap_or_default(),
                    )
                    .call();

                let any_input = vars.iter().any(|v| v.is_input(db));
                let any_in_out = vars.iter().any(|v| v.is_in_out(db));

                if any_input {
                    diag.with_note(
                        "a FUNCTION or METHOD call supplies every VAR_INPUT without a default"
                            .to_string(),
                    );
                }
                if any_in_out {
                    diag.with_note(
                        "VAR_IN_OUT parameters bind to caller-side l-values and must always be supplied"
                            .to_string(),
                    );
                }
                if vars
                    .iter()
                    .any(|v| !v.is_in_out(db) && v.conformand(db).is_some())
                {
                    diag.with_note(
                        "an ARRAY[*] parameter has the bounds of the array the call binds to it"
                            .to_string(),
                    );
                }

                for var in vars {
                    diag.with_related(Related::new(
                        format!(
                            "parameter '{}' is declared here",
                            var.name_with_case(db).text(db)
                        ),
                        var.scope_id(db).file(db),
                        var.get_name_span(db),
                    ));
                }

                diag
            }
            Self::UnknownInputParameter { func, param } => {
                let mut diag = diag()
                    .message(format!("unknown input parameter '{}'", param.as_str(db)))
                    .range(crate::denormalize(db, file, &param.get_span(db)).unwrap_or_default())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .call();

                fuzzy_callable_type_parameters(
                    db,
                    *func,
                    &mut diag,
                    param.ident(db).text(db).as_str(),
                );

                diag
            }
            Self::UnknownOutputParameter { func, param } => {
                let mut diag = diag()
                    .message(format!("unknown output parameter '{}'", param.as_str(db)))
                    .range(crate::denormalize(db, file, &param.get_span(db)).unwrap_or_default())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .call();

                fuzzy_callable_type_parameters(
                    db,
                    *func,
                    &mut diag,
                    param.ident(db).text(db).as_str(),
                );

                diag
            }
            Self::OutputParameterUsedAsInput {
                func,
                expr,
                var,
                param,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "output parameter at index '{}' cannot be used as input",
                        param
                    ))
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .call();
                diag.with_help(format!(
                    "bind it by name: {} => <variable>",
                    var.get_name_with_case(db).text(db)
                ));

                diag
            }
            Self::ConformandRequiresVariable { func, var, expr } => {
                let mut diag = diag()
                    .message(format!(
                        "ARRAY[*] parameter '{}' of '{}' requires a variable, not a value",
                        var.name_with_case(db).text(db),
                        func.get_name_with_case(db).text(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(
                    "an ARRAY[*] is bound to a variable, or a row of one, and takes its bounds"
                        .to_string(),
                );
                diag.with_help("store the value in a variable, and pass the variable".to_string());
                diag.with_related(Related::new(
                    format!(
                        "parameter '{}' is declared here",
                        var.name_with_case(db).text(db)
                    ),
                    var.scope_id(db).file(db),
                    var.get_name_span(db),
                ));
                diag
            }
            Self::InOutParameterRequiresLValue { func, var, expr } => {
                let mut diag = diag()
                    .message(format!(
                        "VAR_IN_OUT parameter '{}' of '{}' requires a variable, not a value",
                        var.name_with_case(db).text(db),
                        func.get_name_with_case(db).text(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(
                    "a literal, an expression or a call result has no address for a VAR_IN_OUT to bind"
                        .to_string(),
                );
                diag.with_related(Related::new(
                    format!(
                        "parameter '{}' is declared here",
                        var.name_with_case(db).text(db)
                    ),
                    var.scope_id(db).file(db),
                    var.get_name_span(db),
                ));

                diag
            }
            Self::InOutParameterBoundWithArrow { func, var, param } => {
                let mut diag = diag()
                    .message(format!(
                        "VAR_IN_OUT parameter '{}' of '{}' cannot be bound with '=>'",
                        var.name_with_case(db).text(db),
                        func.get_name_with_case(db).text(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &param.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(format!(
                    "VAR_IN_OUT is bound by reference at call entry: use {} := <variable>",
                    var.name_with_case(db).text(db),
                ));
                diag.with_related(Related::new(
                    format!(
                        "parameter '{}' is declared here",
                        var.name_with_case(db).text(db)
                    ),
                    var.scope_id(db).file(db),
                    var.get_name_span(db),
                ));

                diag
            }
            Self::CallNonCallableType { typ, func_call } => {
                let mut diag = diag()
                    .message(format!("'{}' is not a callable type", typ.type_name(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &func_call.path(db).get_span(db))
                            .unwrap_or_default(),
                    )
                    .call();

                if let Type::FunctionBlock(_) = typ {
                    diag.with_help(
                        "declare an instance of the FUNCTION_BLOCK and call the instance".into(),
                    );
                }

                diag
            }
            Self::AmbiguousOverload {
                func_call,
                name,
                candidates,
                why,
                by_name,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "call to '{}' is ambiguous: {} overloads accept these arguments",
                        name.text(db),
                        candidates.len()
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &func_call.path(db).get_span(db))
                            .unwrap_or_default(),
                    )
                    .call();

                for c in candidates {
                    diag.with_related(Related::new(
                        "a candidate is declared here".to_string(),
                        c.get_scope_id(db).file(db),
                        c.get_name_span(db),
                    ));
                }
                let (note, help) = match why {
                    Ambiguity::Return => (
                        "the overloads differ only in their return type, and nothing here expects one"
                            .to_string(),
                        Some("assign the call to a variable of that type"),
                    ),
                    Ambiguity::Null => (
                        "NULL is a reference of any type".to_string(),
                        Some("pass a variable of the reference type"),
                    ),
                    Ambiguity::Implementer { instance } => (
                        format!(
                            "'{}' implements the interface each overload takes",
                            instance.type_name(db)
                        ),
                        None,
                    ),
                    Ambiguity::Widening => (
                        "an argument widens to each overload".to_string(),
                        Some("pick one with a typed literal or a conversion, such as `DINT#5` or `INT_TO_DINT(x)`"),
                    ),
                    Ambiguity::Defaults => (
                        "the overloads differ only in inputs this call leaves to their defaults"
                            .to_string(),
                        Some("pass one of those inputs to pick an overload"),
                    ),
                };
                diag.with_advice(Some(note), help);
                match by_name.as_slice() {
                    [] if matches!(why, Ambiguity::Implementer { .. }) => {
                        diag.with_note(
                            "they name that parameter alike, so no call can pick one of them \
                             with this argument"
                                .to_string(),
                        );
                    }
                    [] => {}
                    names => {
                        let named: Vec<String> = names
                            .iter()
                            .map(|n| format!("'{} := ...'", n.text(db)))
                            .collect();
                        diag.with_note(format!(
                            "they name that parameter differently: naming it picks one, as {}",
                            named.join(" or ")
                        ));
                    }
                }
                diag
            }
            Self::NoMatchingOverload {
                func_call,
                name,
                arg_types,
                candidates,
            } => {
                let names = |types: &[Type<'db>]| {
                    types
                        .iter()
                        // An untyped literal is named by the type it defaults
                        // to: a parameter list wants "(STRING)", not the hover
                        // form "({string} 'text')".
                        .map(|t| match t {
                            Type::Infer(it) => Type::Elementary(it.to_spec(db)).type_name(db),
                            _ => t.type_name(db),
                        })
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                let mut diag = diag()
                    .message(format!(
                        "no overload of '{}' accepts ({})",
                        name.text(db),
                        names(arg_types)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &func_call.path(db).get_span(db))
                            .unwrap_or_default(),
                    )
                    .call();

                for c in candidates {
                    let params = crate::hir_ty::head::signature::function_signature(db, *c).params;
                    diag.with_related(Related::new(
                        format!("overload accepting ({})", names(&params)),
                        c.get_scope_id(db).file(db),
                        c.get_name_span(db),
                    ));
                }
                diag
            }
            Self::NonVariadicTypeForVariable { var, typ } => {
                let mut diag = diag()
                    .message(format!("'{}' cannot be variadic", typ.type_name(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &var.spec(db).get_span(db))
                            .unwrap_or_default(),
                    )
                    .call();

                diag.with_note("only an elementary type can be variadic".into());
                diag
            }
            Self::MultipleVariadicVariables { first, second } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' is a second variadic parameter",
                        second.name_with_case(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &second.get_name_span(db)).unwrap_or_default(),
                    )
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "first variadic parameter '{}' is declared here",
                        first.name_with_case(db).text(db)
                    ),
                    first.scope_id(db).file(db),
                    first.get_name_span(db),
                ));
                diag
            }
            Self::EmptyVariadicCall {
                func,
                var,
                func_call,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "the call to '{}' passes no argument to the variadic parameter '{}'",
                        func.get_name_with_case(db).text(db),
                        var.name_with_case(db).text(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &func_call.path(db).get_span(db))
                            .unwrap_or_default(),
                    )
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "variadic parameter '{}' is declared here",
                        var.name_with_case(db).text(db)
                    ),
                    var.scope_id(db).file(db),
                    var.get_name_span(db),
                ));

                diag
            }
            Self::NonVariadicFoldParameter {
                call_site,
                name,
                pack,
            } => {
                let mut diag = diag()
                    .message(format!("'{}' is not a variadic parameter", name.text(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &call_site.get_span(db)).unwrap_or_default(),
                    )
                    .call();
                match pack {
                    Some(pack) => {
                        let pack_name = pack.name_with_case(db).text(db);
                        diag.with_related(Related::new(
                            format!("the variadic parameter here is '{pack_name}'"),
                            pack.scope_id(db).file(db),
                            pack.get_name_span(db),
                        ));
                        diag.with_note(format!(
                            "a fold reads it: `...{pack_name}+` adds every argument the call passed"
                        ));
                    }
                    None => {
                        diag.with_note("the POU declares no variadic parameter".to_string());
                        diag.with_help(
                            "declare one in VAR_INPUT, as `values : INT...`".to_string(),
                        );
                    }
                }
                diag
            }
            Self::VariadicOutsideFold { expr, var } => {
                let name = var.name_with_case(db).text(db);
                let mut diag = diag()
                    .message(format!(
                        "variadic parameter '{name}' is used outside a fold"
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_related(Related::new(
                    format!("'{name}' is declared variadic here"),
                    var.scope_id(db).file(db),
                    var.get_name_span(db),
                ));
                diag.with_note(
                    "a pack stands for as many parameters as the call passed".to_string(),
                );
                diag.with_help(format!(
                    "read it with a fold: `...{name}+` adds them, `...{name}=` compares them"
                ));
                diag
            }
        }
    }
}
