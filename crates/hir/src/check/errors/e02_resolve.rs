// SPDX-FileCopyrightText: 2026 Clauzel Adrien
// SPDX-License-Identifier: AGPL-3.0-only

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
        /// As written.
        ident: Ident,
        ty: Type<'db>,
    },
    NoSuchFieldPathExpr {
        expr: PathExpr<'db>,
        /// As written.
        ident: Ident,
        ty: Type<'db>,
    },
    NoNamespaceItemFound {
        path: SpanNamespaceAccess<'db>,
    },
    UsingNamespaceNotFound {
        call_site: CallSite<'db>,
        /// As written: no declaration exists to spell it.
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
        global_var: VariableDecl<'db>,
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
    /// A variable named where its storage is not: it passed the check, then
    /// codegen found no such local or no such field of the instance.
    OutOfReach {
        expr: PathExpr<'db>,
        var: VariableDecl<'db>,
        why: Unreachable,
    },
}

/// Why a variable found by name cannot be used where it is named.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum Unreachable {
    /// A VAR_TEMP lives while its own body runs.
    Temp { route: TempRoute },
    /// A VAR_EXTERNAL names a global, so no instance holds it.
    External,
    /// A METHOD's variables live while it runs: `inst.M.x`.
    CallVariable,
}

/// How a VAR_TEMP was named from outside its own body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, salsa::Update)]
pub enum TempRoute {
    /// By name, in a METHOD of its block.
    Method,
    /// By name, in another body: a derived block's, or one of its methods.
    OtherBody,
    /// Through an instance or `THIS^`.
    Path,
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
            Self::OutOfReach { .. } => "E0209",
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
                        "no item '{}' found in scope",
                        expr.ident(db).as_str(db)
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();

                let mut query = Query::new(expr.ident(db).ident(db).text(db).to_string());
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
                    expr.ident(db).as_str(db),
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
                        let mut query = Query::new(path.path.target.with_case.text(db).to_string());
                        query.exact();
                        let items = SymbolSearch::new(|pou, db| !matches!(pou, Pou::Function(_)))
                            .with_scope(path.scope_id)
                            .with_query(query)
                            .only_pous()
                            .search(db);

                        list_candidates(
                            db,
                            path.path.target.with_case.text(db).as_str(),
                            &mut diag,
                            &items,
                            Some(path.scope_id),
                        );

                        let ns_kw = NamespacePath::from((db, &path.path.target.ident(db)));

