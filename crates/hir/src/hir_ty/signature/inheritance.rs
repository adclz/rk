use crate::{
    AstId, HasModifiers, HasName, HasVisibility, HirNodeInfo, Modifier, Visibility,
    hir_def::{
        expressions::spec::Spec,
        interned::{identifier::Ident, namespace::{NamespaceAccess, SpanNamespaceAccess}},
        pous::{class::MethodDecl, interface::MethodPrototype, pou::Pou, variable::VariableDecl},
        scope::ScopeId,
    },
    hir_ty::{name_res::resolve_namespace_access, ty::Type},
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
            MethodRef::Prototype(p) => Visibility::default(),
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

    pub type_of_namespace_accesses: FxHashMap<NamespaceAccess<'db>, Type<'db>>,

    pub unresolved: Vec<SpanNamespaceAccess<'db>>,
}

impl<'db> InheritedMethodSet<'db> {
    fn new(
        db: &'db dyn WorkspaceDataBase,
        methods: FxHashMap<Ident, InheritedMethod<'db>>,
        duplicates: Vec<(InheritedMethod<'db>, InheritedMethod<'db>)>,
        type_of_namespace_accesses: FxHashMap<NamespaceAccess<'db>, Type<'db>>,
        unresolved: Vec<SpanNamespaceAccess<'db>>,
    ) -> Self {
        InheritedMethodSet {
            methods,
            duplicates,
            type_of_namespace_accesses,
            unresolved,
        }
    }
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

fn inherit_result<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: Pou<'db>,
) -> InheritedMethodSet<'db> {
    InheritedMethodSet::default()
}

#[salsa::tracked(returns(ref), cycle_result = inherit_result)]
pub fn inherited_methods<'db>(
    db: &'db dyn WorkspaceDataBase,
    pou: Pou<'db>,
) -> InheritedMethodSet<'db> {
    let mut methods = FxHashMap::default();
    let mut duplicates = vec![];
    let mut type_of_namespace_accesses = FxHashMap::default();
    let mut unresolved = vec![];

    let mut inherit_from = |access: NamespaceAccess<'db>, src: Pou<'db>| {
        type_of_namespace_accesses.insert(access.clone(), Type::new_pou(db, src));
        for method in src.get_scope_id(db).def_map(db).declared_methods.iter() {
            let m = InheritedMethod::new(src, *method.1);
            if let Some(dup) = methods.insert(*method.0, m) {
                duplicates.push((dup, m));
            }
        }
    };

    match pou {
        Pou::Class(class) => {
            if let Some(base) = class.extends(db) {

                debug_assert!(base.scope_id == pou.get_scope_id(db));
                debug_assert!(base.path.target.scope_id == pou.get_scope_id(db));

                match resolve_namespace_access(db, &base.path) {
                    Some(pou) => {
                        inherit_from(base.path.clone(), pou);
                    }
                    _ => unresolved.push(base.clone()),
                }
            }
            for iface in class.implements(db) {

                debug_assert!(iface.scope_id == pou.get_scope_id(db));
                debug_assert!(iface.path.target.scope_id == pou.get_scope_id(db));

                match resolve_namespace_access(db, &iface.path) {
                    Some(pou) if matches!(pou, Pou::Interface(_)) => {
                        inherit_from(iface.path.clone(), pou);
                    }
                    _ => unresolved.push(iface.clone()),
                }
            }
        }

        Pou::Interface(iface) => {
            if let Some(extends) = iface.extends(db) {
                for iface in extends {

                    debug_assert!(iface.scope_id == pou.get_scope_id(db));
                    debug_assert!(iface.path.target.scope_id == pou.get_scope_id(db));

                    match resolve_namespace_access(db, &iface.path) {
                        Some(pou) => {
                            inherit_from(iface.path.clone(), pou);
                        }
                        _ => unresolved.push(iface.clone()),
                    }
                }
            }
        }

        Pou::FunctionBlock(fb) => {
            if let Some(base) = fb.extends(db) {

                debug_assert!(base.scope_id == pou.get_scope_id(db));
                debug_assert!(base.path.target.scope_id == pou.get_scope_id(db));

                match resolve_namespace_access(db, &base.path) {
                    Some(pou) => {
                        inherit_from(base.path.clone(), pou);
                    }
                    _ => unresolved.push(base.clone()),
                }
            }

            for iface in fb.implements(db) {

                debug_assert!(iface.scope_id == pou.get_scope_id(db));
                debug_assert!(iface.path.target.scope_id == pou.get_scope_id(db));

                match resolve_namespace_access(db, &iface.path) {
                    Some(pou) if matches!(pou, Pou::Interface(_)) => {
                        inherit_from(iface.path.clone(), pou);
                    }
                    _ => {
                        unresolved.push(iface.clone());
                    }
                }
            }
        }

        _ => {}
    }

    InheritedMethodSet::new(db, methods, duplicates, type_of_namespace_accesses, unresolved)
}
