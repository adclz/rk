use db::WorkspaceDataBase;

use crate::{
    HasName,
    hir_def::{
        expressions::spec::{ElementarySpec, Spec, SpecKind},
        interned::{
            identifier::Ident,
            namespace::{NamespaceAccess, NamespacePath},
        },
        pous::{class::MethodDecl, function::Function, pou::Pou},
        program::ProgramDecl,
        scope::{ScopeId, ScopeKind},
        semantic_index::{get_scope, semantic_index},
        using::Using,
    },
    hir_ty::{
        head::signature::{function_required_arity, function_signature},
        index_graphs::{
            namespace_index, namespace_pou_candidates, pou_candidates, pou_index, program_index,
        },
        ty::{CallableType, Type},
    },
};

/// Result of POU name resolution, distinguishing unique matches from ambiguities.
#[derive(Debug, Clone)]
pub enum PouResolution<'db> {
    /// `Option<Using>` is `Some` when the POU was found via a USING directive.
    Found(Pou<'db>, Option<Using<'db>>),
    /// Two or more USING directives at the same scope level import different POUs
    /// with this name.
    Ambiguous(Vec<(Pou<'db>, NamespacePath)>),
    NotFound,
}

impl<'db> PouResolution<'db> {
    /// Extract the POU if uniquely resolved, discarding ambiguities.
    pub fn found(self) -> Option<Pou<'db>> {
        match self {
            Self::Found(pou, _) => Some(pou),
            _ => None,
        }
    }
}

/// Result of resolving a name in a scope.
#[derive(Debug, Clone)]
pub enum NameResolution<'db> {
    /// Resolved to a POU (function, function block, class, etc.)
    /// `Option<Using>` is `Some` when the POU was found via a USING directive.
    Pou(Pou<'db>, Option<Using<'db>>),
    /// Resolved to a PROGRAM declaration (only visible from config scopes)
    Program(ProgramDecl<'db>),
    /// Resolved to the method's own name (self-reference)
    MethodSelf(MethodDecl<'db>),
    /// Two or more USING directives import different POUs with the same name.
    Ambiguous(Vec<(Pou<'db>, NamespacePath)>),
    /// Not found
    NotFound,
}

/// Single entry point for resolving a name ([`NamespaceAccess`]) to its declaration.
///
/// Resolution order (first match wins):
/// 1. Self-reference (method or POU referencing its own name)
/// 2. POU via namespace access (local scope → parent/USING → global)
///
/// This function is used by both head-level (spec) and body-level (path expr) resolution.
pub fn resolve_name<'db>(
    db: &'db dyn WorkspaceDataBase,
    access: &NamespaceAccess<'db>,
    scope: ScopeId<'db>,
) -> NameResolution<'db> {
    // Namespace-qualified names skip directly to namespace lookup (no ambiguity possible)
    if access.namespace.is_some() {
        return match resolve_namespace_access(db, access) {
            PouResolution::Found(pou, using) => NameResolution::Pou(pou, using),
            // Qualified names can't be ambiguous — the user chose the namespace
            PouResolution::Ambiguous(_) => unreachable!(),
            PouResolution::NotFound => NameResolution::NotFound,
        };
    }

    let name = access.target.ident;

    // 1. Self-reference: a POU or method referencing its own name takes priority
    //    over parent scope lookups (which may return a different duplicate).
    match get_scope(db, scope).kind {
        ScopeKind::MethodDecl(method) if name == method.name(db) => {
            return NameResolution::MethodSelf(method);
        }
        ScopeKind::Pou(pou) if name == pou.get_name_ident(db) => {
            if let Pou::Function(f) = pou {
                return NameResolution::Pou(pou, None);
            }
        }
        _ => {}
    }

    // 2. POU resolution (local → parent/USING → global)
    match resolve_namespace_access(db, access) {
        PouResolution::Found(pou, using) => NameResolution::Pou(pou, using),
        PouResolution::Ambiguous(candidates) => NameResolution::Ambiguous(candidates),
        PouResolution::NotFound => {
            // 4. Program resolution (config scopes only — programs are not visible to other POUs)
            if is_config_scope(db, scope)
                && let Some(prog) = program_index(db, name)
            {
                return NameResolution::Program(prog);
            }
            NameResolution::NotFound
        }
    }
}

/// Resolve a namespace access to a POU declaration.
#[tracing::instrument(skip_all)]
pub(crate) fn resolve_namespace_access<'db>(
    db: &'db dyn WorkspaceDataBase,
    access: &NamespaceAccess<'db>,
) -> PouResolution<'db> {
    let target = &access.target;

    match &access.namespace {
        // Namespace-qualified: look up directly in the namespace's local_pous.
        // No ambiguity is possible here — the user specified which namespace.
        Some(path) => {
            for ns in namespace_index(db, **path).iter() {
                if let PouResolution::Found(pou, using) =
                    pou_names_res(db, target.ident, ns.scope_id(db))
                {
                    return PouResolution::Found(pou, using);
                }
            }
            PouResolution::NotFound
        }
        // Unqualified: resolve via scope chain, USING can be ambiguous
        None => pou_names_res(db, target.ident, target.scope_id),
    }
}

