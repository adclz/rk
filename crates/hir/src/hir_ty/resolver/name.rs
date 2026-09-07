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
        ScopeKind::MethodDecl(method) if name.caseless(db) == method.name(db).caseless(db) => {
            return NameResolution::MethodSelf(method);
        }
        ScopeKind::Pou(pou) if name.caseless(db) == pou.get_name_ident(db).caseless(db) => {
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
#[tracing::instrument(level = "trace", skip_all)]
pub(crate) fn resolve_namespace_access<'db>(
    db: &'db dyn WorkspaceDataBase,
    access: &NamespaceAccess<'db>,
) -> PouResolution<'db> {
    let target = &access.target;

    match &access.namespace {
        // Namespace-qualified: look up directly in the namespace's local_pous.
        // No ambiguity is possible here — the user specified which namespace.
        Some(path) => {
            // Relative to where it was written, then absolute.
            let path = crate::hir_ty::index_graphs::absolute_namespace_path(
                db,
                target.scope_id,
                **path,
            );
            for ns in namespace_index(db, path).iter() {
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
    if let Some(pou) = scope.def_map(db).local_pous.get(&name.caseless(db)) {
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

#[tracing::instrument(level = "trace", skip_all)]
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
                if let Some(p) = ns
                    .scope_id(db)
                    .def_map(db)
                    .local_pous
                    .get(&name.caseless(db))
                {
                    return PouResolution::Found(*p, None);
                }
            }
        }

        // The global namespace outranks its USINGs the same way: a top-level
        // declaration (this file's or any other's) shadows an import — the
        // rule every USING-like construct converges on. Before this arm the
        // walk fell through to the USING matches and `pou_index` was only
        // the post-walk fallback, so `USING Std.Timers` silently WON over
        // the workspace's own top-level TON.
        if matches!(scope.kind, ScopeKind::Global)
            && let Some(pou) = pou_index(db, name)
        {
            return PouResolution::Found(pou, None);
        }

        // Collect ALL USING matches at this scope level
        let mut matches: Vec<(Pou<'db>, NamespacePath, Using<'db>)> = vec![];
        for using in &scope.usings {
            let ns_path: NamespacePath = crate::hir_ty::index_graphs::absolute_namespace_path(
                db,
                scope.id,
                *using.path(db),
            );
            for ns in namespace_index(db, ns_path).iter() {
                if let Some(pou) = ns
                    .scope_id(db)
                    .def_map(db)
                    .local_pous
                    .get(&name.caseless(db))
                {
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
                if matches.iter().all(|(p, path, _)| {
                    path.caseless(db) == first_path.caseless(db) && matches!(p, Pou::Function(_))
                }) {
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
    /// An overload set in which no overload accepts the argument types, though
    /// at least one takes the argument count: the whole set, for the error.
    None(Vec<Function<'db>>),
}

/// The types that DISCRIMINATE this function's symbol among its overloads:
/// its parameter signature, plus its return type when a same-name sibling
/// ties on parameters (a RETURN-directed set — params alone would give two
/// functions one symbol). `None` when the name is not overloaded at all.
///
/// This is resolution's knowledge — which declarations share a name and how
/// they differ — exposed so the code generator renders symbols without
/// re-deriving candidate sets itself.
///
/// A plain function, not a salsa query: cross-file lookups stay unmemoized so
/// their dependencies flow to the per-file extraction queries (see the header
/// of `index_graphs.rs`).
pub fn overload_discriminant<'db>(
    db: &'db dyn WorkspaceDataBase,
    f: Function<'db>,
) -> Option<Vec<Type<'db>>> {
    let name = f.name(db);
    let candidates = match function_namespace_path(db, f) {
        Some(path) => namespace_pou_candidates(db, path, name),
        None => pou_candidates(db, name),
    };
    let siblings: Vec<Function<'db>> = candidates
        .into_iter()
        .filter_map(|p| match p {
            Pou::Function(other) => Some(other),
            _ => None,
        })
        .collect();
    if siblings.len() <= 1 {
        return None;
    }
    let sig = function_signature(db, f);
    let params_tied = siblings
        .iter()
        .any(|other| *other != f && function_signature(db, *other).params == sig.params);
    let mut discriminant = sig.params;
    if params_tied && let Some(ret) = sig.ret {
        discriminant.push(ret);
    }
    Some(discriminant)
}

/// Every FUNCTION the call could name: the declarations `name` resolves to,
/// in the namespace the callee was found in. This is what [`select_overload`]
/// picks from, and what signature help lists so an overloaded name shows
/// every candidate rather than whichever one resolution landed on.
pub fn overload_set<'db>(
    db: &'db dyn WorkspaceDataBase,
    first: Function<'db>,
) -> Vec<Function<'db>> {
    let name = first.name(db);
    let candidates = match function_namespace_path(db, first) {
        Some(path) => namespace_pou_candidates(db, path, name),
        None => pou_candidates(db, name),
    };
    let mut functions: Vec<Function<'db>> = candidates
        .into_iter()
        .filter_map(|p| match p {
            Pou::Function(f) => Some(f),
            _ => None,
        })
        .collect();
    // A TRUE duplicate (same params, same return) is E0101 at the
    // declaration; the call resolves against the surviving first as if the
    // twin did not exist — one error, not ambiguity noise on every call.
    let mut seen: Vec<crate::hir_ty::head::signature::FunctionSignature<'db>> = Vec::new();
    functions.retain(|f| {
        let key = function_signature(db, *f);
        if seen.contains(&key) {
            false
        } else {
            seen.push(key);
            true
        }
    });
    functions
}

/// Select the FUNCTION overload whose signature matches `arg_types`.
///
/// Ordinary name resolution binds a bare function name to the *first* same-name
/// FUNCTION in scope. When that name is an overload set, this re-selects by
/// matching the call's argument types against each overload's params
/// ([`function_signature`]). The selection lives here — in the POU-finding
/// module — so call resolution only supplies the arg types and stays unaware
/// that overloading exists.
///
/// Ranking (never guesses): an argument matches a parameter *exactly* (same
/// type / literal-of-default-type) or by *widening* (implicit cast). An
/// overload that's exact on every argument wins outright. Among the rest, a
/// candidate that is at least as good on EVERY argument and strictly better on
/// one DOMINATES — `(REAL, REAL)` beats `(LREAL, LREAL)` for `(REAL, INT)`
/// arguments, since exact beats widened on the first and they tie on the
/// second. Incomparable candidates — each better somewhere, as with
/// `f(INT, REAL)` vs `f(REAL, INT)` on two widening arguments — stay
/// [`OverloadPick::Ambiguous`]: dominance never picks by majority. None
/// viable ⇒ [`OverloadPick::None`] when some candidate took the argument
/// count, since the TYPES are what failed; the first-match used to stand in
/// and report its own parameter mismatch, naming a type nobody wrote. When
/// no candidate takes the count either, the first-match stays so the arity
/// error surfaces.
pub fn select_overload<'db>(
    db: &'db dyn WorkspaceDataBase,
    callable: CallableType<'db>,
    arg_types: &[Type<'db>],
    // The type the call's VALUE lands in, when the consuming site knows it —
    // an assignment's target, an initializer's declared type. What a
    // RETURN-directed overload set (same params, different returns) is
    // picked by; `None` leaves such a set ambiguous (E0237).
    expected: Option<Type<'db>>,
) -> OverloadPick<'db> {
    let CallableType::Function(first) = callable else {
        return OverloadPick::One(callable);
    };

    let functions = overload_set(db, first);
    if functions.len() <= 1 {
        // Not an overload set — nothing to pick.
        return OverloadPick::One(callable);
    }

    let mut exact: Vec<Function<'db>> = Vec::new();
    let mut viable: Vec<(Function<'db>, Vec<ArgMatch>)> = Vec::new();
    let mut takes_the_count = false;
    let set = functions.clone();
    for f in functions {
        let sig = function_signature(db, f);
        // Viable arg counts: at least the required params, at most all of them
        // (trailing defaulted params may be omitted).
        if arg_types.len() < function_required_arity(db, f) || arg_types.len() > sig.params.len() {
            continue;
        }
        takes_the_count = true;
        let mut matches = Vec::with_capacity(arg_types.len());
        let mut ok = true;
        for (arg, param) in arg_types.iter().zip(sig.params.iter()) {
            match classify_arg(db, *arg, *param) {
                ArgMatch::No => {
                    ok = false;
                    break;
                }
                m => matches.push(m),
            }
        }
        if ok {
            if matches.iter().all(|m| matches!(m, ArgMatch::Exact)) {
                exact.push(f);
            }
            viable.push((f, matches));
        }
    }

    // An all-exact match wins when it is UNIQUE. Two distinct signatures
    // cannot both exactly equal a non-empty argument tuple, but the EMPTY
    // tuple is all-exact against every fully-defaulted candidate — this used
    // to be a single `Option` slot each candidate overwrote, so the last one
    // in discovery order silently won a zero-arg call.
    match exact.len() {
        1 => return OverloadPick::One(CallableType::Function(exact[0])),
        0 => {}
        _ => return pick_by_arity_then_return(db, exact, arg_types.len(), expected),
    }

    // Dominance: drop every candidate that another candidate beats — at least
    // as good on every argument, strictly better on one. What survives is the
    // set of candidates no one is uniformly better than; only a singleton is
    // an answer, anything else is genuinely incomparable.
    let dominated = |a: &[ArgMatch], b: &[ArgMatch]| -> bool {
        // `b` dominates `a`
        a.len() == b.len()
            && a.iter().zip(b).all(|(x, y)| y.at_least(x))
            && a.iter().zip(b).any(|(x, y)| y.better(x))
    };
    let undominated: Vec<&(Function<'db>, Vec<ArgMatch>)> = viable
        .iter()
        .filter(|(_, m)| !viable.iter().any(|(_, other)| dominated(m, other)))
        .collect();

    // An argument that already failed to type (`Type::Never`) has been
    // reported where it failed; naming it `{unknown}` in a second error
    // would be the cascade the contract forbids.
    let all_typed = !arg_types.iter().any(|t| t.is_never());
    match undominated.len() {
        0 if takes_the_count && all_typed => OverloadPick::None(set),
        0 => OverloadPick::One(callable),
        1 => OverloadPick::One(CallableType::Function(undominated[0].0)),
        _ => pick_by_arity_then_return(
            db,
            undominated.into_iter().map(|(f, _)| *f).collect(),
            arg_types.len(),
            expected,
        ),
    }
}

/// Break a tie by ARITY, then by RETURN. The whole preference rule, in one
/// place:
///
/// a candidate requiring NO defaults dominates one that pads with them —
/// `add(10)` picks `add(a)` over `add(a, b := 5)`; among candidates that all
/// pad, nothing breaks the tie (two one-default candidates stay ambiguous,
/// like the zero-argument set). What arity cannot settle, the RETURN type
/// does, when the consuming site expects exactly one candidate's return.
/// What neither settles is E0237, never a silent pick.
fn pick_by_arity_then_return<'db>(
    db: &'db dyn WorkspaceDataBase,
    tie: Vec<Function<'db>>,
    arg_count: usize,
    expected: Option<Type<'db>>,
) -> OverloadPick<'db> {
    let full_arity: Vec<Function<'db>> = tie
        .iter()
        .filter(|f| function_signature(db, **f).params.len() == arg_count)
        .copied()
        .collect();
    match full_arity.len() {
        1 => OverloadPick::One(CallableType::Function(full_arity[0])),
        0 => pick_by_return(db, tie, expected),
        _ => pick_by_return(db, full_arity, expected),
    }
}

/// Break a tie among argument-equivalent candidates by RETURN type: the one
/// whose return equals what the site expects, when exactly one does.
fn pick_by_return<'db>(
    db: &'db dyn WorkspaceDataBase,
    tie: Vec<Function<'db>>,
    expected: Option<Type<'db>>,
) -> OverloadPick<'db> {
    if let Some(expected) = expected {
        // Assigning to a function's own NAME targets its return slot — the
        // same peel `set_target_type` and the coercion record apply.
        let expected = match expected {
            Type::Function(_) | Type::MethodDecl(_) => {
                expected.with_return_type(db).unwrap_or(expected)
            }
            other => other,
        };
        let expected = expected.normalize(db);
        let by_return: Vec<Function<'db>> = tie
            .iter()
            .filter(|f| {
                function_signature(db, **f)
                    .ret
                    .is_some_and(|r| r == expected)
            })
            .copied()
            .collect();
        if by_return.len() == 1 {
            return OverloadPick::One(CallableType::Function(by_return[0]));
        }
    }
    OverloadPick::Ambiguous(tie)
}

enum ArgMatch {
    Exact,
    Widen,
    No,
}

impl ArgMatch {
    /// `self` is at least as good a match as `other`.
    fn at_least(&self, other: &ArgMatch) -> bool {
        matches!(self, ArgMatch::Exact) || matches!(other, ArgMatch::Widen | ArgMatch::No)
    }

    /// `self` is strictly better than `other`.
    fn better(&self, other: &ArgMatch) -> bool {
        matches!(self, ArgMatch::Exact) && !matches!(other, ArgMatch::Exact)
    }
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

/// The namespace path enclosing `scope_id`, or `None` at top level. The one
/// climb: overload gathering, symbol naming and display all ask this, and
/// each had its own copy of the walk.
pub fn enclosing_namespace_path<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope_id: crate::hir_def::scope::ScopeId<'db>,
) -> Option<NamespacePath> {
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

fn function_namespace_path<'db>(
    db: &'db dyn WorkspaceDataBase,
    f: Function<'db>,
) -> Option<NamespacePath> {
    enclosing_namespace_path(db, f.scope_id(db))
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
