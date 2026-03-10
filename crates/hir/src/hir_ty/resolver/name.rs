use db::WorkspaceDataBase;

use crate::{
    HasName,
    hir_def::{
        expressions::spec::{ElementarySpec, Spec, SpecKind},
        interned::{
            identifier::Ident,
            namespace::{NamespaceAccess, NamespacePath},
        },
        pous::{class::MethodDecl, generics::GenericParam, pou::Pou},
        program::ProgramDecl,
        scope::{ScopeId, ScopeKind},
        semantic_index::{get_scope, semantic_index},
    },
    hir_ty::{
        index_graphs::{namespace_index, pou_index, program_index},
        ty::Type,
    },
};

/// Result of POU name resolution, distinguishing unique matches from ambiguities.
#[derive(Debug, Clone)]
pub enum PouResolution<'db> {
    Found(Pou<'db>),
    /// Two or more USING directives at the same scope level import different POUs
    /// with this name.
    Ambiguous(Vec<(Pou<'db>, NamespacePath)>),
    NotFound,
}

impl<'db> PouResolution<'db> {
    /// Extract the POU if uniquely resolved, discarding ambiguities.
    pub fn found(self) -> Option<Pou<'db>> {
        match self {
            Self::Found(pou) => Some(pou),
            _ => None,
        }
    }
}

/// Result of resolving a name in a scope.
#[derive(Debug, Clone)]
pub enum NameResolution<'db> {
    /// Resolved to a generic type parameter
    Generic(GenericParam<'db>),
    /// Resolved to a POU (function, function block, class, etc.)
    Pou(Pou<'db>),
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
/// 2. Generic type parameters (from [`ScopeId::generics`])
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
            PouResolution::Found(pou) => NameResolution::Pou(pou),
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
                return NameResolution::Pou(pou);
            }
        }
        _ => {}
    }

    // 2. Generic parameters (works at both head and body level)
    if let Some(generics) = scope.generics(db) {
        for generic in generics {
            if generic.name(db) == name {
                return NameResolution::Generic(*generic);
            }
        }
    }

    // 3. POU resolution (local → parent/USING → global)
    match resolve_namespace_access(db, access) {
        PouResolution::Found(pou) => NameResolution::Pou(pou),
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
                if let PouResolution::Found(pou) = pou_names_res(db, target.ident, ns.scope_id(db))
                {
                    return PouResolution::Found(pou);
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
        return PouResolution::Found(*pou);
    }

    // Checks for parent POUs and those imported via USING directives
    match find_in_parent_pous(db, name, scope) {
        PouResolution::NotFound => match pou_index(db, name) {
            Some(pou) => PouResolution::Found(pou),
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
                    return PouResolution::Found(*p);
                }
            }
        }

        // Collect ALL USING matches at this scope level
        let mut matches: Vec<(Pou<'db>, NamespacePath)> = vec![];
        for using in &scope.usings {
            let ns_path: NamespacePath = *using.path(db);
            for ns in namespace_index(db, ns_path).iter() {
                if let Some(pou) = ns.scope_id(db).def_map(db).local_pous.get(&name) {
                    // Deduplicate by POU identity (shared namespaces across files)
                    if !matches.iter().any(|(p, _)| p == pou) {
                        matches.push((*pou, ns_path));
                    }
                }
            }
        }

        match matches.len() {
            0 => continue,
            1 => return PouResolution::Found(matches[0].0),
            _ => return PouResolution::Ambiguous(matches),
        }
    }

    PouResolution::NotFound
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
            SpecKind::SizedWString(_) => Type::Elementary(ElementarySpec::WString),
            SpecKind::Ref(ref_to) => Type::RefTo(*ref_to),
            SpecKind::Struct(strukt) => Type::Struct(*strukt),
            SpecKind::Array(arr) => Type::Array(*arr),
            SpecKind::ArrayConformand(a) => Type::ArrayConformand(*a),
            SpecKind::Enum(enm) => Type::Enum(*enm),
            SpecKind::Subrange(sub) => Type::SubRange(*sub),
            SpecKind::Target(t) => match resolve_name(db, &t.path, spec.scope_id(db)) {
                NameResolution::Generic(g) => Type::Generic(g),
                NameResolution::Pou(pou) => Type::new_pou(db, pou),
                NameResolution::Program(p) => Type::Program(p),
                NameResolution::MethodSelf(m) => Type::MethodDecl(m.into()),
                NameResolution::Ambiguous(_) | NameResolution::NotFound => Type::Never,
            },
        }
    }
}
