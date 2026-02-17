use crate::{HasName, Modifier};
use db::WorkspaceDataBase;

use crate::{
    hir_def::{
        interned::identifier::Ident,
        pous::{
            class::Class, data_type::DataType, function::Function, function_block::FunctionBlock,
            interface::Interface,
        },
        scope::ScopeId,
    },
    {AstId, HirNodeInfo},
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum Pou<'db> {
    Function(Function<'db>),
    FunctionBlock(FunctionBlock<'db>),
    Class(Class<'db>),
    Interface(Interface<'db>),
    DataType(DataType<'db>),
}

impl<'db> Pou<'db> {
    pub fn modifier(&'db self, db: &'db dyn WorkspaceDataBase) -> Modifier {
        match self {
            Pou::Class(class) => class.modifier(db),
            Pou::FunctionBlock(fb) => fb.modifier(db),
            _ => Modifier::empty(),
        }
    }

    pub fn is_generic(&self, db: &'db dyn WorkspaceDataBase) -> bool {
        match self {
            Pou::Function(f) => !f.generics(db).is_empty(),
            // Other POUs don't support generics yet
            _ => false,
        }
    }
}

impl<'db> HirNodeInfo<'db> for Pou<'db> {
    fn get_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        match self {
            Pou::Function(f) => f.get_id(db),
            Pou::FunctionBlock(fb) => fb.get_id(db),
            Pou::Class(c) => c.get_id(db),
            Pou::Interface(i) => i.get_id(db),
            Pou::DataType(dt) => dt.get_id(db),
        }
    }

    fn get_scope_id(&self, db: &'db dyn WorkspaceDataBase) -> ScopeId<'db> {
        match self {
            Pou::Function(f) => f.get_scope_id(db),
            Pou::FunctionBlock(fb) => fb.get_scope_id(db),
            Pou::Class(c) => c.get_scope_id(db),
            Pou::Interface(i) => i.get_scope_id(db),
            Pou::DataType(dt) => dt.get_scope_id(db),
        }
    }
}

impl<'db> HasName<'db> for Pou<'db> {
    fn get_name_ident(&self, db: &'db dyn WorkspaceDataBase) -> Ident {
        match self {
            Pou::Function(f) => f.get_name_ident(db),
            Pou::FunctionBlock(fb) => fb.get_name_ident(db),
            Pou::Class(c) => c.get_name_ident(db),
            Pou::Interface(i) => i.get_name_ident(db),
            Pou::DataType(dt) => dt.get_name_ident(db),
        }
    }

    fn get_name_id(&self, db: &'db dyn WorkspaceDataBase) -> AstId {
        match self {
            Pou::Function(f) => f.get_name_id(db),
            Pou::FunctionBlock(fb) => fb.get_name_id(db),
            Pou::Class(c) => c.get_name_id(db),
            Pou::Interface(i) => i.get_name_id(db),
            Pou::DataType(dt) => dt.get_name_id(db),
        }
    }
}
