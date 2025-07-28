use std::ops::Deref;

use crate::{
    hir::{interned::identifier::Ident, pous::{data_type::DataType, pou::{Pou, PouDecl}}, scopes::scope::PouId},
    parser::{semantic_index::SemanticIndexBuilder, ParseInit, ParseSpec},
};
use auto_lsp::core::ast::AstNode;
use auto_lsp::{
    anyhow,
    default::db::{file::File, BaseDatabase},
};
use rustc_hash::FxHashMap;


impl SemanticIndexBuilder<'_> {
    pub fn parse_data_type(&mut self, data_type: &ast::generated::DataTypeDecl) -> anyhow::Result<()> {
        let mut types = FxHashMap::default();
        data_type.parse(self.db, self.file, &mut types)?;

        for (id, decl) in types {
            self.pou_keys.insert(id, decl);
        }

        Ok(())
    }
}

pub trait ParseDataType<'db> {
    fn parse(
        &self,
        db: &'db dyn BaseDatabase,
        file: File,
        types: &mut FxHashMap<PouId, PouDecl<'db>>,
    ) -> anyhow::Result<()>;
}

impl<'db> ParseDataType<'db> for ast::generated::DataTypeDecl {
    fn parse(
        &self,
        db: &'db dyn BaseDatabase,
        file: File,
        types: &mut FxHashMap<PouId, PouDecl<'db>>
    ) -> anyhow::Result<()> {
        type Spec = ast::generated::ArrayTypeSpec_EnumTypeSpec_RefTypeSpec_SimpleTypeSpec_StrTypeSpec_StructTypeSpec_SubrangeTypeSpec;

        for child in &self.children {
            let name = Ident::from_node(db, file, child.name.deref())?;

            let spec = match child.spec.deref() {
                Spec::ArrayTypeSpec(a) => a.to_spec(db, file),
                Spec::EnumTypeSpec(a) => a.to_spec(db, file),
                Spec::SimpleTypeSpec(a) => a.to_spec(db, file),
                Spec::StrTypeSpec(a) => a.to_spec(db, file),
                Spec::StructTypeSpec(a) => a.to_spec(db, file),
                Spec::SubrangeTypeSpec(a) => a.to_spec(db, file),
                Spec::RefTypeSpec(a) => a.to_spec(db, file),
            }?;

            type Init = ast::generated::ArrayTypeInit_SimpleTypeInit_StructTypeInit;

            let init = match child.init.as_deref() {
                Some(Init::ArrayTypeInit(a)) => Some(a.to_init(db, file)?),
                Some(Init::SimpleTypeInit(a)) => Some(a.to_init(db, file)?),
                Some(Init::StructTypeInit(a)) => Some(a.to_init(db, file)?),
                None => None,
            };

            types.insert(
                PouId::from(child.get_id()),
                PouDecl::new(
                db,
                Pou::DataType(DataType::new(db, spec, init)),
                child.get_span(),
                name,
                child.name.get_span()
            ));
        }

        Ok(())
    }
}
