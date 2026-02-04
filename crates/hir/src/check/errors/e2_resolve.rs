use auto_lsp::lsp_types::DiagnosticSeverity;
use db::WorkspaceDataBase;
use ide_diagnostic::{ErrorCode, IdeDiagnostic, diag};

use crate::{
    CallSite, HasName, HirNodeInfo,
    check::errors::analysis_error::ToIdeDiagnostic,
    hir_def::{
        expressions::{
            expression::{Expr, FuncCall, InitExpr, PathExpr},
            spec::Spec,
        },
        interned::{
            identifier::{Ident, SpanIdent},
            namespace::{NamespacePath, SpanNamespaceAccess},
        },
        pous::{pou::Pou, variable::VariableDecl},
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    },
    hir_ty::ty::{CallableType, Type},
    query_string::{
        method::fuzzy_callable_type_parameters, query::Query, scope::ScopeSearchCtx,
        strukt::fuzzy_struct_fields,
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
    FunctionAsVariableType {
        expr: Spec<'db>,
        ty: Type<'db>,
    },
    NamespaceNotFound {
        call_site: CallSite<'db>,
        path: NamespacePath,
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
            Self::FunctionAsVariableType { .. } => "E0215",
            Self::NamespaceNotFound { .. } => "E0216",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::NoItemInScope { .. } => "no item found in scope",
            Self::NoNamespaceItemFound { .. } => "no namespace item found",
            Self::NamespaceNotFound { .. } => "namespace not found",
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
            Self::FunctionAsVariableType { .. } => "invalid variable type",
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
                let items = ScopeSearchCtx::new(*scope)
                    .with_query(query)
                    .only_variables()
                    .search_unfiltered(db);

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
                let items = ScopeSearchCtx::new(*scope)
                    .with_query(query)
                    .only_pous()
                    .search(db, |p| matches!(p, Pou::Function(_)));

                list_pou_candidates(
                    db,
                    expr.ident(db).text(db).as_str(),
                    &mut diag,
                    items.imported_pous(),
                );
                diag
            }
            Self::NoNamespaceItemFound { path } => diag()
                .message(format!("io item found for path '{}'", path.to_string(db)))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(path.get_span(db))
                .call(),
            Self::NamespaceNotFound { call_site, path } => diag()
                .message(format!("namespace '{}' not found", path.to_string(db)))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(call_site.get_span(db))
                .call(), // add recovery checks?
            Self::NoSuchFieldPathExpr { expr, ident, ty } => {
                let mut diag = diag()
                    .message(format!(
                        "'{}' has no field named '{}'",
                        ty.with_name(db).unwrap_or_else(|| ty.full_type_name(db)),
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
                        ty.with_name(db).unwrap_or_else(|| ty.full_type_name(db)),
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
                    ty.full_type_name(db)
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
            Self::FunctionAsVariableType { expr, ty } => diag()
                .message(format!(
                    "'{}' is a function and cannot be used as a variable type",
                    ty.type_name(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(expr.get_span(db))
                .call(),
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
