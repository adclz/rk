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
        head::signature::function_signature,
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
/// 2. In a configuration, a PROGRAM (programs are not visible to other POUs)
/// 3. POU via namespace access (local scope → parent/USING → global)
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

    let name = access.target.ident(db);

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

    // 2. In a configuration, a PROGRAM before a POU of its name. The two
    //    are refused together (E0102), and a program instance still names
    //    the program.
    if let Some(prog) = program_index(db, name)
        && is_config_scope(db, scope)
    {
        return NameResolution::Program(prog);
    }

    // 3. POU resolution (local → parent/USING → global)
    match resolve_namespace_access(db, access) {
        PouResolution::Found(pou, using) => NameResolution::Pou(pou, using),
        PouResolution::Ambiguous(candidates) => NameResolution::Ambiguous(candidates),
        PouResolution::NotFound => NameResolution::NotFound,
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
                path.path(db),
            );
            for ns in namespace_index(db, path).iter() {
                if let PouResolution::Found(pou, using) =
                    pou_names_res(db, target.ident(db), ns.scope_id(db))
                {
                    return PouResolution::Found(pou, using);
                }
            }
            PouResolution::NotFound
        }
        // Unqualified: resolve via scope chain, USING can be ambiguous
        None => pou_names_res(db, target.ident(db), target.scope_id),
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
            for ns in namespace_index(db, ns.path(db)).iter() {
                if let Some(p) = ns.scope_id(db).def_map(db).local_pous.get(&name) {
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
                using.path(db).path(db),
            );
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
                // Identical signatures are E0102 duplicates, equally-viable
                // calls E0809. Matches from DIFFERENT paths, or involving
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
    /// Several overloads fit the arguments alike (E0809); the call site says
    /// why, since what would pick one depends on it.
    Ambiguous(Vec<Function<'db>>),
    /// An overload set in which no overload accepts the argument types, though
    /// at least one binds the arguments: the whole set, for the error.
    None(Vec<Function<'db>>),
    /// An argument failed to resolve, which was reported where it failed: an
    /// overload set picks nothing on it, and reports nothing more.
    Unresolved,
}

/// How a call's arguments fit one overload, as the call site binds them.
pub enum CandidateFit {
    /// The arguments do not bind: a name it does not declare, too many or too
    /// few of them.
    Unbound,
    /// They bind, and an argument's type does not fit its parameter.
    Mismatch,
    /// Each argument's match, in call order, and whether an omitted input was
    /// filled with its default.
    Fits { args: Vec<ArgMatch>, padded: bool },
}

/// What tells an overloaded FUNCTION's symbol apart from its siblings'.
/// The types are as DECLARED, so an alias keeps the name that distinguishes
/// it; the tie that decides `ret` is judged on normalized ones, as
/// resolution judges it.
pub struct OverloadDiscriminant<'db> {
    /// The `VAR_INPUT` and `VAR_IN_OUT` types, in order.
    pub params: Vec<Type<'db>>,
    /// The return type, only when a sibling ties on parameters (a
    /// RETURN-directed set): kept apart from `params`, since `f(INT) : INT`
    /// and `f(INT, INT)` would otherwise read the same.
    pub ret: Option<Type<'db>>,
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
) -> Option<OverloadDiscriminant<'db>> {
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
        .any(|other| *other != f && function_signature(db, *other).same_params(db, &sig));
    let (params, ret) = crate::hir_ty::head::signature::declared_types(db, f);
    Some(OverloadDiscriminant {
        params,
        ret: ret.filter(|_| params_tied),
    })
}

