use std::sync::Arc;

use crate::{
    hir_def::{
        expressions::spec::Spec, interned::identifier::{Ident, SpanIdent}, modifier::Modifier, pous::{
            class::MethodDecl, interface::MethodPrototype, pou::{Pou, PouDecl}, variable::VariableDecl,
        }, scope::FileScopeId, visibility::Visibility
    }, hir_ty::{
        name_res::resolve_namespace_access,
        ty::{Ty, TyKind},
    }, AstId, HirNodeInfo
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

#[derive(Default, Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct Methods<'db> {
    pub inherited_methods: FxHashMap<Ident, InheritedMethod<'db>>,
    pub declared_methods: FxHashMap<Ident, MethodRef<'db>>,

    pub inherited_duplicates: Vec<(InheritedMethod<'db>, InheritedMethod<'db>)>,
    pub declared_duplicates: Vec<(MethodRef<'db>, MethodRef<'db>)>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, salsa::Update)]
pub struct InheritedMethod<'db> {
    pub source: PouDecl<'db>,
    pub method: MethodRef<'db>,
}

impl<'db> InheritedMethod<'db> {
    fn new(source: PouDecl<'db>, method: MethodRef<'db>) -> Self {
        Self { source, method }
    }
}

fn method_initial<'db>(db: &'db dyn BaseDatabase, ty: PouDecl<'db>) -> Arc<Methods<'db>> {
    Arc::new(Methods::default())
}

fn method_cycle<'db>(
    db: &'db dyn BaseDatabase,
    value: &Arc<Methods<'db>>,
    count: u32,
    ty: PouDecl<'db>,
) -> salsa::CycleRecoveryAction<Arc<Methods<'db>>> {
    salsa::CycleRecoveryAction::Iterate
}

#[salsa::tracked(cycle_initial = method_initial, cycle_fn=method_cycle)]
pub fn method_table<'db>(db: &'db dyn BaseDatabase, pou: PouDecl<'db>) -> Arc<Methods<'db>> {
    let mut inherited_methods: std::collections::HashMap<
        Ident,
        InheritedMethod<'_>,
        rustc_hash::FxBuildHasher,
    > = FxHashMap::default();
    let mut declared_methods: std::collections::HashMap<
        Ident,
        MethodRef<'db>,
        rustc_hash::FxBuildHasher,
    > = FxHashMap::default();

    let mut inherited_duplicates: Vec<(InheritedMethod<'_>, InheritedMethod<'_>)> = vec![];
    let mut declared_duplicates: Vec<(MethodRef<'db>, MethodRef<'_>)> = vec![];

    match pou.pou(db) {
        Pou::Class(class) => {
            // Inherit base
            if let Some(base) = class.extends(db)
                && let Some(base) = resolve_namespace_access(db, base.scope_id, base.path)
            {
                for (name, entry) in &method_table(db, base).declared_methods {
                    let method = InheritedMethod::new(base, *entry);
                    if let Some(m) = inherited_methods.insert(*name, method) {
                        inherited_duplicates.push((m, method));
                    }
                }
            }

            // Inherit interfaces (abstract signatures only)
            for iface in class.implements(db) {
                if let Some(iface) = resolve_namespace_access(db, iface.scope_id, iface.path) {
                    for (name, entry) in &method_table(db, iface).declared_methods {
                        let method = InheritedMethod::new(iface, *entry);
                        if let Some(m) = inherited_methods.insert(*name, method) {
                            inherited_duplicates.push((m, method));
                        }
                    }
                }
            }

            // Add this class’s own methods
            for m in class.methods(db) {
                if let Some(dup) = declared_methods.insert(*m.name(db), m.into()) {
                    declared_duplicates.push((dup, m.into()));
                }
            }
        }

        Pou::Interface(interface) => {
            // Methods = abstract signatures
            for m in interface.methods(db) {
                if let Some(dup) = declared_methods.insert(*m.name(db), m.into()) {
                    declared_duplicates.push((dup, m.into()));
                }
            }

            if let Some(iface) = interface.extends(db) {
                for iface in iface {
                    if let Some(iface) = resolve_namespace_access(db, iface.scope_id, iface.path) {
                        for (name, entry) in
                            &method_table(db, iface).declared_methods
                        {
                            let method = InheritedMethod::new(iface, *entry);
                            if let Some(m) = inherited_methods.insert(*name, method) {
                                inherited_duplicates.push((m, method));
                            }
                        }
                    }
                }
            }
        }

        Pou::FunctionBlock(fb) => {
            for m in fb.methods(db) {
                if let Some(dup) = declared_methods.insert(*m.name(db), m.into()) {
                    declared_duplicates.push((dup, m.into()));
                }
            }

            if let Some(base) = fb.extends(db) {
                if let Some(base) = resolve_namespace_access(db, base.scope_id, base.path) {
                    for (name, entry) in &method_table(db, base).declared_methods {
                        let method = InheritedMethod::new(base, *entry);
                        if let Some(m) = inherited_methods.insert(*name, method) {
                            inherited_duplicates.push((m, method));
                        }
                    }
                }
            }
        }
        _ => {}
    }

    Arc::new(Methods {
        inherited_methods,
        declared_methods,
        inherited_duplicates,
        declared_duplicates,
    })
}
