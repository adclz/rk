use db::WorkspaceDataBase;

use crate::{
    hir_def::{
        expressions::spec::{ElementarySpec, Spec, SpecKind},
        interned::{
            identifier::Ident,
            namespace::NamespaceAccess,
        },
        pous::{class::MethodDecl, generics::GenericParam, pou::Pou},
        scope::{ScopeId, ScopeKind},
        semantic_index::{get_scope, semantic_index},
    },
    hir_ty::{
        index_graphs::{namespace_index, pou_index},
        ty::Type,
    },
};

/// Result of resolving a name in a scope.
#[derive(Debug, Clone, Copy)]
pub enum NameResolution<'db> {
    /// Resolved to a generic type parameter
    Generic(GenericParam<'db>),
    /// Resolved to a POU (function, function block, class, etc.)
    Pou(Pou<'db>),
    /// Resolved to the method's own name (self-reference)
    MethodSelf(MethodDecl<'db>),
    /// Not found
    NotFound,
}

/// Single entry point for resolving a name ([`NamespaceAccess`]) to its declaration.
///
/// Resolution order (first match wins):
/// 1. Method self-reference (when inside a [`ScopeKind::MethodDecl`])
/// 2. Generic type parameters (from [`ScopeId::generics`])
/// 3. POU via namespace access (local scope → parent/USING → global)
///
/// Note: only methods need explicit self-reference handling (step 1) because
/// [`MethodDecl`]s are not registered in any scope's `local_pous` — they live in
/// `declared_methods`. POUs (functions, FBs, etc.) self-resolve naturally through
/// step 3, since they are declared in their parent scope's `local_pous`.
///
/// This function is used by both head-level (spec) and body-level (path expr) resolution.
pub fn resolve_name<'db>(
    db: &'db dyn WorkspaceDataBase,
    access: &NamespaceAccess<'db>,
    scope: ScopeId<'db>,
) -> NameResolution<'db> {
    // Namespace-qualified names skip directly to namespace lookup
    if access.namespace.is_some() {
        return match resolve_namespace_access(db, access) {
            Some(pou) => NameResolution::Pou(pou),
            None => NameResolution::NotFound,
        };
    }

    let name = access.target.ident;

    // 1. Method self-reference
    if let ScopeKind::MethodDecl(method) = get_scope(db, scope).kind {
        if name == method.name(db) {
            return NameResolution::MethodSelf(method);
        }
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
        Some(pou) => NameResolution::Pou(pou),
        None => NameResolution::NotFound,
    }
}

/// Resolve a namespace access to a POU declaration.
#[tracing::instrument(skip_all)]
pub(crate) fn resolve_namespace_access<'db>(
    db: &'db dyn WorkspaceDataBase,
    access: &NamespaceAccess<'db>,
) -> Option<Pou<'db>> {
    let target = &access.target;

    match &access.namespace {
        // There's a namespace specified, so we look for it
        Some(path) => namespace_index(db, **path)
            .iter()
            .find_map(|ns| pou_names_res(db, target.ident, ns.scope_id(db))),
        // None, look for the POU in the current scope
        None => pou_names_res(db, target.ident, target.scope_id),
    }
}

pub fn pou_names_res<'db>(
    db: &'db dyn WorkspaceDataBase,
    name: Ident,
    scope: ScopeId<'db>,
) -> Option<Pou<'db>> {
    // Checks for POUs declared in the current scope
    scope
        .def_map(db)
        .local_pous
        .get(&name)
        .copied()
        .or_else(|| {
            // Checks for parent POUs and those imported via USING directives
            find_in_parent_pous(db, name, scope).or_else(|| pou_index(db, name))
        })
}

#[tracing::instrument(skip_all)]
pub fn find_in_parent_pous<'db>(
    db: &'db dyn WorkspaceDataBase,
    name: Ident,
    scope: ScopeId<'db>,
) -> Option<Pou<'db>> {
    let it = semantic_index(db, scope.file(db)).scope_iterator(db, scope);
    for scope in it {
        // Find POUs in all shared namespaces
        if let ScopeKind::Namespace(ns) = scope.kind {
            for ns in namespace_index(db, *ns.path(db)).iter() {
                if let Some(p) = ns.scope_id(db).def_map(db).local_pous.get(&name) {
                    return Some(*p);
                }
            }
        }

        // Find POUs in all USING directives
        for using in &scope.usings {
            for ns in namespace_index(db, *using.path(db)).iter() {
                if let Some(p) = ns.scope_id(db).def_map(db).local_pous.get(&name) {
                    return Some(*p);
                }
            }
        }
    }

    None
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
                NameResolution::MethodSelf(m) => Type::MethodDecl(m.into()),
                NameResolution::NotFound => Type::Never,
            },
        }
    }
}