/// Select the FUNCTION overload a call's arguments fit.
///
/// Ordinary name resolution binds a bare function name to the *first* same-name
/// FUNCTION in scope. When that name is an overload set, this re-selects among
/// its members. How the arguments fit each one is the call site's to say
/// (`fit`): it binds them as it would bind a plain call, by name, with
/// defaults and variadics, and matches each against the parameter it lands
/// on. The ranking lives here — in the POU-finding module — so call
/// resolution stays unaware of how overloads compete.
///
/// Ranking (never guesses): an argument matches its parameter *exactly* (same
/// type / literal-of-default-type) or by *widening* (implicit cast). An
/// overload that's exact on every argument wins outright. Among the rest, a
/// candidate that is at least as good on EVERY argument and strictly better on
/// one DOMINATES — `(REAL, REAL)` beats `(LREAL, LREAL)` for `(REAL, INT)`
/// arguments, since exact beats widened on the first and they tie on the
/// second. Incomparable candidates — each better somewhere, as with
/// `f(INT, REAL)` vs `f(REAL, INT)` on two widening arguments — stay
/// [`OverloadPick::Ambiguous`]: dominance never picks by majority. None
/// viable ⇒ [`OverloadPick::None`] when some candidate binds the arguments,
/// since the TYPES are what failed; the first-match used to stand in and
/// report its own parameter mismatch, naming a type nobody wrote. When no
/// candidate binds them either, the first-match stays so the binding error
/// surfaces.
pub fn select_overload<'db>(
    db: &'db dyn WorkspaceDataBase,
    callable: CallableType<'db>,
    // The type the call's VALUE lands in, when the consuming site knows it —
    // an assignment's target, an initializer's declared type. What a
    // RETURN-directed overload set (same params, different returns) is
    // picked by; `None` leaves such a set ambiguous (E0809).
    expected: Option<Type<'db>>,
    // An argument that already failed to type (`Type::Never`).
    unresolved_argument: bool,
    mut fit: impl FnMut(Function<'db>) -> CandidateFit,
) -> OverloadPick<'db> {
    let CallableType::Function(first) = callable else {
        return OverloadPick::One(callable);
    };

    let functions = crate::hir_ty::head::signature::overload_set(db, first);
    if functions.len() <= 1 {
        // Not an overload set — nothing to pick.
        return OverloadPick::One(callable);
    }
    // It matches every parameter, so it would pick whichever one the other
    // arguments leave; it was reported where it failed.
    if unresolved_argument {
        return OverloadPick::Unresolved;
    }

    let mut exact: Vec<(Function<'db>, bool)> = Vec::new();
    let mut viable: Vec<(Function<'db>, Vec<ArgMatch>, bool)> = Vec::new();
    let mut binds = false;
    for f in functions.iter().copied() {
        match fit(f) {
            CandidateFit::Unbound => {}
            CandidateFit::Mismatch => binds = true,
            CandidateFit::Fits { args, padded } => {
                binds = true;
                if args.iter().all(|m| matches!(m, ArgMatch::Exact)) {
                    exact.push((f, padded));
                }
                viable.push((f, args, padded));
            }
        }
    }

    // An all-exact match wins when it is UNIQUE. Two distinct signatures
    // cannot both exactly equal a non-empty argument tuple, but the EMPTY
    // tuple is all-exact against every fully-defaulted candidate — this used
    // to be a single `Option` slot each candidate overwrote, so the last one
    // in discovery order silently won a zero-arg call.
    match exact.len() {
        1 => return OverloadPick::One(CallableType::Function(exact[0].0)),
        0 => {}
        _ => return pick_by_defaults_then_return(db, exact, expected),
    }

    // Dominance: drop every candidate that another candidate beats — at least
    // as good on every argument, strictly better on one. What survives is the
    // set of candidates no one is uniformly better than; only a singleton is
    // an answer, anything else is genuinely incomparable. The arguments are
    // the call's own, in its order, so each position compares one argument.
    let dominated = |a: &[ArgMatch], b: &[ArgMatch]| -> bool {
        // `b` dominates `a`
        a.len() == b.len()
            && a.iter().zip(b).all(|(x, y)| y.at_least(x))
            && a.iter().zip(b).any(|(x, y)| y.better(x))
    };
    let undominated: Vec<(Function<'db>, bool)> = viable
        .iter()
        .filter(|(_, m, _)| !viable.iter().any(|(_, other, _)| dominated(m, other)))
        .map(|(f, _, padded)| (*f, *padded))
        .collect();

    match undominated.len() {
        0 if binds => OverloadPick::None(functions),
        0 => OverloadPick::One(callable),
        1 => OverloadPick::One(CallableType::Function(undominated[0].0)),
        _ => pick_by_defaults_then_return(db, undominated, expected),
    }
}

/// Break a tie by DEFAULTS, then by RETURN. The whole preference rule, in one
/// place:
///
/// a candidate the call fills without defaults dominates one that pads with
/// them — `add(10)` picks `add(a)` over `add(a, b := 5)`; among candidates
/// that all pad, nothing breaks the tie (two one-default candidates stay
/// ambiguous, like the zero-argument set). What defaults cannot settle, the
/// RETURN type does, when the consuming site expects exactly one candidate's
/// return. What neither settles is E0809, never a silent pick.
fn pick_by_defaults_then_return<'db>(
    db: &'db dyn WorkspaceDataBase,
    tie: Vec<(Function<'db>, bool)>,
    expected: Option<Type<'db>>,
) -> OverloadPick<'db> {
    let unpadded: Vec<Function<'db>> = tie
        .iter()
        .filter(|(_, padded)| !padded)
        .map(|(f, _)| *f)
        .collect();
    match unpadded.len() {
        1 => OverloadPick::One(CallableType::Function(unpadded[0])),
        0 => pick_by_return(db, tie.into_iter().map(|(f, _)| f).collect(), expected),
        _ => pick_by_return(db, unpadded, expected),
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

/// How one argument matches the parameter it is bound to.
pub enum ArgMatch {
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

/// How a value of type `arg` matches an input parameter of type `param`, by
/// the rule a plain call checks it with: the coercion, with the argument's
/// adjustments (`REF(x)` is a reference to `x`), so what selection accepts
/// is what the call then accepts. The same type, compared structurally (two
/// `REF_TO INT` written apart are one type), is exact; anything else the
/// coercion takes is a widening.
pub fn classify_arg<'db>(
    db: &'db dyn WorkspaceDataBase,
    resolver: crate::hir_ty::resolver::Resolver<'db>,
    arg: Type<'db>,
    adjustments: Option<&[crate::hir_ty::body::Adjustment<'db>]>,
    param: Type<'db>,
) -> ArgMatch {
    use crate::hir_ty::body::AdjustmentInfo;
    let arg_n = arg.normalize(db);
    let param_n = param.normalize(db);
    // An untyped literal matches its default type (INT / REAL / STRING)
    // exactly when its value fits it, and ADOPTS any other type its value
    // fits. That is `check_as`, the one the inference table asks once the
    // slot is known, so the overload picked and the coercion that follows
    // cannot disagree. The cast table has no say: it grades conversions
    // between types, and a literal has none yet - `5` is a valid BYTE and
    // `'a'` a valid CHAR, which no INT->BYTE or STRING->CHAR entry says (nor
    // should). `70000` is no INT, so the INT overload is not its exact match.
    if let Type::Infer(it) = arg_n {
        return match param_n {
            Type::Elementary(p) if it.check_as(db, p).is_ok() => match it.to_spec(db) == p {
                true => ArgMatch::Exact,
                false => ArgMatch::Widen,
            },
            _ => ArgMatch::No,
        };
    }
    // The coercion takes resolved types only; a value still partly untyped
    // has no type to compare yet.
    if arg_n.has_infer()
        || param
            .coerce_with_type(db, arg, adjustments, resolver)
            .is_err()
    {
        return ArgMatch::No;
    }
    // A reference binds invariantly: the coercion took it only if the
    // pointee is the parameter's own.
    if adjustments.and_then(|a| a.as_reference()).is_some()
        || crate::hir_ty::infer::coerce::same_type(db, param_n, arg_n)
    {
        return ArgMatch::Exact;
    }
    ArgMatch::Widen
}

/// The namespace path enclosing `scope_id`, or `None` at top level. The one
/// climb: overload gathering, symbol naming and display all ask this, and
/// each had its own copy of the walk.
pub fn enclosing_namespace_path<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope_id: crate::hir_def::scope::ScopeId<'db>,
) -> Option<NamespacePath> {
    enclosing_namespace(db, scope_id).map(|ns| ns.path(db))
}

/// The namespace enclosing `scope_id`, or `None` at top level: its path as
/// written is what a symbol shows.
pub fn enclosing_namespace<'db>(
    db: &'db dyn WorkspaceDataBase,
    scope_id: crate::hir_def::scope::ScopeId<'db>,
) -> Option<crate::hir_def::namespace::NamespaceDecl<'db>> {
    if scope_id.is_global(db) {
        return None;
    }
    for scope in semantic_index(db, scope_id.file(db)).scope_iterator(db, scope_id) {
        if let ScopeKind::Namespace(ns) = scope.kind {
            return Some(ns);
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
