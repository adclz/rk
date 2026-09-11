use crate::CallSite;
use crate::HasName;
use crate::HirNodeInfo;
use crate::check::errors::ToIdeDiagnostic;
use crate::hir_def::expressions::expression::InitExpr;
use crate::hir_def::expressions::expression::PathExpr;
use crate::hir_def::expressions::spec::SpecKind;
use crate::hir_def::interned::identifier::Ident;
use crate::hir_def::interned::namespace::NamespacePath;
use crate::hir_def::interned::namespace::SpanNamespaceAccess;
use crate::hir_def::pous::pou::Pou;
use crate::hir_def::pous::variable::VariableDecl;
use crate::hir_def::scope::ScopeId;
use crate::hir_def::scope::ScopeKind;
use crate::hir_def::semantic_index::get_scope;
use crate::hir_ty::index_graphs::namespace_index;
use crate::hir_ty::ty::Type;
use crate::query_string::fields::fuzzy_type_fields;
use crate::query_string::fields::suggest_similar_note;
use crate::query_string::query::Query;
use crate::query_string::scope::SearchResult;
use crate::query_string::scope::SymbolSearch;
use auto_lsp::lsp_types::DiagnosticSeverity;
use auto_lsp::tree_sitter;
use db::WorkspaceDataBase;
use ide_diagnostic::ErrorCode;
use ide_diagnostic::IdeDiagnostic;
use ide_diagnostic::Related;
use ide_diagnostic::diag;

#[derive(Clone, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum ResolveError<'db> {
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
    NoNamespaceItemFound {
        path: SpanNamespaceAccess<'db>,
    },
    UsingNamespaceNotFound {
        call_site: CallSite<'db>,
        path: NamespacePath,
    },
    /// Two or more items with the same name are available in scope.
    MultipleItemsInScope {
        name: Ident,
        span: tree_sitter::Range,
        candidates: Vec<(Pou<'db>, NamespacePath)>,
    },
    /// A VAR_EXTERNAL declaration references a name not present in any accessible VAR_GLOBAL.
    ExternalVarNotFound {
        var: VariableDecl<'db>,
    },
    /// A VAR_EXTERNAL aliases its VAR_GLOBAL's storage by name, so the two
    /// declarations must agree about the type, subrange included. A `REAL`
    /// external over an `INT` global stores the wrong lane; a plain `INT`
    /// external over an `INT (0..10)` global goes around the range check.
    ExternalVarTypeMismatch {
        var: VariableDecl<'db>,
        external: Type<'db>,
        global: Type<'db>,
    },
    /// A RETAIN/NON_RETAIN qualifier on a variable of a stateless POU
    /// (FUNCTION or METHOD). Retentive behavior requires instance storage —
    /// only FUNCTION_BLOCK, CLASS, and PROGRAM variables (and VAR_GLOBAL)
    /// can be retentive.
    RetainInStatelessPou {
        var: VariableDecl<'db>,
        pou_kind: &'static str,
    },
}

impl<'db> ErrorCode for ResolveError<'db> {
    fn code(&self) -> &'static str {
        match self {
            Self::NoItemInScope { .. } => "E0201",
            Self::NoSuchFieldInitExpr { .. } => "E0202",
            Self::NoSuchFieldPathExpr { .. } => "E0202",
            Self::NoNamespaceItemFound { .. } => "E0203",
            Self::UsingNamespaceNotFound { .. } => "E0204",
            Self::MultipleItemsInScope { .. } => "E0205",
            Self::ExternalVarNotFound { .. } => "E0206",
            Self::ExternalVarTypeMismatch { .. } => "E0207",
            Self::RetainInStatelessPou { .. } => "E0208",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            Self::NoItemInScope { .. } => "no item found in scope",
            Self::NoSuchFieldInitExpr { .. } => "no such field",
            Self::NoSuchFieldPathExpr { .. } => "no such field",
            Self::NoNamespaceItemFound { .. } => "no namespace item found",
            Self::UsingNamespaceNotFound { .. } => "namespace not found",
            Self::MultipleItemsInScope { .. } => "multiple items in scope",
            Self::ExternalVarNotFound { .. } => "external variable not found",
            Self::ExternalVarTypeMismatch { .. } => "external variable type mismatch",
            Self::RetainInStatelessPou { .. } => "invalid retentive qualifier",
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
                query.similar();
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

                        let ns_kw = crate::hir_ty::index_graphs::absolute_namespace_path(
                            db,
                            path.scope_id,
                            ns_kw,
                        );
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

                        let full_path = crate::hir_ty::index_graphs::absolute_namespace_path(
                            db,
                            path.scope_id,
                            full_path,
                        );
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

                // Sorted, because `counts` is a hash map: without this the
                // notes, the related labels and the suggestion below each come
                // out in whatever order the map happened to hold, so the same
                // source reports differently from one build to the next.
                let mut duplicated: Vec<_> = counts
                    .iter()
                    .filter(|(_, count)| **count > 1)
                    .map(|(ns, _)| ns)
                    .collect();
                duplicated.sort_by_key(|ns| ns.to_string(db));
                let mut distinct: Vec<_> = counts
                    .iter()
                    .filter(|(_, count)| **count == 1)
                    .map(|(ns, _)| ns.to_string(db))
                    .collect();
                distinct.sort();

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

                    // Same reason, one level down: the declarations inside a
                    // namespace are pointed at in source order, not discovery
                    // order.
                    let mut in_ns: Vec<_> =
                        candidates.iter().filter(|(_, pou_ns)| pou_ns == *ns).collect();
                    in_ns.sort_by_key(|(pou, _)| {
                        (
                            pou.get_scope_id(db).file(db).url(db).to_string(),
                            pou.get_span(db).start_byte,
                        )
                    });
                    for (pou, _) in in_ns {
                        diag.with_related(Related::new(
                            format!("'{}' declared here", name),
                            pou.get_scope_id(db).file(db),
                            pou.get_span(db),
                        ));
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
            Self::ExternalVarNotFound { var } => diag()
                .message(format!(
                    "external variable '{}' not found in any accessible VAR_GLOBAL",
                    var.name(db).text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &var.get_span(db)).unwrap_or_default())
                .call(),
            Self::ExternalVarTypeMismatch {
                var,
                external,
                global,
            } => diag()
                .message(format!(
                    "'{}' is declared '{}' here but its VAR_GLOBAL is '{}': an external must repeat the global's type exactly",
                    var.name(db).text(db),
                    crate::check::errors::e07_subrange::with_bounds(db, *external),
                    crate::check::errors::e07_subrange::with_bounds(db, *global),
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &var.get_span(db)).unwrap_or_default())
                .call(),
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
