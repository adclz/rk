use auto_lsp::default::db::BaseDatabase;
use indexmap::IndexMap;
use rustc_hash::FxHashMap;

use crate::{
    hir_def::{
        interned::identifier::Ident,
        pous::{
            pou::Pou,
            variable::{VariableDecl, VariableKind},
        },
        scope::{ScopeId, ScopeKind},
        semantic_index::{get_scope},
    },
    hir_ty::inheritance_solver::MethodRef,
};

#[salsa::tracked]
impl<'db> ScopeId<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn local_variables(self, db: &'db dyn BaseDatabase) -> IndexMap<Ident, VariableDecl<'db>> {
        match get_scope(db, self).kind {
            ScopeKind::Global | ScopeKind::Namespace(_) => IndexMap::default(),
            ScopeKind::Pou(pou) => 
                match pou.pou(db) {
                Pou::Function(f) => local_variables(db, f.variables(db)),
                Pou::FunctionBlock(fb) => local_variables(db, fb.variables(db)),
                Pou::Class(cl) => local_variables(db, cl.variables(db)),
                _ => IndexMap::default(),
            },
            ScopeKind::MethodDecl(m) => local_variables(db, m.variables(db))
        }
    }

    #[salsa::tracked(returns(ref))]
    pub fn global_variables(self, db: &'db dyn BaseDatabase) -> IndexMap<Ident, VariableDecl<'db>> {
        match get_scope(db, self).kind {
            ScopeKind::Global | ScopeKind::Namespace(_) => IndexMap::default(),
            ScopeKind::Pou(pou) => match pou.pou(db) {
                Pou::Function(f) => global_variables(db, f.variables(db)),
                Pou::FunctionBlock(fb) => global_variables(db, fb.variables(db)),
                Pou::Class(cl) => global_variables(db, cl.variables(db)),
                _ => IndexMap::default(),
            },
            ScopeKind::MethodDecl(m) => global_variables(db, m.variables(db))
        }
    }

    #[salsa::tracked(returns(ref))]
    pub fn declared_methods(self, db: &'db dyn BaseDatabase) -> FxHashMap<Ident, MethodRef<'db>> {
        match get_scope(db, self).kind {
            ScopeKind::Global | ScopeKind::Namespace(_) | ScopeKind::MethodDecl(_) => FxHashMap::default(),
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
) -> IndexMap<Ident, VariableDecl<'db>> {
    let mut variables = IndexMap::default();
    for v in vars {
        variables.insert(*v.name(db), *v);
    }
    variables
}

fn local_variables<'db>(
    db: &'db dyn BaseDatabase,
    vars: &[VariableDecl<'db>],
) -> IndexMap<Ident, VariableDecl<'db>> {
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
