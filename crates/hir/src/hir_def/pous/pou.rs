use crate::hir_def::modifier::Modifier;
use auto_lsp::default::db::BaseDatabase;

use crate::{
    hir_def::{
        interned::identifier::Ident,
        pous::{
            class::Class, data_type::DataType, function::Function, function_block::FunctionBlock,
            interface::Interface,
        },
        scope::FileScopeId,
    },
    {AstId, HirNodeInfo},
};

#[salsa::tracked(debug)]
pub struct PouDecl<'db> {
    #[returns(ref)]
    pub pou: Pou<'db>,

    #[returns(ref)]
    pub name: Ident,

    #[tracked]
    #[no_eq]
    pub id: AstId,

    #[tracked]
    #[no_eq]
    pub name_id: AstId,

    pub scope_id: FileScopeId<'db>,
}

impl<'db> PouDecl<'db> {
    pub fn modifier(&'db self, db: &'db dyn BaseDatabase) -> Modifier {
        match self.pou(db) {
            Pou::Class(class) => class.modifier(db),
            Pou::FunctionBlock(fb) => fb.modifier(db),
            _ => Modifier::empty(),
        }
    }
}

impl<'db> HirNodeInfo<'db> for PouDecl<'db> {
    fn get_id(&'db self, db: &'db dyn BaseDatabase) -> AstId {
        self.id(db)
    }

    fn get_name_id(&'db self, db: &'db dyn BaseDatabase) -> Option<AstId> {
        Some(self.name_id(db))
    }

    fn get_scope_id(&self, db: &'db dyn BaseDatabase) -> FileScopeId<'db> {
        self.scope_id(db)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, salsa::Update, salsa::Supertype)]
pub enum Pou<'db> {
    Function(Function<'db>),
    FunctionBlock(FunctionBlock<'db>),
    Class(Class<'db>),
    Interface(Interface<'db>),
    DataType(DataType<'db>),
}
