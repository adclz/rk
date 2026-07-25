use crate::{
    AstId, HasModifiers, HasName, HasVisibility, HirNodeInfo, Modifier, Visibility,
    hir_def::{
        expressions::spec::{Spec, SpecKind},
        interned::identifier::Ident,
        pous::{class::MethodDecl, interface::MethodPrototype, pou::Pou, variable::VariableDecl},
        scope::ScopeId,
    },
    hir_ty::resolver::name::resolve_namespace_access,
};
use db::WorkspaceDataBase;
use rustc_hash::FxHashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum MethodRef<'db> {
    Prototype(MethodPrototype<'db>),
    Declared(MethodDecl<'db>),
}

impl<'db> HasModifiers<'db> for MethodRef<'db> {
    fn get_modifiers(&self, db: &'db dyn WorkspaceDataBase) -> Modifier {
        match self {
            MethodRef::Prototype(p) => Modifier::default(),
            MethodRef::Declared(d) => d.modifier(db),
        }
    }
}

impl<'db> HasVisibility<'db> for MethodRef<'db> {
    fn get_visibility(&self, db: &'db dyn WorkspaceDataBase) -> Visibility {
        match self {
            MethodRef::Prototype(_) => Visibility::PUBLIC,
            MethodRef::Declared(d) => d.visibility(db),
        }
    }
}

impl<'db> MethodRef<'db> {
    pub fn return_type(&self, db: &'db dyn WorkspaceDataBase) -> Option<&'db Spec<'db>> {
        match self {
            MethodRef::Prototype(p) => p.return_type(db),
            MethodRef::Declared(d) => d.return_type(db),
        }
    }

    pub fn visibility(&self, db: &'db dyn WorkspaceDataBase) -> Visibility {
        match self {
            MethodRef::Prototype(p) => Visibility::PUBLIC,
            MethodRef::Declared(d) => d.visibility(db),
        }
    }

    pub fn modifier(&self, db: &'db dyn WorkspaceDataBase) -> Modifier {
        match self {
            MethodRef::Prototype(p) => Modifier::EMPTY,
            MethodRef::Declared(d) => d.modifier(db),
        }
    }

    pub fn variables(&self, db: &'db dyn WorkspaceDataBase) -> &'db Vec<VariableDecl<'db>> {
        match self {
            MethodRef::Prototype(p) => p.variables(db),
            MethodRef::Declared(d) => d.variables(db),
        }
    }

    pub fn is_prototype(&self) -> bool {
        matches!(self, MethodRef::Prototype(_))
    }

    pub fn is_declared(&self) -> bool {
        matches!(self, MethodRef::Declared(_))
    }
}

impl<'db> HirNodeInfo<'db> for MethodRef<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        match self {
            MethodRef::Prototype(p) => p.get_id(db),
            MethodRef::Declared(d) => d.get_id(db),
        }
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        match self {
            MethodRef::Prototype(p) => p.get_scope_id(db),
            MethodRef::Declared(d) => d.get_scope_id(db),
        }
    }
}

impl<'db> HasName<'db> for MethodRef<'db> {
    fn get_name_ident(&self, db: &'db dyn WorkspaceDataBase) -> Ident {
        match self {
            MethodRef::Prototype(p) => p.get_name_ident(db),
            MethodRef::Declared(d) => d.get_name_ident(db),
        }
    }

    fn get_name_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        match self {
            MethodRef::Prototype(p) => p.get_name_id(db),
            MethodRef::Declared(d) => d.get_name_id(db),
        }
    }
}

impl<'db> From<MethodPrototype<'db>> for MethodRef<'db> {
    fn from(value: MethodPrototype<'db>) -> Self {
        MethodRef::Prototype(value)
    }
}

impl<'db> From<MethodDecl<'db>> for MethodRef<'db> {
    fn from(value: MethodDecl<'db>) -> Self {
        MethodRef::Declared(value)
    }
}

impl<'db> From<&MethodPrototype<'db>> for MethodRef<'db> {
    fn from(value: &MethodPrototype<'db>) -> Self {
        MethodRef::Prototype(*value)
    }
}

impl<'db> From<&MethodDecl<'db>> for MethodRef<'db> {
    fn from(value: &MethodDecl<'db>) -> Self {
        MethodRef::Declared(*value)
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct InheritedMethodSet<'db> {
    pub methods: FxHashMap<Ident, InheritedMethod<'db>>,

    pub duplicates: Vec<(InheritedMethod<'db>, InheritedMethod<'db>)>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct InheritedMethod<'db> {
    pub source: Pou<'db>,
    pub method: MethodRef<'db>,
}

impl<'db> InheritedMethod<'db> {
    fn new(source: Pou<'db>, method: MethodRef<'db>) -> Self {
        Self { source, method }
    }
}

