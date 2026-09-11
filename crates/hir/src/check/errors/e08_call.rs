use crate::CallSite;
use crate::HasName;
use crate::HirNodeInfo;
use crate::check::errors::ToIdeDiagnostic;
use crate::hir_def::expressions::expression::Expr;
use crate::hir_def::expressions::expression::FuncCall;
use crate::hir_def::expressions::expression::PathExpr;
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
    /// Multibit access offset exceeds the size of the base type.
    MultibitsOutOfRange {
        expr: PathExpr<'db>,
        /// The declaration to point at, when the base IS one. A slice of a
        /// struct field or an array element has no declaration of its own.
        var: Option<VariableDecl<'db>>,
        offset: usize,
        /// Width in bits of one slice - 1 for `%X`, 8 for `%B`, and so on.
        access_bits: usize,
        /// Largest offset the base type admits, or `None` when the base is too
        /// narrow to hold even one slice (`%D` on a `WORD`) - there is no valid
        /// offset then, so reporting a range would contradict itself.
        max_offset: Option<usize>,
        base_type: Type<'db>,
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
        name: Ident,
        candidates: Vec<Function<'db>>,
    },
    /// No overload of the set accepts the call's argument types. The first
    /// overload used to stand in and report ITS parameter mismatch, so the
    /// message named a type nobody wrote: "expected 'CHAR', got 'DATE'" for
    /// a date assertion.
    NoMatchingOverload {
        func_call: FuncCall<'db>,
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
    NonVariadicFoldParameter {
        var: VariableDecl<'db>,
        call_site: CallSite<'db>,
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
            Self::InOutParameterBoundWithArrow { .. } => "E0807",
            Self::MultibitsOutOfRange { .. } => "E0808",
            Self::CallNonCallableType { .. } => "E0808",
            Self::AmbiguousOverload { .. } => "E0809",
            Self::NoMatchingOverload { .. } => "E0810",
            Self::NonVariadicTypeForVariable { .. } => "E0811",
            Self::MultipleVariadicVariables { .. } => "E0812",
            Self::EmptyVariadicCall { .. } => "E0813",
            Self::NonVariadicFoldParameter { .. } => "E0814",
            Self::VariadicMixedWithOtherInputs { .. } => "E0815",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::IncorrectNumberOfParameters { .. } => "function call parameter mismatch",
            Self::MissingRequiredParameter { .. } => "missing required parameter",
            Self::UnknownInputParameter { .. } => "function call parameter mismatch",
            Self::UnknownOutputParameter { .. } => "function call parameter mismatch",
            Self::OutputParameterUsedAsInput { .. } => "function call parameter mismatch",
            Self::InOutParameterRequiresLValue { .. } => "VAR_IN_OUT argument must be a variable",
            Self::InOutParameterBoundWithArrow { .. } => {
                "VAR_IN_OUT parameter bound with output syntax"
            }
            Self::MultibitsOutOfRange { .. } => "multibit access out of range",
            Self::CallNonCallableType { .. } => "semantic violation",
            Self::AmbiguousOverload { .. } => "ambiguous overloaded call",
            Self::NoMatchingOverload { .. } => "no matching overload",
            Self::NonVariadicTypeForVariable { .. } => "invalid type",
            Self::MultipleVariadicVariables { .. } => "invalid variadic declaration",
            Self::EmptyVariadicCall { .. } => "variadic call without arguments",
            Self::NonVariadicFoldParameter { .. } => "type mismatch",
            Self::VariadicMixedWithOtherInputs { .. } => "invalid variadic declaration",
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
                        "variadic parameter '{}' must be the only VAR_INPUT parameter",
                        variadic_var.name(db).text(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &other_var.get_span(db)).unwrap_or_default(),
                    )
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "variadic parameter '{}' declared here",
                        variadic_var.name(db).text(db)
                    ),
                    variadic_var.scope_id(db).file(db),
                    variadic_var.get_span(db),
                ));
                diag.with_note(
                    "a variadic parameter must be the only parameter in VAR_INPUT".into(),
                );
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
                        callable.get_name_ident(db).text(db),
                        actual,
                        match actual {
                            1 => "",
                            _ => "s",
                        },
                    )
                } else {
                    format!(
                        "'{}' expects {} parameter{}, but got {}",
                        callable.get_name_ident(db).text(db),
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
                    .map(|v| format!("'{}'", v.name(db).text(db)))
                    .collect();
                let names_joined = names.join(", ");

                let mut diag = diag()
                    .message(format!(
                        "call to '{}' is missing {} required parameter{}: {}",
                        func.get_name_ident(db).text(db),
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
                        "VAR_INPUT on FUNCTION/METHOD parameters must be supplied unless the declaration provides a scalar default value"
                            .to_string(),
                    );
                }
                if any_in_out {
                    diag.with_note(
                        "VAR_IN_OUT parameters bind to caller-side l-values and must always be supplied"
                            .to_string(),
                    );
                }

                for var in vars {
                    diag.with_related(Related::new(
                        format!("parameter '{}' declared here", var.name(db).text(db)),
                        var.scope_id(db).file(db),
                        var.get_span(db),
                    ));
                }

                diag
            }
            Self::UnknownInputParameter { func, param } => {
                let mut diag = diag()
                    .message(format!("unknown input parameter '{}'", param.text(db)))
                    .range(crate::denormalize(db, file, &param.get_span(db)).unwrap_or_default())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .call();

                fuzzy_callable_type_parameters(db, *func, &mut diag, param.text(db).as_str());

                diag
            }
            Self::UnknownOutputParameter { func, param } => {
                let mut diag = diag()
                    .message(format!("unknown output parameter '{}'", param.text(db)))
                    .range(crate::denormalize(db, file, &param.get_span(db)).unwrap_or_default())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .call();

                fuzzy_callable_type_parameters(db, *func, &mut diag, param.text(db).as_str());

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
                diag.with_note(format!(
                    "use formal syntax instead: {} => <variable>",
                    var.get_name_ident(db).text(db)
                ));

                diag
            }
            Self::InOutParameterRequiresLValue { func, var, expr } => {
                let mut diag = diag()
                    .message(format!(
                        "VAR_IN_OUT parameter '{}' of '{}' requires a variable, not a value",
                        var.name(db).text(db),
                        func.get_name_ident(db).text(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(
                    "VAR_IN_OUT binds the callee to the caller's storage by reference; a literal, expression, or call result has no address to bind"
                        .to_string(),
                );
                diag.with_related(Related::new(
                    format!("parameter '{}' declared here", var.name(db).text(db)),
                    var.scope_id(db).file(db),
                    var.get_span(db),
                ));

                diag
            }
            Self::InOutParameterBoundWithArrow { func, var, param } => {
                let mut diag = diag()
                    .message(format!(
                        "VAR_IN_OUT parameter '{}' of '{}' cannot be bound with '=>'",
                        var.name(db).text(db),
                        func.get_name_ident(db).text(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &param.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(format!(
                    "VAR_IN_OUT is bound by reference at call entry: use {} := <variable>",
                    var.name(db).text(db),
                ));
                diag.with_related(Related::new(
                    format!("parameter '{}' declared here", var.name(db).text(db)),
                    var.scope_id(db).file(db),
                    var.get_span(db),
                ));

                diag
            }
            Self::MultibitsOutOfRange {
                expr,
                var,
                offset,
                access_bits,
                max_offset,
                base_type,
            } => {
                let message = match max_offset {
                    Some(max) => format!(
                        "offset {} is out of range for type '{}' (valid range: 0..{})",
                        offset,
                        base_type.type_name(db),
                        max,
                    ),
                    None => format!(
                        "a {}-bit access does not fit in type '{}'",
                        access_bits,
                        base_type.type_name(db),
                    ),
                };
                let mut diag = diag()
                    .message(message)
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();

                if let Some(var) = var {
                    diag.with_related(Related::new(
                        format!("'{}' is declared here", var.name(db).text(db)),
                        var.scope_id(db).file(db),
                        var.get_span(db),
                    ));
                }

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
                    diag.with_note(
                        "to call a FUNCTION_BLOCK, you need to instantiate it first.".into(),
                    );
                }

                diag
            }
            Self::AmbiguousOverload {
                func_call,
                name,
                candidates,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "call to '{}' is ambiguous: {} overloads accept these arguments: disambiguate with an explicit cast",
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
                        "candidate overload declared here".to_string(),
                        c.get_scope_id(db).file(db),
                        c.get_span(db),
                    ));
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
                        c.get_span(db),
                    ));
                }
                diag
            }
            Self::NonVariadicTypeForVariable { var, typ } => {
                let mut diag = diag()
                    .message(format!(
                        "variable '{}' is declared as variadic but has non-variadic type '{}'",
                        var.name(db).text(db),
                        typ.type_name(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &var.get_span(db)).unwrap_or_default())
                    .call();

                diag.with_note("only elementary types can be variadic".into());
                diag
            }
            Self::MultipleVariadicVariables { first, second } => {
                let mut diag = diag()
                    .message("only one variadic variable is allowed per POU".to_string())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &second.get_span(db)).unwrap_or_default())
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "first variadic variable '{}' declared here",
                        first.name(db).text(db)
                    ),
                    first.scope_id(db).file(db),
                    first.get_span(db),
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
                        "call to '{}' must pass at least one argument to variadic parameter '{}'",
                        func.get_name_ident(db).text(db),
                        var.name(db).text(db),
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
                        "variadic parameter '{}' declared here",
                        var.name(db).text(db)
                    ),
                    var.scope_id(db).file(db),
                    var.get_span(db),
                ));

                diag
            }
            Self::NonVariadicFoldParameter { var, call_site } => {
                let mut diag = diag()
                    .message(format!(
                        "variable '{}' is not variadic",
                        var.get_name_ident(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &call_site.get_span(db)).unwrap_or_default(),
                    )
                    .call();

                diag.with_note("... can only be used on VAR_INPUT variables that are declared variadic with the same operator (e.g: INT...)".into());
                diag
            }
        }
    }
}