pub fn pou_names_res<'db>(
    db: &'db dyn WorkspaceDataBase,
    name: Ident,
    scope: ScopeId<'db>,
) -> PouResolution<'db> {
    // Checks for POUs declared in the current scope
    if let Some(pou) = scope.def_map(db).local_pous.get(&name) {
        return PouResolution::Found(*pou, None);
    }

    // Checks for parent POUs and those imported via USING directives
    match find_in_parent_pous(db, name, scope) {
        PouResolution::NotFound => match pou_index(db, name) {
            Some(pou) => PouResolution::Found(pou, None),
            None => PouResolution::NotFound,
        },
        result => result,
    }
}

#[tracing::instrument(skip_all)]
pub fn find_in_parent_pous<'db>(
    db: &'db dyn WorkspaceDataBase,
    name: Ident,
    scope: ScopeId<'db>,
) -> PouResolution<'db> {
    let it = semantic_index(db, scope.file(db)).scope_iterator(db, scope);
    for scope in it {
        // Namespace siblings take priority over USING — no ambiguity
        if let ScopeKind::Namespace(ns) = scope.kind {
            for ns in namespace_index(db, *ns.path(db)).iter() {
                if let Some(p) = ns.scope_id(db).def_map(db).local_pous.get(&name) {
                    return PouResolution::Found(*p, None);
                }
            }
        }

        // Collect ALL USING matches at this scope level
        let mut matches: Vec<(Pou<'db>, NamespacePath, Using<'db>)> = vec![];
        for using in &scope.usings {
            let ns_path: NamespacePath = *using.path(db);
            for ns in namespace_index(db, ns_path).iter() {
                if let Some(pou) = ns.scope_id(db).def_map(db).local_pous.get(&name) {
                    // Deduplicate by POU identity (shared namespaces across files)
                    if !matches.iter().any(|(p, _, _)| p == pou) {
                        matches.push((*pou, ns_path, *using));
                    }
                }
            }
        }

        match matches.len() {
            0 => continue,
            1 => return PouResolution::Found(matches[0].0, Some(matches[0].2)),
            _ => {
                // Same-name FUNCTIONs reachable through ONE namespace path are
                // an overload set, not an ambiguity — files reopening a
                // namespace (a library's included) overload each other, and
                // the call site picks by signature (`select_overload`).
                // Identical signatures are E0101 duplicates, equally-viable
                // calls E0237. Matches from DIFFERENT paths, or involving
                // non-overloadable POUs, stay genuinely ambiguous.
                let first_path = matches[0].1;
                if matches
                    .iter()
                    .all(|(p, path, _)| *path == first_path && matches!(p, Pou::Function(_)))
                {
                    return PouResolution::Found(matches[0].0, Some(matches[0].2));
                }
                return PouResolution::Ambiguous(
                    matches.into_iter().map(|(p, ns, _)| (p, ns)).collect(),
                );
            }
        }
    }

    PouResolution::NotFound
}

