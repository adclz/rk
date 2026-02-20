use std::fmt::Display;

use auto_lsp::{
    core::span::Span,
    default::db::file::File,
    lsp_types::{DiagnosticSeverity, DiagnosticTag},
};
use db::{WorkspaceDataBase, workspace::Workspace};
use ide_diagnostic::{ErrorCode, IdeDiagnostic, Related, diag};

use crate::{
    CallSite, HasName, HirNodeInfo,
    check::errors::analysis_error::ToIdeDiagnostic,
    hir_def::{
        expressions::{
            expression::{Expr, FuncCall, InitExpr, PathExpr},
            spec::{Spec, SpecKind},
        },
        interned::{
            identifier::{Ident, SpanIdent},
            namespace::{NamespacePath, SpanNamespaceAccess},
        },
        namespace::NamespaceDecl,
        pous::{pou::Pou, variable::VariableDecl},
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::{
        name_res::namespace_index,
        ty::{CallableType, Type},
    },
    query_string::{
        method::fuzzy_callable_type_parameters, namespace::NamespaceSearchCtx, query::Query,
        scope::ScopeSearchCtx, strukt::fuzzy_struct_fields,
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
        prog_type: SpanNamespaceAccess<'db>,
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
            Self::FunctionAsType { .. } => "invalid type",
            Self::NoConfigFileFound { .. } => "configuration error",
            Self::UnknownProgType { .. } | Self::UnknownTaskRef { .. } => "configuration error",
            Self::ExternalVarNotFound { .. } => "external variable not found",
            Self::AccessDeclTypeMismatch { .. } => "access declaration type mismatch",
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
                let items = ScopeSearchCtx::new(*scope, |_, _| true)
                    .with_query(query)
                    .only_variables()
                    .search(db);

                if let ScopeKind::Pou(pou) = get_scope(db, *scope).kind {
                    list_variable_candidates(
                        db,
                        pou.get_name_ident(db).text(db).as_str(),
                        &mut diag,
                        items.variables(),
                    )
                }

                let mut query = Query::new(expr.ident(db).text(db).to_string());
                query.exact();
                let items = ScopeSearchCtx::new(*scope, |pou, db| {
                    match pou {
                        Pou::Function(_) => true,
                        // enum types are allowed and all variants should be suggested
                        Pou::DataType(typ) => {
                            matches!(typ.spec(db).kind(db), SpecKind::Enum(_))
                        }
                        _ => false,
                    }
                })
                .with_query(query)
                .only_pous()
                .search(db);

                list_pou_candidates(
                    db,
                    expr.ident(db).text(db).as_str(),
                    &mut diag,
                    items.imported_pous(),
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
                        let items = ScopeSearchCtx::new(path.scope_id, |pou, db| match pou {
                            Pou::Function(_) => false,
                            _ => true,
                        })
                        .with_query(query)
                        .only_pous()
                        .search(db);

                        list_pou_candidates(
                            db,
                            path.path.target.ident.text(db).as_str(),
                            &mut diag,
                            items.imported_pous(),
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
                    Some(path) => {
                        let ctx = NamespaceSearchCtx::new(path.path);
                        let items = ctx.search(db);

                        if !namespace_index(db, path.path).is_empty() {
                            diag.with_note(format!(
                                r#"namespace named '{}' exists but it cannot be used as an item, you can either:
- Import the namespace via an USING directive: 'USING {}'
- Import an item from this namespace: '{}.<POU>'"#,
                                path_str,
                                path_str,
                                path_str,
                            ));
                        } else {
                            list_namespace_candidates(
                                db,
                                path.path.to_string(db).as_str(),
                                &mut diag,
                                &items,
                            );
                        }
                    }
                }

                diag
            } // todo: add recovery just as above
            Self::UsingNamespaceNotFound { call_site, path } => {
                let mut diag = diag()
                    .message(format!("namespace '{}' not found", path.to_string(db)))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(call_site.get_span(db))
                    .call();

                let ctx = NamespaceSearchCtx::new(*path);
                let items = ctx.search(db);

                list_namespace_candidates(db, path.to_string(db).as_str(), &mut diag, &items);
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

                if let Type::Struct(strukt) = ty.normalize(db) {
                    fuzzy_struct_fields(db, strukt, &mut diag, ident.text(db).as_str())
                };

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
            Self::UnknownProgType { prog_type } => diag()
                .message(format!(
                    "program type '{}' not found",
                    prog_type.to_string(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(prog_type.get_span(db))
                .call(),
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
        }
    }
}

fn list_variable_candidates<'db, I>(
    db: &'db dyn WorkspaceDataBase,
    scope_name: &str,
    diag: &mut IdeDiagnostic,
    mut candidates: I,
) where
    I: Iterator<Item = &'db VariableDecl<'db>>,
{
    // Collect up to 6 candidates to check if there are more than 5
    let mut collected = Vec::with_capacity(6);
    for candidate in candidates.by_ref().take(6) {
        collected.push(candidate);
    }

    if !collected.is_empty() {
        let count = collected.len();
        let display_count = count.min(5);

        let mut note = format!(
            "'{}' has item{} with similar name:\n",
            scope_name,
            if count > 1 { "s" } else { "" }
        );

        for (i, candidate) in collected.iter().take(display_count).enumerate() {
            if i > 0 {
                note.push('\n');
            }
            note.push_str(&format!("- {}", candidate.name(db).text(db)));
        }

        if count > 5 {
            note.push_str("\n  ...");
        }

        diag.with_note(note);
    }
}

fn list_pou_candidates<'db, I>(
    db: &'db dyn WorkspaceDataBase,
    scope_name: &str,
    diag: &mut IdeDiagnostic,
    mut candidates: I,
) where
    I: Iterator<Item = (NamespacePath, Pou<'db>)>,
{
    // Collect up to 6 candidates to check if there are more than 5
    let mut collected = Vec::with_capacity(6);
    for candidate in candidates.by_ref().take(6) {
        collected.push(candidate);
    }

    if !collected.is_empty() {
        let count = collected.len();
        let display_count = count.min(5);

        let mut note = match collected.len() {
            1 => format!(
                "an item named '{}' is available, but needs to be imported:\n",
                scope_name
            ),
            _ => format!(
                "items named '{}' are available, but need to be imported:\n",
                scope_name
            ),
        };

        for (i, (namespace, candidate)) in collected.iter().take(display_count).enumerate() {
            if i > 0 {
                note.push('\n');
            }
            note.push_str(&format!("- USING {}", namespace.to_string(db)));
        }

        if count > 5 {
            note.push_str("\n  ...");
        }

        diag.with_note(note);
    }
}

fn list_namespace_candidates(
    db: &dyn WorkspaceDataBase,
    ns: impl Display,
    diag: &mut IdeDiagnostic,
    candidates: &Vec<NamespaceDecl>,
) {
    if !candidates.is_empty() {
        let mut note = format!(
            "no namespace named '{}' found, but the following namespace{} {} similar path:\n",
            ns,
            match candidates.len() {
                1 => "",
                _ => "s",
            },
            match candidates.len() {
                1 => "has",
                _ => "have",
            },
        );

        for (i, candidate) in candidates.iter().take(5).enumerate() {
            if i > 0 {
                note.push('\n');
            }
            note.push_str(&format!("- {}", candidate.path(db).to_string(db)));
        }

        if candidates.len() > 5 {
            note.push_str("\n  ...");
        }

        diag.with_note(note);
    }
}
