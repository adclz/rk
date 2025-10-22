use std::{collections::BTreeMap, sync::Arc};

use crate::{
    AstId, HirNodeInfo,
    hir_def::{
        expressions::spec::Spec,
        interned::{identifier::Ident, namespace::SpanNamespaceAccess},
        modifier::Modifier,
        pous::{
            class::MethodDecl,
            interface::MethodPrototype,
            pou::{Pou, PouDecl},
            variable::VariableDecl,
        },
        scope::FileScopeId,
        visibility::Visibility,
    },
    hir_ty::name_res::resolve_namespace_access,
};
use auto_lsp::{core::span::Span, default::db::BaseDatabase};
use rustc_hash::FxHashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum MethodRef<'db> {
    Prototype(MethodPrototype<'db>),
    Declared(MethodDecl<'db>),
}

impl<'db> MethodRef<'db> {
    pub fn name(&self, db: &'db dyn BaseDatabase) -> &'db Ident {
        match self {
            MethodRef::Prototype(p) => p.name(db),
            MethodRef::Declared(d) => d.name(db),
        }
    }

    pub fn name_span(&self, db: &'db dyn BaseDatabase) -> Span {
        match self {
            MethodRef::Prototype(p) => p.get_name_span(db).unwrap(),
            MethodRef::Declared(d) => d.get_name_span(db).unwrap(),
        }
    }

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
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        match self {
            MethodRef::Prototype(p) => p.get_id(db),
            MethodRef::Declared(d) => d.get_id(db),
        }
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        match self {
            MethodRef::Prototype(p) => p.get_scope_id(db),
            MethodRef::Declared(d) => d.get_scope_id(db),
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

/// Unsure if this should be interned.
#[salsa::interned(debug)]
pub struct InheritedMethodSet<'db> {
    #[returns(ref)]
    pub methods: BTreeMap<Ident, InheritedMethod<'db>>,
    #[returns(ref)]
    pub duplicates: Vec<(InheritedMethod<'db>, InheritedMethod<'db>)>,
    #[returns(ref)]
    pub unresolved: Vec<SpanNamespaceAccess<'db>>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update)]
pub struct InheritedMethod<'db> {
    pub source: PouDecl<'db>,
    pub method: MethodRef<'db>,
}

impl<'db> InheritedMethod<'db> {
    fn new(source: PouDecl<'db>, method: MethodRef<'db>) -> Self {
        Self { source, method }
    }
}

fn inherited_method_initial<'db>(
    db: &'db dyn BaseDatabase,
    pou: PouDecl<'db>,
) -> InheritedMethodSet<'db> {
    InheritedMethodSet::new(db, BTreeMap::new(), vec![], vec![])
}

fn inherited_method_cycle<'db>(
    db: &'db dyn BaseDatabase,
    value: &InheritedMethodSet<'db>,
    count: u32,
    pou: PouDecl<'db>,
) -> salsa::CycleRecoveryAction<InheritedMethodSet<'db>> {
    salsa::CycleRecoveryAction::Iterate
}

#[salsa::tracked(returns(ref))]
pub fn to_method_ref<'db>(db: &'db dyn BaseDatabase, pou: PouDecl<'db>) -> Vec<MethodRef<'db>> {
    match pou.pou(db) {
        Pou::Class(class) => class.methods(db).iter().map(|m| m.into()).collect(),
        Pou::Interface(interface) => interface.methods(db).iter().map(|m| m.into()).collect(),
        Pou::FunctionBlock(fb) => fb.methods(db).iter().map(|m| m.into()).collect(),
        _ => Default::default(),
    }
}

#[salsa::tracked(returns(ref))]
pub fn declared_methods<'db>(
    db: &'db dyn BaseDatabase,
    pou: PouDecl<'db>,
) -> FxHashMap<Ident, MethodRef<'db>> {
    to_method_ref(db, pou)
        .iter()
        .map(|m| (m.name(db).clone(), *m))
        .collect()
}

#[salsa::tracked(cycle_initial = inherited_method_initial, cycle_fn = inherited_method_cycle)]
pub fn inherited_methods<'db>(
    db: &'db dyn BaseDatabase,
    pou: PouDecl<'db>,
) -> InheritedMethodSet<'db> {
    let mut methods = BTreeMap::new();
    let mut duplicates = vec![];
    let mut unresolved = vec![];

    let mut inherit_from = |src: PouDecl<'db>| {
        for method in declared_methods(db, src).iter() {
            let m = InheritedMethod::new(src, *method.1);
            if let Some(dup) = methods.insert(*method.0, m) {
                duplicates.push((dup, m));
            }
        }
    };

    match pou.pou(db) {
        Pou::Class(class) => {
            if let Some(base) = class.extends(db) {
                match resolve_namespace_access(db, base.scope_id, base.path) {
                    Some(base) => {
                        inherit_from(base);
                    }
                    _ => unresolved.push(*base),
                }
            }
            for iface in class.implements(db) {
                match resolve_namespace_access(db, iface.scope_id, iface.path) {
                    Some(iface) => {
                        inherit_from(iface);
                    }
                    _ => unresolved.push(*iface),
                }
            }
        }

        Pou::Interface(iface) => {
            if let Some(extends) = iface.extends(db) {
                for iface in extends {
                    match resolve_namespace_access(db, iface.scope_id, iface.path) {
                        Some(iface) => {
                            inherit_from(iface);
                        }
                        _ => unresolved.push(*iface),
                    }
                }
            }
        }

        Pou::FunctionBlock(fb) => {
            if let Some(base) = fb.extends(db) {
                match resolve_namespace_access(db, base.scope_id, base.path) {
                    Some(base) => {
                        inherit_from(base);
                    }
                    _ => unresolved.push(*base),
                }
            }
        }

        _ => {}
    }

    InheritedMethodSet::new(db, methods, duplicates, unresolved)
}
