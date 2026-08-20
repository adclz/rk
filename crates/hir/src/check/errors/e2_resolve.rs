use auto_lsp::{
    default::db::file::File,
    lsp_types::{DiagnosticSeverity, DiagnosticTag},
    tree_sitter,
};
use db::{WorkspaceDataBase, workspace::Workspace};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

use crate::{
    CallSite, HasName, HirNodeInfo,
    check::errors::ToIdeDiagnostic,
    hir_def::{
        config::ConfigDecl,
        expressions::{
            expression::{Expr, FuncCall, InitExpr, PathExpr},
            spec::{Spec, SpecKind},
        },
        interned::{
            identifier::{Ident, SpanIdent},
            namespace::{NamespacePath, SpanNamespaceAccess},
        },
        pous::{function::Function, pou::Pou, variable::VariableDecl},
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{
        index_graphs::namespace_index,
        ty::{CallableType, Type},
    },
    query_string::{
        fields::{fuzzy_type_fields, suggest_similar_note},
        method::fuzzy_callable_type_parameters,
        query::Query,
        scope::{SearchResult, SymbolSearch},
    },
};

/// Why a TASK cannot be scheduled. Only cyclic tasks with a literal, non-zero
/// INTERVAL are; each other shape gets its own message rather than a shared
/// "unsupported", because the fix differs in every case.
/// Which parsed-but-inert CONFIGURATION construct was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum UnsupportedConfigKind {
    /// `PROGRAM P WITH T : Type (in := src)` — the connection list is read and
    /// then discarded: no copy is emitted around the scan, and the names are
    /// never resolved, so a typo passes silently.
    ProgramConnection,
    /// `PROGRAM P WITH T : Type (fb WITH other_task)` — associating a nested
    /// FB with its own task.
    FbTaskAssociation,
    /// `VAR_CONFIG PA.x : INT := 42;` — resolved and type-checked against the
    /// instance's field, then thrown away, so the field keeps its declared
    /// value. The validation makes this one especially misleading.
    InstanceInit,
}

impl UnsupportedConfigKind {
    fn message(self) -> &'static str {
        match self {
            Self::ProgramConnection => {
                "program connection lists are parsed but not wired up yet, so this has no effect"
            }
            Self::FbTaskAssociation => {
                "associating a function block with its own task is not supported yet"
            }
            Self::InstanceInit => {
                "VAR_CONFIG is checked but not applied yet, so this value never reaches the instance"
            }
        }
    }

    fn note(self) -> &'static str {
        match self {
            Self::ProgramConnection => "assign it in the program body instead",
            Self::FbTaskAssociation => "run the function block from its enclosing program's task",
            Self::InstanceInit => "set the value in the program's own VAR declaration instead",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum UnschedulableReason {
    /// `SINGLE := <event>` — event-driven tasks are not implemented.
    EventDriven,
    /// Neither SINGLE nor INTERVAL. Nothing triggers the task, so a program
    /// bound to it never runs.
    NoTrigger,
    /// INTERVAL names something whose value is not fixed at compile time — a
    /// non-CONSTANT global, a directly represented variable, an unknown name.
    /// A CONSTANT global IS accepted; the period just has to be knowable.
    NonLiteralInterval,
    /// INTERVAL is zero, which describes no cadence at all.
    ZeroInterval,
}

impl UnschedulableReason {
    fn message(self) -> &'static str {
        match self {
            Self::EventDriven => {
                "event-driven tasks (SINGLE) are not supported yet; only cyclic tasks run"
            }
            Self::NoTrigger => "a TASK needs an INTERVAL to run its programs",
            Self::NonLiteralInterval => {
                "INTERVAL must be a TIME literal or a CONSTANT global holding one"
            }
            Self::ZeroInterval => "INTERVAL must be greater than zero",
        }
    }

    /// The follow-up a user needs to actually fix it. The message says what is
    /// wrong; this says what to write instead.
    fn note(self) -> Option<&'static str> {
        match self {
            Self::EventDriven => Some("use a cyclic period, e.g. `INTERVAL := T#10ms`"),
            Self::NoTrigger => Some("add `INTERVAL := T#10ms`"),
            // The likeliest cause is a global that is simply not marked
            // CONSTANT — the value looks fixed right there in the source, so
            // without this the rejection reads as arbitrary.
            Self::NonLiteralInterval => Some("declare the global `VAR_GLOBAL CONSTANT`"),
            Self::ZeroInterval => None,
        }
    }
}

/// Which declaration section an `{extern}` FUNCTION cannot carry, and why.
/// One code (E0243), one message shape each — the fix differs per case.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum ExternForbiddenKind {
    /// `VAR_IN_OUT` hands the host a pointer into caller storage with a
    /// mutation contract; an extern's interface is copies only.
    InOut,
    /// A struct/array/STRING `VAR_OUTPUT` has no WASM result type to ride.
    AggregateOutput,
}

