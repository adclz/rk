use auto_lsp::default::db::BaseDatabase;
use indexmap::IndexMap;
use rustc_hash::FxHashMap;

use crate::{
    HasName, hir_def::{
        expressions::{
            expression::{Elementary, Expr, ExprKind, PrimaryExpr},
            spec::{Struct, StructElement},
        },
        interned::identifier::Ident,
        pous::{
            class::MethodDecl,
            pou::{Pou},
            variable::{VariableDecl, VariableKind},
        },
        scope::{ScopeId, ScopeKind},
        semantic_index::get_scope,
    }, hir_ty::{inheritance_solver::MethodRef, name_res::resolve_namespace_access}
};

pub type FxIndexMap<K, V> = IndexMap<K, V, rustc_hash::FxBuildHasher>;

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct LocalDefMap<'db> {
    /// Local POUs accessible in this scope
    pub local_pous: FxHashMap<Ident, Pou<'db>>,
    /// Local variables accessible in this scope (VARIABLES with Input, Output, InOut specifiers)
    ///
    /// We use [`IndexMap`] here to preserve the order of declaration
    pub local_variables: FxIndexMap<Ident, VariableDecl<'db>>,
    /// Global variables accessible in this scope (all VARIABLES)
    pub global_variables: FxHashMap<Ident, VariableDecl<'db>>,
    /// Methods declared in this scope (for CLASSes, INTERFACEs, FUNCTION BLOCKs)
    pub declared_methods: FxHashMap<Ident, MethodRef<'db>>,
}