                        let ns_kw = crate::hir_ty::index_graphs::absolute_namespace_path(
                            db,
                            path.scope_id,
                            ns_kw,
                        );
                        if !namespace_index(db, ns_kw).is_empty() {
                            diag.with_note(format!("'{path_str}' is a namespace, not an item"));
                            diag.with_help(format!(
                                "import it with 'USING {path_str}', or name an item of it, as '{path_str}.<POU>'"
                            ));
                        }
                    }
                    Some(namespace) => {
                        // Build the full path (namespace prefix + target) to check
                        // if it's a known namespace (e.g. ["Std"] + "Counters" = ["Std", "Counters"])
                        let mut full_fragments = namespace.path(db).fragments(db).to_vec();
                        full_fragments.push(path.path.target.ident(db));
                        let full_path = NamespacePath::new(db, full_fragments);

                        let full_path = crate::hir_ty::index_graphs::absolute_namespace_path(
                            db,
                            path.scope_id,
                            full_path,
                        );
                        if !namespace_index(db, full_path).is_empty() {
                            diag.with_note(format!("'{path_str}' is a namespace, not an item"));
                            diag.with_help(format!(
                                "import it with 'USING {path_str}', or name an item of it, as '{path_str}.<POU>'"
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
                // As a candidate writes it: they all name it, in some case.
                let name = candidates
                    .first()
                    .map(|(pou, _)| pou.get_name_with_case(db).text(db))
                    .unwrap_or(name.text(db));
                let spelled = |ns: &crate::hir_def::interned::namespace::NamespacePath| {
                    crate::hir_ty::display::namespace_spelling(db, *ns)
                };

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
                duplicated.sort_by_key(|ns| spelled(ns));
                let mut distinct: Vec<_> = counts
                    .iter()
                    .filter(|(_, count)| **count == 1)
                    .map(|(ns, _)| spelled(ns))
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
                        spelled(ns),
                    ));

                    // Same reason, one level down: the declarations inside a
                    // namespace are pointed at in source order, not discovery
                    // order.
                    let mut in_ns: Vec<_> = candidates
                        .iter()
                        .filter(|(_, pou_ns)| pou_ns == *ns)
                        .collect();
                    in_ns.sort_by_key(|(pou, _)| {
                        (
                            pou.get_scope_id(db).file(db).url(db).to_string(),
                            pou.get_span(db).start_byte,
                        )
                    });
                    for (pou, _) in in_ns {
                        diag.with_related(Related::new(
                            format!("'{}' is declared here", name),
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

                    diag.with_help(format!("qualify the name: {}", qualified.join(" or ")));
                }

                diag
            }
            Self::ExternalVarNotFound { var } => diag()
                .message(format!(
                    "no VAR_GLOBAL is named '{}'",
                    var.name_with_case(db).text(db)
                ))
                .severity(DiagnosticSeverity::ERROR)
                .desc(self)
                .range(crate::denormalize(db, file, &var.get_name_span(db)).unwrap_or_default())
                .call(),
            Self::ExternalVarTypeMismatch {
                var,
                external,
                global_var,
                global,
            } => {
                // A STRING shows the capacity it was declared with, which is
                // what two otherwise equal STRINGs disagree about.
                let shown = |decl: &VariableDecl<'db>, ty: Type<'db>| {
                    match crate::hir_ty::infer::normalize::string_capacity(db, decl.spec(db)) {
                        Some(capacity) => format!("STRING[{capacity}]"),
                        None => crate::check::errors::e07_subrange::with_bounds(db, ty),
                    }
                };
                let mut diag = diag()
                    .message(format!(
                        "'{}' is declared '{}' and its VAR_GLOBAL is '{}'",
                        var.name_with_case(db).text(db),
                        shown(var, *external),
                        shown(global_var, *global),
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(
                        crate::denormalize(db, file, &var.spec(db).get_span(db))
                            .unwrap_or_default(),
                    )
                    .call();
                diag.with_note(
                    "a VAR_EXTERNAL repeats the type of its VAR_GLOBAL exactly".to_string(),
                );
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
                        var.name_with_case(db).text(db),
                        qualifier,
                        pou_kind,
                    ))
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &var.get_name_span(db)).unwrap_or_default())
                    .call();
                diag.with_note(
                    "only a variable of a FUNCTION_BLOCK, CLASS or PROGRAM, or a VAR_GLOBAL, has storage to retain"
                        .to_string(),
                );

                diag
            }
            Self::OutOfReach { expr, var, why } => {
                let name = var.name_with_case(db).text(db).to_string();
                let owner = match get_scope(db, var.get_scope_id(db)).kind {
                    ScopeKind::Pou(pou) => Type::new_pou(db, pou).type_name(db),
                    ScopeKind::MethodDecl(m) => Type::MethodDecl(m.into()).type_name(db),
                    _ => String::new(),
                };
                let (message, note, help) = match why {
                    Unreachable::Temp {
                        route: TempRoute::Method,
                    } => (
                        format!("VAR_TEMP '{name}' of '{owner}' cannot be used in a METHOD"),
                        "a VAR_TEMP belongs to the body that declares it".to_string(),
                        Some("declare one in the METHOD".to_string()),
                    ),
                    Unreachable::Temp {
                        route: TempRoute::OtherBody,
                    } => (
                        format!("VAR_TEMP '{name}' of '{owner}' cannot be used in a derived block"),
                        format!(
                            "SUPER() runs the body of '{owner}' with that body's own VAR_TEMPs"
                        ),
                        Some("declare one in the derived block".to_string()),
                    ),
                    Unreachable::Temp {
                        route: TempRoute::Path,
                    } => (
                        format!(
                            "VAR_TEMP '{name}' of '{owner}' cannot be reached through an instance"
                        ),
                        "a VAR_TEMP exists only while the body runs".to_string(),
                        None,
                    ),
                    Unreachable::External => (
                        format!(
                            "VAR_EXTERNAL '{name}' of '{owner}' cannot be reached through an instance"
                        ),
                        format!("it names the VAR_GLOBAL '{name}', which no instance holds"),
                        Some("declare that global VAR_EXTERNAL where it is used".to_string()),
                    ),
                    Unreachable::CallVariable => (
                        format!("'{name}' of METHOD '{owner}' cannot be reached from outside it"),
                        "a METHOD's variables exist only while it runs".to_string(),
                        Some(
                            "take its result from the call, and its outputs with '=>'".to_string(),
                        ),
                    ),
                };
                let mut diag = diag()
                    .message(message)
                    .severity(DiagnosticSeverity::ERROR)
                    .desc(self)
                    .range(crate::denormalize(db, file, &expr.get_span(db)).unwrap_or_default())
                    .call();
                diag.with_related(Related::new(
                    format!("'{name}' is declared here"),
                    var.get_scope_id(db).file(db),
                    var.get_name_span(db),
                ));
                diag.with_advice(Some(note), help);
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
        .map(|v| v.name_with_case(db).text(db).to_string())
        .collect();
    if !var_names.is_empty() {
        let owner = scope
            .map(|s| get_scope(db, s).kind)
            .and_then(|k| match k {
                ScopeKind::Pou(pou) => Some(pou.get_name_with_case(db).text(db).to_string()),
                _ => None,
            })
            .unwrap_or_else(|| name.to_string());
        suggest_similar_note(&owner, "item", diag, var_names.iter().map(|n| n.as_str()));
    }

    // Suggest THIS.variable for variables in the parent FB/class scope (method context only)
    let this_names: Vec<_> = results
        .this_variables()
        .map(|v| v.name_with_case(db).text(db).to_string())
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
        .map(|p| p.get_name_with_case(db).text(db).to_string())
        .collect();
    if !local_names.is_empty() {
        let count = local_names.len().min(5);
        let mut note = match count {
            1 => "an item with a similar name is in scope:\n".to_string(),
            _ => "items with similar names are in scope:\n".to_string(),
        };
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
                pou.get_name_with_case(db).text(db).to_string(),
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
                pou.get_name_with_case(db).text(db),
                crate::hir_ty::display::namespace_spelling(db, *namespace)
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
            note.push_str(&format!("- {}", candidate.path_with_case(db).to_string(db)));
        }

        if count > 5 {
            note.push_str("\n  ...");
        }

        diag.with_note(note);
    }
}
