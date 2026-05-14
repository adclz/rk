use auto_lsp::{
    core::span::Span,
    default::db::file::File,
    lsp_types::{DiagnosticSeverity, DiagnosticTag},
};
use db::{WorkspaceDataBase, workspace::Workspace};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

use crate::{
    CallSite, HasName, HirNodeInfo,
    check::errors::ToIdeDiagnostic,
    hir_def::{
        expressions::{
            expression::{Expr, FuncCall, InitExpr, PathExpr},
            spec::{Spec, SpecKind},
        },
        interned::{
            identifier::{Ident, SpanIdent},
            namespace::{NamespacePath, SpanNamespaceAccess},
        },
        pous::{pou::Pou, variable::VariableDecl},
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
    /// The program type referenced in a PROGRAM configuration entry does not exist.
    UnknownProgType {
        prog_type: Spec<'db>,
    },
    /// The task name referenced in a `WITH <task>` clause does not exist in the config.
    UnknownTaskRef {
        task: SpanIdent<'db>,
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
        span: Span,
        candidates: Vec<(Pou<'db>, NamespacePath)>,
    },
    /// Multibit access offset exceeds the size of the base type.
    MultibitsOutOfRange {
        expr: PathExpr<'db>,
        var: VariableDecl<'db>,
        offset: usize,
        max_offset: usize,
        base_type: Type<'db>,
    },
    /// An extern pragma references a variable that does not exist in scope.
    ExternVariableNotFound {
        ident: SpanIdent<'db>,
        scope: ScopeId<'db>,
    },
    /// INTO(ref) references an identifier not found in scope.
    IntoRefNotFound {
        spec: Spec<'db>,
        ident: Ident,
    },
    /// INTO(ref) references a non-elementary type (e.g. a struct or array variable).
    IntoRefNotAny {
        spec: Spec<'db>,
        ident: Ident,
        ty: Type<'db>,
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
            Self::UnknownProgType { .. } => "E0218",
            Self::UnknownTaskRef { .. } => "E0219",
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
            Self::ExternVariableNotFound { .. } => "E0230",
            Self::IntoRefNotFound { .. } => "E0231",
            Self::IntoRefNotAny { .. } => "E0232",
            Self::MissingRequiredParameter { .. } => "E0233",
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
            Self::UnknownProgType { .. } | Self::UnknownTaskRef { .. } => "configuration error",
            Self::ExternalVarNotFound { .. } => "external variable not found",
            Self::AccessDeclTypeMismatch { .. } => "access declaration type mismatch",
            Self::ConfigInstInitUnknownInstance { .. }
            | Self::ConfigInstInitFieldNotFound { .. } => "configuration error",
            Self::MultipleItemsInScope { .. } => "multiple items in scope",
            Self::MultibitsOutOfRange { .. } => "multibit access out of range",
            Self::ExternVariableNotFound { .. } => "extern variable not found",
            Self::IntoRefNotFound { .. } => "INTO reference not found",
            Self::IntoRefNotAny { .. } => "INTO reference must be an ANY type",
            Self::MissingRequiredParameter { .. } => "missing required parameter",
        }
    }
}

impl<'db> ToIdeDiagnostic<'db> for ResolveError<'db> {
    fn to_diagnostic(&self, db: &'db dyn WorkspaceDataBase) -> IdeDiagnostic {
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
                .range(func_call.path(db).get_span(db))
                .call(),
            Self::UnknownNonFormalParameter { func, expr, param } => diag()
                .message(format!("no parameter at index '{}'", param))
                .range(expr.get_span(db))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .call(),
            Self::UnknownInputParameter { func, param } => {
                let mut diag = diag()
                    .message(format!("unknown input parameter '{}'", param.text(db)))
                    .range(param.get_span(db))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .call();

                fuzzy_callable_type_parameters(db, *func, &mut diag, param.text(db).as_str());

                diag
            }
            Self::UnknownOutputParameter { func, param } => {
                let mut diag = diag()
                    .message(format!("unknown output parameter '{}'", param.text(db)))
                    .range(param.get_span(db))
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
                    .range(expr.get_span(db))
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
                    .range(expr.get_span(db))
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
                    .range(path.get_span(db))
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
                    .range(call_site.get_span(db))
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
                    .range(expr.get_span(db))
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
                    .range(expr.get_span(db))
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
                .range(expr.get_span(db))
                .call(),
            Self::IndexNonArrayTypeInitExpr { expr, ty } => ide_diagnostic::diag()
                .message(format!("cannot index into type '{}'", ty.type_name(db)))
                .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                .desc(self)
                .range(expr.get_span(db))
                .call(),
            Self::IndexNonArrayTypePathExpr { expr, ty } => ide_diagnostic::diag()
                .message(format!("cannot index into type '{}'", ty.type_name(db)))
                .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                .desc(self)
                .range(expr.get_span(db))
                .call(),
            Self::NoFieldOnElementaryType { expr, ty } => ide_diagnostic::diag()
                .message(format!(
                    "type '{}' is an elementary type and cannot be initiliazed with '()'",
                    ty.type_name(db)
                ))
                .severity(auto_lsp::lsp_types::DiagnosticSeverity::ERROR)
                .desc(self)
                .range(expr.get_span(db))
                .call(),
            Self::FunctionAsType { expr, ty } => diag()
                .message(format!(
                    "'{}' is a function and cannot be used as a variable or data type",
                    ty.type_name(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(expr.get_span(db))
                .call(),
            Self::NoConfigFileFound { file } => {
                let mut diag = diag()
                    .message("no configuration file found".to_string())
                    .severity(DiagnosticSeverity::HINT)
                    .tags(vec![DiagnosticTag::UNNECESSARY])
                    .desc(self)
                    .range(Span::from(file.document(db).tree.root_node().range()))
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
            Self::UnknownProgType { prog_type } => {
                let name = if let SpecKind::Target(target) = prog_type.kind(db) {
                    target.path.to_string(db)
                } else {
                    String::new()
                };
                diag()
                    .message(format!("program type '{name}' not found"))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(prog_type.get_span(db))
                    .call()
            }
            Self::UnknownTaskRef { task } => diag()
                .message(format!(
                    "task '{}' not found in this configuration",
                    task.ident.text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(task.get_span(db))
                .call(),
            Self::ExternalVarNotFound { var } => diag()
                .message(format!(
                    "external variable '{}' not found in any accessible VAR_GLOBAL",
                    var.name(db).text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(var.get_span(db))
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
                    .range(spec.get_span(db))
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
                .range(instance_name.get_span(db))
                .call(),
            Self::ConfigInstInitFieldNotFound { field, parent_type } => diag()
                .message(format!(
                    "'{}' has no field named '{}'",
                    parent_type.type_name(db),
                    field.text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(field.get_span(db))
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
                    .range(var.get_span(db))
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
                    .range(var.get_span(db))
                    .call();

                diag.with_note("variadic parameters are only allowed in VAR_INPUT sections".into());
                diag
            }
            Self::MultipleVariadicVariables { first, second } => {
                let mut diag = diag()
                    .message("only one variadic variable is allowed per POU".to_string())
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(second.get_span(db))
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
                    .range(other_var.get_span(db))
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
                max_offset,
                base_type,
            } => {
                let mut diag = diag()
                    .message(format!(
                        "offset {} is out of range for type '{}' (valid range: 0..{})",
                        offset,
                        base_type.type_name(db),
                        max_offset,
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(expr.get_span(db))
                    .call();

                diag.with_related(Related::new(
                    format!("'{}' is declared here", var.name(db).text(db)),
                    var.scope_id(db).file(db),
                    var.get_span(db),
                ));

                diag
            }
            Self::ExternVariableNotFound { ident, scope } => {
                let name = ident.text(db);

                let mut diag = diag()
                    .message(format!(
                        "no variable '{}' found in scope for extern pragma",
                        name,
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(ident.get_span(db))
                    .call();

                // Search for similar names to suggest
                let mut query = Query::new(name.to_string());
                query.fuzzy();
                let results = SymbolSearch::new(|_, _| true)
                    .with_scope(*scope)
                    .with_query(query)
                    .search(db);
                list_candidates(db, name, &mut diag, &results, Some(*scope));

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
                    .range(*span)
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
            Self::IntoRefNotFound { spec, ident } => {
                let name = ident.text(db);
                diag()
                    .message(format!("INTO reference '{}' not found in scope", name))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(spec.get_span(db))
                    .call()
            }
            Self::IntoRefNotAny { spec, ident, ty } => {
                let name = ident.text(db);
                let mut diag = diag()
                    .message(format!(
                        "INTO reference '{}' must have an ANY type, got '{}'",
                        name,
                        ty.type_name(db),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(spec.get_span(db))
                    .call();

                diag.with_note(
                    "only variables with ANY types (e.g. ANY_INT, ANY_REAL, ANY_BIT) can be used as INTO references".into(),
                );

                let scope = spec.scope_id(db);
                if let Some(ret_spec) = scope.return_type(db)
                    && let SpecKind::Simple(elem) = ret_spec.kind(db)
                    && elem.is_any()
                {
                    let scope_kind = get_scope(db, scope).kind;
                    let callable_name = match scope_kind {
                        ScopeKind::Pou(pou) => Some(pou.get_name_ident(db).text(db)),
                        ScopeKind::MethodDecl(m) => Some(m.get_name_ident(db).text(db)),
                        ScopeKind::MethodProt(m) => Some(m.get_name_ident(db).text(db)),
                        _ => None,
                    };
                    if let Some(callable_name) = callable_name {
                        diag.with_note(format!(
                            "you may also use INTO({}) to reference the return type '{}'",
                            callable_name,
                            elem.type_name(),
                        ));
                    }
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
                    .range(func_call.path(db).get_span(db))
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