#[salsa::tracked]
impl<'db> ScopeId<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn def_map(self, db: &'db dyn BaseDatabase) -> LocalDefMap<'db> {
        LocalDefMap {
            local_pous: self.local_pous(db).clone(),
            local_variables: self.local_variables(db),
            global_variables: self.global_variables(db),
            declared_methods: self.declared_methods(db),
        }
    }

    fn local_pous(&self, db: &'db dyn BaseDatabase) -> FxHashMap<Ident, Pou<'db>> {
        match get_scope(db, *self).kind {
            ScopeKind::Namespace(ns) => {
                let mut result = FxHashMap::default();
                ns.pous(db).iter().for_each(|pou| {
                    result.insert(pou.get_name_ident(db), *pou);
                });
                result
            }
            _ => FxHashMap::default(),
        }
    }

    pub fn can_have_local_variables(&self, db: &'db dyn BaseDatabase) -> bool {
        match get_scope(db, *self).kind {
            ScopeKind::Global | ScopeKind::Namespace(_) => false,
            ScopeKind::Pou(pou) => match pou {
                Pou::Function(_) | Pou::FunctionBlock(_) | Pou::Class(_) => true,
                _ => false,
            },
            ScopeKind::MethodDecl(m) => true,
        }
    }

    fn local_variables(&self, db: &'db dyn BaseDatabase) -> FxIndexMap<Ident, VariableDecl<'db>> {
        match get_scope(db, *self).kind {
            ScopeKind::Global | ScopeKind::Namespace(_) => IndexMap::default(),
            ScopeKind::Pou(pou) => match pou {
                Pou::Function(f) => local_variables(db, f.variables(db)),
                Pou::FunctionBlock(fb) => local_variables(db, fb.variables(db)),
                Pou::Class(cl) => local_variables(db, cl.variables(db)),
                _ => IndexMap::default(),
            },
            ScopeKind::MethodDecl(m) => local_variables(db, m.variables(db)),
        }
    }

    fn global_variables(&self, db: &'db dyn BaseDatabase) -> FxHashMap<Ident, VariableDecl<'db>> {
        match get_scope(db, *self).kind {
            ScopeKind::Global | ScopeKind::Namespace(_) => FxHashMap::default(),
            ScopeKind::Pou(pou) => match pou {
                Pou::Function(f) => global_variables(db, f.variables(db)),
                Pou::FunctionBlock(fb) => global_variables(db, fb.variables(db)),
                Pou::Class(cl) => global_variables(db, cl.variables(db)),
                _ => FxHashMap::default(),
            },
            ScopeKind::MethodDecl(m) => global_variables(db, m.variables(db)),
        }
    }

    fn declared_methods(&self, db: &'db dyn BaseDatabase) -> FxHashMap<Ident, MethodRef<'db>> {
        match get_scope(db, *self).kind {
            ScopeKind::Global | ScopeKind::Namespace(_) | ScopeKind::MethodDecl(_) => {
                FxHashMap::default()
            }
            ScopeKind::Pou(pou) => match pou {
                Pou::Class(class) => class
                    .methods(db)
                    .iter()
                    .map(|m| (m.get_name_ident(db), m.into()))
                    .collect(),
                Pou::Interface(interface) => interface
                    .methods(db)
                    .iter()
                    .map(|m| (m.get_name_ident(db), m.into()))
                    .collect(),
                Pou::FunctionBlock(fb) => fb
                    .methods(db)
                    .iter()
                    .map(|m| (m.get_name_ident(db), m.into()))
                    .collect(),
                _ => Default::default(),
            },
        }
    }

    #[salsa::tracked(returns(ref))]
    pub fn inheritors(self, db: &'db dyn BaseDatabase) -> Vec<Pou<'db>> {
        match get_scope(db, self).kind {
            ScopeKind::Pou(pou) => match pou {
                Pou::Class(class) => {
                    let mut inheritors = vec![];
                    if let Some(base) = class.extends(db)
                        && let Some(base) = resolve_namespace_access(db, &base.path)
                    {
                        inheritors.push(base);
                    }
                    for iface in class.implements(db) {
                        if let Some(iface) = resolve_namespace_access(db, &iface.path) {
                            inheritors.push(iface);
                        }
                    }
                    inheritors
                }
                Pou::Interface(interface) => {
                    let mut inheritors = vec![];
                    if let Some(extends) = interface.extends(db) {
                        for iface in extends {
                            if let Some(iface) = resolve_namespace_access(db, &iface.path) {
                                inheritors.push(iface);
                            }
                        }
                    }
                    inheritors
                }
                Pou::FunctionBlock(fb) => {
                    let mut inheritors = vec![];
                    if let Some(base) = fb.extends(db)
                        && let Some(base) = resolve_namespace_access(db, &base.path)
                    {
                        inheritors.push(base);
                    }

                    for iface in fb.implements(db) {
                        if let Some(iface) = resolve_namespace_access(db, &iface.path) {
                            inheritors.push(iface);
                        }
                    }
                    inheritors
                }
                _ => vec![],
            },
            _ => vec![],
        }
    }
}

#[salsa::tracked]
impl<'db> Struct<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn resolve_elements(
        self,
        db: &'db dyn BaseDatabase,
    ) -> FxHashMap<Ident, StructElement<'db>> {
        self.elements(db)
            .iter()
            .map(|element| (element.get_name_ident(db), *element))
            .collect()
    }
}

#[salsa::tracked]
impl<'db> Expr<'db> {
    #[salsa::tracked]
    pub fn as_range(self, db: &'db dyn BaseDatabase) -> Option<u64> {
        match self.expr(db) {
            ExprKind::PrimaryExpr(PrimaryExpr::Literal(Elementary::InferInteger(v))) => {
                v.as_u64(db).ok()
            }
            _ => None,
        }
    }
}

fn global_variables<'db>(
    db: &'db dyn BaseDatabase,
    vars: &[VariableDecl<'db>],
) -> FxHashMap<Ident, VariableDecl<'db>> {
    let mut variables = FxHashMap::default();
    for v in vars {
        variables.insert(v.get_name_ident(db), *v);
    }
    variables
}

fn local_variables<'db>(
    db: &'db dyn BaseDatabase,
    vars: &[VariableDecl<'db>],
) -> FxIndexMap<Ident, VariableDecl<'db>> {
    let mut variables = IndexMap::default();
    for v in vars {
        match v.kind(db) {
            VariableKind::Input => {
                variables.insert(v.get_name_ident(db), *v);
            }
            VariableKind::Output => {
                variables.insert(v.get_name_ident(db), *v);
            }
            VariableKind::InOut => {
                variables.insert(v.get_name_ident(db), *v);
            }
            _ => continue,
        };
    }
    variables
}