impl ExternForbiddenKind {
    fn section(self) -> &'static str {
        match self {
            Self::InOut => "VAR_IN_OUT",
            Self::AggregateOutput => "VAR_OUTPUT",
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::InOut => "cannot cross a WASM import: an extern takes copies, not references",
            Self::AggregateOutput => {
                "cannot be a WASM result: only scalar outputs cross an import"
            }
        }
    }

    fn note(self) -> &'static str {
        match self {
            Self::InOut => {
                "an extern FUNCTION receives VAR_INPUT copies and returns scalar \
                 VAR_OUTPUT results (the return value last)"
            }
            Self::AggregateOutput => "return scalars, or split the aggregate into scalar outputs",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum ResolveError<'db> {
    IncorrectNumberOfParameters {
        expected: usize,
        actual: usize,
        func_call: FuncCall<'db>,
        callable: CallableType<'db>,
    },
    UnknownNonFormalParameter {
        func: CallableType<'db>,
        expr: Expr<'db>,
        param: usize,
    },
    OutputParameterUsedAsInput {
        func: CallableType<'db>,
        var: VariableDecl<'db>,
        expr: Expr<'db>,
        param: usize,
    },
    UnknownInputParameter {
        func: CallableType<'db>,
        param: SpanIdent<'db>,
    },
    UnknownOutputParameter {
        func: CallableType<'db>,
        param: SpanIdent<'db>,
    },
    NoNamespaceItemFound {
        path: SpanNamespaceAccess<'db>,
    },
    NoItemInScope {
        expr: PathExpr<'db>,
        scope: ScopeId<'db>,
    },
    NoSuchFieldInitExpr {
        expr: InitExpr<'db>,
        ident: Ident,
        ty: Type<'db>,
    },
    NoSuchFieldPathExpr {
        expr: PathExpr<'db>,
        ident: Ident,
        ty: Type<'db>,
    },
    IndexNonArrayTypeInitExpr {
        expr: InitExpr<'db>,
        ty: Type<'db>,
    },
    IndexNonArrayTypePathExpr {
        expr: PathExpr<'db>,
        ty: Type<'db>,
    },
    DerefNonRefType {
        expr: PathExpr<'db>,
        ty: Type<'db>,
    },
    NoFieldOnElementaryType {
        expr: InitExpr<'db>,
        ty: Type<'db>,
    },
    FunctionAsType {
        expr: Spec<'db>,
        ty: Type<'db>,
    },
    UsingNamespaceNotFound {
        call_site: CallSite<'db>,
        path: NamespacePath,
    },
    NoConfigFileFound {
        file: File,
    },
    /// The task name referenced in a `WITH <task>` clause does not exist in the config.
    UnknownTaskRef {
        task: SpanIdent<'db>,
    },
    /// The workspace declares more than one CONFIGURATION. One workspace
    /// builds one PLC, and a POU is a type usable in any of them, so a second
    /// configuration makes "which globals are in scope here" unanswerable.
    MultipleConfigurations {
        config: ConfigDecl<'db>,
        /// Every OTHER configuration, so they can be reached from here.
        others: Vec<ConfigDecl<'db>>,
    },
    /// A TASK's PRIORITY is not a number this compiler can represent. Held as
    /// source text until here, so an unusable value would otherwise reach the
    /// scheduler as "no priority" and quietly sort last.
    InvalidPriority {
        task: SpanIdent<'db>,
        value: Ident,
    },
    /// A PROGRAM instance carries no `WITH <task>`, so nothing would ever run it.
    ProgramWithoutTask {
        instance: SpanIdent<'db>,
    },
    /// A configuration construct that is parsed but does nothing. Reported so a
    /// user is not left believing state they wrote is being applied — the
    /// silent version is worse than a rejection, because the compiler accepts
    /// the input and then ignores it.
    UnsupportedConfigElement {
        expr: PathExpr<'db>,
        kind: UnsupportedConfigKind,
    },
    /// A TASK the scheduler cannot honour. `reason` says which rule it broke,
    /// so the four causes do not collapse into one message.
    UnschedulableTask {
        task: SpanIdent<'db>,
        reason: UnschedulableReason,
    },
    /// A VAR_EXTERNAL declaration references a name not present in any accessible VAR_GLOBAL.
    ExternalVarNotFound {
        var: VariableDecl<'db>,
    },
    /// A VAR_ACCESS declaration's type does not match the referenced variable's actual type.
    AccessDeclTypeMismatch {
        var_origin: VariableDecl<'db>,
        spec: Spec<'db>,
        expected: Type<'db>,
        actual: Type<'db>,
    },
    /// A VAR_CONFIG path's first segment doesn't match any program instance in the configuration.
    ConfigInstInitUnknownInstance {
        instance_name: SpanIdent<'db>,
    },
    /// A VAR_CONFIG path references a field that doesn't exist on the resolved type.
    ConfigInstInitFieldNotFound {
        field: SpanIdent<'db>,
        parent_type: Type<'db>,
    },
    /// Only elementary types can be variadic.
    NonVariadicTypeForVariable {
        var: VariableDecl<'db>,
        typ: Type<'db>,
    },
    /// Variadic variable declared outside of VAR_INPUT.
    VariadicNotInInput {
        var: VariableDecl<'db>,
    },
    /// More than one variadic variable declared.
    MultipleVariadicVariables {
        first: VariableDecl<'db>,
        second: VariableDecl<'db>,
    },
    /// Variadic parameter must be the only VAR_INPUT parameter.
    VariadicMixedWithOtherInputs {
        variadic_var: VariableDecl<'db>,
        other_var: VariableDecl<'db>,
    },
    /// Two or more items with the same name are available in scope.
    MultipleItemsInScope {
        name: Ident,
        span: tree_sitter::Range,
        candidates: Vec<(Pou<'db>, NamespacePath)>,
    },
    /// Multibit access offset exceeds the size of the base type.
    MultibitsOutOfRange {
        expr: PathExpr<'db>,
        var: VariableDecl<'db>,
        offset: usize,
        /// Width in bits of one slice - 1 for `%X`, 8 for `%B`, and so on.
        access_bits: usize,
        /// Largest offset the base type admits, or `None` when the base is too
        /// narrow to hold even one slice (`%D` on a `WORD`) - there is no valid
        /// offset then, so reporting a range would contradict itself.
        max_offset: Option<usize>,
        base_type: Type<'db>,
    },
    /// What an `{extern}` FUNCTION declared that a WASM import cannot carry.
    /// The import's interface is copies in (`VAR_INPUT`) and scalar results
    /// out (`VAR_OUTPUT` in declaration order, the return type last).
    ExternForbiddenSection {
        var: VariableDecl<'db>,
        kind: ExternForbiddenKind,
    },
    /// An `{extern}` FUNCTION with statements — the import IS the body.
    ExternWithBody { site: CallSite<'db> },
    /// `{extern}` on something other than a FUNCTION.
    ExternOutsideFunction {
        anchor: SpanIdent<'db>,
        pou_kind: &'static str,
    },
    /// A direct variable used anywhere: read or written in a body, or named
    /// by a declaration's `AT` clause. The address is TYPED — `X/B/W/D/L`
    /// names the width — but nothing maps it to an I/O image
    DirectVariableUnsupported {
        site: CallSite<'db>,
        /// The address AS WRITTEN
        address: compact_str::CompactString,
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
    /// A VAR_IN_OUT argument is not an l-value (a variable, field, or array
    /// element). VAR_IN_OUT binds the callee to the caller's storage by
    /// reference, so a literal, arithmetic expression, or call result has no
    /// address to bind.
    InOutParameterRequiresLValue {
        func: CallableType<'db>,
        var: VariableDecl<'db>,
        expr: Expr<'db>,
    },
    /// A RETAIN/NON_RETAIN qualifier on a variable of a stateless POU
    /// (FUNCTION or METHOD). Retentive behavior requires instance storage —
    /// only FUNCTION_BLOCK, CLASS, and PROGRAM variables (and VAR_GLOBAL)
    /// can be retentive.
    RetainInStatelessPou {
        var: VariableDecl<'db>,
        pou_kind: &'static str,
    },
    /// A VAR_IN_OUT parameter bound with output syntax (`v => x`). VAR_IN_OUT
    /// is bound by reference at call entry with `:=`; `=>` is an output
    /// copy-back binding and would leave the reference unbound.
    InOutParameterBoundWithArrow {
        func: CallableType<'db>,
        var: VariableDecl<'db>,
        param: SpanIdent<'db>,
    },
    /// Several FUNCTION overloads are equally viable for the given argument
    /// types — the compiler won't guess. The caller must disambiguate with an
    /// explicit cast. `candidates` are the conflicting overloads.
    AmbiguousOverload {
        func_call: FuncCall<'db>,
        name: Ident,
        candidates: Vec<Function<'db>>,
    },
}

impl<'db> ErrorCode for ResolveError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::NoItemInScope { .. } => "E0204",
            Self::IncorrectNumberOfParameters { .. } => "E0205",
            Self::UnknownNonFormalParameter { .. } => "E0206",
            Self::OutputParameterUsedAsInput { .. } => "E0207",
            Self::UnknownInputParameter { .. } => "E0208",
            Self::UnknownOutputParameter { .. } => "E0209",
            Self::NoNamespaceItemFound { .. } => "E0210",
            Self::NoSuchFieldPathExpr { .. } => "E0211",
            Self::NoSuchFieldInitExpr { .. } => "E0211",
            Self::DerefNonRefType { .. } => "E0212",
            Self::IndexNonArrayTypeInitExpr { .. } => "E0213",
            Self::IndexNonArrayTypePathExpr { .. } => "E0213",
            Self::NoFieldOnElementaryType { .. } => "E0214",
            Self::FunctionAsType { .. } => "E0215",
            Self::UsingNamespaceNotFound { .. } => "E0216",
            Self::NoConfigFileFound { .. } => "E0217",
            Self::UnknownTaskRef { .. } => "E0219",
            Self::InvalidPriority { .. } => "E0241",
            Self::MultipleConfigurations { .. } => "E0242",
            Self::ExternalVarNotFound { .. } => "E0220",
            Self::AccessDeclTypeMismatch { .. } => "E0221",
            Self::ConfigInstInitUnknownInstance { .. } => "E0222",
            Self::ConfigInstInitFieldNotFound { .. } => "E0223",
            Self::NonVariadicTypeForVariable { var, typ } => "E0224",
            Self::MultipleItemsInScope { .. } => "E0225",
            Self::VariadicNotInInput { .. } => "E0226",
            Self::MultipleVariadicVariables { .. } => "E0227",
            Self::VariadicMixedWithOtherInputs { .. } => "E0228",
            Self::MultibitsOutOfRange { .. } => "E0229",
            Self::ExternForbiddenSection { .. } | Self::ExternWithBody { .. } => "E0243",
            Self::ExternOutsideFunction { .. } => "E0244",
            Self::DirectVariableUnsupported { .. } => "E0245",
            Self::MissingRequiredParameter { .. } => "E0233",
            Self::InOutParameterRequiresLValue { .. } => "E0234",
            Self::RetainInStatelessPou { .. } => "E0235",
            Self::InOutParameterBoundWithArrow { .. } => "E0236",
            Self::AmbiguousOverload { .. } => "E0237",
            Self::ProgramWithoutTask { .. } => "E0238",
            Self::UnsupportedConfigElement { .. } => "E0240",
            Self::UnschedulableTask { .. } => "E0239",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::NoItemInScope { .. } => "no item found in scope",
            Self::NoNamespaceItemFound { .. } => "no namespace item found",
            Self::UsingNamespaceNotFound { .. } => "namespace not found",
            Self::IncorrectNumberOfParameters { .. }
            | Self::UnknownNonFormalParameter { .. }
            | Self::OutputParameterUsedAsInput { .. }
            | Self::UnknownInputParameter { .. }
            | Self::UnknownOutputParameter { .. } => "function call parameter mismatch",
            Self::NoSuchFieldInitExpr { .. } | Self::NoSuchFieldPathExpr { .. } => "no such field",
            Self::DerefNonRefType { .. }
            | Self::NoFieldOnElementaryType { .. }
            | Self::IndexNonArrayTypeInitExpr { .. }
            | Self::IndexNonArrayTypePathExpr { .. } => "invalid operation",
            Self::FunctionAsType { .. } | Self::NonVariadicTypeForVariable { .. } => "invalid type",
            Self::VariadicNotInInput { .. }
            | Self::MultipleVariadicVariables { .. }
            | Self::VariadicMixedWithOtherInputs { .. } => "invalid variadic declaration",
            Self::NoConfigFileFound { .. } => "configuration error",
            Self::UnknownTaskRef { .. }
            | Self::InvalidPriority { .. }
            | Self::MultipleConfigurations { .. } => "configuration error",
            Self::ExternalVarNotFound { .. } => "external variable not found",
            Self::AccessDeclTypeMismatch { .. } => "access declaration type mismatch",
            Self::ConfigInstInitUnknownInstance { .. }
            | Self::ConfigInstInitFieldNotFound { .. } => "configuration error",
            Self::MultipleItemsInScope { .. } => "multiple items in scope",
            Self::MultibitsOutOfRange { .. } => "multibit access out of range",
            Self::ExternForbiddenSection { .. } | Self::ExternWithBody { .. } => {
                "not representable on an extern FUNCTION"
            }
            Self::ExternOutsideFunction { .. } => "extern pragma outside a FUNCTION",
            Self::DirectVariableUnsupported { .. } => "direct variable access is not supported",
            Self::MissingRequiredParameter { .. } => "missing required parameter",
            Self::InOutParameterRequiresLValue { .. } => "VAR_IN_OUT argument must be a variable",
            Self::RetainInStatelessPou { .. } => "invalid retentive qualifier",
            Self::InOutParameterBoundWithArrow { .. } => {
                "VAR_IN_OUT parameter bound with output syntax"
            }
            Self::AmbiguousOverload { .. } => "ambiguous overloaded call",
            Self::ProgramWithoutTask { .. } => "program instance never runs",
            Self::UnsupportedConfigElement { .. } => "unsupported configuration element",
            Self::UnschedulableTask { .. } => "task cannot be scheduled",
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for ResolveError<'db> {
    fn to_diagnostic(
        &self,
        db: &'db dyn WorkspaceDataBase,
        file: auto_lsp::default::db::file::File,
    ) -> IdeDiagnostic {
        match self {
            Self::IncorrectNumberOfParameters {
                expected,
                actual,
                func_call,
                callable,
            } => diag()
                .message(format!(
                    "'{}' expects {} parameter{}, but got {}",
                    callable.get_name_ident(db).text(db),
                    expected,
                    match expected {
                        1 => "",
                        _ => "s",
                    },
                    actual
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(
                    crate::denormalize(db, file, &func_call.path(db).get_span(db))
                        .unwrap_or_default(),
                )
                .call(),
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
            Self::UnknownNonFormalParameter { func, expr, param } => diag()
                .message(format!("no parameter at index '{}'", param))
                .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .call(),
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
            Self::NoItemInScope { expr, scope } => {
                let mut diag = diag()
                    .message(format!(
                        "no item {:?} found in scope",
                        expr.ident(db).text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();

                let mut query = Query::new(expr.ident(db).text(db).to_string());
                query.fuzzy();
                let items = SymbolSearch::new(|pou, db| {
                    match pou {
                        Pou::Function(_) => true,
                        // enum types are allowed and all variants should be suggested
                        Pou::DataType(typ) => {
                            matches!(typ.spec(db).kind(db), SpecKind::Enum(_))
                        }
                        _ => false,
                    }
                })
                .with_scope(*scope)
                .with_query(query)
                .search(db);

                list_candidates(
                    db,
                    expr.ident(db).text(db).as_str(),
                    &mut diag,
                    &items,
                    Some(*scope),
                );
                diag
            }
            Self::NoNamespaceItemFound { path } => {
                let mut diag = diag()
                    .message(format!("no item found for path '{}'", path.to_string(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &path.get_span(db)).unwrap_or_default())
                    .call();

                let path_str = path.path.to_string(db);

                // if there's no namespace path but just a target ident,
                // we can try to find POUs with that name and suggest importing them
                match &path.path.namespace {
                    None => {
                        let mut query = Query::new(path.path.target.ident.text(db).to_string());
                        query.exact();
                        let items = SymbolSearch::new(|pou, db| !matches!(pou, Pou::Function(_)))
                            .with_scope(path.scope_id)
                            .with_query(query)
                            .only_pous()
                            .search(db);

                        list_candidates(
                            db,
                            path.path.target.ident.text(db).as_str(),
                            &mut diag,
                            &items,
                            Some(path.scope_id),
                        );

                        let ns_kw = NamespacePath::from((db, &path.path.target.ident));

                        if !namespace_index(db, ns_kw).is_empty() {
                            diag.with_note(format!(
                                r#"namespace named '{}' exists but it cannot be used as an item, you can either:
- Import the namespace via an USING directive: 'USING {}'
- Import an item from this namespace: '{}.<POU>'"#,
                                path_str,
                                path_str,
                                path_str,
                            ));
                        }
                    }
                    Some(namespace) => {
                        // Build the full path (namespace prefix + target) to check
                        // if it's a known namespace (e.g. ["Std"] + "Counters" = ["Std", "Counters"])
                        let mut full_fragments = namespace.fragments(db).to_vec();
                        full_fragments.push(path.path.target.ident);
                        let full_path = NamespacePath::new(db, full_fragments);

                        if !namespace_index(db, full_path).is_empty() {
                            diag.with_note(format!(
                                r#"namespace named '{}' exists but it cannot be used as an item, you can either:
- Import the namespace via an USING directive: 'USING {}'
- Import an item from this namespace: '{}.<POU>'"#,
                                path_str,
                                path_str,
                                path_str,
                            ));
                        } else {
                            let mut ns_query = Query::new(path.to_string(db));
                            ns_query.prefix();
                            let results = SymbolSearch::new(|_, _| true)
                                .with_query(ns_query)
                                .only_namespaces()
                                .search(db);

                            list_candidates(db, &path.to_string(db), &mut diag, &results, None);
                        }
                    }
                }

                diag
            }
            Self::UsingNamespaceNotFound { call_site, path } => {
                let mut diag = diag()
                    .message(format!("namespace '{}' not found", path.to_string(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &call_site.get_span(db)).unwrap_or_default(),
                    )
                    .call();

                let mut ns_query = Query::new(path.to_string(db));
                ns_query.prefix();
                let results = SymbolSearch::new(|_, _| true)
                    .with_query(ns_query)
                    .only_namespaces()
                    .search(db);

                list_candidates(db, path.to_string(db).as_str(), &mut diag, &results, None);
                diag
            }
            Self::NoSuchFieldPathExpr { expr, ident, ty } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' has no field named '{}'",
                        ty.type_name(db),
                        ident.text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();

                ty.with_location(db, &mut diag);
                fuzzy_type_fields(db, *ty, &mut diag, ident.text(db).as_str());
                diag
            }
            Self::NoSuchFieldInitExpr { expr, ident, ty } => {
                let mut diag = ide_diagnostic::diag()
                    .message(format!(
                        "'{}' has no field named '{}'",
                        ty.type_name(db),
                        ident.text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();

                fuzzy_type_fields(db, *ty, &mut diag, ident.text(db).as_str());
                diag
            }
            Self::DerefNonRefType { expr, ty } => diag()
                .message(format!(
                    "cannot dereference non-reference type '{}'",
                    ty.type_name(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                .call(),
            Self::IndexNonArrayTypeInitExpr { expr, ty } => ide_diagnostic::diag()
                .message(format!("cannot index into type '{}'", ty.type_name(db)))
                .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                .call(),
            Self::IndexNonArrayTypePathExpr { expr, ty } => ide_diagnostic::diag()
                .message(format!("cannot index into type '{}'", ty.type_name(db)))
                .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                .call(),
            Self::NoFieldOnElementaryType { expr, ty } => ide_diagnostic::diag()
                .message(format!(
                    "type '{}' is an elementary type and cannot be initiliazed with '()'",
                    ty.type_name(db)
                ))
                .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                .call(),
            Self::FunctionAsType { expr, ty } => diag()
                .message(format!(
                    "'{}' is a function and cannot be used as a variable or data type",
                    ty.type_name(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                .call(),
            Self::NoConfigFileFound { file } => {
                let mut diag = diag()
                    .message("no configuration file found".to_string())
                    .severity(DiagnosticSeverity::HINT)
                    .tags(vec![DiagnosticTag::UNNECESSARY])
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &file.document(db).tree.root_node().range())
                            .unwrap_or_default(),
                    )
                    .call();

                diag.with_note(format!(
                    "a configuration file is required at the root of your workspace (inside '{}')",
                    Workspace::get(db)
                        .workspace_folder(db)
                        .map(|w| w.to_string_lossy())
                        .unwrap_or_default()
                ));

                diag
            }
            Self::UnknownTaskRef { task } => diag()
                .message(format!(
                    "task '{}' not found in this configuration",
                    task.ident.text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &task.get_span(db)).unwrap_or_default())
                .call(),
            Self::MultipleConfigurations { config, others } => {
                let mut diag = diag()
                    .message(format!(
                        "a workspace can only have one CONFIGURATION; this one declares {}",
                        others.len() + 1
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &config.get_name_span(db)).unwrap_or_default(),
                    )
                    .call();

                for other in others {
                    diag.with_related(Related::new(
                        format!("'{}' is declared here", other.name(db).text(db)),
                        other.get_scope_id(db).file(db),
                        other.get_name_span(db),
                    ));
                }
                diag
            }
            Self::InvalidPriority { task, value } => {
                let mut diag = diag()
                    .message(format!(
                        "task '{}' has an unusable PRIORITY '{}'",
                        task.ident.text(db),
                        value.text(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &task.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note("PRIORITY must fit in a 32-bit unsigned integer; 0 is the most urgent".into());
                diag
            }
            Self::ProgramWithoutTask { instance } => diag()
                .message(format!(
                    "program instance '{}' has no WITH <task>, so it will never run",
                    instance.ident.text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &instance.get_span(db)).unwrap_or_default())
                .call(),
            Self::UnsupportedConfigElement { expr, kind } => {
                let mut diag = diag()
                    .message(kind.message().to_string())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(kind.note().to_string());
                diag
            }
            Self::UnschedulableTask { task, reason } => {
                let mut diag = diag()
                    .message(format!(
                        "task '{}' cannot be scheduled: {}",
                        task.ident.text(db),
                        reason.message()
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &task.get_span(db)).unwrap_or_default())
                    .call();
                if let Some(note) = reason.note() {
                    diag.with_note(note.to_string());
                }
                diag
            }
            Self::ExternalVarNotFound { var } => diag()
                .message(format!(
                    "external variable '{}' not found in any accessible VAR_GLOBAL",
                    var.name(db).text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &var.get_span(db)).unwrap_or_default())
                .call(),
            Self::AccessDeclTypeMismatch {
                var_origin,
                spec,
                expected,
                actual,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "access declaration expects '{}', but variable has type '{}'",
                        expected.type_name(db),
                        actual.type_name(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &spec.get_span(db)).unwrap_or_default())
                    .call();

                diag.with_related(Related::new(
                    format!(
                        "variable '{}' is declared here",
                        var_origin.name(db).text(db),
                    ),
                    var_origin.get_scope_id(db).file(db),
                    var_origin.get_name_span(db),
                ));

                diag
            }
            Self::ConfigInstInitUnknownInstance { instance_name } => diag()
                .message(format!(
                    "no program instance '{}' found in this configuration",
                    instance_name.text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(
                    crate::denormalize(db, file, &instance_name.get_span(db)).unwrap_or_default(),
                )
                .call(),
            Self::ConfigInstInitFieldNotFound { field, parent_type } => diag()
                .message(format!(
                    "'{}' has no field named '{}'",
                    parent_type.type_name(db),
                    field.text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &field.get_span(db)).unwrap_or_default())
                .call(),
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
            Self::VariadicNotInInput { var } => {
                let mut diag = diag()
                    .message(format!(
                        "variadic variable '{}' must be declared in VAR_INPUT",
                        var.name(db).text(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &var.get_span(db)).unwrap_or_default())
                    .call();

                diag.with_note("variadic parameters are only allowed in VAR_INPUT sections".into());
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

                diag.with_related(Related::new(
                    format!("'{}' is declared here", var.name(db).text(db)),
                    var.scope_id(db).file(db),
                    var.get_span(db),
                ));

                diag
            }
            Self::ExternForbiddenSection { var, kind } => {
                let mut diag = diag()
                    .message(format!(
                        "{} '{}' {}",
                        kind.section(),
                        var.name(db).text(db),
                        kind.message(),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &var.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(kind.note().to_string());
                diag
            }
            Self::ExternWithBody { site } => {
                let mut diag = diag()
                    .message("an extern FUNCTION has no statements".to_string())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &site.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(
                    "FUNCTIONs marked with {extern} act as external calls, they can not have a body"
                        .to_string(),
                );
                diag
            }
            Self::DirectVariableUnsupported { site, address } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' cannot be read or written: there is no I/O mapping",
                        address
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &site.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(
                    "the address is understood and X/B/W/D/L names the width, but nothing \
                     connects it to a process image yet"
                        .to_string(),
                );
                diag
            }
            Self::ExternOutsideFunction { anchor, pou_kind } => {
                let mut diag = diag()
                    .message(format!("an {{extern}} pragma cannot be placed on a {pou_kind}"))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &anchor.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(
                    "{extern} pragmas can ony be used with FUNCTION"
                        .to_string(),
                );
                diag
            }
            Self::MultipleItemsInScope {
                name,
                span,
                candidates,
            } => {
                let name = name.text(db);

                // Count how many times each namespace appears
                let mut counts = rustc_hash::FxHashMap::default();
                for (_, ns) in candidates {
                    *counts.entry(*ns).or_insert(0usize) += 1;
                }

                let duplicated: Vec<_> = counts
                    .iter()
                    .filter(|(_, count)| **count > 1)
                    .map(|(ns, _)| ns)
                    .collect();
                let distinct: Vec<_> = counts
                    .iter()
                    .filter(|(_, count)| **count == 1)
                    .map(|(ns, _)| ns.to_string(db))
                    .collect();

                let mut diag = diag()
                    .message(format!(
                        "multiple items named '{}' available in scope",
                        name
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, span).unwrap_or_default())
                    .call();

                for ns in &duplicated {
                    diag.with_note(format!(
                        "'{}' is declared multiple times in namespace '{}'",
                        name,
                        ns.to_string(db),
                    ));

                    for (pou, pou_ns) in candidates {
                        if pou_ns == *ns {
                            diag.with_related(Related::new(
                                format!("'{}' declared here", name),
                                pou.get_scope_id(db).file(db),
                                pou.get_span(db),
                            ));
                        }
                    }
                }

                if distinct.len() > 1 {
                    let qualified: Vec<_> = distinct
                        .iter()
                        .map(|ns| format!("{}.{}", ns, name))
                        .collect();
                    diag.with_note(format!(
                        "qualify the name to resolve the ambiguity: {}",
                        qualified.join(" or "),
                    ));
                }

                diag
            }
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
            Self::RetainInStatelessPou { var, pou_kind } => {
                let qualifier = if var.qualifier(db).contains(crate::Qualifier::RETAIN) {
                    "RETAIN"
                } else {
                    "NON_RETAIN"
                };
                let mut diag = diag()
                    .message(format!(
                        "'{}' cannot be {}: a {} is stateless",
                        var.name(db).text(db),
                        qualifier,
                        pou_kind,
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &var.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(
                    "retentive behavior requires instance storage; only FUNCTION_BLOCK, CLASS, and PROGRAM variables (and VAR_GLOBAL) can be RETAIN/NON_RETAIN"
                        .to_string(),
                );

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
        }
    }
}

fn list_candidates<'db>(
    db: &'db dyn WorkspaceDataBase,
    name: &str,
    diag: &mut IdeDiagnostic,
    results: &SearchResult<'db>,
    scope: Option<ScopeId<'db>>,
) {
    // Suggest variables with similar names (scoped to their POU)
    let var_names: Vec<_> = results
        .variables()
        .map(|v| v.name(db).text(db).to_string())
        .collect();
    if !var_names.is_empty() {
        let owner = scope
            .map(|s| get_scope(db, s).kind)
            .and_then(|k| match k {
                ScopeKind::Pou(pou) => Some(pou.get_name_ident(db).text(db).to_string()),
                _ => None,
            })
            .unwrap_or_else(|| name.to_string());
        suggest_similar_note(&owner, "item", diag, var_names.iter().map(|n| n.as_str()));
    }

    // Suggest THIS.variable for variables in the parent FB/class scope (method context only)
    let this_names: Vec<_> = results
        .this_variables()
        .map(|v| v.name(db).text(db).to_string())
        .collect();
    if !this_names.is_empty() {
        let count = this_names.len().min(5);
        let mut note = format!(
            "{} available via THIS:\n",
            if count > 1 { "items" } else { "an item" }
        );
        for (i, name) in this_names.iter().take(count).enumerate() {
            if i > 0 {
                note.push('\n');
            }
            note.push_str(&format!("- THIS.{}", name));
        }
        if this_names.len() > 5 {
            note.push_str("\n  ...");
        }
        diag.with_note(note);
    }

    // Suggest local POUs with similar names
    let local_names: Vec<_> = results
        .local_pous()
        .map(|p| p.get_name_ident(db).text(db).to_string())
        .collect();
    if !local_names.is_empty() {
        let count = local_names.len().min(5);
        let mut note = format!(
            "{} with similar name available in scope:\n",
            if count > 1 { "items" } else { "an item" }
        );
        for (i, name) in local_names.iter().take(count).enumerate() {
            if i > 0 {
                note.push('\n');
            }
            note.push_str(&format!("- {}", name));
        }
        if local_names.len() > 5 {
            note.push_str("\n  ...");
        }
        diag.with_note(note);
    }

    // Suggest imported POUs that need a USING directive
    // Deduplicate by (namespace, pou name) to avoid repeating the same suggestion
    let mut seen = rustc_hash::FxHashSet::default();
    let imported: Vec<_> = results
        .imported_pous()
        .filter(|(ns, pou)| {
            seen.insert((
                ns.to_string(db),
                pou.get_name_ident(db).text(db).to_string(),
            ))
        })
        .take(6)
        .collect();
    if !imported.is_empty() {
        let count = imported.len();
        let display_count = count.min(5);

        let mut note = match count {
            1 => {
                "an item with a similar name is available, but needs to be imported:\n".to_string()
            }
            _ => "items with similar names are available, but need to be imported:\n".to_string(),
        };

        for (i, (namespace, pou)) in imported.iter().take(display_count).enumerate() {
            if i > 0 {
                note.push('\n');
            }
            note.push_str(&format!(
                "- '{}' via USING {}",
                pou.get_name_ident(db).text(db),
                namespace.to_string(db)
            ));
        }

        if count > 5 {
            note.push_str("\n  ...");
        }

        diag.with_note(note);
    }

    // Suggest namespaces with similar paths
    let ns_candidates: Vec<_> = results.namespaces().take(6).collect();
    if !ns_candidates.is_empty() {
        let count = ns_candidates.len();
        let display_count = count.min(5);
        let mut note = format!(
            "no namespace named '{}' found, but the following namespace{} {} similar path:\n",
            name,
            if count > 1 { "s" } else { "" },
            if count > 1 { "have" } else { "has" },
        );

        for (i, candidate) in ns_candidates.iter().take(display_count).enumerate() {
            if i > 0 {
                note.push('\n');
            }
            note.push_str(&format!("- {}", candidate.path(db).to_string(db)));
        }

        if count > 5 {
            note.push_str("\n  ...");
        }

        diag.with_note(note);
    }
}