/// Try to resolve a Spec to a Pou via SpecKind::Target.
fn resolve_spec_to_pou<'db>(db: &'db dyn WorkspaceDataBase, spec: &Spec<'db>) -> Option<Pou<'db>> {
    if let SpecKind::Target(target) = spec.kind(db) {
        resolve_namespace_access(db, &target.path).found()
    } else {
        None
    }
}

#[salsa::tracked(returns(ref))]
pub fn inherited_methods<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: Pou<'db>,
) -> InheritedMethodSet<'db> {
    let mut methods = FxHashMap::default();
    let mut duplicates = vec![];

    let mut inherit_from = |src: Pou<'db>| {
        for method in src.get_scope_id(db).def_map(db).declared_methods.iter() {
            let m = InheritedMethod::new(src, *method.1);
            if let Some(dup) = methods.insert(*method.0, m) {
                duplicates.push((dup, m));
            }
        }
    };

    match pou {
        Pou::Class(class) => {
            if let Some(base) = class.extends(db)
                && let Some(pou) = resolve_spec_to_pou(db, base)
            {
                inherit_from(pou);
            }
            for iface in class.implements(db) {
                if let Some(pou) = resolve_spec_to_pou(db, iface)
                    && matches!(pou, Pou::Interface(_))
                {
                    inherit_from(pou);
                }
            }
        }

        Pou::Interface(iface) => {
            if let Some(extends) = iface.extends(db) {
                for spec in extends {
                    if let Some(pou) = resolve_spec_to_pou(db, spec) {
                        inherit_from(pou);
                    }
                }
            }
        }

        Pou::FunctionBlock(fb) => {
            if let Some(base) = fb.extends(db)
                && let Some(pou) = resolve_spec_to_pou(db, base)
            {
                inherit_from(pou);
            }

            for iface in fb.implements(db) {
                if let Some(pou) = resolve_spec_to_pou(db, iface)
                    && matches!(pou, Pou::Interface(_))
                {
                    inherit_from(pou);
                }
            }
        }

        _ => {}
    }

    InheritedMethodSet {
        methods,
        duplicates,
    }
}

/// One instance member (a field of an FB/CLASS instance), paired with the POU
/// that DECLARES it — which may be a base of the POU being queried.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct InstanceMember<'db> {
    /// The POU this member is declared on. For an inherited member this is a
    /// base, not the queried POU.
    pub owner: Pou<'db>,
    pub var: VariableDecl<'db>,
}

/// Every instance member of a POU, in **layout order**: the base-most POU's
/// members first (the whole `EXTENDS` chain, recursively), each level in
/// declaration order.
///
/// This is the authoritative answer to "what state does an instance of this POU
/// hold?", so consumers never walk `EXTENDS` themselves. MIR in particular must
/// only compute offsets from this list: a derived instance has to be
/// layout-compatible with its base, because an inherited method is compiled
/// once against the base's offsets and then invoked with a derived instance
/// pointer — which the base-first ordering guarantees.
///
/// Sections that are not instance state are excluded here, once, rather than in
/// each consumer:
/// - `VAR_EXTERNAL` references a global; it resolves to the global's address.
/// - `VAR_TEMP` is per-invocation scratch and lives as a body local.
#[salsa::tracked(returns(ref))]
pub fn instance_members<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: Pou<'db>,
) -> Vec<InstanceMember<'db>> {
    let mut out = Vec::new();
    let mut visited = Vec::new();
    collect_instance_members(db, pou, &mut out, &mut visited);
    out
}

fn collect_instance_members<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: Pou<'db>,
    out: &mut Vec<InstanceMember<'db>>,
    visited: &mut Vec<Pou<'db>>,
) {
    // Cyclic EXTENDS is reported separately (E05xx); stop here so resolution
    // terminates regardless.
    if visited.contains(&pou) {
        return;
    }
    visited.push(pou);

    if let Some(base) = base_pou(db, pou) {
        collect_instance_members(db, base, out, visited);
    }

    let vars = match pou {
        Pou::FunctionBlock(fb) => fb.variables(db),
        Pou::Class(class) => class.variables(db),
        _ => return,
    };
    for var in vars {
        use crate::hir_def::pous::variable::VariableKind;
        match var.kind(db) {
            VariableKind::External | VariableKind::Temp => continue,
            _ => {}
        }
        out.push(InstanceMember {
            owner: pou,
            var: *var,
        });
    }
}

/// The POU a FUNCTION_BLOCK or CLASS directly `EXTENDS`, if any.
///
/// The single place `EXTENDS` is followed for base resolution — used by
/// [`instance_members`] and by consumers that need the base itself (`SUPER()`).
pub fn base_pou<'db>(db: &'db dyn WorkspaceDataBase, pou: Pou<'db>) -> Option<Pou<'db>> {
    let spec = match pou {
        Pou::FunctionBlock(fb) => fb.extends(db)?,
        Pou::Class(class) => class.extends(db)?,
        _ => return None,
    };
    resolve_spec_to_pou(db, spec)
}
