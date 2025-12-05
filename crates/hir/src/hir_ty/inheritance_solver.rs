use crate::{
    AstId, HasName, HirNodeInfo, hir_def::{
        expressions::spec::Spec,
        interned::{identifier::Ident, namespace::SpanNamespaceAccess},
        modifier::Modifier,
        pous::{
            class::MethodDecl,
            interface::MethodPrototype,
            pou::{Pou},
            variable::VariableDecl,
        },
        scope::ScopeId,
        visibility::Visibility,
    }, hir_ty::name_res::resolve_namespace_access
};
use auto_lsp::{core::span::Span, default::db::BaseDatabase};
use rustc_hash::FxHashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum MethodRef<'db> {
    Prototype(MethodPrototype<'db>),
    Declared(MethodDecl<'db>),
}

impl<'db> MethodRef<'db> {
    pub fn return_type(&self, db: &'db dyn BaseDatabase) -> Option<&'db Spec<'db>> {
        match self {
            MethodRef::Prototype(p) => p.return_type(db),
            MethodRef::Declared(d) => d.return_type(db),
        }
    }

    pub fn visibility(&self, db: &'db dyn BaseDatabase) -> Visibility {
        match self {
            MethodRef::Prototype(p) => Visibility::PUBLIC,
            MethodRef::Declared(d) => d.visibility(db),
        }
    }

    pub fn modifier(&self, db: &'db dyn BaseDatabase) -> Modifier {
        match self {
            MethodRef::Prototype(p) => Modifier::EMPTY,
            MethodRef::Declared(d) => d.modifier(db),
        }
    }

    pub fn variables(&self, db: &'db dyn BaseDatabase) -> &'db Vec<VariableDecl<'db>> {
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
    fn get_id(&self, db: &'db dyn BaseDatabase) -> AstId {
        match self {
            MethodRef::Prototype(p) => p.get_id(db),
            MethodRef::Declared(d) => d.get_id(db),
        }
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> ScopeId<'db> {
        match self {
            MethodRef::Prototype(p) => p.get_scope_id(db),
            MethodRef::Declared(d) => d.get_scope_id(db),
        }
    }
}

impl<'db> HasName<'db> for MethodRef<'db> {
    fn get_name_ident(&self, db: &'db dyn BaseDatabase) -> Ident {
        match self {
            MethodRef::Prototype(p) => p.get_name_ident(db),
            MethodRef::Declared(d) => d.get_name_ident(db),
        }
    }

    fn get_name_id(&self, db: &'db dyn BaseDatabase) -> AstId {
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

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct InheritedMethodSet<'db> {
    pub methods: FxHashMap<Ident, InheritedMethod<'db>>,

    pub duplicates: Vec<(InheritedMethod<'db>, InheritedMethod<'db>)>,

    pub unresolved: Vec<SpanNamespaceAccess<'db>>
}

impl<'db> InheritedMethodSet<'db> {
    fn new(
        db: &'db dyn BaseDatabase,
        methods: FxHashMap<Ident, InheritedMethod<'db>>,
        duplicates: Vec<(InheritedMethod<'db>, InheritedMethod<'db>)>,
        unresolved: Vec<SpanNamespaceAccess<'db>>,
    ) -> Self {
        InheritedMethodSet {
            methods,
            duplicates,
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

#[salsa::tracked(returns(ref))]
pub fn inherited_methods<'db>(
    db: &'db dyn BaseDatabase,
    pou: Pou<'db>,
) -> InheritedMethodSet<'db> {
    let mut methods = FxHashMap::default();
    let mut duplicates = vec![];
    let mut unresolved = vec![];

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
            if let Some(base) = class.extends(db) {
                debug_assert!(base.scope_id == pou.get_scope_id(db));
                debug_assert!(base.path.target.scope_id == pou.get_scope_id(db));
                match resolve_namespace_access(db, &base.path) {
                    Some(base) => {
                        inherit_from(base);
                    }
                    _ => unresolved.push(base.clone()),
                }
            }
            for iface in class.implements(db) {
                match resolve_namespace_access(db, &iface.path) {
                    Some(iface) if matches!(iface, Pou::Interface(_)) => {
                        inherit_from(iface);
                    }
                    _ => unresolved.push(iface.clone()),
                }
            }
        }

        Pou::Interface(iface) => {
            if let Some(extends) = iface.extends(db) {
                for iface in extends {
                    match resolve_namespace_access(db, &iface.path) {
                        Some(iface) => {
                            inherit_from(iface);
                        }
                        _ => unresolved.push(iface.clone()),
                    }
                }
            }
        }

        Pou::FunctionBlock(fb) => {
            if let Some(base) = fb.extends(db) {
                match resolve_namespace_access(db, &base.path) {
                    Some(base) => {
                        inherit_from(base);
                    }
                    _ => unresolved.push(base.clone()),
                }
            }

            for iface in fb.implements(db) {
                debug_assert!(iface.scope_id == pou.get_scope_id(db));
                debug_assert!(iface.path.target.scope_id == pou.get_scope_id(db));
                match resolve_namespace_access(db, &iface.path) {
                    Some(iface) if matches!(iface, Pou::Interface(_)) => {
                        inherit_from(iface);
                    }
                    _ => {
                        unresolved.push(iface.clone());
                    }
                }
            }
        }

        _ => {}
    }

    InheritedMethodSet::new(db, methods, duplicates, unresolved)
}