/// Outcome of overload selection.
pub enum OverloadPick<'db> {
    /// A unique callable to use — either the resolved overload, or the input
    /// unchanged when the name isn't an overload set or nothing better matched.
    One(CallableType<'db>),
    /// Several overloads are equally viable for the given argument types; the
    /// caller must disambiguate with an explicit cast.
    Ambiguous(Vec<Function<'db>>),
}

/// Select the FUNCTION overload whose signature matches `arg_types`.
///
/// Ordinary name resolution binds a bare function name to the *first* same-name
/// FUNCTION in scope. When that name is an overload set, this re-selects by
/// matching the call's argument types against each overload's signature
/// ([`function_signature`]). The selection lives here — in the POU-finding
/// module — so call resolution only supplies the arg types and stays unaware
/// that overloading exists.
///
/// Ranking (never guesses): an argument matches a parameter *exactly* (same
/// type / literal-of-default-type) or by *widening* (implicit cast). An overload
/// that's exact on every argument is unique and wins. Otherwise a single viable
/// overload is used; two or more viable ⇒ [`OverloadPick::Ambiguous`]; none ⇒
/// keep the first-match so the ordinary param-mismatch error surfaces.
pub fn select_overload<'db>(
    db: &'db dyn WorkspaceDataBase,
    callable: CallableType<'db>,
    arg_types: &[Type<'db>],
) -> OverloadPick<'db> {
    let CallableType::Function(first) = callable else {
        return OverloadPick::One(callable);
    };

    let name = first.name(db);
    let candidates = match function_namespace_path(db, first) {
        Some(path) => namespace_pou_candidates(db, path, name),
        None => pou_candidates(db, name),
    };
    let functions: Vec<Function<'db>> = candidates
        .into_iter()
        .filter_map(|p| match p {
            Pou::Function(f) => Some(f),
            _ => None,
        })
        .collect();
    if functions.len() <= 1 {
        // Not an overload set — nothing to pick.
        return OverloadPick::One(callable);
    }

    let mut exact: Option<Function<'db>> = None;
    let mut viable: Vec<Function<'db>> = Vec::new();
    for f in functions {
        let sig = function_signature(db, f);
        // Viable arg counts: at least the required params, at most all of them
        // (trailing defaulted params may be omitted).
        if arg_types.len() < function_required_arity(db, f) || arg_types.len() > sig.len() {
            continue;
        }
        let mut all_exact = true;
        let mut ok = true;
        for (arg, param) in arg_types.iter().zip(sig.iter()) {
            match classify_arg(db, *arg, *param) {
                ArgMatch::Exact => {}
                ArgMatch::Widen => all_exact = false,
                ArgMatch::No => {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            if all_exact {
                exact = Some(f);
            }
            viable.push(f);
        }
    }

    // An all-exact match is unique — two distinct signatures can't both exactly
    // equal the same argument tuple — so it always wins.
    if let Some(f) = exact {
        return OverloadPick::One(CallableType::Function(f));
    }
    match viable.len() {
        0 => OverloadPick::One(callable),
        1 => OverloadPick::One(CallableType::Function(viable[0])),
        _ => OverloadPick::Ambiguous(viable),
    }
}

enum ArgMatch {
    Exact,
    Widen,
    No,
}

/// Classify how argument type `arg` matches parameter type `param`.
fn classify_arg<'db>(db: &'db dyn WorkspaceDataBase, arg: Type<'db>, param: Type<'db>) -> ArgMatch {
    let arg = arg.normalize(db);
    let param = param.normalize(db);
    if arg == param {
        return ArgMatch::Exact;
    }
    // An untyped literal matches its default type (INT / REAL) exactly, and
    // widens to any larger compatible type (INT->DINT, INT->REAL, REAL->LREAL).
    // The direction matters: a float literal must NOT match an integer param
    // even though INT widens to REAL.
    if let Type::Infer(it) = arg {
        let default = it.to_spec(db);
        if let Type::Elementary(p) = param {
            if default == p {
                return ArgMatch::Exact;
            }
            if p.implicit_cast(default).is_some() {
                return ArgMatch::Widen;
            }
        }
        return ArgMatch::No;
    }
    if let (Type::Elementary(a), Type::Elementary(p)) = (arg, param)
        && p.implicit_cast(a).is_some()
    {
        return ArgMatch::Widen;
    }
    ArgMatch::No
}

/// The namespace path a function is declared in, or `None` for a top-level
/// (global) declaration. Mirrors the namespace walk in `qualified_pou_ident`,
/// so an overload set is gathered from the same scope the name resolved in.
fn function_namespace_path<'db>(
    db: &'db dyn WorkspaceDataBase,
    f: Function<'db>,
) -> Option<NamespacePath> {
    let scope_id = f.scope_id(db);
    if scope_id.is_global(db) {
        return None;
    }
    for scope in semantic_index(db, scope_id.file(db)).scope_iterator(db, scope_id) {
        if let ScopeKind::Namespace(ns) = scope.kind {
            return Some(*ns.path(db));
        }
    }
    None
}

/// Returns true if the given scope (or any of its ancestors) is a config scope.
fn is_config_scope<'db>(db: &'db dyn WorkspaceDataBase, scope: ScopeId<'db>) -> bool {
    if get_scope(db, scope).is_config() {
        return true;
    }
    for ancestor in semantic_index(db, scope.file(db)).scope_iterator(db, scope) {
        if ancestor.is_config() {
            return true;
        }
    }
    false
}

impl<'db> Type<'db> {
    /// Resolve a type specification to a [`Type`].
    ///
    /// For simple/elementary specs, this is a direct mapping.
    /// For target specs (referencing POUs or generics), this uses [`resolve_name`]
    /// to look up the declaration.
    pub(crate) fn resolve_spec(db: &'db dyn WorkspaceDataBase, spec: Spec<'db>) -> Self {
        match spec.kind(db) {
            SpecKind::Simple(elem) => Type::Elementary(*elem),
            SpecKind::SizedString(_) => Type::Elementary(ElementarySpec::String),
            SpecKind::Ref(ref_to) => Type::RefTo(*ref_to),
            SpecKind::Struct(strukt) => Type::Struct(*strukt),
            SpecKind::Array(arr) => Type::Array(*arr),
            SpecKind::ArrayConformand(a) => Type::ArrayConformand(*a),
            SpecKind::Enum(enm) => Type::Enum(*enm),
            SpecKind::Subrange(sub) => Type::SubRange(*sub),
            SpecKind::Target(t) => match resolve_name(db, &t.path, spec.scope_id(db)) {
                NameResolution::Pou(pou, _) => Type::new_pou(db, pou),
                NameResolution::Program(p) => Type::Program(p),
                NameResolution::MethodSelf(m) => Type::MethodDecl(m.into()),
                NameResolution::Ambiguous(_) | NameResolution::NotFound => Type::Never,
            },
        }
    }
}
