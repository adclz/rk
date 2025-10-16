use auto_lsp::default::db::BaseDatabase;
use indexmap::IndexMap;

use crate::{hir_def::{interned::identifier::Ident, pous::{pou::{Pou, PouDecl}, variable::{VariableDecl, VariableKind}}}, hir_ty::inheritance_solver::MethodRef};

#[salsa::tracked]
impl<'db> PouDecl<'db> {
    #[salsa::tracked(returns(ref))]
    pub fn global_variables(self, db: &'db dyn BaseDatabase) -> IndexMap<Ident, VariableDecl<'db>> {
        match self.pou(db) {
            Pou::Function(f) => global_variables(db, f.variables(db)),
            Pou::FunctionBlock(fb) => global_variables(db, fb.variables(db)),
            Pou::Class(cl) =>  global_variables(db, cl.variables(db)),
            Pou::Interface(_) | Pou::DataType(_) => IndexMap::default(),
        }
    }
}

pub trait LocalVariables<'db>: Copy {
    fn local_variables(self, db: &'db dyn BaseDatabase) -> &'db IndexMap<Ident, VariableDecl<'db>>;
}

#[salsa::tracked]
impl<'db> LocalVariables<'db> for PouDecl<'db> {
    #[salsa::tracked(returns(ref))]
    fn local_variables(self, db: &'db dyn BaseDatabase) -> IndexMap<Ident, VariableDecl<'db>> {
        match self.pou(db) {
            Pou::Function(f) => local_variables(db, f.variables(db)),
            Pou::FunctionBlock(fb) => local_variables(db, fb.variables(db)),
            Pou::Class(cl) =>  local_variables(db, cl.variables(db)),
            Pou::Interface(_) | Pou::DataType(_) => IndexMap::default(),
        }
    }    
}

#[salsa::tracked]
impl<'db> LocalVariables<'db> for MethodRef<'db> {
    #[salsa::tracked(returns(ref))]
    fn local_variables(self, db: &'db dyn BaseDatabase) -> IndexMap<Ident, VariableDecl<'db>> {
        match self {
            MethodRef::Prototype(p) => local_variables(db, p.variables(db)),
            MethodRef::Declared(d) => local_variables(db, d.variables(db)),
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
