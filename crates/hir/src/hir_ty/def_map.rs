use std::sync::{LazyLock, RwLock};

use auto_lsp::default::db::{BaseDatabase, file::File};
use indexmap::IndexMap;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    hir_def::{
        interned::{identifier::Ident, namespace::NamespacePath},
        namespace::NamespaceDecl,
        pous::{
            pou::{Pou, PouDecl},
            variable::{VariableDecl, VariableKind},
        },
        scope::{ScopeId, ScopeKind},
        semantic_index::{SemanticIndex, get_scope, semantic_index},
    },
    hir_ty::{inheritance_solver::MethodRef, name_res::global_namespace_index},
};

pub type FxIndexMap<K, V> = IndexMap<K, V, rustc_hash::FxBuildHasher>;

#[derive(Debug, Clone, PartialEq, Eq, salsa::Update)]
pub struct LocalDefMap<'db> {
    /// Local POUs accessible in this scope
    pub local_pous: FxHashMap<Ident, PouDecl<'db>>,
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
            local_pous: self.local_pous(db),
            local_variables: self.local_variables(db),
            global_variables: self.global_variables(db),
            declared_methods: self.declared_methods(db),
        }
    }

    fn local_pous(&self, db: &'db dyn BaseDatabase) -> FxHashMap<Ident, PouDecl<'db>> {
        match get_scope(db, *self).kind {
            ScopeKind::Namespace(ns) =>  {
                let mut result = FxHashMap::default();
                ns.pous(db).iter().for_each(|pou| {
                    result.insert(*pou.name(db), *pou);
                });
                result
            }
            _ => FxHashMap::default(),
        }

    }

    fn local_variables(&self, db: &'db dyn BaseDatabase) -> FxIndexMap<Ident, VariableDecl<'db>> { 
        match get_scope(db, *self).kind {
            ScopeKind::Global | ScopeKind::Namespace(_) => IndexMap::default(),
            ScopeKind::Pou(pou) => match pou.pou(db) {
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
            ScopeKind::Pou(pou) => match pou.pou(db) {
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
            ScopeKind::Pou(pou) => match pou.pou(db) {
                Pou::Class(class) => class
                    .methods(db)
                    .iter()
                    .map(|m| (*m.name(db), m.into()))
                    .collect(),
                Pou::Interface(interface) => interface
                    .methods(db)
                    .iter()
                    .map(|m| (*m.name(db), m.into()))
                    .collect(),
                Pou::FunctionBlock(fb) => fb
                    .methods(db)
                    .iter()
                    .map(|m| (*m.name(db), m.into()))
                    .collect(),
                _ => Default::default(),
            },
        }
    }
}

fn global_variables<'db>(
    db: &'db dyn BaseDatabase,
    vars: &[VariableDecl<'db>],
) -> FxHashMap<Ident, VariableDecl<'db>> {
    let mut variables = FxHashMap::default();
    for v in vars {
        variables.insert(*v.name(db), *v);
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
                variables.insert(*v.name(db), *v);
            }
            VariableKind::Output => {
                variables.insert(*v.name(db), *v);
            }
            VariableKind::InOut => {
                variables.insert(*v.name(db), *v);
            }
            _ => continue,
        };
    }
    variables
}
